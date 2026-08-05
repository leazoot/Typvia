//! Domain model types for all Typvia entities (PRD §15).
//!
//! Every enum stored as TEXT in SQLite has its Rust definition here as the
//! single source of truth; unknown TEXT values surface as [`UnknownEnumValue`]
//! so callers handle forward compatibility explicitly.

mod ai_action;
mod app_rule;
mod device;
mod enums;
mod keyboard_snapshot;
mod organization;
mod snippet;
mod sync_record;
mod template;
mod validation;

pub use ai_action::AiAction;
pub use app_rule::AppRule;
pub use device::Device;
pub use enums::{
    AppRuleType, Platform, SecurityLevel, SnippetType, SyncEntityType, TemplateFieldType,
    TriggerMode, TrustLevel, UnknownEnumValue,
};
pub use keyboard_snapshot::{FolderMetadata, KeyboardSnapshot, SNAPSHOT_VERSION};
pub use organization::{Folder, SnippetTag, Tag};
pub use snippet::{Snippet, SnippetContent, SnippetVersion};
pub use sync_record::SyncRecord;
pub use template::TemplateField;
pub use validation::ValidationError;

/// UTC timestamp in milliseconds since the Unix epoch (database convention).
pub type TimestampMs = i64;

/// Entity identifiers are opaque strings; the concrete format (UUID text) is
/// fixed by the schema task (TASK-016) and never interpreted by the model.
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
