// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Snippet CRUD, batch operations, toggles, usage tracking, and the
//! trigger-conflict / duplicate checks.

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
     last_used_at, usage_count, version, deleted_at, conflict_of";

/// Recycle-bin retention before automatic cleanup (30 days).
pub const TRASH_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// One Library list scope: a saved view or a single folder. Trashed rows are
/// excluded from every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListScope<'a> {
    /// Every live snippet.
    All,
    /// Snippets used at least once, most recently used first.
    Recent,
    /// Snippets used at least once, most often used first.
    ///
    /// Not a variation on Recent. "What I reached for last" and "what I reach
    /// for" are different questions, and a screen headed *most used* that is
    /// really showing *last used* answers neither.
    Used,
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
            ListScope::Used => "usage_count > 0",
            ListScope::Starred => "is_favorite = 1",
            ListScope::Unsorted => "folder_id IS NULL",
            ListScope::Folder(_) => "folder_id = :folder",
        }
    }

    /// Recent orders by usage recency, Used by usage count; every other scope
    /// by last update. Each order ends in `id` so a page boundary cannot
    /// duplicate or skip a row when two snippets tie.
    fn order_clause(self) -> &'static str {
        match self {
            ListScope::Recent => "last_used_at DESC, id",
            // The recency tiebreak matters: with a young library most counts
            // are 1, and without it the list would be ordered by nothing the
            // reader can see.
            ListScope::Used => "usage_count DESC, last_used_at DESC, id",
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

/// A reader-chosen order for a Library list, independent of its scope: the
/// same three orders apply inside every collection, not only in the saved
/// views that happen to be ordered that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOrder {
    /// Most recently used first; snippets never used follow, newest edit first.
    LastUsed,
    /// Most recently created first.
    Created,
    /// Most often used first, recency breaking ties.
    UsageCount,
}

