//! Host-side use-case composition: repository writes paired with search
//! index maintenance. This lives in the host (not core) because the crate
//! dependency direction is search → core — only the application can see
//! both sides. No SQL here; repositories stay the only SQL location.

use std::str::FromStr;

use rusqlite::Connection;
use typvia_core::model::{
    Folder, SecurityLevel, Snippet, SnippetContent, SnippetType, SnippetVersion, Tag, TriggerMode,
};
use typvia_core::repo::{
    FolderRepo, ListScope, RepoError, SnippetRepo, TRASH_RETENTION_MS, TagRepo, VersionRepo, new_id,
};
use typvia_search::{SearchIndex, Searcher};

use crate::dto::{
    FolderCountDto, FolderCreateInput, FolderDto, FolderUpdateInput, LibraryCountsDto,
    SearchHitDto, SnippetCreateInput, SnippetDto, SnippetUpdateInput, TagDto,
};
use crate::error::IpcError;

/// v1.0 has a single implicit workspace (PRD §15.1).
const WORKSPACE_ID: &str = "default";

/// List queries stay bounded (database rules): reject silly page sizes
/// instead of silently clamping them.
const MAX_PAGE_LIMIT: u32 = 500;

fn check_limit(limit: u32) -> Result<(), IpcError> {
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        return Err(IpcError::validation("limit must be between 1 and 500"));
    }
    Ok(())
}

fn parse_snippet_type(value: &str) -> Result<SnippetType, IpcError> {
    SnippetType::from_str(value).map_err(|_| IpcError::validation("unknown snippet type"))
}

fn parse_trigger_mode(value: Option<&str>) -> Result<Option<TriggerMode>, IpcError> {
    value
        .map(|v| TriggerMode::from_str(v).map_err(|_| IpcError::validation("unknown trigger mode")))
        .transpose()
}

fn check_trigger_free(
    repo: &SnippetRepo<'_>,
    trigger: Option<&str>,
    exclude_id: Option<&str>,
) -> Result<(), IpcError> {
    if let Some(trigger) = trigger
        && repo.find_trigger_conflict(trigger, exclude_id)?.is_some()
    {
        return Err(IpcError::conflict("trigger already in use"));
    }
    Ok(())
}

