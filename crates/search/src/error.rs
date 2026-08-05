//! Search-layer error type, pre-classified for the IPC error mapping.
//!
//! Messages never carry snippet content; only structural information.

use std::fmt;

/// Errors surfaced by index maintenance and queries.
#[derive(Debug)]
pub enum SearchError {
    /// System error: underlying SQLite failure.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
        }
    }
}

impl std::error::Error for SearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
        }
    }
}

impl From<rusqlite::Error> for SearchError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}