impl ListOrder {
    /// Each order ends in `id` so a page boundary cannot duplicate or skip a
    /// row. SQLite sorts NULL below every value, so the descending recency
    /// order already puts never-used snippets last — an `IS NULL` term would
    /// only stop the index from answering the order.
    fn order_clause(self) -> &'static str {
        match self {
            ListOrder::LastUsed => "last_used_at DESC, updated_at DESC, id",
            ListOrder::Created => "created_at DESC, id",
            ListOrder::UsageCount => "usage_count DESC, last_used_at DESC, id",
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
                created_at, updated_at, last_used_at, usage_count, version, deleted_at,
                conflict_of
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
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
                snippet.conflict_of,
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
                version = ?21, deleted_at = ?22, conflict_of = ?23
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
                snippet.conflict_of,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Hard-deletes a snippet.
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

    /// Lists the live, enabled, normal-security snippets that carry a trigger —
    /// exactly the set the Espanso adapter compiles. Sensitive snippets,
    /// disabled snippets and recycle-bin rows are excluded at the query level
    /// (the compiler re-checks the same invariants). Ordered deterministically
    /// so the generated config is stable across regenerations.
    pub fn list_triggered_active(&self) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL
               AND is_enabled = 1
               AND security_level = 'normal'
               AND \"trigger\" IS NOT NULL
             ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Lists the live, enabled snippets of every security level — exactly
    /// the keyboard-snapshot content set.
    /// Sensitive rows come back as ciphertext, as stored. Ordered
    /// deterministically so identical database state yields identical
    /// snapshots.
    pub fn list_snapshot_active(&self) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL AND is_enabled = 1
             ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Lists the live conflict copies awaiting the user's decision — the
    /// rows a three-way merge parked with `conflict_of` pointing at the
    /// snippet whose body they lost to. Oldest first, so the resolution
    /// screen works through them in the order they arrived.
    pub fn list_conflict_copies(&self) -> Result<Vec<Snippet>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL AND conflict_of IS NOT NULL
             ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], row_to_snippet)?;
        collect_snippets(rows)
    }

    /// Clears a conflict copy's `conflict_of` marker, promoting it to an
    /// ordinary snippet. Used by the resolution write path; leaves every
    /// other column untouched so no version is implied.
    pub fn clear_conflict_of(&self, id: &str) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE snippet SET conflict_of = NULL WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
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
    /// snippet type, bounded by limit/offset. Sensitive snippets appear only
    /// when the type filter asks for them explicitly (the vault list) —
    /// every other scope excludes them.
    pub fn list_scoped(
        &self,
        scope: ListScope<'_>,
        snippet_type: Option<SnippetType>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, RepoError> {
        self.list_scoped_where(scope, snippet_type, None, limit, offset, false)
    }

    /// [`Self::list_scoped`] in a reader-chosen order rather than the scope's
    /// own; the filters are identical.
    pub fn list_scoped_ordered(
        &self,
        scope: ListScope<'_>,
        snippet_type: Option<SnippetType>,
        order: ListOrder,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, RepoError> {
        self.list_scoped_where(scope, snippet_type, Some(order), limit, offset, false)
    }

    /// Recent list for the panel calling surface: same Recent semantics as
    /// [`Self::list_scoped`], but sensitive rows stay listed — the panel is
    /// where the vault verify-then-insert flow lives, unlike the Library
    /// (that rule covers Library entry points only).
    pub fn list_recent_including_sensitive(&self, limit: u32) -> Result<Vec<Snippet>, RepoError> {
        self.list_scoped_where(ListScope::Recent, None, None, limit, 0, true)
    }

    fn list_scoped_where(
        &self,
        scope: ListScope<'_>,
        snippet_type: Option<SnippetType>,
        order: Option<ListOrder>,
        limit: u32,
        offset: u32,
        include_sensitive: bool,
    ) -> Result<Vec<Snippet>, RepoError> {
        let type_text = snippet_type.map(|t| t.as_str());
        let folder = scope.folder_id();
        let sql = scoped_list_sql(scope, type_text, order, include_sensitive);

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
            "SELECT COUNT(*) FROM snippet
             WHERE deleted_at IS NULL{} AND {}",
            sensitive_exclusion(type_text),
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

    /// Every sensitive snippet id, trashed rows included — the vault-reset
    /// deletion set: once the vault key material is destroyed their
    /// ciphertext is unrecoverable, so no row may stay behind.
    pub fn list_sensitive_ids(&self) -> Result<Vec<String>, RepoError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM snippet WHERE security_level = 'sensitive' ORDER BY id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok(ids)
    }

    /// Active temporary snippets whose last edit is older than `ttl_ms` —
    /// the expiry sweep's candidate set. Only the
    /// `temporary` type is ever affected.
    pub fn list_expired_temporary(
        &self,
        now: TimestampMs,
        ttl_ms: i64,
    ) -> Result<Vec<String>, RepoError> {
        let cutoff = now.saturating_sub(ttl_ms);
        let mut stmt = self.conn.prepare(
            "SELECT id FROM snippet \
             WHERE deleted_at IS NULL AND type = 'temporary' AND updated_at <= ?1 \
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok(ids)
    }

    /// Refreshes the edit clock without touching content — a restored
    /// temporary snippet restarts its expiry window.
    pub fn touch_updated_at(&self, id: &str, now: TimestampMs) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE snippet SET updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
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
        self.in_batch_transaction(|conn| {
            for id in ids {
                let changed = conn.execute(
                    "UPDATE snippet SET folder_id = ?2 WHERE id = ?1",
                    params![id, folder_id],
                )?;
                if changed == 0 {
                    return Err(RepoError::NotFound);
                }
            }
            Ok(())
        })
    }

    /// Runs a batch mutation atomically: inside a caller's transaction it
    /// joins it (the caller owns atomicity, e.g. for the sync outbox hook);
    /// otherwise it opens its own.
    fn in_batch_transaction(
        &self,
        work: impl FnOnce(&Connection) -> Result<(), RepoError>,
    ) -> Result<(), RepoError> {
        if self.conn.is_autocommit() {
            let tx = self.conn.unchecked_transaction()?;
            work(&tx)?;
            tx.commit()?;
            Ok(())
        } else {
            work(self.conn)
        }
    }

    /// Enables or disables a batch of snippets atomically.
    pub fn batch_set_enabled(&self, ids: &[SnippetId], value: bool) -> Result<(), RepoError> {
        self.in_batch_transaction(|conn| {
            for id in ids {
                let changed = conn.execute(
                    "UPDATE snippet SET is_enabled = ?2 WHERE id = ?1",
                    params![id, value],
                )?;
                if changed == 0 {
                    return Err(RepoError::NotFound);
                }
            }
            Ok(())
        })
    }

    /// Attaches one tag to one snippet — a single idempotent statement, safe
    /// inside a caller's transaction (used by the backup restore).
    pub fn add_tag(&self, snippet_id: &str, tag_id: &str) -> Result<(), RepoError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO snippet_tag (snippet_id, tag_id) VALUES (?1, ?2)",
            params![snippet_id, tag_id],
        )?;
        Ok(())
    }

    /// Attaches one tag to a batch of snippets; already-tagged pairs are kept.
    pub fn batch_add_tag(&self, ids: &[SnippetId], tag_id: &str) -> Result<(), RepoError> {
        self.in_batch_transaction(|conn| {
            for id in ids {
                conn.execute(
                    "INSERT OR IGNORE INTO snippet_tag (snippet_id, tag_id) VALUES (?1, ?2)",
                    params![id, tag_id],
                )?;
            }
            Ok(())
        })
    }

    /// Detaches a tag from a batch of snippets.
    pub fn batch_remove_tag(&self, ids: &[SnippetId], tag_id: &str) -> Result<(), RepoError> {
        self.in_batch_transaction(|conn| {
            for id in ids {
                conn.execute(
                    "DELETE FROM snippet_tag WHERE snippet_id = ?1 AND tag_id = ?2",
                    params![id, tag_id],
                )?;
            }
            Ok(())
        })
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

/// Library queries never see sensitive rows unless the caller asked for the
/// sensitive type explicitly — that request is the vault's own list.
/// Returns a constant SQL fragment; no external input involved.
fn sensitive_exclusion(type_text: Option<&str>) -> &'static str {
    if type_text == Some("sensitive") {
        ""
    } else {
        " AND security_level != 'sensitive'"
    }
}

