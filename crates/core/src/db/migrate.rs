// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Versioned, sequential, reversible schema migrations.
//!
//! Each migration is an up/down SQL pair applied inside a transaction
//! together with the `user_version` bump, so a failure leaves the previous
//! schema fully intact. Published migrations are append-only: fixes ship as
//! new migrations, never edits.

use rusqlite::Connection;

use super::DbError;

/// One reversible schema change.
pub struct Migration {
    /// Sequential version this migration upgrades the schema to.
    pub version: u32,
    /// Short human-readable name (matches the SQL file names).
    pub name: &'static str,
    /// SQL applied when upgrading to `version`.
    pub up: &'static str,
    /// SQL applied when rolling back from `version`.
    pub down: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        up: include_str!("../../migrations/0001_initial_schema.up.sql"),
        down: include_str!("../../migrations/0001_initial_schema.down.sql"),
    },
    Migration {
        version: 2,
        name: "trash_and_versions",
        up: include_str!("../../migrations/0002_trash_and_versions.up.sql"),
        down: include_str!("../../migrations/0002_trash_and_versions.down.sql"),
    },
    Migration {
        version: 3,
        name: "search_index_language",
        up: include_str!("../../migrations/0003_search_index_language.up.sql"),
        down: include_str!("../../migrations/0003_search_index_language.down.sql"),
    },
    Migration {
        version: 4,
        name: "vault_key_header",
        up: include_str!("../../migrations/0004_vault_key_header.up.sql"),
        down: include_str!("../../migrations/0004_vault_key_header.down.sql"),
    },
    Migration {
        version: 5,
        name: "sync_orchestration",
        up: include_str!("../../migrations/0005_sync_orchestration.up.sql"),
        down: include_str!("../../migrations/0005_sync_orchestration.down.sql"),
    },
    Migration {
        version: 6,
        name: "conflict_marker",
        up: include_str!("../../migrations/0006_conflict_marker.up.sql"),
        down: include_str!("../../migrations/0006_conflict_marker.down.sql"),
    },
    Migration {
        version: 7,
        name: "key_update_cursor",
        up: include_str!("../../migrations/0007_key_update_cursor.up.sql"),
        down: include_str!("../../migrations/0007_key_update_cursor.down.sql"),
    },
    Migration {
        version: 8,
        name: "ai_providers",
        up: include_str!("../../migrations/0008_ai_providers.up.sql"),
        down: include_str!("../../migrations/0008_ai_providers.down.sql"),
    },
    Migration {
        version: 9,
        name: "ai_egress_log",
        up: include_str!("../../migrations/0009_ai_egress_log.up.sql"),
        down: include_str!("../../migrations/0009_ai_egress_log.down.sql"),
    },
    Migration {
        version: 10,
        name: "ai_action_params",
        up: include_str!("../../migrations/0010_ai_action_params.up.sql"),
        down: include_str!("../../migrations/0010_ai_action_params.down.sql"),
    },
    Migration {
        version: 11,
        name: "recovery_catchup_root",
        up: include_str!("../../migrations/0011_recovery_catchup_root.up.sql"),
        down: include_str!("../../migrations/0011_recovery_catchup_root.down.sql"),
    },
    Migration {
        version: 12,
        name: "webdav_transport",
        up: include_str!("../../migrations/0012_webdav_transport.up.sql"),
        down: include_str!("../../migrations/0012_webdav_transport.down.sql"),
    },
    Migration {
        version: 13,
        name: "snippet_embedding",
        up: include_str!("../../migrations/0013_snippet_embedding.up.sql"),
        down: include_str!("../../migrations/0013_snippet_embedding.down.sql"),
    },
    Migration {
        version: 14,
        name: "snippet_usage_order",
        up: include_str!("../../migrations/0014_snippet_usage_order.up.sql"),
        down: include_str!("../../migrations/0014_snippet_usage_order.down.sql"),
    },
    Migration {
        version: 15,
        name: "snippet_list_order",
        up: include_str!("../../migrations/0015_snippet_list_order.up.sql"),
        down: include_str!("../../migrations/0015_snippet_list_order.down.sql"),
    },
];

