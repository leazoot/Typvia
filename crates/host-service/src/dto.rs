//! IPC data-transfer shapes. Commands parse these, validate, and map to core
//! model types — core types never cross the WebView boundary directly.
//!
//! A sensitive snippet's body is ciphertext in core and is never serialized
//! here (`SnippetDto.body` is `None`). Sensitive content is created/edited
//! through the vault commands: the plaintext is supplied on the create/update
//! input, encrypted host-side, and only ever read back through
//! an explicit `vault_reveal` on an unlocked session.

use serde::{Deserialize, Serialize};
use typvia_core::model::{Folder, Snippet, SnippetContent, Tag, TemplateField};
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

/// Observable vault-session state for the Vault page. Carries no
/// key material — only whether a vault exists, whether it is currently
/// unlocked, and the timestamps the UI uses to render the auto-lock countdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatusDto {
    /// A device vault has been set up (a key header exists).
    pub initialized: bool,
    /// The session currently holds the master key.
    pub unlocked: bool,
    /// When the current unlock happened (ms), if unlocked.
    pub unlocked_at: Option<i64>,
    /// Last activity that deferred the idle timer (ms), if unlocked.
    pub last_activity_at: Option<i64>,
    /// Idle window before auto-lock (ms); drives the re-lock countdown.
    pub idle_timeout_ms: i64,
}

/// A template field crossing the IPC boundary. `snippetId` is supplied by the
/// command from its own parameter, never trusted from the client, so it is
/// absent here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateFieldDto {
    /// Empty on a newly-authored field; the repository assigns an id.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub default_value: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    pub validation: Option<String>,
    pub is_required: bool,
    #[serde(default)]
    pub sort_order: i32,
    pub platform_overrides: Option<String>,
}

impl From<TemplateField> for TemplateFieldDto {
    fn from(f: TemplateField) -> Self {
        Self {
            id: f.id,
            name: f.name,
            label: f.label,
            field_type: f.field_type.as_str().to_string(),
            default_value: f.default_value,
            options: f.options,
            validation: f.validation,
            is_required: f.is_required,
            sort_order: f.sort_order,
            platform_overrides: f.platform_overrides,
        }
    }
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

/// Browser-integration status for the desktop Settings block.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserIntegrationStatusDto {
    /// Whether the browser-snapshot data plane is switched on.
    pub enabled: bool,
    /// Whether at least one native-messaging host manifest is registered.
    pub host_installed: bool,
}

/// Semantic-search status for the desktop Settings block.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatusDto {
    /// All model files are on disk (the feature's on state).
    pub model_present: bool,
    pub downloading: bool,
    pub download_received: u64,
    pub download_total: u64,
    pub download_failed: bool,
    /// Vectors stored for the current model.
    pub embedded_count: i64,
    /// Live snippets still waiting for a (fresh) vector.
    pub pending_count: i64,
    pub model_id: String,
}

/// Espanso integration status for the Settings block and Home rail.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EspansoStatusDto {
    /// Managed-engine state code: `running` | `retrying` | `failed`
    /// | `off` | `conflict` | `takeover_pending` | `standing_aside`
    /// | `unavailable`.
    pub state: String,
    /// Reported engine version, when the binary is present.
    pub version: Option<String>,
    /// Absolute path of the generated config inside the private engine dirs.
    pub config_path: Option<String>,
    /// Whether the generated config currently exists (integration turned on).
    pub enabled: bool,
    /// Number of triggers that would be written from the current snippet set.
    pub trigger_count: usize,
    /// Recorded coexistence answer: `takeover` | `stand_aside`, when chosen.
    pub coexistence_choice: Option<String>,
}

/// Result of writing (or clearing) the espanso config.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EspansoSyncDto {
    /// Whether the config file exists after the operation.
    pub enabled: bool,
    /// Triggers written (0 after a disable).
    pub trigger_count: usize,
}

/// A match from an imported file that Typvia could not take in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EspansoSkippedDto {
    pub trigger: Option<String>,
    pub reason: String,
}

/// Summary of an Espanso YAML import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EspansoImportDto {
    /// Number of snippets created.
    pub imported: usize,
    /// Triggers skipped because they were already in use.
    pub conflicts: Vec<String>,
    /// Matches skipped as unsupported, with reasons.
    pub skipped: Vec<EspansoSkippedDto>,
}

/// An entry from a generic import file that could not be taken in. The label
/// identifies the entry (title, trigger, row number…) when one is readable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkippedDto {
    pub label: Option<String>,
    pub reason: String,
}

/// Summary of a Markdown / JSON / CSV import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReportDto {
    /// Number of snippets created.
    pub imported: usize,
    /// Triggers skipped because they were already in use.
    pub conflicts: Vec<String>,
    /// Entries skipped as unimportable, with reasons.
    pub skipped: Vec<ImportSkippedDto>,
}

/// What a backup restore brought back (counts only, no content).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreDto {
    pub snippets: usize,
    pub folders: usize,
    pub tags: usize,
    pub versions: usize,
    /// Whether wrapped vault key material was present and restored.
    pub vault_restored: bool,
}

/// One version-history row (metadata only — bodies travel one at a time
/// through `history_get`, so listing never decrypts sensitive content).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetaDto {
    pub version: u32,
    pub title: String,
    pub created_at: i64,
}

/// A snippet's history for the History screen, newest first.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDto {
    /// The snippet's live version number.
    pub current: u32,
    pub entries: Vec<VersionMetaDto>,
}

/// One full history entry for the diff view. `body` is plaintext: sensitive
/// entries are decrypted host-side and only cross IPC on an unlocked session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBodyDto {
    pub version: u32,
    pub title: String,
    pub body: String,
    pub created_at: i64,
}

/// One per-snippet application rule with the snippet title
/// joined in for the settings list. Titles are index-safe metadata even for
/// sensitive snippets, so this never leaks content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRuleDto {
    pub id: String,
    pub snippet_id: String,
    pub snippet_title: String,
    pub platform: String,
    pub app_identifier: String,
    pub rule_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppRuleCreateInput {
    pub snippet_id: String,
    pub app_identifier: String,
    pub rule_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppRuleUpdateInput {
    pub app_identifier: String,
    pub rule_type: String,
}

/// Panel result page: the rows that survived app-rule filtering plus how many
/// were hidden, so the panel can state the real count — an omission must
/// never be the only signal that rows are missing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelResultsDto {
    pub rows: Vec<SnippetDto>,
    pub hidden_by_rules: u32,
}

/// One snippet awaiting the user's conflict decision: the body that stayed
/// on the entity and the parked copy that carries the other device's body.
/// Sensitive pairs come through
/// with both bodies `None` — a locked conflict is read as two locked rows,
/// never decrypted for the comparison.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPairDto {
    /// The snippet that kept the entity identity (triggers, usage, links).
    pub source: SnippetDto,
    /// The conflict copy pointing at it through `conflict_of`.
    pub copy: SnippetDto,
    /// Both sides are ciphertext; resolving in favour of the copy needs an
    /// unlocked vault because the envelope is bound to the copy's id.
    pub sensitive: bool,
}

/// Which body survives a conflict decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKeep {
    /// Keep the snippet's current body; the copy moves to the recycle bin.
    Source,
    /// Move the copy's body onto the snippet (a new version); the copy then
    /// moves to the recycle bin.
    Copy,
    /// Keep both: the copy becomes an ordinary independent snippet.
    Both,
}

/// First-run onboarding state: whether the flow already ran (or was
/// skipped) on this data directory.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatusDto {
    pub completed: bool,
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
