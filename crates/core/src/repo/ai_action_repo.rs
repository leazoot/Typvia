// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AiAction CRUD.
//!
//! ai_action is a syncable entity, but its sync payload path is not built
//! yet (crates/sync payload.rs: UnsupportedEntity), so writes here do NOT
//! announce through sync_hooks — announcing would make every write fail
//! against the current observer. The hook call joins when the payload path
//! lands.
//!
//! `provider_id` and `model` are stored as TEXT NOT NULL where the empty
//! string means "not configured"; this layer maps '' ↔ None so the
//! rest of the code only ever sees the honest Option form.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::{AiAction, AiActionParams};

/// AI-action repository over a single connection.
pub struct AiActionRepo<'c> {
    conn: &'c Connection,
}

const COLUMNS: &str = "id, name, prompt_template, provider_id, model, input_source, \
                       output_mode, permission_scope, params, created_at, updated_at";

impl<'c> AiActionRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated action.
    pub fn insert(&self, action: &AiAction) -> Result<(), RepoError> {
        action.validate()?;
        match self.conn.execute(
            "INSERT INTO ai_action (id, name, prompt_template, provider_id, model,
                                    input_source, output_mode, permission_scope, params,
                                    created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                action.id,
                action.name,
                action.prompt_template,
                action.provider_id.as_deref().unwrap_or(""),
                action.model.as_deref().unwrap_or(""),
                action.input_source.as_str(),
                action.output_mode.as_str(),
                action.permission_scope.as_str(),
                action.params.to_json(),
                action.created_at,
                action.updated_at,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_constraint_violation(&e) => {
                Err(RepoError::Conflict("action id already exists"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Rewrites an action's definition fields.
    pub fn update(&self, action: &AiAction) -> Result<(), RepoError> {
        action.validate()?;
        let changed = self.conn.execute(
            "UPDATE ai_action SET name = ?2, prompt_template = ?3, provider_id = ?4,
                    model = ?5, input_source = ?6, output_mode = ?7,
                    permission_scope = ?8, params = ?9, updated_at = ?10
             WHERE id = ?1",
            params![
                action.id,
                action.name,
                action.prompt_template,
                action.provider_id.as_deref().unwrap_or(""),
                action.model.as_deref().unwrap_or(""),
                action.input_source.as_str(),
                action.output_mode.as_str(),
                action.permission_scope.as_str(),
                action.params.to_json(),
                action.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Deletes one action.
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM ai_action WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Loads one action by id.
    pub fn get(&self, id: &str) -> Result<Option<AiAction>, RepoError> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM ai_action WHERE id = ?1"),
                params![id],
                row_to_action,
            )
            .optional()?
            .transpose()
    }

    /// All actions, stable order, paged.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<AiAction>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM ai_action ORDER BY name, id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_action)?;
        let mut actions = Vec::new();
        for row in rows {
            actions.push(row??);
        }
        Ok(actions)
    }
}

fn is_constraint_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn optional_text(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

type ActionRowResult = Result<AiAction, RepoError>;

fn row_to_action(row: &Row<'_>) -> rusqlite::Result<ActionRowResult> {
    let input_source: String = row.get("input_source")?;
    let output_mode: String = row.get("output_mode")?;
    let permission_scope: String = row.get("permission_scope")?;
    let params: String = row.get("params")?;
    // Stored TEXT enums parse explicitly; unknown values surface as errors
    // instead of being silently dropped (forward-compat rule).
    let parsed = (|| -> ActionRowResult {
        Ok(AiAction {
            id: row.get("id")?,
            name: row.get("name")?,
            prompt_template: row.get("prompt_template")?,
            provider_id: optional_text(row.get("provider_id")?),
            model: optional_text(row.get("model")?),
            input_source: input_source.parse().map_err(RepoError::from)?,
            output_mode: output_mode.parse().map_err(RepoError::from)?,
            permission_scope: permission_scope.parse().map_err(RepoError::from)?,
            params: AiActionParams::from_json(&params).map_err(RepoError::from)?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })();
    match parsed {
        Ok(action) => Ok(Ok(action)),
        Err(RepoError::Sqlite(e)) => Err(e),
        Err(other) => Ok(Err(other)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::{AiActionInputSource, AiActionOutputMode, AiActionPermissionScope};

    fn setup() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn action(id: &str, name: &str) -> AiAction {
        AiAction {
            id: id.to_string(),
            name: name.to_string(),
            prompt_template: "Translate the input into English.".to_string(),
            provider_id: None,
            model: None,
            input_source: AiActionInputSource::Selection,
            output_mode: AiActionOutputMode::Replace,
            permission_scope: AiActionPermissionScope::NormalOnly,
            params: AiActionParams::default(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn round_trips_an_action_through_insert_get_update_delete() {
        let conn = setup();
        let repo = AiActionRepo::new(&conn);
        let mut a = action("a1", "Translate");
        repo.insert(&a).unwrap();
        assert_eq!(repo.get("a1").unwrap().unwrap(), a);

        a.provider_id = Some("p1".to_string());
        a.model = Some("llama3".to_string());
        a.params = AiActionParams::from_json(r#"{"temperature":0.7}"#).unwrap();
        a.updated_at = 2;
        repo.update(&a).unwrap();
        assert_eq!(repo.get("a1").unwrap().unwrap(), a);

        repo.delete("a1").unwrap();
        assert_eq!(repo.get("a1").unwrap(), None);
        assert!(matches!(
            repo.delete("a1").unwrap_err(),
            RepoError::NotFound
        ));
    }

    #[test]
    fn unconfigured_provider_and_model_read_back_as_none() {
        let conn = setup();
        let repo = AiActionRepo::new(&conn);
        repo.insert(&action("a1", "Summarize")).unwrap();
        // The storage form is the empty string (TEXT NOT NULL)...
        let stored: (String, String) = conn
            .query_row(
                "SELECT provider_id, model FROM ai_action WHERE id = 'a1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (String::new(), String::new()));
        // ...but the model layer only ever sees None.
        let read = repo.get("a1").unwrap().unwrap();
        assert_eq!(read.provider_id, None);
        assert_eq!(read.model, None);
    }

    #[test]
    fn params_round_trip_keeps_unknown_keys_in_storage() {
        let conn = setup();
        let repo = AiActionRepo::new(&conn);
        let mut a = action("a1", "Rewrite");
        a.params = AiActionParams::from_json(r#"{"temperature":0.3,"top_p":0.9}"#).unwrap();
        repo.insert(&a).unwrap();

        // A read-modify-write by a build that does not know `top_p` must
        // not drop it (forward-compatibility rule).
        let mut read = repo.get("a1").unwrap().unwrap();
        read.name = "Rewrite politely".to_string();
        read.updated_at = 2;
        repo.update(&read).unwrap();
        let stored: String = conn
            .query_row("SELECT params FROM ai_action WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(stored.contains("top_p"));
    }

    #[test]
    fn rejects_duplicates_and_reports_unknown_stored_enums() {
        let conn = setup();
        let repo = AiActionRepo::new(&conn);
        repo.insert(&action("a1", "A")).unwrap();
        assert!(matches!(
            repo.insert(&action("a1", "B")).unwrap_err(),
            RepoError::Conflict(_)
        ));

        conn.execute(
            "INSERT INTO ai_action (id, name, prompt_template, provider_id, model,
                                    input_source, output_mode, permission_scope, params,
                                    created_at, updated_at)
             VALUES ('a2', 'Future', 'p', '', '', 'telepathy', 'replace', 'normal_only',
                     '{}', 1, 1)",
            [],
        )
        .unwrap();
        match repo.get("a2").unwrap_err() {
            RepoError::UnknownEnum(e) => {
                assert_eq!(e.enum_name, "AiActionInputSource");
                assert_eq!(e.value, "telepathy");
            }
            other => panic!("expected UnknownEnum, got {other:?}"),
        }
    }
}
