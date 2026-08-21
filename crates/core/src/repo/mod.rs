// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Repository layer: the only place (besides migrations) where SQL lives.
//!
//! All queries are parameterized; list queries are always bounded. Callers
//! hand in validated model values — every write validates again at the door
//! so unvalidated data can never reach storage.

mod ai_action_repo;
mod ai_egress_log_repo;
mod ai_provider_repo;
mod app_meta_repo;
mod app_rule_repo;
mod device_repo;
mod embedding_repo;
mod folder_repo;
mod snippet_repo;
mod sync_outbox_repo;
mod sync_state_repo;
mod tag_repo;
mod template_field_repo;
mod vault_key_repo;
mod version_repo;

use std::fmt;

pub use ai_action_repo::AiActionRepo;
pub use ai_egress_log_repo::AiEgressLogRepo;
pub use ai_provider_repo::AiProviderRepo;
pub use app_meta_repo::AppMetaRepo;
pub use app_rule_repo::AppRuleRepo;
pub use device_repo::DeviceRepo;
pub use embedding_repo::{EmbeddingRepo, PendingEmbedding, StoredEmbedding};
pub use folder_repo::FolderRepo;
pub use snippet_repo::{ListScope, SnippetRepo, TRASH_RETENTION_MS};
pub use sync_outbox_repo::SyncOutboxRepo;
pub use sync_state_repo::SyncStateRepo;
pub use tag_repo::TagRepo;
pub use template_field_repo::TemplateFieldRepo;
pub use vault_key_repo::VaultKeyRepo;
pub use version_repo::{VERSION_KEEP_MAX, VERSION_KEEP_MIN, VERSION_MAX_AGE_MS, VersionRepo};

use crate::model::{UnknownEnumValue, ValidationError};

/// Generates a new UUID v4 text id.
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
