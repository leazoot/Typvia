//! Host-side use-case composition: repository writes paired with search
//! index maintenance. This lives in the host (not core) because the crate
//! dependency direction is search → core — only the application can see
//! both sides. No SQL here; repositories stay the only SQL location.

use std::str::FromStr;

use rusqlite::Connection;
use typvia_core::model::{
    Folder, SecurityLevel, Snippet, SnippetContent, SnippetType, Tag, TriggerMode,
};
use typvia_core::repo::{FolderRepo, SnippetRepo, TRASH_RETENTION_MS, TagRepo, new_id};
use typvia_search::{SearchIndex, Searcher};

use crate::dto::{
    FolderCreateInput, FolderDto, FolderUpdateInput, SearchHitDto, SnippetCreateInput, SnippetDto,
    SnippetUpdateInput, TagDto,
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
    let repo = SnippetRepo::new(conn);
    check_trigger_free(&repo, snippet.trigger.as_deref(), None)?;
    repo.insert(&snippet)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
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
    // Content-version history semantics (append + bump) belong to the editor
    // save flow (TASK-030); until then `version` is left untouched.
    check_trigger_free(&repo, snippet.trigger.as_deref(), Some(&snippet.id))?;
    repo.update(&snippet)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
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
    folder.name = input.name;
    folder.parent_id = input.parent_id;
    folder.sort_order = input.sort_order;
    folder.updated_at = now;
    repo.update(&folder)?;
    Ok(folder.into())
}

pub fn folder_delete(conn: &Connection, id: &str) -> Result<(), IpcError> {
    Ok(FolderRepo::new(conn).delete(id)?)
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
