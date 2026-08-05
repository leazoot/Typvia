//! Repository layer: the only place (besides migrations) where SQL lives.
//!
//! All queries are parameterized; list queries are always bounded. Callers
//! hand in validated model values — every write validates again at the door
//! so unvalidated data can never reach storage.

mod folder_repo;
mod snippet_repo;
mod tag_repo;

use std::fmt;

pub use folder_repo::FolderRepo;
pub use snippet_repo::SnippetRepo;
pub use tag_repo::TagRepo;

use crate::model::{UnknownEnumValue, ValidationError};

/// Generates a new UUID v4 text id (docs/05_DATA_MODEL.md §5.3).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Repository-layer error, pre-classified for the IPC error mapping.
#[derive(Debug)]
pub enum RepoError {
    /// Business error: the value failed validation.
    Validation(ValidationError),
    /// Business error: a uniqueness or reference rule was violated
    /// (duplicate tag name, missing folder, folder cycle, ...).
    Conflict(&'static str),
    /// Business error: the addressed row does not exist.
    NotFound,
    /// System error: a stored TEXT enum value this build does not know.
    UnknownEnum(UnknownEnumValue),
    /// System error: underlying SQLite failure.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => write!(f, "{e}"),
            Self::Conflict(rule) => write!(f, "conflict: {rule}"),
            Self::NotFound => f.write_str("not found"),
            Self::UnknownEnum(e) => write!(f, "{e}"),
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
        }
    }
}

impl std::error::Error for RepoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(e) => Some(e),
            Self::UnknownEnum(e) => Some(e),
            Self::Sqlite(e) => Some(e),
            Self::Conflict(_) | Self::NotFound => None,
        }
    }
}

impl From<ValidationError> for RepoError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

impl From<UnknownEnumValue> for RepoError {
    fn from(e: UnknownEnumValue) -> Self {
        Self::UnknownEnum(e)
    }
}

impl From<rusqlite::Error> for RepoError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}
