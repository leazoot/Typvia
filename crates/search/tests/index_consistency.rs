//! Index consistency after snippet CRUD: every searchable field is
//! matchable, updates/deletes/trash keep the index in step, CJK substrings
//! match, and rebuild restores the whole index.

#![allow(clippy::unwrap_used)]

use std::time::Instant;

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{
    Folder, SecurityLevel, Snippet, SnippetContent, SnippetType, Tag, TriggerMode,
};
use typvia_core::repo::{FolderRepo, SnippetRepo, TagRepo, new_id};
use typvia_search::SearchIndex;
use typvia_search::segment::query_phrase;

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
        conflict_of: None,
    }
}

/// Snippet ids matching a single query term, via the same phrase building
/// the future query parser uses.
fn matches(conn: &Connection, term: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT snippet_id FROM snippet_fts WHERE snippet_fts MATCH ?1 ORDER BY snippet_id",
        )
        .unwrap();
    let rows = stmt
        .query_map([query_phrase(term)], |row| row.get::<_, String>(0))
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

#[test]
fn indexed_snippet_is_searchable_by_every_field() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let tags = TagRepo::new(&conn);
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);

    let folder = Folder {
        id: new_id(),
        parent_id: None,
        name: "Deploystuff".to_string(),
        sort_order: 0,
        created_at: 1_000,
        updated_at: 1_000,
    };
    folders.insert(&folder).unwrap();
    let tag = Tag {
        id: new_id(),
        name: "opslabel".to_string(),
        created_at: 1_000,
    };
    tags.insert(&tag).unwrap();

    let mut s = snippet("Restartguide", "systemctl restart nginx");
    s.description = Some("nightly runbook".to_string());
    s.folder_id = Some(folder.id.clone());
    s.trigger = Some(":rsx".to_string());
    s.trigger_mode = Some(TriggerMode::Delimiter);
    s.language = Some("bash".to_string());
    snippets.insert(&s).unwrap();
    snippets.batch_add_tag(&[s.id.clone()], &tag.id).unwrap();

    index.sync_snippet(&s.id).unwrap();

    for term in [
        "Restartguide", // title
        "systemctl",    // content
        "runbook",      // description
        "opslabel",     // tag name
        "Deploystuff",  // folder name
        "rsx",          // trigger (punctuation stripped by unicode61)
        "bash",         // language
    ] {
        assert_eq!(matches(&conn, term), vec![s.id.clone()], "term: {term}");
    }
}

#[test]
fn update_replaces_the_index_row() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);
    let mut s = snippet("Oldtitle", "body words");
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    s.title = "Newtitle".to_string();
    snippets.update(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    assert!(matches(&conn, "Oldtitle").is_empty());
    assert_eq!(matches(&conn, "Newtitle"), vec![s.id.clone()]);
    // Exactly one index row remains for the snippet.
    let rows: i64 = conn
        .query_row(
            "SELECT count(*) FROM snippet_fts WHERE snippet_id = ?1",
            [&s.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn hard_delete_removes_the_index_row() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);
    let s = snippet("Doomed", "body");
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    snippets.delete(&s.id).unwrap();
    index.sync_snippet(&s.id).unwrap();

    assert!(matches(&conn, "Doomed").is_empty());
}

#[test]
fn trashing_removes_and_restoring_reindexes() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);
    let s = snippet("Ghostly", "body");
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    snippets.soft_delete(&s.id, 5_000).unwrap();
    index.sync_snippet(&s.id).unwrap();
    assert!(matches(&conn, "Ghostly").is_empty());

    snippets.restore_from_trash(&s.id).unwrap();
    index.sync_snippet(&s.id).unwrap();
    assert_eq!(matches(&conn, "Ghostly"), vec![s.id.clone()]);
}

#[test]
fn cjk_substrings_match_as_phrases() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);
    let s = snippet("发票抬头", "公司发票抬头与税号信息");
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    for term in ["发票", "抬头", "票抬", "税号"] {
        assert_eq!(matches(&conn, term), vec![s.id.clone()], "term: {term}");
    }
    assert!(matches(&conn, "收据").is_empty());
}

