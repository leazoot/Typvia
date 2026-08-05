//! FTS5 index maintenance: per-snippet incremental sync and full rebuild.
//!
//! `SOURCE_SELECT` below is the search crate's single read-only view over the
//! core tables (snippet, tag, folder); everything else here touches only
//! `snippet_fts`. The snippet ciphertext column is deliberately absent from
//! that view, so sensitive bodies can never reach this crate (red line).

use rusqlite::{Connection, OptionalExtension, Row, params};
use typvia_core::model::SecurityLevel;

use crate::error::SearchError;
use crate::segment::index_text;

/// Maintains `snippet_fts` for one connection. Trashed (`deleted_at` set)
/// snippets are never indexed; purging expired trash therefore needs no
/// extra index work — rows already left the index when they were trashed.
pub struct SearchIndex<'c> {
    conn: &'c Connection,
}

/// Searchable fields of one live snippet as read from the main tables.
struct SourceRow {
    snippet_id: String,
    title: String,
    content: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    folder_name: Option<String>,
    trigger: Option<String>,
    language: Option<String>,
    is_sensitive: bool,
}

const SOURCE_SELECT: &str = "SELECT s.id, s.title, s.content_plaintext, s.description,
        (SELECT group_concat(t.name, ' ')
           FROM snippet_tag st JOIN tag t ON t.id = st.tag_id
          WHERE st.snippet_id = s.id),
        f.name, s.\"trigger\", s.language, s.security_level
   FROM snippet s LEFT JOIN folder f ON f.id = s.folder_id
  WHERE s.deleted_at IS NULL";

impl<'c> SearchIndex<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Brings the index row for `snippet_id` in line with the main tables:
    /// re-indexes a live snippet, removes a trashed or deleted one. Call
    /// after any write that touches a snippet's searchable fields, its tags
    /// or its folder assignment.
    pub fn sync_snippet(&self, snippet_id: &str) -> Result<(), SearchError> {
        let source = self
            .conn
            .query_row(
                &format!("{SOURCE_SELECT} AND s.id = ?1"),
                params![snippet_id],
                row_to_source,
            )
            .optional()?;

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM snippet_fts WHERE snippet_id = ?1",
            params![snippet_id],
        )?;
        if let Some(row) = source {
            insert_fts_row(&tx, &row)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Drops the whole index and re-populates it from every live snippet.
    /// Returns the number of rows indexed. Runs in one transaction, so a
    /// failure leaves the previous index intact.
    pub fn rebuild(&self) -> Result<usize, SearchError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM snippet_fts", [])?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare(SOURCE_SELECT)?;
            let rows = stmt.query_map([], row_to_source)?;
            for row in rows {
                insert_fts_row(&tx, &row?)?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }
}

fn row_to_source(row: &Row<'_>) -> rusqlite::Result<SourceRow> {
    let security_level: String = row.get(8)?;
    Ok(SourceRow {
        snippet_id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        description: row.get(3)?,
        tags: row.get(4)?,
        folder_name: row.get(5)?,
        trigger: row.get(6)?,
        language: row.get(7)?,
        is_sensitive: security_level == SecurityLevel::Sensitive.as_str(),
    })
}

fn insert_fts_row(conn: &Connection, row: &SourceRow) -> Result<(), SearchError> {
    // Sensitive snippets contribute only title, tags and description
    // (docs/PRD.md §12.2); all other columns stay empty for them.
    let (content, folder_name, trigger, language) = if row.is_sensitive {
        (None, None, None, None)
    } else {
        (
            row.content.as_deref(),
            row.folder_name.as_deref(),
            row.trigger.as_deref(),
            row.language.as_deref(),
        )
    };
    conn.execute(
        "INSERT INTO snippet_fts (snippet_id, title, content, description, tags,
                                  folder_name, \"trigger\", language)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.snippet_id,
            index_text(&row.title),
            index_opt(content),
            index_opt(row.description.as_deref()),
            index_opt(row.tags.as_deref()),
            index_opt(folder_name),
            index_opt(trigger),
            index_opt(language),
        ],
    )?;
    Ok(())
}

fn index_opt(value: Option<&str>) -> String {
    value.map(index_text).unwrap_or_default()
}
