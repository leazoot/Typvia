// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Domain model types for all Typvia entities.
//!
//! Every enum stored as TEXT in SQLite has its Rust definition here as the
//! single source of truth; unknown TEXT values surface as [`UnknownEnumValue`]
//! so callers handle forward compatibility explicitly.

mod ai_action;
mod ai_egress;
mod ai_provider;
mod app_rule;
mod device;
mod enums;
mod keyboard_snapshot;
mod organization;
mod snippet;
mod sync_record;
mod sync_state;
mod template;
mod validation;
mod vault_key;

pub use ai_action::{AiAction, AiActionParams};
pub use ai_egress::AiEgressRecord;
pub use ai_provider::AiProvider;
pub use app_rule::AppRule;
pub use device::Device;
pub use enums::{
    AiActionInputSource, AiActionOutputMode, AiActionPermissionScope, AiProviderKind,
    AiRequestClass, AppRuleType, KeyDomain, OutboxState, PendingReason, Platform, SecurityLevel,
    SnippetType, SyncEntityType, TemplateFieldType, TransportKind, TriggerMode, TrustLevel,
    UnknownEnumValue,
};
pub use keyboard_snapshot::{FolderMetadata, KeyboardSnapshot, SNAPSHOT_VERSION, SnapshotSnippet};
pub use organization::{Folder, SnippetTag, Tag};
pub use snippet::{Snippet, SnippetContent, SnippetVersion};
pub use sync_record::SyncRecord;
pub use sync_state::{OutboxRecord, PendingRemoteRecord, SyncConfig, SyncShadow};
pub use template::TemplateField;
pub use validation::ValidationError;
pub use vault_key::{DomainKey, VaultKeyHeader};

/// UTC timestamp in milliseconds since the Unix epoch (database convention).
pub type TimestampMs = i64;

/// Entity identifiers are opaque strings; the concrete format (UUID text) is
/// fixed by the schema and never interpreted by the model.
pub type SnippetId = String;
/// See [`SnippetId`].
pub type WorkspaceId = String;
/// See [`SnippetId`].
pub type FolderId = String;
/// See [`SnippetId`].
pub type TagId = String;
/// See [`SnippetId`].
pub type TemplateFieldId = String;
/// See [`SnippetId`].
pub type AppRuleId = String;
/// See [`SnippetId`].
pub type DeviceId = String;
/// See [`SnippetId`].
pub type SyncRecordId = String;
/// See [`SnippetId`].
pub type AiActionId = String;
/// See [`SnippetId`].
pub type AiProviderId = String;
