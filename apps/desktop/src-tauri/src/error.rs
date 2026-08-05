//! The stable IPC error shape (docs/03_ARCHITECTURE.md: user / business /
//! system classification). Messages carry rule and field names only — never
//! snippet content, key material or raw SQL (PRD §16.5).

use serde::Serialize;
use typvia_core::db::DbError;
use typvia_core::repo::RepoError;
use typvia_search::SearchError;

use crate::injector::InjectorError;

/// Stable machine-readable error codes shared with the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    /// User error: the input failed validation and can be corrected.
    Validation,
    /// Business error: a uniqueness / reference / state rule was violated.
    Conflict,
    /// Business error: the addressed record does not exist.
    NotFound,
    /// Business error: injection needs OS permission the app lacks; the
    /// frontend degrades to copy on this code.
    PermissionDenied,
    /// System error: storage or internal failure; not user-correctable.
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
}

impl IpcError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::Validation,
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::Conflict,
            message: message.into(),
        }
    }

    pub fn not_found() -> Self {
        Self {
            code: IpcErrorCode::NotFound,
            message: "not found".into(),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::PermissionDenied,
            message: message.into(),
        }
    }

    /// System errors keep a generic message: backend details (SQL text,
    /// paths) must not cross the IPC boundary.
    pub fn system() -> Self {
        Self {
            code: IpcErrorCode::System,
            message: "internal storage error".into(),
        }
    }
}

impl From<InjectorError> for IpcError {
    fn from(error: InjectorError) -> Self {
        // InjectorError Display strings are static and payload-free, so they
        // are safe to forward. PermissionDenied is a recoverable business
        // error (offer copy); the rest are non-correctable system failures.
        match error {
            InjectorError::PermissionDenied => Self::permission_denied(error.to_string()),
            InjectorError::Clipboard | InjectorError::Synthesis | InjectorError::Unsupported => {
                Self::system()
            }
        }
    }
}

impl From<RepoError> for IpcError {
    fn from(error: RepoError) -> Self {
        match error {
            // ValidationError messages are static field/rule strings.
            RepoError::Validation(e) => Self::validation(e.to_string()),
            RepoError::Conflict(rule) => Self::conflict(rule),
            RepoError::NotFound => Self::not_found(),
            RepoError::UnknownEnum(_) | RepoError::Sqlite(_) => Self::system(),
        }
    }
}

impl From<SearchError> for IpcError {
    fn from(_: SearchError) -> Self {
        Self::system()
    }
}

impl From<DbError> for IpcError {
    fn from(_: DbError) -> Self {
        Self::system()
    }
}
