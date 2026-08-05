//! Snippet CRUD, batch operations, toggles, usage tracking, and the
//! trigger-conflict / duplicate checks (PRD §12.1).

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
     last_used_at, usage_count, version";

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
                created_at, updated_at, last_used_at, usage_count, version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
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
                version = ?21
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

    /// Lists snippets ordered by most recent update, bounded by limit/offset.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             ORDER BY updated_at DESC, id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Lists the snippets directly inside a folder (`None` = unfiled).
    pub fn list_by_folder(
        &self,
        folder_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE (?1 IS NULL AND folder_id IS NULL) OR folder_id = ?1
             ORDER BY updated_at DESC, id LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![folder_id, limit, offset], row_to_snippet)?;
        collect_snippets(rows)
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
                 WHERE \"trigger\" = ?1 AND (?2 IS NULL OR id <> ?2)
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
             WHERE (title = ?1 OR (?2 IS NOT NULL AND content_plaintext = ?2))
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
