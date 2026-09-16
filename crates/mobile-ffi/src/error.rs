// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The error shape the platform layer sees.
//!
//! One variant per stable host-service error code, so a host's `catch` can
//! branch on the classification (correctable input, business rule, missing
//! record, permission, offline, internal) without parsing strings. The
//! carried reason is a rule or field name produced by the shared layer; it
//! never contains snippet content, key material or SQL, and the two variants
//! that could not promise that carry no reason at all.
//!
//! The field is called `reason` rather than `message` because the generated
//! Kotlin makes each variant a `Throwable`, and a `message` of its own
//! collides with the one every Throwable already has — the bindings then do
//! not compile at all. One word, two platforms.

use typvia_host_service::error::{IpcError, IpcErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum CoreError {
    /// The input failed validation and the user can correct it.
    Validation { reason: String },
    /// A uniqueness, reference or state rule was violated.
    Conflict { reason: String },
    /// The addressed record does not exist.
    NotFound,
    /// The action needs an unlocked vault, or a permission the app lacks.
    PermissionDenied { reason: String },
    /// An app rule blocks this action for the destination app.
    RuleBlocked { reason: String },
    /// A remote service could not be reached. The product keeps working
    /// locally, so this is an offline state to show, not a failure to report.
    Unavailable { reason: String },
    /// Storage or internal failure, not user-correctable. Carries no detail:
    /// the underlying reason can name paths and SQL.
    System,
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::System => write!(f, "internal storage error"),
            Self::Validation { reason }
            | Self::Conflict { reason }
            | Self::PermissionDenied { reason }
            | Self::RuleBlocked { reason }
            | Self::Unavailable { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<typvia_core::repo::RepoError> for CoreError {
    fn from(error: typvia_core::repo::RepoError) -> Self {
        IpcError::from(error).into()
    }
}

impl From<IpcError> for CoreError {
    fn from(error: IpcError) -> Self {
        let reason = error.message;
        match error.code {
            IpcErrorCode::Validation => Self::Validation { reason },
            IpcErrorCode::Conflict => Self::Conflict { reason },
            IpcErrorCode::NotFound => Self::NotFound,
            IpcErrorCode::PermissionDenied => Self::PermissionDenied { reason },
            IpcErrorCode::RuleBlocked => Self::RuleBlocked { reason },
            IpcErrorCode::Unavailable => Self::Unavailable { reason },
            IpcErrorCode::System => Self::System,
        }
    }
}
