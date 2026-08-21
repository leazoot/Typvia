// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Tag CRUD; tag names are unique.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::Tag;

/// Tag repository over a single connection.
pub struct TagRepo<'c> {
    conn: &'c Connection,
}

impl<'c> TagRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated tag; duplicate names are a business conflict.
    pub fn insert(&self, tag: &Tag) -> Result<(), RepoError> {
        tag.validate()?;
        match self.conn.execute(
            "INSERT INTO tag (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![tag.id, tag.name, tag.created_at],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => {
                Err(RepoError::Conflict("tag name already exists"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Loads one tag by id.
    pub fn get(&self, id: &str) -> Result<Option<Tag>, RepoError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, created_at FROM tag WHERE id = ?1",
                params![id],
                row_to_tag,
            )
            .optional()?)
    }

    /// Renames a tag; renaming onto an existing name is a conflict.
    pub fn rename(&self, id: &str, new_name: &str) -> Result<(), RepoError> {
        if new_name.trim().is_empty() {
            return Err(RepoError::Conflict("tag name must not be blank"));
        }
        let changed = match self.conn.execute(
            "UPDATE tag SET name = ?2 WHERE id = ?1",
            params![id, new_name],
        ) {
            Ok(n) => n,
            Err(e) if is_unique_violation(&e) => {
                return Err(RepoError::Conflict("tag name already exists"));
            }
            Err(e) => return Err(e.into()),
        };
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Deletes a tag; snippet links cascade away.
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM tag WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// All tags in name order (tag counts stay small; no pagination).
    pub fn list_all(&self) -> Result<Vec<Tag>, RepoError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM tag ORDER BY name")?;
        let rows = stmt.query_map([], row_to_tag)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn row_to_tag(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get("id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
    })
}