pub fn snippet_create(
    conn: &Connection,
    input: SnippetCreateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let snippet = Snippet {
        id: new_id(),
        workspace_id: WORKSPACE_ID.to_string(),
        title: input.title,
        content: SnippetContent::Plaintext(input.body),
        snippet_type: parse_snippet_type(&input.snippet_type)?,
        description: input.description,
        folder_id: input.folder_id,
        trigger: input.trigger,
        trigger_mode: parse_trigger_mode(input.trigger_mode.as_deref())?,
        language: input.language,
        security_level: SecurityLevel::Normal,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: Vec::new(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
    };
    check_trigger_free(&SnippetRepo::new(conn), snippet.trigger.as_deref(), None)?;
    // Insert and its v1 history entry land atomically (fail-closed rule).
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).insert(&snippet)?;
    VersionRepo::new(&tx).append(&history_entry(&snippet, now))?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Snapshot of a snippet's current state as a history row.
fn history_entry(snippet: &Snippet, now: i64) -> SnippetVersion {
    SnippetVersion {
        id: new_id(),
        snippet_id: snippet.id.clone(),
        version: snippet.version,
        title: snippet.title.clone(),
        content: snippet.content.clone(),
        created_at: now,
    }
}

pub fn snippet_update(
    conn: &Connection,
    input: SnippetUpdateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    let mut snippet = repo.get(&input.id)?.ok_or_else(IpcError::not_found)?;
    if snippet.security_level == SecurityLevel::Sensitive {
        return Err(IpcError::conflict(
            "sensitive snippets are edited through the vault flow",
        ));
    }
    let content_before = (
        snippet.title.clone(),
        snippet.content.clone(),
        snippet.snippet_type,
    );
    snippet.title = input.title;
    snippet.content = SnippetContent::Plaintext(input.body);
    snippet.snippet_type = parse_snippet_type(&input.snippet_type)?;
    snippet.description = input.description;
    snippet.folder_id = input.folder_id;
    snippet.trigger = input.trigger;
    snippet.trigger_mode = parse_trigger_mode(input.trigger_mode.as_deref())?;
    snippet.language = input.language;
    snippet.is_favorite = input.is_favorite;
    snippet.is_pinned = input.is_pinned;
    snippet.is_enabled = input.is_enabled;
    snippet.updated_at = now;
    // Version history is append-on-write (PRD §12.16): a content change bumps
    // the version and records the new state. Metadata-only edits (folder,
    // trigger, toggles) do not create versions. Retention pruning is open
    // (OQ-R5); `prune_versions` stays the hook.
    let content_changed = content_before
        != (
            snippet.title.clone(),
            snippet.content.clone(),
            snippet.snippet_type,
        );
    if content_changed {
        snippet.version += 1;
    }
    check_trigger_free(&repo, snippet.trigger.as_deref(), Some(&snippet.id))?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).update(&snippet)?;
    if content_changed {
        VersionRepo::new(&tx).append(&history_entry(&snippet, now))?;
    }
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Ranked search that returns full row data for the Library list. Runs the
/// STAGE-05 Searcher, then loads each hit's snippet in ranked order — one
/// lock, at most `limit` point lookups (candidate pool is already capped).
pub fn search_library(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let hits = Searcher::new(conn).search(query, limit, 0)?;
    let repo = SnippetRepo::new(conn);
    let mut rows = Vec::with_capacity(hits.len());
    for hit in hits {
        // A hit can race a deletion; skipping is correct, not an error.
        if let Some(snippet) = repo.get(&hit.snippet_id)? {
            rows.push(snippet.into());
        }
    }
    Ok(rows)
}

/// Offline sensitive-content scan (PRD §12.10): advisory kinds only, never
/// matched text — safe to cross the IPC boundary and to show in the editor.
pub fn detect_sensitive(text: &str) -> Vec<String> {
    typvia_core::sensitive::detect(text)
        .into_iter()
        .map(|kind| kind.as_str().to_string())
        .collect()
}

pub fn snippet_get(conn: &Connection, id: &str) -> Result<SnippetDto, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    Ok(snippet.into())
}

