//! Snippet CRUD, batch operations, toggles, usage tracking, and the
//! trigger-conflict / duplicate checks (PRD §12.1).

use rusqlite::types::ToSql;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::{
    Platform, SecurityLevel, Snippet, SnippetContent, SnippetId, SnippetType, TagId, TimestampMs,
    TriggerMode,
};

/// Snippet repository over a single connection.
pub struct SnippetRepo<'c> {
    conn: &'c Connection,
}

const SELECT_COLUMNS: &str = "id, workspace_id, title, content_plaintext, content_ciphertext, \
     type, description, folder_id, \"trigger\", trigger_mode, language, security_level, \
     is_favorite, is_pinned, is_enabled, platform_scope, created_at, updated_at, \
     last_used_at, usage_count, version, deleted_at";

/// Recycle-bin retention before automatic cleanup (PRD §12.16: 30 days).
pub const TRASH_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// One Library list scope: a saved view or a single folder. Trashed rows are
/// excluded from every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListScope<'a> {
    /// Every live snippet.
    All,
    /// Snippets used at least once, most recently used first.
    Recent,
    /// Favorites.
    Starred,
    /// Live snippets outside any folder.
    Unsorted,
    /// Live snippets directly inside one folder.
    Folder(&'a str),
}

impl<'a> ListScope<'a> {
    /// Static WHERE fragment. Only compile-time constants reach the SQL text;
    /// the folder id travels as the `:folder` bind parameter.
    fn where_clause(self) -> &'static str {
        match self {
            ListScope::All => "1 = 1",
            ListScope::Recent => "last_used_at IS NOT NULL",
            ListScope::Starred => "is_favorite = 1",
            ListScope::Unsorted => "folder_id IS NULL",
            ListScope::Folder(_) => "folder_id = :folder",
        }
    }

    /// Recent orders by usage recency; every other scope by last update.
    fn order_clause(self) -> &'static str {
        match self {
            ListScope::Recent => "last_used_at DESC, id",
            _ => "updated_at DESC, id",
        }
    }

    fn folder_id(self) -> Option<&'a str> {
        match self {
            ListScope::Folder(id) => Some(id),
            _ => None,
        }
    }
}

