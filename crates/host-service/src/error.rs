// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The stable IPC error shape: user / business / system classification.
//! Messages carry rule and field names only — never snippet content, key
//! material or raw SQL.

use serde::Serialize;
use typvia_core::db::DbError;
use typvia_core::repo::RepoError;
use typvia_core::vault::VaultError;
use typvia_search::SearchError;

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
    /// Business error: an app rule blocks this action for the destination
    /// app. Distinct from `PermissionDenied` so the panel shows the rule
    /// notice instead of degrading to copy or prompting an unlock.
    RuleBlocked,
    /// Business error: a remote service could not be reached (offline, the
    /// server is down, or a backoff is in force). Distinct from `System` so
    /// the UI can show the offline state instead of a failure — the product
    /// keeps working locally either way.
    Unavailable,
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

    pub fn rule_blocked(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::RuleBlocked,
            message: message.into(),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::Unavailable,
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

impl From<VaultError> for IpcError {
    fn from(error: VaultError) -> Self {
        // VaultError Display strings are static and payload-free, keeping the
        // log red line. Unlock failures collapse to permission_denied without
        // saying which part was wrong; corrupt/storage/secure-store faults are
        // non-correctable system errors.
        match error {
            VaultError::NotInitialized => Self::conflict("vault is not set up"),
            VaultError::AlreadyInitialized => Self::conflict("vault is already set up"),
            VaultError::WrongPassword => Self::permission_denied("unlock failed"),
            VaultError::Throttled { .. } => {
                Self::permission_denied("too many attempts, try again later")
            }
            VaultError::Locked => Self::permission_denied("unlock the vault first"),
            VaultError::BiometricUnavailable => Self::conflict("biometric unlock is not set up"),
            VaultError::Corrupt | VaultError::SecureStore(_) | VaultError::Storage(_) => {
                Self::system()
            }
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