pub fn snippet_list(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list(limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn snippet_list_by_folder(
    conn: &Connection,
    folder_id: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list_by_folder(folder_id, limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Maps the wire view name + optional folder id onto a repo scope. The view
/// vocabulary is the Library rail: all | recent | starred | unsorted | folder.
fn parse_scope<'a>(view: &str, folder_id: Option<&'a str>) -> Result<ListScope<'a>, IpcError> {
    match (view, folder_id) {
        ("folder", Some(id)) => Ok(ListScope::Folder(id)),
        ("folder", None) => Err(IpcError::validation("folder view requires folderId")),
        (_, Some(_)) => Err(IpcError::validation(
            "folderId only applies to the folder view",
        )),
        ("all", None) => Ok(ListScope::All),
        ("recent", None) => Ok(ListScope::Recent),
        ("starred", None) => Ok(ListScope::Starred),
        ("unsorted", None) => Ok(ListScope::Unsorted),
        _ => Err(IpcError::validation("unknown library view")),
    }
}

pub fn snippet_list_page(
    conn: &Connection,
    view: &str,
    folder_id: Option<&str>,
    snippet_type: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let scope = parse_scope(view, folder_id)?;
    let type_filter = snippet_type.map(parse_snippet_type).transpose()?;
    let rows = SnippetRepo::new(conn).list_scoped(scope, type_filter, limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Exact row count behind one Library page query — the virtual list sizes
/// its scroll range from this, so it honours the same type filter.
pub fn snippet_count(
    conn: &Connection,
    view: &str,
    folder_id: Option<&str>,
    snippet_type: Option<&str>,
) -> Result<u32, IpcError> {
    let scope = parse_scope(view, folder_id)?;
    let type_filter = snippet_type.map(parse_snippet_type).transpose()?;
    Ok(SnippetRepo::new(conn).count_scoped(scope, type_filter)?)
}

pub fn library_counts(conn: &Connection) -> Result<LibraryCountsDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    Ok(LibraryCountsDto {
        total: repo.count_scoped(ListScope::All, None)?,
        recent: repo.count_scoped(ListScope::Recent, None)?,
        starred: repo.count_scoped(ListScope::Starred, None)?,
        unsorted: repo.count_scoped(ListScope::Unsorted, None)?,
        folders: repo
            .count_by_folder()?
            .into_iter()
            .map(|(folder_id, count)| FolderCountDto { folder_id, count })
            .collect(),
    })
}

pub fn snippet_trash(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    SnippetRepo::new(conn).soft_delete(id, now)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

pub fn snippet_restore(conn: &Connection, id: &str) -> Result<(), IpcError> {
    SnippetRepo::new(conn).restore_from_trash(id)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

pub fn snippet_delete_forever(conn: &Connection, id: &str) -> Result<(), IpcError> {
    SnippetRepo::new(conn).delete(id)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

pub fn trash_list(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list_trashed(limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn trash_purge_expired(conn: &Connection, now: i64) -> Result<usize, IpcError> {
    // Trashed rows are never in the search index, so no index work here.
    Ok(SnippetRepo::new(conn).purge_expired_trash(now, TRASH_RETENTION_MS)?)
}

fn check_ids(ids: &[String]) -> Result<(), IpcError> {
    if ids.is_empty() {
        return Err(IpcError::validation("no snippets selected"));
    }
    if ids.len() > MAX_PAGE_LIMIT as usize {
        return Err(IpcError::validation("too many snippets in one batch"));
    }
    Ok(())
}

/// Moves a batch into a folder (`None` = unfiled) atomically, then re-indexes
/// each row — the folder name is part of the search index.
pub fn snippet_batch_move(
    conn: &Connection,
    ids: &[String],
    folder_id: Option<&str>,
) -> Result<(), IpcError> {
    check_ids(ids)?;
    SnippetRepo::new(conn).batch_move(ids, folder_id)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Tags a batch atomically, then re-indexes (tags are searchable).
pub fn snippet_batch_add_tag(
    conn: &Connection,
    ids: &[String],
    tag_id: &str,
) -> Result<(), IpcError> {
    check_ids(ids)?;
    SnippetRepo::new(conn).batch_add_tag(ids, tag_id)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Moves a batch into the recycle bin atomically; trashed rows leave the
/// search index.
pub fn snippet_batch_trash(conn: &Connection, ids: &[String], now: i64) -> Result<(), IpcError> {
    check_ids(ids)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let repo = SnippetRepo::new(&tx);
    for id in ids {
        repo.soft_delete(id, now)?;
    }
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Every snippet id inside a folder subtree (the folder itself plus all
/// descendants), collected via the repositories — no SQL in the host.
fn snippet_ids_in_subtree(conn: &Connection, folder_id: &str) -> Result<Vec<String>, IpcError> {
    let folders = FolderRepo::new(conn);
    let snippets = SnippetRepo::new(conn);
    let mut queue = vec![folder_id.to_string()];
    let mut ids = Vec::new();
    while let Some(current) = queue.pop() {
        for child in folders.list_children(Some(&current))? {
            queue.push(child.id);
        }
        let mut offset = 0;
        loop {
            let page = snippets.list_by_folder(Some(&current), MAX_PAGE_LIMIT, offset)?;
            let page_len = page.len();
            ids.extend(page.into_iter().map(|s| s.id));
            if page_len < MAX_PAGE_LIMIT as usize {
                break;
            }
            offset += MAX_PAGE_LIMIT;
        }
    }
    Ok(ids)
}

pub fn folder_create(
    conn: &Connection,
    input: FolderCreateInput,
    now: i64,
) -> Result<FolderDto, IpcError> {
    let folder = Folder {
        id: new_id(),
        parent_id: input.parent_id,
        name: input.name,
        sort_order: input.sort_order,
        created_at: now,
        updated_at: now,
    };
    FolderRepo::new(conn).insert(&folder)?;
    Ok(folder.into())
}

pub fn folder_update(
    conn: &Connection,
    input: FolderUpdateInput,
    now: i64,
) -> Result<FolderDto, IpcError> {
    let repo = FolderRepo::new(conn);
    let mut folder = repo.get(&input.id)?.ok_or_else(IpcError::not_found)?;
    let renamed = folder.name != input.name;
    folder.name = input.name;
    folder.parent_id = input.parent_id;
    folder.sort_order = input.sort_order;
    folder.updated_at = now;
    repo.update(&folder)?;
    // The folder name is indexed with each snippet: a rename must re-index
    // the folder's direct contents.
    if renamed {
        let index = SearchIndex::new(conn);
        let mut offset = 0;
        loop {
            let page =
                SnippetRepo::new(conn).list_by_folder(Some(&folder.id), MAX_PAGE_LIMIT, offset)?;
            let page_len = page.len();
            for snippet in page {
                index.sync_snippet(&snippet.id)?;
            }
            if page_len < MAX_PAGE_LIMIT as usize {
                break;
            }
            offset += MAX_PAGE_LIMIT;
        }
    }
    Ok(folder.into())
}

/// Deletes a folder: child folders cascade away and contents fall back to
/// unfiled (schema rules), so the affected snippets are re-indexed after.
pub fn folder_delete(conn: &Connection, id: &str) -> Result<(), IpcError> {
    let affected = snippet_ids_in_subtree(conn, id)?;
    FolderRepo::new(conn).delete(id)?;
    let index = SearchIndex::new(conn);
    for snippet_id in &affected {
        index.sync_snippet(snippet_id)?;
    }
    Ok(())
}

pub fn folder_list_children(
    conn: &Connection,
    parent_id: Option<&str>,
) -> Result<Vec<FolderDto>, IpcError> {
    let rows = FolderRepo::new(conn).list_children(parent_id)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn tag_create(conn: &Connection, name: String, now: i64) -> Result<TagDto, IpcError> {
    let tag = Tag {
        id: new_id(),
        name,
        created_at: now,
    };
    TagRepo::new(conn).insert(&tag)?;
    Ok(tag.into())
}

pub fn tag_list(conn: &Connection) -> Result<Vec<TagDto>, IpcError> {
    let rows = TagRepo::new(conn).list_all()?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn tag_rename(conn: &Connection, id: &str, name: &str) -> Result<(), IpcError> {
    Ok(TagRepo::new(conn).rename(id, name)?)
}

pub fn tag_delete(conn: &Connection, id: &str) -> Result<(), IpcError> {
    Ok(TagRepo::new(conn).delete(id)?)
}

pub fn search_snippets(
    conn: &Connection,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<SearchHitDto>, IpcError> {
    check_limit(limit)?;
    let hits = Searcher::new(conn).search(query, limit, offset)?;
    Ok(hits.into_iter().map(Into::into).collect())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::IpcErrorCode;

    fn test_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn create_input(title: &str, trigger: Option<&str>) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: format!("{title} body"),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: trigger.map(str::to_string),
            trigger_mode: trigger.map(|_| "delimiter".to_string()),
            language: None,
        }
    }

    #[test]
    fn create_then_query_roundtrip_via_list_get_and_search() {
        let conn = test_conn();
        let created =
            snippet_create(&conn, create_input("Docker tail logs", Some(";dl")), 1).unwrap();
        assert_eq!(created.security_level, "normal");
        assert_eq!(created.body.as_deref(), Some("Docker tail logs body"));

        let fetched = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(fetched.title, "Docker tail logs");

        let listed = snippet_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 1);

        // The create path must have indexed the snippet: search finds it.
        let hits = search_snippets(&conn, "docker", 10, 0).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet_id, created.id);
        assert!(!hits[0].is_sensitive);
    }

    #[test]
    fn update_reindexes_and_respects_trigger_conflicts() {
        let conn = test_conn();
        let a = snippet_create(&conn, create_input("First", Some(";a")), 1).unwrap();
        let b = snippet_create(&conn, create_input("Second", Some(";b")), 2).unwrap();

        // Renaming updates the index.
        let updated = snippet_update(
            &conn,
            SnippetUpdateInput {
                id: b.id.clone(),
                title: "Kubernetes restart".to_string(),
                body: "kubectl rollout restart".to_string(),
                snippet_type: "command".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";b".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
                is_favorite: true,
                is_pinned: false,
                is_enabled: true,
            },
            5,
        )
        .unwrap();
        assert!(updated.is_favorite);
        let hits = search_snippets(&conn, "kubernetes", 10, 0).unwrap();
        assert_eq!(hits.len(), 1);

        // Stealing another snippet's trigger is a business conflict.
        let err = snippet_update(
            &conn,
            SnippetUpdateInput {
                id: b.id,
                title: "Second".to_string(),
                body: "body".to_string(),
                snippet_type: "command".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";a".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
            },
            6,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        drop(a);
    }

    #[test]
    fn trash_flow_removes_from_search_and_restores() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Trash me", None), 1).unwrap();

        snippet_trash(&conn, &created.id, 10).unwrap();
        assert!(search_snippets(&conn, "trash", 10, 0).unwrap().is_empty());
        assert_eq!(trash_list(&conn, 50, 0).unwrap().len(), 1);

        snippet_restore(&conn, &created.id).unwrap();
        assert_eq!(search_snippets(&conn, "trash", 10, 0).unwrap().len(), 1);

        snippet_trash(&conn, &created.id, 20).unwrap();
        // Not yet expired: retention window keeps it.
        assert_eq!(trash_purge_expired(&conn, 21).unwrap(), 0);
        assert_eq!(
            trash_purge_expired(&conn, 20 + TRASH_RETENTION_MS).unwrap(),
            1
        );
        assert_eq!(
            snippet_get(&conn, &created.id).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn error_mapping_covers_the_three_classes() {
        let conn = test_conn();

        // User error: blank title fails validation.
        let err = snippet_create(&conn, create_input("", None), 1).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        // User error: unknown enum input.
        let mut bad = create_input("Ok", None);
        bad.snippet_type = "nonsense".to_string();
        assert_eq!(
            snippet_create(&conn, bad, 1).unwrap_err().code,
            IpcErrorCode::Validation
        );

        // Business error: missing row.
        assert_eq!(
            snippet_get(&conn, "missing").unwrap_err().code,
            IpcErrorCode::NotFound
        );

        // Business error: duplicate trigger at creation.
        snippet_create(&conn, create_input("One", Some(";x")), 1).unwrap();
        let err = snippet_create(&conn, create_input("Two", Some(";x")), 2).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);

        // Oversized page limits are rejected, not clamped.
        assert_eq!(
            snippet_list(&conn, 0, 0).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn save_flow_appends_versions_only_on_content_change() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Draft", None), 1).unwrap();
        assert_eq!(created.version, 1);
        let history_count = |id: &str| {
            VersionRepo::new(&conn)
                .list(id, 100, 0)
                .map(|rows| rows.len())
                .unwrap()
        };
        // Creation recorded v1.
        assert_eq!(history_count(&created.id), 1);

        let mut input = SnippetUpdateInput {
            id: created.id.clone(),
            title: "Draft".to_string(),
            body: "Draft body".to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
        };

        // Metadata-only change: no bump, no new history row.
        input.is_favorite = true;
        let updated = snippet_update(&conn, input.clone(), 2).unwrap();
        assert_eq!(updated.version, 1);
        assert_eq!(history_count(&created.id), 1);

        // Content change: bump + snapshot of the new state.
        input.body = "Draft body, revised".to_string();
        let updated = snippet_update(&conn, input.clone(), 3).unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(history_count(&created.id), 2);

        input.title = "Draft renamed".to_string();
        let updated = snippet_update(&conn, input, 4).unwrap();
        assert_eq!(updated.version, 3);
        assert_eq!(history_count(&created.id), 3);
    }

    #[test]
    fn detect_sensitive_maps_kinds_to_stable_codes() {
        let kinds = detect_sensitive(
            "password = hunter2-not-real\n-----BEGIN RSA PRIVATE KEY-----\nFAKE\n-----END RSA PRIVATE KEY-----",
        );
        assert!(kinds.contains(&"pem_private_key".to_string()));
        assert!(kinds.contains(&"password_field".to_string()));
        assert!(detect_sensitive("just a plain sentence").is_empty());
    }

    #[test]
    fn search_library_returns_full_rows_in_rank_order() {
        let conn = test_conn();
        // Title-exact should outrank a content match (STAGE-05 tiers).
        let mut content_hit = create_input("Restart notes", None);
        content_hit.body = "docker restart tips".to_string();
        snippet_create(&conn, content_hit, 1).unwrap();
        let title_hit = snippet_create(&conn, create_input("Docker", Some(";dk")), 2).unwrap();

        let rows = search_library(&conn, "docker", 50).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, title_hit.id);
        assert_eq!(rows[0].trigger.as_deref(), Some(";dk"));
        assert!(rows[0].body.is_some(), "full row data, not just hits");

        // Queries with nothing indexable return empty, not an error.
        assert!(search_library(&conn, "   ", 50).unwrap().is_empty());
        assert_eq!(
            search_library(&conn, "docker", 0).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn library_page_and_counts_reflect_scopes() {
        let conn = test_conn();
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Infra".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();

        let mut filed = create_input("Filed one", None);
        filed.folder_id = Some(folder.id.clone());
        snippet_create(&conn, filed, 1).unwrap();
        let mut text_kind = create_input("Loose text", None);
        text_kind.snippet_type = "text".to_string();
        snippet_create(&conn, text_kind, 2).unwrap();
        let starred = snippet_create(&conn, create_input("Starred one", None), 3).unwrap();
        let mut make_fav = SnippetUpdateInput {
            id: starred.id.clone(),
            title: starred.title.clone(),
            body: "Starred one body".to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            is_favorite: true,
            is_pinned: false,
            is_enabled: true,
        };
        snippet_update(&conn, make_fav.clone(), 4).unwrap();

        let counts = library_counts(&conn).unwrap();
        assert_eq!(
            (counts.total, counts.recent, counts.starred, counts.unsorted),
            (3, 0, 1, 2)
        );
        assert_eq!(counts.folders.len(), 1);
        assert_eq!(counts.folders[0].folder_id, folder.id);
        assert_eq!(counts.folders[0].count, 1);

        let page = snippet_list_page(&conn, "folder", Some(&folder.id), None, 50, 0).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].title, "Filed one");

        let texts = snippet_list_page(&conn, "unsorted", None, Some("text"), 50, 0).unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].title, "Loose text");

        // The type-filtered count matches the type-filtered page query.
        assert_eq!(
            snippet_count(&conn, "unsorted", None, Some("text")).unwrap(),
            1
        );
        assert_eq!(snippet_count(&conn, "all", None, None).unwrap(), 3);

        // Validation: unknown view, folder view without id, stray folderId.
        for (view, folder_id) in [("nonsense", None), ("folder", None), ("all", Some("x"))] {
            let err = snippet_list_page(&conn, view, folder_id, None, 50, 0).unwrap_err();
            assert_eq!(err.code, IpcErrorCode::Validation);
        }
        let err = snippet_list_page(&conn, "all", None, Some("nonsense"), 50, 0).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        // A trashed snippet drops out of pages and counts.
        make_fav.is_favorite = false;
        snippet_update(&conn, make_fav, 5).unwrap();
        snippet_trash(&conn, &starred.id, 6).unwrap();
        assert_eq!(library_counts(&conn).unwrap().total, 2);
    }

    #[test]
    fn batch_operations_keep_the_search_index_in_step() {
        let conn = test_conn();
        let a = snippet_create(&conn, create_input("Alpha", None), 1).unwrap();
        let b = snippet_create(&conn, create_input("Beta", None), 2).unwrap();
        let ids = vec![a.id.clone(), b.id.clone()];
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Runbooks".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            3,
        )
        .unwrap();

        // Move: the folder name becomes searchable for both rows.
        snippet_batch_move(&conn, &ids, Some(&folder.id)).unwrap();
        assert_eq!(search_snippets(&conn, "runbooks", 10, 0).unwrap().len(), 2);

        // Tag: the tag name becomes searchable.
        let tag = tag_create(&conn, "prod".to_string(), 4).unwrap();
        snippet_batch_add_tag(&conn, &ids, &tag.id).unwrap();
        assert_eq!(search_snippets(&conn, "prod", 10, 0).unwrap().len(), 2);

        // Folder rename: contents re-index under the new name.
        folder_update(
            &conn,
            FolderUpdateInput {
                id: folder.id.clone(),
                name: "Playbooks".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            5,
        )
        .unwrap();
        assert_eq!(search_snippets(&conn, "playbooks", 10, 0).unwrap().len(), 2);
        assert!(
            search_snippets(&conn, "runbooks", 10, 0)
                .unwrap()
                .is_empty()
        );

        // Folder delete: contents move out (unfiled) and stay alive.
        folder_delete(&conn, &folder.id).unwrap();
        let listed = snippet_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|s| s.folder_id.is_none()));
        assert!(
            search_snippets(&conn, "playbooks", 10, 0)
                .unwrap()
                .is_empty()
        );
        assert_eq!(search_snippets(&conn, "alpha", 10, 0).unwrap().len(), 1);

        // Batch trash: rows leave lists, counts and the index together.
        snippet_batch_trash(&conn, &ids, 10).unwrap();
        assert!(snippet_list(&conn, 50, 0).unwrap().is_empty());
        assert!(search_snippets(&conn, "alpha", 10, 0).unwrap().is_empty());
        assert_eq!(library_counts(&conn).unwrap().total, 0);
        assert_eq!(trash_list(&conn, 50, 0).unwrap().len(), 2);

        // Empty batches are user errors.
        assert_eq!(
            snippet_batch_trash(&conn, &[], 11).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn folder_and_tag_crud_roundtrip() {
        let conn = test_conn();
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Infra".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();
        let child = folder_create(
            &conn,
            FolderCreateInput {
                name: "K8s".to_string(),
                parent_id: Some(folder.id.clone()),
                sort_order: 0,
            },
            2,
        )
        .unwrap();
        assert_eq!(
            folder_list_children(&conn, Some(&folder.id)).unwrap().len(),
            1
        );

        let renamed = folder_update(
            &conn,
            FolderUpdateInput {
                id: child.id.clone(),
                name: "Kubernetes".to_string(),
                parent_id: Some(folder.id.clone()),
                sort_order: 1,
            },
            3,
        )
        .unwrap();
        assert_eq!(renamed.name, "Kubernetes");
        folder_delete(&conn, &child.id).unwrap();

        let tag = tag_create(&conn, "prod".to_string(), 1).unwrap();
        assert_eq!(
            tag_create(&conn, "prod".to_string(), 2).unwrap_err().code,
            IpcErrorCode::Conflict
        );
        tag_rename(&conn, &tag.id, "production").unwrap();
        assert_eq!(tag_list(&conn).unwrap().len(), 1);
        tag_delete(&conn, &tag.id).unwrap();
        assert!(tag_list(&conn).unwrap().is_empty());
    }
}