impl<'c> SnippetRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated snippet.
    pub fn insert(&self, snippet: &Snippet) -> Result<(), RepoError> {
        snippet.validate()?;
        let (plaintext, ciphertext) = content_columns(&snippet.content);
        self.conn.execute(
            "INSERT INTO snippet (
                id, workspace_id, title, content_plaintext, content_ciphertext,
                type, description, folder_id, \"trigger\", trigger_mode, language,
                security_level, is_favorite, is_pinned, is_enabled, platform_scope,
                created_at, updated_at, last_used_at, usage_count, version, deleted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                snippet.id,
                snippet.workspace_id,
                snippet.title,
                plaintext,
                ciphertext,
                snippet.snippet_type.as_str(),
                snippet.description,
                snippet.folder_id,
                snippet.trigger,
                snippet.trigger_mode.map(|m| m.as_str()),
                snippet.language,
                snippet.security_level.as_str(),
                snippet.is_favorite,
                snippet.is_pinned,
                snippet.is_enabled,
                platforms_to_json(&snippet.platform_scope),
                snippet.created_at,
                snippet.updated_at,
                snippet.last_used_at,
                usage_to_db(snippet.usage_count),
                snippet.version,
                snippet.deleted_at,
            ],
        )?;
        Ok(())
    }

    /// Loads one snippet by id.
    pub fn get(&self, id: &str) -> Result<Option<Snippet>, RepoError> {
        self.conn
            .query_row(
                &format!("SELECT {SELECT_COLUMNS} FROM snippet WHERE id = ?1"),
                params![id],
                row_to_snippet,
            )
            .optional()?
            .transpose()
    }

    /// Replaces all row columns of an existing snippet.
    pub fn update(&self, snippet: &Snippet) -> Result<(), RepoError> {
        snippet.validate()?;
        let (plaintext, ciphertext) = content_columns(&snippet.content);
        let changed = self.conn.execute(
            "UPDATE snippet SET
                workspace_id = ?2, title = ?3, content_plaintext = ?4,
                content_ciphertext = ?5, type = ?6, description = ?7, folder_id = ?8,
                \"trigger\" = ?9, trigger_mode = ?10, language = ?11, security_level = ?12,
                is_favorite = ?13, is_pinned = ?14, is_enabled = ?15, platform_scope = ?16,
                created_at = ?17, updated_at = ?18, last_used_at = ?19, usage_count = ?20,
                version = ?21, deleted_at = ?22
             WHERE id = ?1",
            params![
                snippet.id,
                snippet.workspace_id,
                snippet.title,
                plaintext,
                ciphertext,
                snippet.snippet_type.as_str(),
                snippet.description,
                snippet.folder_id,
                snippet.trigger,
                snippet.trigger_mode.map(|m| m.as_str()),
                snippet.language,
                snippet.security_level.as_str(),
                snippet.is_favorite,
                snippet.is_pinned,
                snippet.is_enabled,
                platforms_to_json(&snippet.platform_scope),
                snippet.created_at,
                snippet.updated_at,
                snippet.last_used_at,
                usage_to_db(snippet.usage_count),
                snippet.version,
                snippet.deleted_at,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Hard-deletes a snippet (recycle-bin soft delete arrives with TASK-018).
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM snippet WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Lists live snippets ordered by most recent update, bounded by
    /// limit/offset. Recycle-bin rows are excluded.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL
             ORDER BY updated_at DESC, id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Lists the live snippets directly inside a folder (`None` = unfiled).
    pub fn list_by_folder(
        &self,
        folder_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL
               AND ((?1 IS NULL AND folder_id IS NULL) OR folder_id = ?1)
             ORDER BY updated_at DESC, id LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![folder_id, limit, offset], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Lists live snippets in a Library scope, optionally narrowed to one
    /// snippet type, bounded by limit/offset.
    pub fn list_scoped(
        &self,
        scope: ListScope<'_>,
        snippet_type: Option<SnippetType>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, RepoError> {
        let type_text = snippet_type.map(|t| t.as_str());
        let folder = scope.folder_id();
        let mut sql = format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL AND {}",
            scope.where_clause()
        );
        if type_text.is_some() {
            sql.push_str(" AND type = :type");
        }
        sql.push_str(&format!(
            " ORDER BY {} LIMIT :limit OFFSET :offset",
            scope.order_clause()
        ));

        let mut binds: Vec<(&str, &dyn ToSql)> = vec![(":limit", &limit), (":offset", &offset)];
        if let Some(folder) = folder.as_ref() {
            binds.push((":folder", folder));
        }
        if let Some(type_text) = type_text.as_ref() {
            binds.push((":type", type_text));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(&binds[..], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Counts live snippets in a Library scope (same filters as
    /// [`Self::list_scoped`]).
    pub fn count_scoped(
        &self,
        scope: ListScope<'_>,
        snippet_type: Option<SnippetType>,
    ) -> Result<u32, RepoError> {
        let type_text = snippet_type.map(|t| t.as_str());
        let folder = scope.folder_id();
        let mut sql = format!(
            "SELECT COUNT(*) FROM snippet WHERE deleted_at IS NULL AND {}",
            scope.where_clause()
        );
        if type_text.is_some() {
            sql.push_str(" AND type = :type");
        }
        let mut binds: Vec<(&str, &dyn ToSql)> = Vec::new();
        if let Some(folder) = folder.as_ref() {
            binds.push((":folder", folder));
        }
        if let Some(type_text) = type_text.as_ref() {
            binds.push((":type", type_text));
        }
        let count = self
            .conn
            .prepare(&sql)?
            .query_row(&binds[..], |row| row.get::<_, u32>(0))?;
        Ok(count)
    }

    /// Counts recycle-bin rows.
    pub fn count_trashed(&self) -> Result<u32, RepoError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM snippet WHERE deleted_at IS NOT NULL",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        Ok(count)
    }

    /// Per-folder live-snippet counts (folders with zero snippets are simply
    /// absent). Feeds the Library rail without one query per folder.
    pub fn count_by_folder(&self) -> Result<Vec<(String, u32)>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT folder_id, COUNT(*) FROM snippet
             WHERE deleted_at IS NULL AND folder_id IS NOT NULL
             GROUP BY folder_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;
        let mut counts = Vec::new();
        for row in rows {
            counts.push(row?);
        }
        Ok(counts)
    }

    /// Moves a live snippet into the recycle bin.
    pub fn soft_delete(&self, id: &str, deleted_at: TimestampMs) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE snippet SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, deleted_at],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Restores a snippet from the recycle bin.
    pub fn restore_from_trash(&self, id: &str) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE snippet SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Lists recycle-bin contents, most recently deleted first.
    pub fn list_trashed(&self, limit: u32, offset: u32) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NOT NULL
             ORDER BY deleted_at DESC, id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Permanently deletes recycle-bin rows whose retention expired; returns
    /// how many rows were purged. Callers pass `now` and a retention window
    /// (default [`TRASH_RETENTION_MS`]).
    pub fn purge_expired_trash(
        &self,
        now: TimestampMs,
        retention_ms: i64,
    ) -> Result<usize, RepoError> {
        let cutoff = now.saturating_sub(retention_ms);
        let purged = self.conn.execute(
            "DELETE FROM snippet WHERE deleted_at IS NOT NULL AND deleted_at <= ?1",
            params![cutoff],
        )?;
        Ok(purged)
    }

    /// Toggles favorite state.
    pub fn set_favorite(&self, id: &str, value: bool) -> Result<(), RepoError> {
        self.set_flag("is_favorite", id, value)
    }

    /// Toggles pinned state.
    pub fn set_pinned(&self, id: &str, value: bool) -> Result<(), RepoError> {
        self.set_flag("is_pinned", id, value)
    }

    /// Toggles enabled state.
    pub fn set_enabled(&self, id: &str, value: bool) -> Result<(), RepoError> {
        self.set_flag("is_enabled", id, value)
    }

    fn set_flag(&self, column: &str, id: &str, value: bool) -> Result<(), RepoError> {
        // `column` is one of three fixed identifiers above, never user input.
        let changed = self.conn.execute(
            &format!("UPDATE snippet SET {column} = ?2 WHERE id = ?1"),
            params![id, value],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Moves a batch of snippets into a folder (`None` = unfiled).
    /// Atomic: any missing snippet aborts the whole batch.
    pub fn batch_move(&self, ids: &[SnippetId], folder_id: Option<&str>) -> Result<(), RepoError> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            let changed = tx.execute(
                "UPDATE snippet SET folder_id = ?2 WHERE id = ?1",
                params![id, folder_id],
            )?;
            if changed == 0 {
                return Err(RepoError::NotFound);
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Enables or disables a batch of snippets atomically.
    pub fn batch_set_enabled(&self, ids: &[SnippetId], value: bool) -> Result<(), RepoError> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            let changed = tx.execute(
                "UPDATE snippet SET is_enabled = ?2 WHERE id = ?1",
                params![id, value],
            )?;
            if changed == 0 {
                return Err(RepoError::NotFound);
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Attaches a tag to a batch of snippets; already-tagged pairs are kept.
    pub fn batch_add_tag(&self, ids: &[SnippetId], tag_id: &str) -> Result<(), RepoError> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "INSERT OR IGNORE INTO snippet_tag (snippet_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Detaches a tag from a batch of snippets.
    pub fn batch_remove_tag(&self, ids: &[SnippetId], tag_id: &str) -> Result<(), RepoError> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "DELETE FROM snippet_tag WHERE snippet_id = ?1 AND tag_id = ?2",
                params![id, tag_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Tag ids attached to a snippet.
    pub fn tag_ids_of(&self, snippet_id: &str) -> Result<Vec<TagId>, RepoError> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag_id FROM snippet_tag WHERE snippet_id = ?1 ORDER BY tag_id")?;
        let rows = stmt.query_map(params![snippet_id], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Records one usage: bumps the counter and the last-used time in a
    /// single lightweight statement, independent of content writes.
    pub fn record_usage(&self, id: &str, used_at: TimestampMs) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE snippet SET usage_count = usage_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![id, used_at],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Returns the id of an enabled snippet already owning `trigger`,
    /// ignoring `exclude_id` (the snippet being edited).
    pub fn find_trigger_conflict(
        &self,
        trigger: &str,
        exclude_id: Option<&str>,
    ) -> Result<Option<SnippetId>, RepoError> {
        let found = self
            .conn
            .query_row(
                "SELECT id FROM snippet
                 WHERE \"trigger\" = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR id <> ?2)
                 LIMIT 1",
                params![trigger, exclude_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(found)
    }

    /// Returns ids of snippets that look like duplicates: same title or the
    /// same plaintext body. Sensitive bodies are ciphertext and intentionally
    /// excluded from content comparison.
    pub fn find_duplicates(
        &self,
        title: &str,
        content_plaintext: Option<&str>,
        exclude_id: Option<&str>,
    ) -> Result<Vec<SnippetId>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM snippet
             WHERE deleted_at IS NULL
               AND (title = ?1 OR (?2 IS NOT NULL AND content_plaintext = ?2))
               AND (?3 IS NULL OR id <> ?3)
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![title, content_plaintext, exclude_id], |row| {
            row.get::<_, String>(0)
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// SQLite integers are i64; the schema CHECK keeps the column non-negative,
/// and values beyond i64::MAX are unreachable in practice (saturating).
fn usage_to_db(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn usage_from_db(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn content_columns(content: &SnippetContent) -> (Option<&str>, Option<&[u8]>) {
    match content {
        SnippetContent::Plaintext(text) => (Some(text.as_str()), None),
        SnippetContent::Ciphertext(bytes) => (None, Some(bytes.as_slice())),
    }
}

fn platforms_to_json(platforms: &[Platform]) -> String {
    let names: Vec<&str> = platforms.iter().map(Platform::as_str).collect();
    // Serializing a list of static strings cannot fail.
    serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
}

fn platforms_from_json(json: &str) -> Result<Vec<Platform>, RepoError> {
    let names: Vec<String> = serde_json::from_str(json)
        .map_err(|_| RepoError::Conflict("platform_scope column is not a JSON string array"))?;
    names
        .iter()
        .map(|n| n.parse::<Platform>().map_err(RepoError::from))
        .collect()
}

type SnippetRowResult = Result<Snippet, RepoError>;

fn row_to_snippet(row: &Row<'_>) -> rusqlite::Result<SnippetRowResult> {
    let plaintext: Option<String> = row.get("content_plaintext")?;
    let ciphertext: Option<Vec<u8>> = row.get("content_ciphertext")?;
    let type_text: String = row.get("type")?;
    let security_text: String = row.get("security_level")?;
    let trigger_mode_text: Option<String> = row.get("trigger_mode")?;
    let platform_json: String = row.get("platform_scope")?;

    let base = Snippet {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        title: row.get("title")?,
        content: SnippetContent::Plaintext(String::new()),
        snippet_type: SnippetType::Text,
        description: row.get("description")?,
        folder_id: row.get("folder_id")?,
        trigger: row.get("trigger")?,
        trigger_mode: None,
        language: row.get("language")?,
        security_level: SecurityLevel::Normal,
        is_favorite: row.get("is_favorite")?,
        is_pinned: row.get("is_pinned")?,
        is_enabled: row.get("is_enabled")?,
        platform_scope: vec![],
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        last_used_at: row.get("last_used_at")?,
        usage_count: usage_from_db(row.get("usage_count")?),
        version: row.get("version")?,
        deleted_at: row.get("deleted_at")?,
    };

    Ok(finish_snippet(
        base,
        plaintext,
        ciphertext,
        type_text,
        security_text,
        trigger_mode_text,
        platform_json,
    ))
}

/// Second mapping stage outside the rusqlite closure so enum/JSON failures
/// surface as `RepoError` instead of being shoehorned into rusqlite errors.
fn finish_snippet(
    mut snippet: Snippet,
    plaintext: Option<String>,
    ciphertext: Option<Vec<u8>>,
    type_text: String,
    security_text: String,
    trigger_mode_text: Option<String>,
    platform_json: String,
) -> SnippetRowResult {
    snippet.content = match (plaintext, ciphertext) {
        (Some(text), None) => SnippetContent::Plaintext(text),
        (None, Some(bytes)) => SnippetContent::Ciphertext(bytes),
        // Unreachable while the schema CHECK holds; classified as conflict
        // rather than panicking on a corrupted row.
        _ => {
            return Err(RepoError::Conflict(
                "snippet row has invalid content columns",
            ));
        }
    };
    snippet.snippet_type = type_text.parse()?;
    snippet.security_level = security_text.parse()?;
    snippet.trigger_mode = trigger_mode_text
        .map(|t| t.parse::<TriggerMode>())
        .transpose()?;
    snippet.platform_scope = platforms_from_json(&platform_json)?;
    Ok(snippet)
}

fn collect_snippets(
    rows: impl Iterator<Item = rusqlite::Result<SnippetRowResult>>,
) -> Result<Vec<Snippet>, RepoError> {
    let mut snippets = Vec::new();
    for row in rows {
        snippets.push(row??);
    }
    Ok(snippets)
}