/// Highest schema version known to this build.
pub fn latest_version() -> u32 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

/// Reads the current schema version (`PRAGMA user_version`).
pub fn schema_version(conn: &Connection) -> Result<u32, DbError> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

/// Migrates an empty or existing database to the latest known version.
pub fn migrate_to_latest(conn: &mut Connection) -> Result<(), DbError> {
    migrate_to(conn, latest_version())
}

/// Migrates up or down to `target`; `0` rolls the schema back entirely.
pub fn migrate_to(conn: &mut Connection, target: u32) -> Result<(), DbError> {
    run_migrations(conn, MIGRATIONS, target)
}

/// Applies the migration steps from the current version to `target`, one
/// transaction per step. Split from the public API so failure behavior is
/// testable with synthetic migration sets.
fn run_migrations(
    conn: &mut Connection,
    migrations: &[Migration],
    target: u32,
) -> Result<(), DbError> {
    let latest = migrations.last().map_or(0, |m| m.version);
    if target > latest {
        return Err(DbError::UnknownTargetVersion {
            requested: target,
            latest,
        });
    }

    let current = schema_version(conn)?;
    // An older binary opening a newer database used to fall through here in
    // silence: the downgrade branch would look for steps between `current`
    // and `target`, find none it has ever heard of, and return Ok — after
    // which the app would run against a schema it does not understand.
    // The check is on the database's own version, not on the caller's target,
    // because no target is reachable from a schema this build cannot see.
    if current > latest {
        return Err(DbError::DatabaseFromNewerBuild {
            found: current,
            supported: latest,
        });
    }

    if target > current {
        for m in migrations
            .iter()
            .filter(|m| m.version > current && m.version <= target)
        {
            apply_step(conn, m.up, m.version)?;
        }
    } else if target < current {
        for m in migrations
            .iter()
            .rev()
            .filter(|m| m.version <= current && m.version > target)
        {
            apply_step(conn, m.down, m.version - 1)?;
        }
    }

    Ok(())
}

