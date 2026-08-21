// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Semantic-embedding storage.
//! Derived data: vectors are regenerable from the local model, never enter
//! sync payloads or snapshots. Red line (stricter than FTS): a sensitive
//! snippet is never embedded — `upsert` refuses non-normal rows, and the
//! pending query structurally excludes them.

use rusqlite::{Connection, OptionalExtension, params};

use super::RepoError;
use crate::model::TimestampMs;

/// One stored vector, f32 little-endian bytes of length `dims * 4`.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEmbedding {
    pub snippet_id: String,
    pub dims: u32,
    pub vector: Vec<u8>,
}

/// A snippet waiting to be embedded (missing or stale vector).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEmbedding {
    pub snippet_id: String,
    pub title: String,
    pub body: String,
}

/// Embedding repository over a single connection.
pub struct EmbeddingRepo<'c> {
    conn: &'c Connection,
}

impl<'c> EmbeddingRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts or replaces the vector for one snippet. Refuses any row
    /// whose security level is not `normal` (red line) and any
    /// vector whose byte length disagrees with `dims`.
    pub fn upsert(
        &self,
        snippet_id: &str,
        model_id: &str,
        dims: u32,
        vector: &[u8],
        now: TimestampMs,
    ) -> Result<(), RepoError> {
        if dims == 0 || vector.len() != dims as usize * 4 {
            return Err(RepoError::Conflict("vector length must equal dims * 4"));
        }
        let security_level: Option<String> = self
            .conn
            .query_row(
                "SELECT security_level FROM snippet WHERE id = ?1",
                params![snippet_id],
                |row| row.get(0),
            )
            .optional()?;
        match security_level.as_deref() {
            None => return Err(RepoError::NotFound),
            Some("normal") => {}
            Some(_) => {
                return Err(RepoError::Conflict(
                    "only normal-security snippets may be embedded",
                ));
            }
        }
        self.conn.execute(
            "INSERT INTO snippet_embedding (snippet_id, model_id, dims, vector, embedded_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(snippet_id) DO UPDATE SET
               model_id = excluded.model_id,
               dims = excluded.dims,
               vector = excluded.vector,
               embedded_at = excluded.embedded_at",
            params![snippet_id, model_id, dims, vector, now],
        )?;
        Ok(())
    }

    /// All vectors produced by `model_id` for live, enabled, normal rows —
    /// the set the in-memory scan serves from.
    pub fn list_for_model(&self, model_id: &str) -> Result<Vec<StoredEmbedding>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT e.snippet_id, e.dims, e.vector
             FROM snippet_embedding e
             JOIN snippet s ON s.id = e.snippet_id
             WHERE e.model_id = ?1
               AND s.deleted_at IS NULL
               AND s.is_enabled = 1
               AND s.security_level = 'normal'
             ORDER BY e.snippet_id",
        )?;
        let rows = stmt.query_map(params![model_id], |row| {
            Ok(StoredEmbedding {
                snippet_id: row.get(0)?,
                dims: row.get(1)?,
                vector: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Snippets that need (re-)embedding under `model_id`: live normal rows
    /// with no vector for this model, or a vector older than the last edit.
    /// Sensitive and deleted rows are structurally excluded (red line).
    pub fn list_pending(
        &self,
        model_id: &str,
        limit: u32,
    ) -> Result<Vec<PendingEmbedding>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.title, s.content_plaintext
             FROM snippet s
             LEFT JOIN snippet_embedding e
               ON e.snippet_id = s.id AND e.model_id = ?1
             WHERE s.deleted_at IS NULL
               AND s.security_level = 'normal'
               AND s.content_plaintext IS NOT NULL
               AND (e.snippet_id IS NULL OR e.embedded_at < s.updated_at)
             ORDER BY s.updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![model_id, limit], |row| {
            Ok(PendingEmbedding {
                snippet_id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Number of live normal rows still missing a fresh vector under
    /// `model_id` (settings display; same predicate as `list_pending`).
    pub fn count_pending(&self, model_id: &str) -> Result<i64, RepoError> {
        let n = self.conn.query_row(
            "SELECT COUNT(*)
             FROM snippet s
             LEFT JOIN snippet_embedding e
               ON e.snippet_id = s.id AND e.model_id = ?1
             WHERE s.deleted_at IS NULL
               AND s.security_level = 'normal'
               AND s.content_plaintext IS NOT NULL
               AND (e.snippet_id IS NULL OR e.embedded_at < s.updated_at)",
            params![model_id],
            |row| row.get(0),
        )?;
        Ok(n)
    }

    /// Drops every stored vector (model deleted / semantic search turned
    /// off). The table is derived data; this is always safe.
    pub fn clear(&self) -> Result<usize, RepoError> {
        Ok(self.conn.execute("DELETE FROM snippet_embedding", [])?)
    }

    /// Number of stored vectors for `model_id` (settings display).
    pub fn count_for_model(&self, model_id: &str) -> Result<i64, RepoError> {
        let n = self.conn.query_row(
            "SELECT COUNT(*) FROM snippet_embedding WHERE model_id = ?1",
            params![model_id],
            |row| row.get(0),
        )?;
        Ok(n)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, TriggerMode};
    use crate::repo::SnippetRepo;

    const NOW: TimestampMs = 1_700_000_000_000;
    const MODEL: &str = "e5-small-v1";

    fn conn() -> Connection {
        let mut conn = crate::db::open_in_memory().unwrap();
        crate::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn snippet(id: &str, security: SecurityLevel, body: SnippetContent) -> Snippet {
        Snippet {
            id: id.to_string(),
            workspace_id: "w1".to_string(),
            title: format!("Title {id}"),
            content: body,
            snippet_type: SnippetType::Text,
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None::<TriggerMode>,
            language: None,
            security_level: security,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: Vec::new(),
            created_at: NOW,
            updated_at: NOW,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        }
    }

    fn insert_normal(conn: &Connection, id: &str) {
        SnippetRepo::new(conn)
            .insert(&snippet(
                id,
                SecurityLevel::Normal,
                SnippetContent::Plaintext(format!("body {id}")),
            ))
            .unwrap();
    }

    fn vector_bytes(dims: u32) -> Vec<u8> {
        (0..dims).flat_map(|i| (i as f32).to_le_bytes()).collect()
    }

    #[test]
    fn upsert_stores_and_replaces_a_vector() {
        let conn = conn();
        insert_normal(&conn, "s1");
        let repo = EmbeddingRepo::new(&conn);

        repo.upsert("s1", MODEL, 4, &vector_bytes(4), NOW).unwrap();
        repo.upsert("s1", MODEL, 4, &vector_bytes(4), NOW + 1)
            .unwrap();

        let stored = repo.list_for_model(MODEL).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].dims, 4);
        assert_eq!(repo.count_for_model(MODEL).unwrap(), 1);
    }

    #[test]
    fn a_sensitive_snippet_is_refused_and_never_stored() {
        let conn = conn();
        SnippetRepo::new(&conn)
            .insert(&snippet(
                "sec1",
                SecurityLevel::Sensitive,
                SnippetContent::Ciphertext(vec![1, 2, 3]),
            ))
            .unwrap();
        let repo = EmbeddingRepo::new(&conn);

        let error = repo
            .upsert("sec1", MODEL, 4, &vector_bytes(4), NOW)
            .unwrap_err();
        assert!(matches!(error, RepoError::Conflict(_)));
        // Red line assertion: zero rows for the sensitive snippet.
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM snippet_embedding", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn a_wrong_length_vector_is_refused() {
        let conn = conn();
        insert_normal(&conn, "s1");
        let repo = EmbeddingRepo::new(&conn);
        let error = repo.upsert("s1", MODEL, 4, &[0u8; 15], NOW).unwrap_err();
        assert!(matches!(error, RepoError::Conflict(_)));
    }

    #[test]
    fn pending_lists_missing_and_stale_rows_only() {
        let conn = conn();
        insert_normal(&conn, "s1");
        insert_normal(&conn, "s2");
        // A sensitive row must never appear in the pending queue.
        SnippetRepo::new(&conn)
            .insert(&snippet(
                "sec1",
                SecurityLevel::Sensitive,
                SnippetContent::Ciphertext(vec![9]),
            ))
            .unwrap();
        let repo = EmbeddingRepo::new(&conn);
        repo.upsert("s1", MODEL, 4, &vector_bytes(4), NOW).unwrap();

        let pending = repo.list_pending(MODEL, 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].snippet_id, "s2");
        assert_eq!(pending[0].body, "body s2");

        // An edit after embedding makes the row stale again.
        conn.execute(
            "UPDATE snippet SET updated_at = ?1 WHERE id = 's1'",
            params![NOW + 10],
        )
        .unwrap();
        let pending = repo.list_pending(MODEL, 10).unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn vectors_cascade_away_with_a_physical_snippet_delete() {
        let conn = conn();
        insert_normal(&conn, "s1");
        let repo = EmbeddingRepo::new(&conn);
        repo.upsert("s1", MODEL, 4, &vector_bytes(4), NOW).unwrap();

        conn.execute("DELETE FROM snippet WHERE id = 's1'", [])
            .unwrap();
        assert_eq!(repo.count_for_model(MODEL).unwrap(), 0);
    }

    #[test]
    fn clear_drops_everything_and_serving_excludes_dead_rows() {
        let conn = conn();
        insert_normal(&conn, "s1");
        insert_normal(&conn, "s2");
        let repo = EmbeddingRepo::new(&conn);
        repo.upsert("s1", MODEL, 4, &vector_bytes(4), NOW).unwrap();
        repo.upsert("s2", MODEL, 4, &vector_bytes(4), NOW).unwrap();

        // Trashed rows keep their vector but leave the serving set.
        conn.execute(
            "UPDATE snippet SET deleted_at = ?1 WHERE id = 's2'",
            params![NOW + 1],
        )
        .unwrap();
        assert_eq!(repo.list_for_model(MODEL).unwrap().len(), 1);

        assert_eq!(repo.clear().unwrap(), 2);
        assert_eq!(repo.count_for_model(MODEL).unwrap(), 0);
    }
}
