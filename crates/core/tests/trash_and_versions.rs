//! Recycle bin (soft delete, 30-day cleanup, restore) and append-only
//! version history including forward-writing restore (PRD §12.16).

#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, SnippetVersion};
use typvia_core::repo::{RepoError, SnippetRepo, TRASH_RETENTION_MS, VersionRepo, new_id};

fn fresh_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    conn
}

fn snippet(title: &str, body: &str) -> Snippet {
    Snippet {
        id: new_id(),
        workspace_id: "w1".to_string(),
        title: title.to_string(),
        content: SnippetContent::Plaintext(body.to_string()),
        snippet_type: SnippetType::Text,
        description: None,
        folder_id: None,
        trigger: None,
        trigger_mode: None,
        language: None,
        security_level: SecurityLevel::Normal,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: vec![],
        created_at: 1_000,
        updated_at: 1_000,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
    }
}

fn version_of(s: &Snippet, version: u32, body: &str, at: i64) -> SnippetVersion {
    SnippetVersion {
        id: new_id(),
        snippet_id: s.id.clone(),
        version,
        title: s.title.clone(),
        content: SnippetContent::Plaintext(body.to_string()),
        created_at: at,
    }
}

#[test]
fn soft_delete_moves_snippet_to_trash_and_out_of_lists() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Trashed", "body");
    repo.insert(&s).unwrap();

    repo.soft_delete(&s.id, 5_000).unwrap();

    assert!(repo.list(10, 0).unwrap().is_empty());
    let trashed = repo.list_trashed(10, 0).unwrap();
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].deleted_at, Some(5_000));
    // Row still loadable directly (needed for restore preview).
    assert!(repo.get(&s.id).unwrap().is_some());
}