/// Runs one migration script and the version bump atomically.
fn apply_step(conn: &mut Connection, sql: &str, resulting_version: u32) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    tx.execute_batch(sql)?;
    tx.pragma_update(None, "user_version", resulting_version)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    /// The case an old install hits after a new one has touched the same
    /// file: the schema is ahead of this binary. It has to refuse and say so.
    /// Returning Ok here means the app then reads and writes a shape it has
    /// never seen, which is how the newer install's data gets damaged by the
    /// older one.
    #[test]
    fn a_database_from_a_newer_build_is_refused_rather_than_silently_accepted() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let ahead = latest_version() + 3;
        conn.pragma_update(None, "user_version", ahead).unwrap();

        let error = migrate_to_latest(&mut conn).unwrap_err();

        assert!(matches!(
            error,
            DbError::DatabaseFromNewerBuild { found, supported }
                if found == ahead && supported == latest_version()
        ));
        // The refusal changes nothing: the file is left exactly as the newer
        // build left it, so that build can still open it.
        assert_eq!(schema_version(&conn).unwrap(), ahead);
    }

    /// Rolling back is refused for the same reason. The down scripts that
    /// would undo the unknown versions shipped with the build that added
    /// them, so a rollback from here would strip the schema down past tables
    /// it cannot recreate.
    #[test]
    fn a_rollback_from_a_newer_schema_is_refused_too() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let ahead = latest_version() + 1;
        conn.pragma_update(None, "user_version", ahead).unwrap();

        assert!(matches!(
            migrate_to(&mut conn, 0),
            Err(DbError::DatabaseFromNewerBuild { .. })
        ));
        assert_eq!(schema_version(&conn).unwrap(), ahead);
    }

    #[test]
    fn initializes_an_empty_database_to_the_latest_version() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());

        let tables = table_names(&conn);
        for expected in [
            "ai_action",
            "app_rule",
            "device",
            "domain_key",
            "folder",
            "key_header",
            "snippet",
            "snippet_tag",
            "sync_config",
            "sync_pending_record",
            "sync_record",
            "sync_shadow",
            "tag",
            "template_field",
        ] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
        // FTS5 registers its shadow tables under the virtual table name.
        assert!(tables.iter().any(|t| t == "snippet_fts"));
    }

    #[test]
    fn migrating_twice_is_idempotent() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
    }

    #[test]
    fn rolls_back_to_an_empty_schema() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        migrate_to(&mut conn, 0).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        assert_eq!(table_names(&conn), Vec::<String>::new());
    }

    #[test]
    fn rollback_then_upgrade_restores_the_full_schema() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        migrate_to(&mut conn, 0).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"snippet".to_string()));
    }

    #[test]
    fn partial_rollback_to_version_one_keeps_the_initial_schema() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        migrate_to(&mut conn, 1).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 1);
        let tables = table_names(&conn);
        assert!(tables.contains(&"snippet".to_string()));
        assert!(!tables.contains(&"snippet_version".to_string()));
    }

    #[test]
    fn fts_language_column_exists_only_from_version_three() {
        let fts_columns = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn.prepare("PRAGMA table_info(snippet_fts)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(fts_columns(&conn).contains(&"language".to_string()));

        migrate_to(&mut conn, 2).unwrap();
        assert!(!fts_columns(&conn).contains(&"language".to_string()));

        migrate_to_latest(&mut conn).unwrap();
        assert!(fts_columns(&conn).contains(&"language".to_string()));
    }

    #[test]
    fn vault_key_tables_exist_only_from_version_four() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let tables = table_names(&conn);
        assert!(tables.contains(&"key_header".to_string()));
        assert!(tables.contains(&"domain_key".to_string()));

        migrate_to(&mut conn, 3).unwrap();
        let tables = table_names(&conn);
        assert!(!tables.contains(&"key_header".to_string()));
        assert!(!tables.contains(&"domain_key".to_string()));

        // Re-upgrading rebuilds them (idempotent up path).
        migrate_to_latest(&mut conn).unwrap();
        let tables = table_names(&conn);
        assert!(tables.contains(&"key_header".to_string()));
        assert!(tables.contains(&"domain_key".to_string()));
    }

    #[test]
    fn sync_orchestration_state_exists_only_from_version_five() {
        let record_columns = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn.prepare("PRAGMA table_info(sync_record)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        for column in ["key_id", "signature", "state", "server_seq"] {
            assert!(record_columns(&conn).contains(&column.to_string()));
        }
        assert!(table_names(&conn).contains(&"sync_shadow".to_string()));
        // The single config row exists right after migration.
        let config_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_config", [], |row| row.get(0))
            .unwrap();
        assert_eq!(config_rows, 1);

        migrate_to(&mut conn, 4).unwrap();
        assert!(!record_columns(&conn).contains(&"state".to_string()));
        let tables = table_names(&conn);
        for dropped in ["sync_shadow", "sync_pending_record", "sync_config"] {
            assert!(!tables.contains(&dropped.to_string()), "left {dropped}");
        }

        // Re-upgrading rebuilds everything (idempotent up path).
        migrate_to_latest(&mut conn).unwrap();
        assert!(record_columns(&conn).contains(&"state".to_string()));
        assert!(table_names(&conn).contains(&"sync_config".to_string()));
    }

    #[test]
    fn list_order_indexes_exist_only_from_version_fifteen() {
        let indexes = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'snippet'",
                )
                .unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        let wanted = ["idx_snippet_last_used_order", "idx_snippet_created_order"];

        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(
            wanted
                .iter()
                .all(|name| indexes(&conn).contains(&name.to_string()))
        );

        migrate_to(&mut conn, 14).unwrap();
        assert!(
            wanted
                .iter()
                .all(|name| !indexes(&conn).contains(&name.to_string()))
        );
        // The usage index belongs to version 14 and must survive the rollback.
        assert!(indexes(&conn).contains(&"idx_snippet_usage_count".to_string()));

        // Re-upgrading restores both (idempotent up path).
        migrate_to_latest(&mut conn).unwrap();
        assert!(
            wanted
                .iter()
                .all(|name| indexes(&conn).contains(&name.to_string()))
        );
    }

    #[test]
    fn conflict_marker_column_exists_only_from_version_six() {
        let snippet_columns = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn.prepare("PRAGMA table_info(snippet)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(snippet_columns(&conn).contains(&"conflict_of".to_string()));

        migrate_to(&mut conn, 5).unwrap();
        assert!(!snippet_columns(&conn).contains(&"conflict_of".to_string()));

        // Re-upgrading restores the column (idempotent up path).
        migrate_to_latest(&mut conn).unwrap();
        assert!(snippet_columns(&conn).contains(&"conflict_of".to_string()));
    }

    #[test]
    fn key_update_cursor_column_exists_only_from_version_seven() {
        let config_columns = |conn: &Connection| -> Vec<String> {
            let mut stmt = conn.prepare("PRAGMA table_info(sync_config)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(config_columns(&conn).contains(&"key_update_seq".to_string()));
        // The single row keeps a usable cursor without any write.
        let cursor: i64 = conn
            .query_row(
                "SELECT key_update_seq FROM sync_config WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cursor, 0);

        migrate_to(&mut conn, 6).unwrap();
        assert!(!config_columns(&conn).contains(&"key_update_seq".to_string()));

        // Re-upgrading restores the column (idempotent up path).
        migrate_to_latest(&mut conn).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(config_columns(&conn).contains(&"key_update_seq".to_string()));
    }

    #[test]
    fn ai_provider_table_exists_only_from_version_eight() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"ai_provider".to_string()));

        // Deliberately NO foreign key from ai_action.provider_id: actions
        // sync between devices while providers are device-local, so a
        // remotely-applied action may reference a provider this machine
        // has not configured.
        let insert_action = "INSERT INTO ai_action (id, name, prompt_template, provider_id, \
             model, input_source, output_mode, permission_scope, created_at, updated_at) \
             VALUES (?1, 'n', 'p', ?2, 'm', 'selection', 'replace', 'normal_only', 1, 1)";
        conn.execute(
            insert_action,
            rusqlite::params!["a1", "not-configured-here"],
        )
        .unwrap();

        // Red line: no column of ai_provider can hold key material.
        let columns: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(ai_provider)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(
            columns,
            vec![
                "id",
                "name",
                "kind",
                "base_url",
                "model",
                "timeout_ms",
                "created_at",
                "updated_at"
            ]
        );

        // Downgrade removes the table; existing actions are untouched.
        migrate_to(&mut conn, 7).unwrap();
        assert!(!table_names(&conn).contains(&"ai_provider".to_string()));
        let survived: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_action", [], |r| r.get(0))
            .unwrap();
        assert_eq!(survived, 1);

        // Re-upgrading is idempotent.
        migrate_to_latest(&mut conn).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"ai_provider".to_string()));
        let carried: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_action", [], |r| r.get(0))
            .unwrap();
        assert_eq!(carried, 1);
    }

    #[test]
    fn ai_egress_log_table_exists_only_from_version_nine() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"ai_egress_log".to_string()));

        // Red line: the log structurally cannot hold prompt
        // content, response content or key material — these metadata
        // columns are all it has.
        let columns: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(ai_egress_log)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(
            columns,
            vec![
                "id",
                "occurred_at",
                "provider_id",
                "request_class",
                "request_bytes"
            ]
        );

        // Deliberately NO foreign key onto ai_provider: the audit trail
        // must survive provider deletion.
        conn.execute(
            "INSERT INTO ai_egress_log (occurred_at, provider_id, request_class, request_bytes) \
             VALUES (1, 'deleted-provider', 'completion', 42)",
            [],
        )
        .unwrap();

        // Negative byte counts are rejected at the schema level.
        let rejected = conn.execute(
            "INSERT INTO ai_egress_log (occurred_at, provider_id, request_class, request_bytes) \
             VALUES (1, 'p1', 'completion', -1)",
            [],
        );
        assert!(rejected.is_err());

        // Downgrade drops the log; re-upgrading is idempotent.
        migrate_to(&mut conn, 8).unwrap();
        assert!(!table_names(&conn).contains(&"ai_egress_log".to_string()));
        migrate_to_latest(&mut conn).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"ai_egress_log".to_string()));
    }

    #[test]
    fn ai_action_params_and_app_meta_exist_only_from_version_ten() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        assert!(table_names(&conn).contains(&"app_meta".to_string()));

        // Rows written before the column existed read back the '{}' default.
        migrate_to(&mut conn, 9).unwrap();
        conn.execute(
            "INSERT INTO ai_action (id, name, prompt_template, provider_id, model, \
             input_source, output_mode, permission_scope, created_at, updated_at) \
             VALUES ('a1', 'Translate', 'Translate.', '', '', 'selection', 'replace', \
             'normal_only', 1, 1)",
            [],
        )
        .unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let params: String = conn
            .query_row("SELECT params FROM ai_action WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(params, "{}");

        // Downgrade removes the column and the marker table; the action
        // row itself survives. Re-upgrading is idempotent.
        migrate_to(&mut conn, 9).unwrap();
        assert!(!table_names(&conn).contains(&"app_meta".to_string()));
        let columns: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(ai_action)").unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert!(!columns.contains(&"params".to_string()));
        migrate_to_latest(&mut conn).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let survived: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_action", [], |r| r.get(0))
            .unwrap();
        assert_eq!(survived, 1);
    }

    #[test]
    fn deleting_a_conflict_source_clears_the_marker_on_the_copy() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let insert = "INSERT INTO snippet (id, workspace_id, title, content_plaintext, type, \
             security_level, platform_scope, created_at, updated_at, version, conflict_of) \
             VALUES (?1, 'w', 't', 'b', 'text', 'normal', '[]', 1, 1, 1, ?2)";
        conn.execute(insert, rusqlite::params!["source", Option::<String>::None])
            .unwrap();
        conn.execute(insert, rusqlite::params!["copy", Some("source")])
            .unwrap();
        conn.execute("DELETE FROM snippet WHERE id = 'source'", [])
            .unwrap();
        let marker: Option<String> = conn
            .query_row(
                "SELECT conflict_of FROM snippet WHERE id = 'copy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, None);
    }

    #[test]
    fn rejects_a_target_version_newer_than_this_build() {
        let mut conn = open_in_memory().unwrap();
        let err = migrate_to(&mut conn, latest_version() + 1).unwrap_err();
        assert!(matches!(err, DbError::UnknownTargetVersion { .. }));
    }

    #[test]
    fn failed_migration_leaves_version_and_schema_untouched() {
        let broken = [
            Migration {
                version: 1,
                name: "good",
                up: "CREATE TABLE demo (id TEXT PRIMARY KEY);",
                down: "DROP TABLE demo;",
            },
            Migration {
                version: 2,
                name: "bad",
                up: "CREATE TABLE half (id TEXT PRIMARY KEY); THIS IS NOT SQL;",
                down: "DROP TABLE half;",
            },
        ];
        let mut conn = open_in_memory().unwrap();
        let err = run_migrations(&mut conn, &broken, 2).unwrap_err();
        assert!(matches!(err, DbError::Sqlite(_)));
        // The good step committed; the bad step rolled back completely.
        assert_eq!(schema_version(&conn).unwrap(), 1);
        let tables = table_names(&conn);
        assert!(tables.contains(&"demo".to_string()));
        assert!(
            !tables.contains(&"half".to_string()),
            "partial migration leaked"
        );
    }
}
