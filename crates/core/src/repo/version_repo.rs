//! Append-only snippet version history (PRD §12.16).
//!
//! Every entry captures a past title/body state. Restore writes forward:
//! the restored state becomes a new snippet version and a new history entry,
//! so no existing version is ever lost. Retention is "keep everything" until
//! OQ-R5 is decided; `prune_versions` is the reserved cleanup interface.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::{SnippetContent, SnippetVersion, TimestampMs};

/// Version-history repository over a single connection.
pub struct VersionRepo<'c> {
    conn: &'c Connection,
}

impl<'c> VersionRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Appends a validated history entry. The (snippet, version) pair must
    /// be unused — history rows are never overwritten.
    pub fn append(&self, entry: &SnippetVersion) -> Result<(), RepoError> {
        entry.validate()?;
        let (plaintext, ciphertext) = match &entry.content {
            SnippetContent::Plaintext(text) => (Some(text.as_str()), None),
            SnippetContent::Ciphertext(bytes) => (None, Some(bytes.as_slice())),
        };
        match self.conn.execute(
            "INSERT INTO snippet_version (
                id, snippet_id, version, title, content_plaintext,
                content_ciphertext, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.id,
                entry.snippet_id,
                entry.version,
                entry.title,
                plaintext,
                ciphertext,
                entry.created_at,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(RepoError::Conflict("version already recorded"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Loads one history entry.
    pub fn get(&self, snippet_id: &str, version: u32) -> Result<Option<SnippetVersion>, RepoError> {
        self.conn
            .query_row(
                "SELECT id, snippet_id, version, title, content_plaintext,
                        content_ciphertext, created_at
                 FROM snippet_version WHERE snippet_id = ?1 AND version = ?2",
                params![snippet_id, version],
                row_to_version,
            )
            .optional()?
            .transpose()
    }

    /// Lists a snippet's history, newest version first, bounded.
    pub fn list(
        &self,
        snippet_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<SnippetVersion>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, snippet_id, version, title, content_plaintext,
                    content_ciphertext, created_at
             FROM snippet_version WHERE snippet_id = ?1
             ORDER BY version DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![snippet_id, limit, offset], row_to_version)?;
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row??);
        }
        Ok(versions)
    }

    /// Highest recorded version for a snippet (0 when no history exists).
    pub fn latest_version(&self, snippet_id: &str) -> Result<u32, RepoError> {
        let max: Option<u32> = self.conn.query_row(
            "SELECT MAX(version) FROM snippet_version WHERE snippet_id = ?1",
            params![snippet_id],
            |row| row.get(0),
        )?;
        Ok(max.unwrap_or(0))
    }

    /// Restores an old version by writing it forward: the snippet's title,
    /// body, and version advance to `max(version) + 1`, and the restored
    /// state is appended as that new history entry. Existing entries are
    /// untouched. Atomic.
    pub fn restore_version(
        &self,
        snippet_id: &str,
        version: u32,
        new_entry_id: &str,
        restored_at: TimestampMs,
    ) -> Result<u32, RepoError> {
        let tx = self.conn.unchecked_transaction()?;
        let old = self.get(snippet_id, version)?.ok_or(RepoError::NotFound)?;
        let next = self.latest_version(snippet_id)?.saturating_add(1);

        let (plaintext, ciphertext) = match &old.content {
            SnippetContent::Plaintext(text) => (Some(text.as_str()), None),
            SnippetContent::Ciphertext(bytes) => (None, Some(bytes.as_slice())),
        };
        let changed = tx.execute(
            "UPDATE snippet SET title = ?2, content_plaintext = ?3,
                    content_ciphertext = ?4, version = ?5, updated_at = ?6
             WHERE id = ?1",
            params![
                snippet_id,
                old.title,
                plaintext,
                ciphertext,
                next,
                restored_at
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        tx.execute(
            "INSERT INTO snippet_version (
                id, snippet_id, version, title, content_plaintext,
                content_ciphertext, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_entry_id,
                snippet_id,
                next,
                old.title,
                plaintext,
                ciphertext,
                restored_at
            ],
        )?;
        tx.commit()?;
        Ok(next)
    }

    /// Reserved cleanup interface (OQ-R5): deletes all but the newest
    /// `keep_latest` entries of a snippet. Not called by any policy yet —
    /// current behavior is keep-everything.
    pub fn prune_versions(&self, snippet_id: &str, keep_latest: u32) -> Result<usize, RepoError> {
        let removed = self.conn.execute(
            "DELETE FROM snippet_version
             WHERE snippet_id = ?1 AND version <= (
                 SELECT MAX(version) FROM snippet_version WHERE snippet_id = ?1
             ) - ?2",
            params![snippet_id, keep_latest],
        )?;
        Ok(removed)
    }
}

type VersionRowResult = Result<SnippetVersion, RepoError>;

fn row_to_version(row: &Row<'_>) -> rusqlite::Result<VersionRowResult> {
    let plaintext: Option<String> = row.get("content_plaintext")?;
    let ciphertext: Option<Vec<u8>> = row.get("content_ciphertext")?;
    let content = match (plaintext, ciphertext) {
        (Some(text), None) => SnippetContent::Plaintext(text),
        (None, Some(bytes)) => SnippetContent::Ciphertext(bytes),
        _ => {
            return Ok(Err(RepoError::Conflict(
                "version row has invalid content columns",
            )));
        }
    };
    Ok(Ok(SnippetVersion {
        id: row.get("id")?,
        snippet_id: row.get("snippet_id")?,
        version: row.get("version")?,
        title: row.get("title")?,
        content,
        created_at: row.get("created_at")?,
    }))
}
