// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Folder CRUD with multi-level nesting and cycle protection.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::Folder;

/// Folder repository over a single connection.
pub struct FolderRepo<'c> {
    conn: &'c Connection,
}

impl<'c> FolderRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated folder; the parent must exist when set.
    pub fn insert(&self, folder: &Folder) -> Result<(), RepoError> {
        folder.validate()?;
        self.conn.execute(
            "INSERT INTO folder (id, parent_id, name, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                folder.id,
                folder.parent_id,
                folder.name,
                folder.sort_order,
                folder.created_at,
                folder.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Loads one folder by id.
    pub fn get(&self, id: &str) -> Result<Option<Folder>, RepoError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, parent_id, name, sort_order, created_at, updated_at
                 FROM folder WHERE id = ?1",
                params![id],
                row_to_folder,
            )
            .optional()?)
    }

    /// Updates name, parent, sort order, and updated time. Re-parenting into
    /// the folder's own subtree is rejected to keep the tree acyclic.
    pub fn update(&self, folder: &Folder) -> Result<(), RepoError> {
        folder.validate()?;
        if let Some(parent_id) = &folder.parent_id
            && self.is_in_subtree(parent_id, &folder.id)?
        {
            return Err(RepoError::Conflict(
                "cannot move a folder under its own descendant",
            ));
        }
        let changed = self.conn.execute(
            "UPDATE folder SET parent_id = ?2, name = ?3, sort_order = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                folder.id,
                folder.parent_id,
                folder.name,
                folder.sort_order,
                folder.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Deletes a folder; child folders cascade and snippets fall back to
    /// unfiled (schema `ON DELETE` rules).
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM folder WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Lists direct children of a parent (`None` = top level), in tree order.
    pub fn list_children(&self, parent_id: Option<&str>) -> Result<Vec<Folder>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, name, sort_order, created_at, updated_at
             FROM folder
             WHERE (?1 IS NULL AND parent_id IS NULL) OR parent_id = ?1
             ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map(params![parent_id], row_to_folder)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Lists every folder, parents before their children (depth order), so
    /// the rows can be re-inserted in sequence under the parent FK. Serves
    /// the whole-library backup; the folder tree is small by nature.
    pub fn list_all_parents_first(&self) -> Result<Vec<Folder>, RepoError> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE ordered(id, depth) AS (
                 SELECT id, 0 FROM folder WHERE parent_id IS NULL
                 UNION ALL
                 SELECT f.id, o.depth + 1 FROM folder f JOIN ordered o ON f.parent_id = o.id
             )
             SELECT f.id, f.parent_id, f.name, f.sort_order, f.created_at, f.updated_at
             FROM folder f JOIN ordered o ON f.id = o.id
             ORDER BY o.depth, f.sort_order, f.name",
        )?;
        let rows = stmt.query_map([], row_to_folder)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// True when `candidate` equals `root` or lies anywhere under it.
    fn is_in_subtree(&self, candidate: &str, root: &str) -> Result<bool, RepoError> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "WITH RECURSIVE subtree(id) AS (
                     SELECT id FROM folder WHERE id = ?1
                     UNION ALL
                     SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
                 )
                 SELECT 1 FROM subtree WHERE id = ?2 LIMIT 1",
                params![root, candidate],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }
}

fn row_to_folder(row: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get("id")?,
        parent_id: row.get("parent_id")?,
        name: row.get("name")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
