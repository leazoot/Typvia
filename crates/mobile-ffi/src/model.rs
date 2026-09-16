// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The record shapes that cross into Swift.
//!
//! These mirror `host-service::dto` field for field rather than restating it:
//! the shared layer owns what a snippet is, and this file only re-expresses
//! it in the subset of types UniFFI can carry. Enum-valued columns stay as
//! their controlled TEXT values so an unknown one degrades on the platform
//! side instead of failing to decode.

use typvia_host_service::dto::{
    BackupRestoreDto, FolderDto, HistoryDto, ImportReportDto, LibraryCountsDto, SearchHitDto,
    SnippetCreateInput, SnippetDto, SnippetUpdateInput, TagDto, TemplateFieldDto, VersionBodyDto,
    VersionMetaDto,
};

/// One snippet row.
///
/// `body` is `None` for a sensitive snippet — its content exists only as
/// ciphertext, and reading it is a separate, explicitly gated call. A list
/// therefore never carries a secret, whatever the caller does with it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub snippet_type: String,
    pub security_level: String,
    pub description: Option<String>,
    pub folder_id: Option<String>,
    pub trigger: Option<String>,
    pub trigger_mode: Option<String>,
    pub language: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub is_enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    pub usage_count: u64,
    pub version: u32,
    pub deleted_at: Option<i64>,
}

impl From<SnippetDto> for Snippet {
    fn from(s: SnippetDto) -> Self {
        Self {
            id: s.id,
            title: s.title,
            body: s.body,
            snippet_type: s.snippet_type,
            security_level: s.security_level,
            description: s.description,
            folder_id: s.folder_id,
            trigger: s.trigger,
            trigger_mode: s.trigger_mode,
            language: s.language,
            is_favorite: s.is_favorite,
            is_pinned: s.is_pinned,
            is_enabled: s.is_enabled,
            created_at: s.created_at,
            updated_at: s.updated_at,
            last_used_at: s.last_used_at,
            usage_count: s.usage_count,
            version: s.version,
            deleted_at: s.deleted_at,
        }
    }
}

/// Fields the editor supplies when creating a snippet. Ids, timestamps,
/// version and usage counters are assigned by the shared layer, so they are
/// absent here — a caller cannot forge them.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetDraft {
    pub title: String,
    pub body: String,
    pub snippet_type: String,
    pub description: Option<String>,
    pub folder_id: Option<String>,
    pub trigger: Option<String>,
    pub trigger_mode: Option<String>,
    pub language: Option<String>,
}

impl From<SnippetDraft> for SnippetCreateInput {
    fn from(d: SnippetDraft) -> Self {
        Self {
            title: d.title,
            body: d.body,
            snippet_type: d.snippet_type,
            description: d.description,
            folder_id: d.folder_id,
            trigger: d.trigger,
            trigger_mode: d.trigger_mode,
            language: d.language,
        }
    }
}

/// A full edit of an existing snippet: everything the editor can change,
/// applied as one state rather than a patch.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetEdit {
    pub id: String,
    pub title: String,
    pub body: String,
    pub snippet_type: String,
    pub description: Option<String>,
    pub folder_id: Option<String>,
    pub trigger: Option<String>,
    pub trigger_mode: Option<String>,
    pub language: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub is_enabled: bool,
}

