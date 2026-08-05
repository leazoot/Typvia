//! SQLite connection management and schema migrations.
//!
//! SQLite is the single primary data source (one file per device). The core
//! stays synchronous with a single writer; IO concurrency is a host concern.

mod migrate;

use std::fmt;
use std::path::Path;

use rusqlite::Connection;

pub use migrate::{Migration, latest_version, migrate_to, migrate_to_latest, schema_version};

/// Database-layer error (system error category at the IPC boundary).
///
/// Messages never include row contents; they only describe the operation
/// that failed.
#[derive(Debug)]
pub enum DbError {
    /// Underlying SQLite failure.
    Sqlite(rusqlite::Error),
    /// A migration target version that does not exist.
    UnknownTargetVersion { requested: u32, latest: u32 },
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Self::UnknownTargetVersion { requested, latest } => {
                write!(
                    f,
                    "unknown migration target version {requested} (latest is {latest})"
                )
            }
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::UnknownTargetVersion { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

/// Opens (creating if needed) the database file and applies the mandatory
/// connection configuration. Does not run migrations.
pub fn open(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

/// Opens a fresh in-memory database with the same configuration; used by
/// tests and never in production paths.
pub fn open_in_memory() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    Ok(conn)
}

/// Connection invariants: foreign keys enforced, WAL journal (file-backed
/// databases; in-memory reports its own mode), and a busy timeout so the
/// single writer never fails immediately on a locked database.
fn configure(conn: &Connection) -> Result<(), DbError> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "busy_timeout", 5_000)?;
    // journal_mode returns the resulting mode as a row; the value is only
    // meaningful for file-backed databases.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_a_file_backed_database_in_wal_mode() {
        let dir = std::env::temp_dir().join("typvia-core-db-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("open_test.sqlite3");
        let _ = std::fs::remove_file(&path);

        let conn = open(&path).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn in_memory_connection_enforces_foreign_keys() {
        let conn = open_in_memory().unwrap();
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }
}