#[test]
fn mixed_cjk_ascii_content_matches_both_scripts() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);
    let s = snippet("重启nginx服务", "docker restart web");
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    assert_eq!(matches(&conn, "重启"), vec![s.id.clone()]);
    assert_eq!(matches(&conn, "nginx"), vec![s.id.clone()]);
}

#[test]
fn sensitive_snippet_indexes_only_title_tags_description() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    let tags = TagRepo::new(&conn);
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);

    let folder = Folder {
        id: new_id(),
        parent_id: None,
        name: "Vaultfolder".to_string(),
        sort_order: 0,
        created_at: 1_000,
        updated_at: 1_000,
    };
    folders.insert(&folder).unwrap();
    let tag = Tag {
        id: new_id(),
        name: "secretstag".to_string(),
        created_at: 1_000,
    };
    tags.insert(&tag).unwrap();

    let mut s = snippet("Bankaccess", "placeholder");
    s.snippet_type = SnippetType::Sensitive;
    s.security_level = SecurityLevel::Sensitive;
    s.content = SnippetContent::Ciphertext(vec![0xAA, 0xBB]);
    s.description = Some("online banking note".to_string());
    s.folder_id = Some(folder.id.clone());
    s.trigger = Some(":bank".to_string());
    s.trigger_mode = Some(TriggerMode::Delimiter);
    s.language = Some("plain".to_string());
    snippets.insert(&s).unwrap();
    snippets.batch_add_tag(&[s.id.clone()], &tag.id).unwrap();

    index.sync_snippet(&s.id).unwrap();

    // Indexed: title, tags, description.
    assert_eq!(matches(&conn, "Bankaccess"), vec![s.id.clone()]);
    assert_eq!(matches(&conn, "secretstag"), vec![s.id.clone()]);
    assert_eq!(matches(&conn, "banking"), vec![s.id.clone()]);
    // Withheld: folder name, trigger, language (and body, which is
    // ciphertext and never read by the indexer at all).
    assert!(matches(&conn, "Vaultfolder").is_empty());
    assert!(matches(&conn, "bank").is_empty());
    assert!(matches(&conn, "plain").is_empty());
}

#[test]
fn rebuild_reindexes_live_rows_and_drops_stale_entries() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);

    let a = snippet("Alphaone", "first body");
    let b = snippet("Betatwo", "second body");
    let trashed = snippet("Trashedone", "gone body");
    snippets.insert(&a).unwrap();
    snippets.insert(&b).unwrap();
    snippets.insert(&trashed).unwrap();
    snippets.soft_delete(&trashed.id, 5_000).unwrap();

    // A stale row left over from a snippet that no longer exists.
    conn.execute(
        "INSERT INTO snippet_fts (snippet_id, title) VALUES ('stale-id', 'Staletitle')",
        [],
    )
    .unwrap();

    let indexed = index.rebuild().unwrap();
    assert_eq!(indexed, 2);
    assert_eq!(matches(&conn, "Alphaone"), vec![a.id.clone()]);
    assert_eq!(matches(&conn, "Betatwo"), vec![b.id.clone()]);
    assert!(matches(&conn, "Staletitle").is_empty());
    assert!(matches(&conn, "Trashedone").is_empty());
}

/// Rebuild timing at the 50k target dataset scale. Run manually:
/// `cargo test -p typvia-search --release -- --ignored --nocapture`.
#[test]
#[ignore = "timing measurement, run manually"]
fn rebuild_50k_snippets_timing() {
    let conn = fresh_db();
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);

    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..50_000 {
        let s = snippet(
            &format!("Snippet number {i}"),
            &format!("body text with shared words and unique token tok{i} 以及中文内容片段"),
        );
        snippets.insert(&s).unwrap();
    }
    tx.commit().unwrap();

    let t0 = Instant::now();
    let indexed = index.rebuild().unwrap();
    let elapsed = t0.elapsed();
    eprintln!("REBUILD 50k: {indexed} rows in {elapsed:?}");
    assert_eq!(indexed, 50_000);
}
