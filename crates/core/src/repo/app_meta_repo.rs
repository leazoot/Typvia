// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! One-shot bootstrap markers.
//!
//! Device-local key/value flags for "has this bootstrap already run"
//! questions. Deliberately not a configuration store: user-facing
//! settings live in their own entity tables.

use rusqlite::{Connection, OptionalExtension, params};

use super::RepoError;

/// App-meta marker repository over a single connection.
pub struct AppMetaRepo<'c> {
    conn: &'c Connection,
}

impl<'c> AppMetaRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Reads one marker value.
    pub fn get(&self, key: &str) -> Result<Option<String>, RepoError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Writes (or overwrites) one marker value.
    pub fn set(&self, key: &str, value: &str) -> Result<(), RepoError> {
        self.conn.execute(
            "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};

    #[test]
    fn markers_read_back_and_overwrite() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let repo = AppMetaRepo::new(&conn);
        assert_eq!(repo.get("ai_actions_seeded").unwrap(), None);
        repo.set("ai_actions_seeded", "1").unwrap();
        assert_eq!(
            repo.get("ai_actions_seeded").unwrap(),
            Some("1".to_string())
        );
        repo.set("ai_actions_seeded", "2").unwrap();
        assert_eq!(
            repo.get("ai_actions_seeded").unwrap(),
            Some("2".to_string())
        );
    }
}
