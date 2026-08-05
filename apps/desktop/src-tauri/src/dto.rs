//! IPC data-transfer shapes. Commands parse these, validate, and map to core
//! model types — core types never cross the WebView boundary directly.
//!
//! v1 scope: normal (plaintext) snippets only. Sensitive bodies are
//! ciphertext in core and are never serialized here (`body` is `None`);
//! creating or editing vault content arrives with the vault batch.

use serde::{Deserialize, Serialize};
use typvia_core::model::{Folder, Snippet, SnippetContent, Tag};
use typvia_search::{MatchTier, SearchHit};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetDto {
    pub id: String,
    pub title: String,
    /// Plaintext body; `None` when the snippet is sensitive (ciphertext).
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

impl From<Snippet> for SnippetDto {
    fn from(s: Snippet) -> Self {
        let body = match s.content {
            SnippetContent::Plaintext(text) => Some(text),
            SnippetContent::Ciphertext(_) => None,
        };
        Self {
            id: s.id,
            title: s.title,
            body,
            snippet_type: s.snippet_type.as_str().to_string(),
            security_level: s.security_level.as_str().to_string(),
            description: s.description,
            folder_id: s.folder_id,
            trigger: s.trigger,
            trigger_mode: s.trigger_mode.map(|m| m.as_str().to_string()),
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnippetCreateInput {
    pub title: String,
    pub body: String,
    pub snippet_type: String,
    pub description: Option<String>,
    pub folder_id: Option<String>,
    pub trigger: Option<String>,
    pub trigger_mode: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnippetUpdateInput {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<Folder> for FolderDto {
    fn from(f: Folder) -> Self {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FolderCreateInput {
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FolderUpdateInput {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

impl From<Tag> for TagDto {
    fn from(t: Tag) -> Self {
        Self {
            id: t.id,
            name: t.name,
            created_at: t.created_at,
        }
    }
}

/// Live-snippet count inside one folder (zero-count folders are absent).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCountDto {
    pub folder_id: String,
    pub count: u32,
}

/// Rail numbers for the Library page: the four saved views plus per-folder
/// counts, all in one round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCountsDto {
    pub total: u32,
    pub recent: u32,
    pub starred: u32,
    pub unsorted: u32,
    pub trash: u32,
    pub folders: Vec<FolderCountDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub snippet_id: String,
    pub title: String,
    /// Stable tier code: title_exact | trigger | title_prefix | tag | content.
    pub tier: String,
    pub is_sensitive: bool,
}

impl From<SearchHit> for SearchHitDto {
    fn from(hit: SearchHit) -> Self {
        let tier = match hit.tier {
            MatchTier::TitleExact => "title_exact",
            MatchTier::Trigger => "trigger",
            MatchTier::TitlePrefix => "title_prefix",
            MatchTier::Tag => "tag",
            MatchTier::Content => "content",
        };
        Self {
            snippet_id: hit.snippet_id,
            title: hit.title,
            tier: tier.to_string(),
            is_sensitive: hit.is_sensitive,
        }
    }
}