impl From<SnippetEdit> for SnippetUpdateInput {
    fn from(e: SnippetEdit) -> Self {
        Self {
            id: e.id,
            title: e.title,
            body: e.body,
            snippet_type: e.snippet_type,
            description: e.description,
            folder_id: e.folder_id,
            trigger: e.trigger,
            trigger_mode: e.trigger_mode,
            language: e.language,
            is_favorite: e.is_favorite,
            is_pinned: e.is_pinned,
            is_enabled: e.is_enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<FolderDto> for Folder {
    fn from(f: FolderDto) -> Self {
        Self {
            id: f.id,
            parent_id: f.parent_id,
            name: f.name,
            sort_order: f.sort_order,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

impl From<TagDto> for Tag {
    fn from(t: TagDto) -> Self {
        Self {
            id: t.id,
            name: t.name,
            created_at: t.created_at,
        }
    }
}

/// Live-snippet count inside one folder. Folders with nothing in them are
/// absent rather than present with a zero.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FolderCount {
    pub folder_id: String,
    pub count: u32,
}

/// Every number the library's chapter list needs, in one round trip.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LibraryCounts {
    pub total: u32,
    pub recent: u32,
    pub starred: u32,
    pub unsorted: u32,
    pub trash: u32,
    pub folders: Vec<FolderCount>,
}

impl From<LibraryCountsDto> for LibraryCounts {
    fn from(c: LibraryCountsDto) -> Self {
        Self {
            total: c.total,
            recent: c.recent,
            starred: c.starred,
            unsorted: c.unsorted,
            trash: c.trash,
            folders: c
                .folders
                .into_iter()
                .map(|f| FolderCount {
                    folder_id: f.folder_id,
                    count: f.count,
                })
                .collect(),
        }
    }
}

/// One ranked hit. `tier` is the stable tier code the ranking assigned
/// (`title_exact` | `trigger` | `title_prefix` | `tag` | `content`), which is
/// what a result row renders its match reason from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchHit {
    pub snippet_id: String,
    pub title: String,
    pub tier: String,
    pub is_sensitive: bool,
}

impl From<SearchHitDto> for SearchHit {
    fn from(h: SearchHitDto) -> Self {
        Self {
            snippet_id: h.snippet_id,
            title: h.title,
            tier: h.tier,
            is_sensitive: h.is_sensitive,
        }
    }
}

/// One version-history row. Metadata only: bodies travel one at a time, so
/// listing a sensitive snippet's history decrypts nothing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VersionMeta {
    pub version: u32,
    pub title: String,
    pub created_at: i64,
}

impl From<VersionMetaDto> for VersionMeta {
    fn from(v: VersionMetaDto) -> Self {
        Self {
            version: v.version,
            title: v.title,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct History {
    /// The snippet's live version number.
    pub current: u32,
    /// Newest first.
    pub entries: Vec<VersionMeta>,
}

impl From<HistoryDto> for History {
    fn from(h: HistoryDto) -> Self {
        Self {
            current: h.current,
            entries: h.entries.into_iter().map(Into::into).collect(),
        }
    }
}

/// One full history entry. A sensitive snippet's entry is decrypted on the
/// Rust side and only crosses on an unlocked session.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VersionBody {
    pub version: u32,
    pub title: String,
    pub body: String,
    pub created_at: i64,
}

impl From<VersionBodyDto> for VersionBody {
    fn from(v: VersionBodyDto) -> Self {
        Self {
            version: v.version,
            title: v.title,
            body: v.body,
            created_at: v.created_at,
        }
    }
}

/// One template field. `id` is empty on a newly authored field; the
/// repository assigns it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TemplateField {
    pub id: String,
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub default_value: Option<String>,
    pub options: Vec<String>,
    pub validation: Option<String>,
    pub is_required: bool,
    pub sort_order: i32,
    pub platform_overrides: Option<String>,
}

impl From<TemplateFieldDto> for TemplateField {
    fn from(f: TemplateFieldDto) -> Self {
        Self {
            id: f.id,
            name: f.name,
            label: f.label,
            field_type: f.field_type,
            default_value: f.default_value,
            options: f.options,
            validation: f.validation,
            is_required: f.is_required,
            sort_order: f.sort_order,
            platform_overrides: f.platform_overrides,
        }
    }
}

impl From<TemplateField> for TemplateFieldDto {
    fn from(f: TemplateField) -> Self {
        Self {
            id: f.id,
            name: f.name,
            label: f.label,
            field_type: f.field_type,
            default_value: f.default_value,
            options: f.options,
            validation: f.validation,
            is_required: f.is_required,
            sort_order: f.sort_order,
            platform_overrides: f.platform_overrides,
        }
    }
}

/// One entry an import could not take, and why.
///
/// The reason is the core's own wording, and the label is whatever the file
/// called the entry. Neither carries a body: a report about what failed to
/// import is not a place to print the text that failed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportSkipped {
    pub label: Option<String>,
    pub reason: String,
}

/// What one import did.
///
/// Counts and names, never content. `conflicts` are triggers the library
/// already had — those entries were not written, and saying so by name is
/// what lets a reader go and look at the one they already have.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportReport {
    pub imported: u32,
    pub conflicts: Vec<String>,
    pub skipped: Vec<ImportSkipped>,
}

impl From<ImportReportDto> for ImportReport {
    fn from(r: ImportReportDto) -> Self {
        Self {
            imported: u32::try_from(r.imported).unwrap_or(u32::MAX),
            conflicts: r.conflicts,
            skipped: r
                .skipped
                .into_iter()
                .map(|s| ImportSkipped {
                    label: s.label,
                    reason: s.reason,
                })
                .collect(),
        }
    }
}

/// What a restore brought back: counts only, and whether the vault's wrapped
/// key material came with it. No content crosses here.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BackupRestored {
    pub snippets: u32,
    pub folders: u32,
    pub tags: u32,
    pub versions: u32,
    /// True when the backup carried the vault's wrapped keys. The vault still
    /// opens only with its own master password — nothing here unwraps it.
    pub vault_restored: bool,
}

impl From<BackupRestoreDto> for BackupRestored {
    fn from(r: BackupRestoreDto) -> Self {
        Self {
            snippets: u32::try_from(r.snippets).unwrap_or(u32::MAX),
            folders: u32::try_from(r.folders).unwrap_or(u32::MAX),
            tags: u32::try_from(r.tags).unwrap_or(u32::MAX),
            versions: u32::try_from(r.versions).unwrap_or(u32::MAX),
            vault_restored: r.vault_restored,
        }
    }
}