#[test]
fn soft_delete_twice_reports_not_found() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Once", "body");
    repo.insert(&s).unwrap();
    repo.soft_delete(&s.id, 5_000).unwrap();
    assert!(matches!(
        repo.soft_delete(&s.id, 6_000).unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn restore_brings_snippet_back_to_live_lists() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let s = snippet("Back", "body");
    repo.insert(&s).unwrap();
    repo.soft_delete(&s.id, 5_000).unwrap();

    repo.restore_from_trash(&s.id).unwrap();

    assert_eq!(repo.list(10, 0).unwrap().len(), 1);
    assert!(repo.list_trashed(10, 0).unwrap().is_empty());
    assert_eq!(repo.get(&s.id).unwrap().unwrap().deleted_at, None);
    // Restoring a live snippet is NotFound.
    assert!(matches!(
        repo.restore_from_trash(&s.id).unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn expired_trash_is_purged_and_recent_trash_kept() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let old = snippet("Old", "body");
    let recent = snippet("Recent", "body");
    repo.insert(&old).unwrap();
    repo.insert(&recent).unwrap();

    let now = 100 * TRASH_RETENTION_MS;
    repo.soft_delete(&old.id, now - TRASH_RETENTION_MS - 1)
        .unwrap();
    repo.soft_delete(&recent.id, now - 1_000).unwrap();

    let purged = repo.purge_expired_trash(now, TRASH_RETENTION_MS).unwrap();
    assert_eq!(purged, 1);
    assert!(repo.get(&old.id).unwrap().is_none());
    assert!(repo.get(&recent.id).unwrap().is_some());
}

#[test]
fn trashed_snippets_do_not_claim_triggers_or_duplicates() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut s = snippet("Ghost", "unique body");
    s.trigger = Some(":gh".to_string());
    s.trigger_mode = Some(typvia_core::model::TriggerMode::Delimiter);
    repo.insert(&s).unwrap();
    repo.soft_delete(&s.id, 5_000).unwrap();

    assert_eq!(repo.find_trigger_conflict(":gh", None).unwrap(), None);
    assert!(
        repo.find_duplicates("Ghost", Some("unique body"), None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn version_history_appends_and_lists_newest_first() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let s = snippet("Doc", "v1 body");
    snippets.insert(&s).unwrap();

    versions
        .append(&version_of(&s, 1, "v1 body", 1_000))
        .unwrap();
    versions
        .append(&version_of(&s, 2, "v2 body", 2_000))
        .unwrap();

    let history = versions.list(&s.id, 10, 0).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, 2);
    assert_eq!(history[1].version, 1);
    assert_eq!(versions.latest_version(&s.id).unwrap(), 2);

    // Re-recording an existing version is a conflict, never an overwrite.
    let err = versions
        .append(&version_of(&s, 2, "rewrite", 3_000))
        .unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn restore_writes_forward_and_keeps_all_versions() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let mut s = snippet("Doc", "v1 body");
    snippets.insert(&s).unwrap();
    versions
        .append(&version_of(&s, 1, "v1 body", 1_000))
        .unwrap();

    s.content = SnippetContent::Plaintext("v2 body".to_string());
    s.version = 2;
    snippets.update(&s).unwrap();
    versions
        .append(&version_of(&s, 2, "v2 body", 2_000))
        .unwrap();

    let new_version = versions
        .restore_version(&s.id, 1, &new_id(), 3_000)
        .unwrap();
    assert_eq!(new_version, 3);

    // The live snippet carries the restored body at the new version.
    let live = snippets.get(&s.id).unwrap().unwrap();
    assert_eq!(live.version, 3);
    assert_eq!(
        live.content,
        SnippetContent::Plaintext("v1 body".to_string())
    );

    // No history entry was lost; the restore itself is recorded.
    let history = versions.list(&s.id, 10, 0).unwrap();
    let recorded: Vec<u32> = history.iter().map(|v| v.version).collect();
    assert_eq!(recorded, vec![3, 2, 1]);
    assert_eq!(
        history[0].content,
        SnippetContent::Plaintext("v1 body".to_string())
    );
}

#[test]
fn restore_of_missing_version_is_not_found() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let s = snippet("Doc", "body");
    snippets.insert(&s).unwrap();
    assert!(matches!(
        versions
            .restore_version(&s.id, 7, &new_id(), 1_000)
            .unwrap_err(),
        RepoError::NotFound
    ));
}

#[test]
fn sensitive_history_entries_hold_ciphertext_only() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let mut s = snippet("Vault", "placeholder");
    s.snippet_type = SnippetType::Sensitive;
    s.security_level = SecurityLevel::Sensitive;
    s.content = SnippetContent::Ciphertext(vec![0xAA]);
    snippets.insert(&s).unwrap();

    let entry = SnippetVersion {
        id: new_id(),
        snippet_id: s.id.clone(),
        version: 1,
        title: s.title.clone(),
        content: SnippetContent::Ciphertext(vec![0xAA]),
        created_at: 1_000,
    };
    versions.append(&entry).unwrap();
    let loaded = versions.get(&s.id, 1).unwrap().unwrap();
    assert_eq!(loaded.content, SnippetContent::Ciphertext(vec![0xAA]));
}

#[test]
fn deleting_a_snippet_cascades_its_history() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let s = snippet("Doomed", "body");
    snippets.insert(&s).unwrap();
    versions.append(&version_of(&s, 1, "body", 1_000)).unwrap();

    snippets.delete(&s.id).unwrap();
    assert!(versions.list(&s.id, 10, 0).unwrap().is_empty());
}

#[test]
fn prune_versions_keeps_only_the_newest_entries() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let versions = VersionRepo::new(&conn);
    let s = snippet("Doc", "body");
    snippets.insert(&s).unwrap();
    for v in 1..=5 {
        versions
            .append(&version_of(&s, v, &format!("v{v}"), i64::from(v) * 1_000))
            .unwrap();
    }

    let removed = versions.prune_versions(&s.id, 2).unwrap();
    assert_eq!(removed, 3);
    let remaining: Vec<u32> = versions
        .list(&s.id, 10, 0)
        .unwrap()
        .iter()
        .map(|v| v.version)
        .collect();
    assert_eq!(remaining, vec![5, 4]);
}