/// Builds one scoped page query.
///
/// Only compile-time constants reach the SQL text: the folder id and the type
/// both travel as bind parameters. Separate from the call that runs it so a
/// test can assert the query plan of the statement the repository actually
/// issues, rather than of a copy that is free to drift away from it.
fn scoped_list_sql(
    scope: ListScope<'_>,
    type_text: Option<&str>,
    order: Option<ListOrder>,
    include_sensitive: bool,
) -> String {
    let mut sql = format!(
        "SELECT {SELECT_COLUMNS} FROM snippet
             WHERE deleted_at IS NULL{} AND {}",
        if include_sensitive {
            ""
        } else {
            sensitive_exclusion(type_text)
        },
        scope.where_clause()
    );
    if type_text.is_some() {
        sql.push_str(" AND type = :type");
    }
    sql.push_str(&format!(
        " ORDER BY {} LIMIT :limit OFFSET :offset",
        order.map_or(scope.order_clause(), ListOrder::order_clause)
    ));
    sql
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
        conflict_of: row.get("conflict_of")?,
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

#[cfg(test)]
mod plan_tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};

    fn plan(sql: &str) -> String {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let mut stmt = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("the scoped query is valid SQL");
        // The plan does not run the query, but the statement still has to be
        // bound; the values are irrelevant to the plan SQLite picks.
        let limit: u32 = 3;
        let offset: u32 = 0;
        let binds: &[(&str, &dyn ToSql)] = &[(":limit", &limit), (":offset", &offset)];
        let rows = stmt
            .query_map(binds, |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.join(" | ")
    }

    /// The "most used" list has to hold up at the fifty-thousand-snippet
    /// target, and the thing that would quietly sink it is not the scan — it
    /// is the sort. Without an index covering the order, SQLite materialises
    /// every matching row into a temporary b-tree before it can hand back the
    /// first page, and that cost grows with the library while the page size
    /// stays at three.
    #[test]
    fn the_most_used_list_is_ordered_by_an_index_not_by_a_temporary_sort() {
        let plan = plan(&scoped_list_sql(ListScope::Used, None, None, false));

        assert!(
            plan.contains("idx_snippet_usage_count"),
            "expected the usage index, got: {plan}"
        );
        assert!(
            !plan.contains("TEMP B-TREE"),
            "the order must come from the index, got: {plan}"
        );
    }

    /// The reader-chosen orders face the same fifty-thousand-row library as
    /// the saved views, so they carry the same guarantee.
    #[test]
    fn reader_chosen_orders_over_the_whole_library_come_from_an_index() {
        for (order, index) in [
            (ListOrder::LastUsed, "idx_snippet_last_used_order"),
            (ListOrder::Created, "idx_snippet_created_order"),
            (ListOrder::UsageCount, "idx_snippet_usage_count"),
        ] {
            let plan = plan(&scoped_list_sql(ListScope::All, None, Some(order), false));
            assert!(
                plan.contains(index),
                "{order:?}: expected {index}, got: {plan}"
            );
            assert!(
                !plan.contains("TEMP B-TREE"),
                "{order:?}: the order must come from the index, got: {plan}"
            );
        }
    }
}
