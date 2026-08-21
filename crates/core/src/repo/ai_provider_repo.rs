// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AiProvider CRUD. Device-local
//! rows; nothing here syncs, and no key material ever passes through —
//! the entity has no field capable of holding one.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::RepoError;
use crate::model::AiProvider;

/// AI-provider repository over a single connection.
pub struct AiProviderRepo<'c> {
    conn: &'c Connection,
}

const COLUMNS: &str = "id, name, kind, base_url, model, timeout_ms, created_at, updated_at";

impl<'c> AiProviderRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated provider.
    pub fn insert(&self, provider: &AiProvider) -> Result<(), RepoError> {
        provider.validate()?;
        match self.conn.execute(
            "INSERT INTO ai_provider (id, name, kind, base_url, model, timeout_ms,
                                      created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                provider.id,
                provider.name,
                provider.kind.as_str(),
                provider.base_url,
                provider.model,
                provider.timeout_ms,
                provider.created_at,
                provider.updated_at,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(e) if is_constraint_violation(&e) => {
                Err(RepoError::Conflict("provider id already exists"))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Rewrites a provider's configuration fields.
    pub fn update(&self, provider: &AiProvider) -> Result<(), RepoError> {
        provider.validate()?;
        let changed = self.conn.execute(
            "UPDATE ai_provider SET name = ?2, kind = ?3, base_url = ?4,
                    model = ?5, timeout_ms = ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                provider.id,
                provider.name,
                provider.kind.as_str(),
                provider.base_url,
                provider.model,
                provider.timeout_ms,
                provider.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Deletes one provider. Actions referencing it keep their
    /// `provider_id` (no FK by design) and surface "provider not
    /// configured" at execution time; the caller's UI is responsible for
    /// warning about affected actions. The stored API key is a secure-store
    /// entry the caller removes separately (`ai.api_key.<id>`).
    pub fn delete(&self, id: &str) -> Result<(), RepoError> {
        let changed = self
            .conn
            .execute("DELETE FROM ai_provider WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Loads one provider by id.
    pub fn get(&self, id: &str) -> Result<Option<AiProvider>, RepoError> {
        self.conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM ai_provider WHERE id = ?1"),
                params![id],
                row_to_provider,
            )
            .optional()?
            .transpose()
    }

    /// All providers, stable order, paged (counts stay small; the page
    /// keeps the list-query rule uniform).
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<AiProvider>, RepoError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM ai_provider ORDER BY name, id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_provider)?;
        let mut providers = Vec::new();
        for row in rows {
            providers.push(row??);
        }
        Ok(providers)
    }
}

fn is_constraint_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

type ProviderRowResult = Result<AiProvider, RepoError>;

fn row_to_provider(row: &Row<'_>) -> rusqlite::Result<ProviderRowResult> {
    let kind: String = row.get("kind")?;
    // Stored TEXT enums parse explicitly; unknown values surface as errors
    // instead of being silently dropped (forward-compat rule).
    let parsed = (|| -> ProviderRowResult {
        Ok(AiProvider {
            id: row.get("id")?,
            name: row.get("name")?,
            kind: kind.parse().map_err(RepoError::from)?,
            base_url: row.get("base_url")?,
            model: row.get("model")?,
            timeout_ms: row.get("timeout_ms")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })();
    match parsed {
        Ok(provider) => Ok(Ok(provider)),
        Err(RepoError::Sqlite(e)) => Err(e),
        Err(other) => Ok(Err(other)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::AiProviderKind;

    fn setup() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn provider(id: &str, name: &str) -> AiProvider {
        AiProvider {
            id: id.to_string(),
            name: name.to_string(),
            kind: AiProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "llama3".to_string(),
            timeout_ms: 30_000,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn round_trips_a_provider_through_insert_get_update_delete() {
        let conn = setup();
        let repo = AiProviderRepo::new(&conn);
        let mut p = provider("p1", "Local Ollama");
        repo.insert(&p).unwrap();
        assert_eq!(repo.get("p1").unwrap().unwrap(), p);

        p.name = "Ollama on this Mac".to_string();
        p.kind = AiProviderKind::CustomBaseUrl;
        p.base_url = "https://gateway.example.com/v1".to_string();
        p.updated_at = 2;
        repo.update(&p).unwrap();
        assert_eq!(repo.get("p1").unwrap().unwrap(), p);

        repo.delete("p1").unwrap();
        assert_eq!(repo.get("p1").unwrap(), None);
        assert!(matches!(
            repo.delete("p1").unwrap_err(),
            RepoError::NotFound
        ));
    }

    #[test]
    fn rejects_duplicate_ids_and_missing_updates() {
        let conn = setup();
        let repo = AiProviderRepo::new(&conn);
        repo.insert(&provider("p1", "A")).unwrap();
        assert!(matches!(
            repo.insert(&provider("p1", "B")).unwrap_err(),
            RepoError::Conflict(_)
        ));
        assert!(matches!(
            repo.update(&provider("ghost", "X")).unwrap_err(),
            RepoError::NotFound
        ));
    }

    #[test]
    fn lists_providers_in_stable_name_order_with_paging() {
        let conn = setup();
        let repo = AiProviderRepo::new(&conn);
        repo.insert(&provider("p2", "Beta")).unwrap();
        repo.insert(&provider("p1", "Alpha")).unwrap();
        repo.insert(&provider("p3", "Gamma")).unwrap();

        let first_two = repo.list(2, 0).unwrap();
        assert_eq!(
            first_two
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Beta"]
        );
        let tail = repo.list(2, 2).unwrap();
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].name, "Gamma");
    }

    #[test]
    fn an_unknown_stored_kind_is_reported_not_silently_dropped() {
        let conn = setup();
        conn.execute(
            "INSERT INTO ai_provider (id, name, kind, base_url, model, timeout_ms,
                                      created_at, updated_at)
             VALUES ('p1', 'Future', 'quantum_provider', 'https://x/v1', 'm', 1000, 1, 1)",
            [],
        )
        .unwrap();
        let repo = AiProviderRepo::new(&conn);
        match repo.get("p1").unwrap_err() {
            RepoError::UnknownEnum(e) => {
                assert_eq!(e.enum_name, "AiProviderKind");
                assert_eq!(e.value, "quantum_provider");
            }
            other => panic!("expected UnknownEnum, got {other:?}"),
        }
    }
}
