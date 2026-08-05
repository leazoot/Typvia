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
            "folder",
            "snippet",
            "snippet_tag",
            "sync_record",
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
