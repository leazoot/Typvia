// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Save-time organizing use case: draft in, validated suggestion out.
//! Nothing here writes — the user confirms in the UI and the confirmed
//! values travel through the existing save path
//! (`service::snippet_create` / `snippet_update`), never a new one.
//!
//! The flow is deliberately split into two `Connection` phases with the AI
//! call between them, done by the host WITHOUT holding the database lock
//! (an AI round trip is seconds long; holding the single connection across
//! it would freeze every other command):
//!
//! 1. [`organize_prompt`]: refuse sensitive/secret-bearing drafts, read the
//!    tag/folder vocabulary, build the prompt pair.
//! 2. The host sends the prompt through `crates/ai` (which enforces the
//!    mandatory egress gate and log).
//! 3. [`organize_validate`]: parse the answer as untrusted external input —
//!    unknown enums, taken triggers, unknown tags/folders are dropped, not
//!    stored.

use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use typvia_core::ai_action_seed::ensure_builtin_actions;
use typvia_core::model::{
    AiAction, AiActionInputSource, AiActionOutputMode, AiActionParams, AiActionPermissionScope,
    AiProvider, AiProviderKind, SecurityLevel, SnippetType, TemplateField, TemplateFieldType,
};
use typvia_core::repo::{
    AiActionRepo, AiEgressLogRepo, AiProviderRepo, FolderRepo, SnippetRepo, TagRepo, new_id,
};
use typvia_core::sensitive::{detect, mask_suspected_secrets};

use crate::dto::{FolderDto, TagDto};
use crate::error::IpcError;

/// Draft body chars sent to the provider; longer bodies are cut here so a
/// pasted book does not become a maximal token bill. Suggestions only need
/// the head to classify.
const BODY_PROMPT_CAP: usize = 6_000;
/// Vocabulary caps keep the prompt bounded on large libraries.
const TAG_PROMPT_CAP: usize = 100;
const FOLDER_PROMPT_CAP: usize = 100;
/// Sanity bounds on suggested values; anything beyond is dropped.
const TITLE_MAX_CHARS: usize = 200;
const DESCRIPTION_MAX_CHARS: usize = 500;
const TRIGGER_MAX_CHARS: usize = 32;
const TAGS_MAX: usize = 10;

/// Snippet types the model may suggest. `sensitive` is excluded (it is
/// implied by the security level, and the type alone must never force a
/// level), as are `ai_action` and `temporary` (not save-time categories).
const SUGGESTIBLE_TYPES: [SnippetType; 7] = [
    SnippetType::Text,
    SnippetType::Markdown,
    SnippetType::Code,
    SnippetType::Command,
    SnippetType::Prompt,
    SnippetType::Template,
    SnippetType::Link,
];

/// One configured provider, as Settings shows it. `has_api_key` is filled
/// by the host — key presence lives in the platform secure store, which
/// this crate never touches, and the key value itself never appears in
/// any DTO.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    pub timeout_ms: i64,
    pub has_api_key: bool,
}

fn provider_dto(provider: AiProvider, has_api_key: bool) -> AiProviderDto {
    AiProviderDto {
        id: provider.id,
        name: provider.name,
        kind: provider.kind.as_str().to_string(),
        base_url: provider.base_url,
        model: provider.model,
        timeout_ms: provider.timeout_ms,
        has_api_key,
    }
}

/// Create/update payload. `base_url` and `timeout_ms` arrive already
/// resolved by the host (kind defaults and the URL egress policy live in
/// crates/ai, which this crate does not depend on).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiProviderSaveInput {
    /// Absent on create.
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    pub timeout_ms: i64,
}

/// All configured providers, stable name order. `has_key` reports secure
/// store key presence per provider id (injected by the host).
pub fn ai_provider_list(
    conn: &Connection,
    has_key: impl Fn(&str) -> bool,
) -> Result<Vec<AiProviderDto>, IpcError> {
    let rows = AiProviderRepo::new(conn).list(u32::MAX, 0)?;
    Ok(rows
        .into_iter()
        .map(|p| {
            let has = has_key(&p.id);
            provider_dto(p, has)
        })
        .collect())
}

/// Creates (no id) or rewrites (id present) one provider row.
pub fn ai_provider_save(
    conn: &Connection,
    input: AiProviderSaveInput,
    now: i64,
) -> Result<AiProviderDto, IpcError> {
    let kind = input
        .kind
        .parse::<AiProviderKind>()
        .map_err(|_| IpcError::validation("unknown provider kind"))?;
    let repo = AiProviderRepo::new(conn);
    match input.id {
        None => {
            let provider = AiProvider {
                id: new_id(),
                name: input.name,
                kind,
                base_url: input.base_url,
                model: input.model,
                timeout_ms: input.timeout_ms,
                created_at: now,
                updated_at: now,
            };
            repo.insert(&provider)?;
            Ok(provider_dto(provider, false))
        }
        Some(id) => {
            let existing = repo.get(&id)?.ok_or_else(IpcError::not_found)?;
            let provider = AiProvider {
                id,
                name: input.name,
                kind,
                base_url: input.base_url,
                model: input.model,
                timeout_ms: input.timeout_ms,
                created_at: existing.created_at,
                updated_at: now,
            };
            repo.update(&provider)?;
            Ok(provider_dto(provider, false))
        }
    }
}

/// Deletes one provider row. The secure-store key entry and any actions
/// referencing the provider are the caller's concern (no FK by design; the
/// host removes the key entry alongside this call).
pub fn ai_provider_delete(conn: &Connection, id: &str) -> Result<(), IpcError> {
    AiProviderRepo::new(conn).delete(id)?;
    Ok(())
}

/// The draft being organized, as typed so far in the editor.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizeDraftInput {
    pub title: String,
    pub body: String,
    pub description: Option<String>,
    /// True when the editor already marks this draft sensitive; such
    /// drafts never leave the device.
    pub is_sensitive: bool,
}

/// The prompt pair the host sends through `crates/ai`. Carries draft
/// content — no `Serialize`, so it cannot cross the IPC boundary, and
/// `Debug` renders lengths only (log red line).
pub struct OrganizePrompt {
    pub system: String,
    pub user: String,
}

impl std::fmt::Debug for OrganizePrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrganizePrompt")
            .field("system_len", &self.system.len())
            .field("user_len", &self.user.len())
            .finish()
    }
}

/// A validated, confirmable suggestion. Every field already passed the
/// vocabulary/uniqueness checks; absent fields mean the model offered
/// nothing usable for them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeSuggestionDto {
    pub title: Option<String>,
    pub description: Option<String>,
    pub snippet_type: Option<String>,
    pub security_level: Option<String>,
    pub trigger: Option<String>,
    pub tags: Vec<TagDto>,
    pub folder: Option<FolderDto>,
}

/// Phase 1: refuses drafts that must not egress, then builds the prompt
/// from the draft and the library vocabulary. Read-only.
pub fn organize_prompt(
    conn: &Connection,
    input: &OrganizeDraftInput,
) -> Result<OrganizePrompt, IpcError> {
    if input.is_sensitive {
        return Err(IpcError::conflict(
            "sensitive snippets never leave this device",
        ));
    }
    if input.body.trim().is_empty() {
        return Err(IpcError::validation("draft body must not be blank"));
    }
    // Pre-scan with the sensitive-content detector. The authoritative gate
    // sits in crates/ai; this early check refuses before any provider
    // configuration is even consulted (defense in depth, same pattern as
    // the URL policy's config+transport double enforcement).
    let scanned = format!(
        "{}\n{}\n{}",
        input.title,
        input.body,
        input.description.as_deref().unwrap_or("")
    );
    if !detect(&scanned).is_empty() {
        return Err(IpcError::conflict(
            "this draft looks like it contains a secret, so it was not sent — \
             remove the secret, or save it as a sensitive snippet instead",
        ));
    }

    let tags = TagRepo::new(conn).list_all()?;
    let folders = FolderRepo::new(conn).list_all_parents_first()?;
    let tag_names: Vec<&str> = tags
        .iter()
        .take(TAG_PROMPT_CAP)
        .map(|t| t.name.as_str())
        .collect();
    let folder_names: Vec<&str> = folders
        .iter()
        .take(FOLDER_PROMPT_CAP)
        .map(|f| f.name.as_str())
        .collect();
    let type_names: Vec<&str> = SUGGESTIBLE_TYPES.iter().map(|t| t.as_str()).collect();

    let system = format!(
        "You organize text snippets for a snippet manager. Given a draft, \
         answer with a single JSON object only (no prose, no code fences) \
         using these optional keys:\n\
         \"title\": a short descriptive title (a few words)\n\
         \"description\": one sentence describing what the snippet is for\n\
         \"snippetType\": one of: {}\n\
         \"securityLevel\": \"normal\", or \"sensitive\" when the content \
         should be stored encrypted\n\
         \"trigger\": a short abbreviation without spaces, e.g. \";sig\"\n\
         \"tags\": an array chosen ONLY from the existing tags\n\
         \"folder\": one name chosen ONLY from the existing folders\n\
         Omit any key you have no good suggestion for.\n\
         Existing tags: {}\n\
         Existing folders: {}",
        type_names.join(", "),
        if tag_names.is_empty() {
            "(none)".to_string()
        } else {
            tag_names.join(", ")
        },
        if folder_names.is_empty() {
            "(none)".to_string()
        } else {
            folder_names.join(", ")
        },
    );

    let body: String = input.body.chars().take(BODY_PROMPT_CAP).collect();
    let mut user = String::new();
    if !input.title.trim().is_empty() {
        user.push_str("Current title: ");
        user.push_str(input.title.trim());
        user.push('\n');
    }
    if let Some(description) = input.description.as_deref()
        && !description.trim().is_empty()
    {
        user.push_str("Current description: ");
        user.push_str(description.trim());
        user.push('\n');
    }
    user.push_str("Draft content:\n");
    user.push_str(&body);

    Ok(OrganizePrompt { system, user })
}

/// What the model is asked to return. Unknown keys are ignored; every
/// present value is re-validated below (AI answers are external input).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSuggestion {
    title: Option<String>,
    description: Option<String>,
    snippet_type: Option<String>,
    security_level: Option<String>,
    trigger: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    folder: Option<String>,
}

/// Phase 3: parses and validates the model's answer. Invalid pieces are
/// dropped (never stored); an answer with no JSON at all is a business
/// error the user can retry. Read-only.
pub fn organize_validate(
    conn: &Connection,
    answer: &str,
) -> Result<OrganizeSuggestionDto, IpcError> {
    let raw: RawSuggestion = parse_answer(answer)?;

    let title = raw
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.chars().count() <= TITLE_MAX_CHARS);
    let description = raw
        .description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty() && d.chars().count() <= DESCRIPTION_MAX_CHARS);

    let snippet_type = raw
        .snippet_type
        .as_deref()
        .and_then(|t| t.trim().parse::<SnippetType>().ok())
        .filter(|t| SUGGESTIBLE_TYPES.contains(t))
        .map(|t| t.as_str().to_string());
    let security_level = raw
        .security_level
        .as_deref()
        .and_then(|l| l.trim().parse::<SecurityLevel>().ok())
        .map(|l| l.as_str().to_string());

    let trigger = match raw.trigger.map(|t| t.trim().to_string()) {
        Some(t)
            if !t.is_empty()
                && t.chars().count() <= TRIGGER_MAX_CHARS
                && !t.chars().any(|c| c.is_whitespace() || c.is_control())
                // A taken trigger is dropped, not surfaced: the suggestion
                // must be confirmable as-is.
                && SnippetRepo::new(conn)
                    .find_trigger_conflict(&t, None)?
                    .is_none() =>
        {
            Some(t)
        }
        _ => None,
    };

    // Tags and folders resolve against the existing vocabulary only
    // (case-insensitive on the suggested side); unknown names are dropped.
    let existing_tags = TagRepo::new(conn).list_all()?;
    let mut tags: Vec<TagDto> = Vec::new();
    for suggested in raw.tags {
        let wanted = suggested.trim().to_lowercase();
        if let Some(tag) = existing_tags
            .iter()
            .find(|t| t.name.to_lowercase() == wanted)
            && !tags.iter().any(|t| t.id == tag.id)
        {
            tags.push(tag.clone().into());
            if tags.len() == TAGS_MAX {
                break;
            }
        }
    }
    let folder = match raw.folder.map(|f| f.trim().to_lowercase()) {
        Some(wanted) if !wanted.is_empty() => FolderRepo::new(conn)
            .list_all_parents_first()?
            .into_iter()
            .find(|f| f.name.to_lowercase() == wanted)
            .map(FolderDto::from),
        _ => None,
    };

    Ok(OrganizeSuggestionDto {
        title,
        description,
        snippet_type,
        security_level,
        trigger,
        tags,
        folder,
    })
}

/// Extracts the JSON object from an answer, tolerating models that wrap
/// it in prose or code fences: parse directly, else parse the outermost
/// `{…}` span.
fn parse_answer<T: DeserializeOwned>(answer: &str) -> Result<T, IpcError> {
    let unusable =
        || IpcError::conflict("the AI answer could not be used — try again or rephrase the draft");
    if let Ok(raw) = serde_json::from_str::<T>(answer.trim()) {
        return Ok(raw);
    }
    let start = answer.find('{').ok_or_else(unusable)?;
    let end = answer.rfind('}').ok_or_else(unusable)?;
    if end < start {
        return Err(unusable());
    }
    serde_json::from_str::<T>(&answer[start..=end]).map_err(|_| unusable())
}

/// The fixed text being turned into a template.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractDraftInput {
    pub body: String,
    /// True when the source is marked sensitive; such text never leaves
    /// the device.
    pub is_sensitive: bool,
}

/// One variable proposal: replace every occurrence of `original` in the
/// body with `{{name}}` and configure the resulting field. Applying is
/// per-proposal and happens in the builder's local state — nothing is
/// stored until the user saves through the existing template paths.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableProposalDto {
    pub name: String,
    /// Exact span copied from the body (its presence is validated here).
    pub original: String,
    pub field_type: String,
    pub default_value: Option<String>,
}

/// Field types the model may propose. Choice types need options and
/// `secret_ref` crosses the vault boundary — neither is proposable.
const PROPOSABLE_TYPES: [TemplateFieldType; 5] = [
    TemplateFieldType::SingleLineText,
    TemplateFieldType::MultiLineText,
    TemplateFieldType::Number,
    TemplateFieldType::Date,
    TemplateFieldType::Time,
];

/// Phase 1 of variable extraction: refuses text that must not egress and
/// builds the prompt pair. Needs no database access.
pub fn extract_prompt(input: &ExtractDraftInput) -> Result<OrganizePrompt, IpcError> {
    if input.is_sensitive {
        return Err(IpcError::conflict(
            "sensitive snippets never leave this device",
        ));
    }
    if input.body.trim().is_empty() {
        return Err(IpcError::validation("template text must not be blank"));
    }
    if !detect(&input.body).is_empty() {
        return Err(IpcError::conflict(
            "this text looks like it contains a secret, so it was not sent — \
             remove the secret, or save it as a sensitive snippet instead",
        ));
    }
    let type_names: Vec<&str> = PROPOSABLE_TYPES.iter().map(|t| t.as_str()).collect();
    let system = format!(
        "You turn fixed text into a reusable template. Find the short spans \
         that would change each time the text is used (names, modules, \
         versions, dates, amounts) and answer with a single JSON object \
         only (no prose, no code fences):\n\
         {{\"variables\":[{{\"name\":…,\"original\":…,\"fieldType\":…,\"defaultValue\":…}}]}}\n\
         Rules:\n\
         \"name\": a short identifier with no spaces or braces (Chinese \
         words are fine, e.g. 模块名称)\n\
         \"original\": the exact span copied verbatim from the text\n\
         \"fieldType\": one of: {}\n\
         \"defaultValue\": a sensible default, usually the original span\n\
         Propose only spans that actually appear in the text; leave the \
         list empty when nothing should vary.",
        type_names.join(", "),
    );
    let body: String = input.body.chars().take(BODY_PROMPT_CAP).collect();
    let user = format!("Text:\n{body}");
    Ok(OrganizePrompt { system, user })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawExtraction {
    #[serde(default)]
    variables: Vec<RawVariable>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawVariable {
    name: Option<String>,
    original: Option<String>,
    field_type: Option<String>,
    default_value: Option<String>,
}

/// Phase 3 of variable extraction: validates the model's proposals against
/// the current body. Every surviving proposal is directly applicable —
/// its span exists in the body, its name is legal (the template model's
/// own rules plus a parser round-trip) and collides with nothing. Invalid
/// proposals are dropped, never stored.
pub fn extract_validate(body: &str, answer: &str) -> Result<Vec<VariableProposalDto>, IpcError> {
    let raw: RawExtraction = parse_answer(answer)?;
    let existing = typvia_template::variables(body).unwrap_or_default();
    let mut proposals: Vec<VariableProposalDto> = Vec::new();
    for item in raw.variables {
        let Some(name) = item.name.map(|n| n.trim().to_string()) else {
            continue;
        };
        let Some(original) = item.original else {
            continue;
        };
        // Legality comes from the template model itself (no duplicated
        // rule set), plus a parser round-trip proving `{{name}}` reads
        // back as exactly this variable.
        let as_field = TemplateField {
            id: String::new(),
            snippet_id: String::new(),
            name: name.clone(),
            label: name.clone(),
            field_type: TemplateFieldType::SingleLineText,
            default_value: None,
            options: Vec::new(),
            validation: None,
            is_required: false,
            sort_order: 0,
            platform_overrides: None,
        };
        if as_field.validate().is_err() {
            continue;
        }
        let round_trip = typvia_template::variables(&format!("{{{{{name}}}}}"));
        if !matches!(&round_trip, Ok(vars) if vars.len() == 1 && vars[0] == name) {
            continue;
        }
        if existing.contains(&name) || proposals.iter().any(|p| p.name == name) {
            continue;
        }
        // The span must be applicable: present in the body, not itself a
        // placeholder, and of a sane size.
        if original.trim().is_empty()
            || original.chars().count() > 200
            || original.contains("{{")
            || !body.contains(&original)
            || proposals.iter().any(|p| p.original == original)
        {
            continue;
        }
        let field_type = item
            .field_type
            .as_deref()
            .and_then(|t| t.trim().parse::<TemplateFieldType>().ok())
            .filter(|t| PROPOSABLE_TYPES.contains(t))
            .unwrap_or(TemplateFieldType::SingleLineText);
        let default_value = item
            .default_value
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty() && d.chars().count() <= 200);
        proposals.push(VariableProposalDto {
            name,
            original,
            field_type: field_type.as_str().to_string(),
            default_value,
        });
    }
    Ok(proposals)
}

/// Bound on action list reads; a library will never hold this many actions.
const ACTION_LIST_CAP: u32 = 500;
/// Hard input bound for action runs. Unlike the organize body cap this is
/// an error, not a silent truncation — cutting the text an action operates
/// on would silently change the result.
const ACTION_INPUT_MAX_CHARS: usize = 20_000;

/// One AI action as the Actions page shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiActionDto {
    pub id: String,
    pub name: String,
    pub prompt_template: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub input_source: String,
    pub output_mode: String,
    pub permission_scope: String,
    pub temperature: Option<f32>,
    /// Seeded built-in (fixed `builtin.` id); editable like any action,
    /// the flag only labels it in the UI.
    pub is_builtin: bool,
}

fn action_dto(action: AiAction) -> AiActionDto {
    AiActionDto {
        is_builtin: action.id.starts_with("builtin."),
        id: action.id,
        name: action.name,
        prompt_template: action.prompt_template,
        provider_id: action.provider_id,
        model: action.model,
        input_source: action.input_source.as_str().to_string(),
        output_mode: action.output_mode.as_str().to_string(),
        permission_scope: action.permission_scope.as_str().to_string(),
        temperature: action.params.temperature,
    }
}

/// All actions, stable name order. Lazily seeds the eight built-ins on
/// first use (one-shot per device; a deleted built-in stays deleted).
pub fn ai_action_list(conn: &mut Connection, now: i64) -> Result<Vec<AiActionDto>, IpcError> {
    ensure_builtin_actions(conn, now)?;
    let actions = AiActionRepo::new(conn).list(ACTION_LIST_CAP, 0)?;
    Ok(actions.into_iter().map(action_dto).collect())
}

/// Create/update payload for one action definition.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiActionSaveInput {
    pub id: Option<String>,
    pub name: String,
    pub prompt_template: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub input_source: String,
    pub output_mode: String,
    pub permission_scope: String,
    pub temperature: Option<f32>,
}

fn parse_action_enums(
    input: &AiActionSaveInput,
) -> Result<
    (
        AiActionInputSource,
        AiActionOutputMode,
        AiActionPermissionScope,
    ),
    IpcError,
> {
    let source = input
        .input_source
        .parse()
        .map_err(|_| IpcError::validation("unknown input source"))?;
    let mode = input
        .output_mode
        .parse()
        .map_err(|_| IpcError::validation("unknown output mode"))?;
    let scope = input
        .permission_scope
        .parse()
        .map_err(|_| IpcError::validation("unknown permission scope"))?;
    Ok((source, mode, scope))
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Creates (no id) or rewrites (id present) one action definition. Updates
/// merge `temperature` into the stored params so parameter keys this build
/// does not know survive. A dangling `provider_id` is legal by design —
/// execution reports it.
pub fn ai_action_save(
    conn: &Connection,
    input: AiActionSaveInput,
    now: i64,
) -> Result<AiActionDto, IpcError> {
    let (input_source, output_mode, permission_scope) = parse_action_enums(&input)?;
    let repo = AiActionRepo::new(conn);
    let (id, mut params, created_at) = match &input.id {
        Some(id) => {
            let existing = repo
                .get(id)?
                .ok_or_else(|| IpcError::conflict("this action no longer exists"))?;
            (id.clone(), existing.params, existing.created_at)
        }
        None => (new_id(), AiActionParams::default(), now),
    };
    params.temperature = input.temperature;
    let action = AiAction {
        id,
        name: input.name.trim().to_string(),
        prompt_template: input.prompt_template.trim().to_string(),
        provider_id: normalized_optional(input.provider_id),
        model: normalized_optional(input.model),
        input_source,
        output_mode,
        permission_scope,
        params,
        created_at,
        updated_at: now,
    };
    if input.id.is_some() {
        repo.update(&action)?;
    } else {
        repo.insert(&action)?;
    }
    Ok(action_dto(action))
}

/// Deletes one action (built-ins included; the seed never resurrects them).
pub fn ai_action_delete(conn: &Connection, id: &str) -> Result<(), IpcError> {
    AiActionRepo::new(conn).delete(id)?;
    Ok(())
}

/// The input an action run operates on, captured by the host from the
/// action's configured source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionRunInput {
    pub text: String,
    /// Where the host took the text from; must match the action's
    /// configured `input_source` (the action is a contract, not a hint).
    pub source: String,
    /// True when the text came from a sensitive snippet. Sensitive content
    /// never leaves the device, under either permission scope (red line).
    pub is_sensitive: bool,
}

/// Everything the host needs for the provider round trip of one action
/// run. Carries user content — no `Serialize`, so it cannot cross the IPC
/// boundary, and `Debug` renders lengths only (log red line).
pub struct ActionRunPrompt {
    pub provider_id: String,
    /// Action-level model override; `None` falls back to the provider's
    /// default model.
    pub model: Option<String>,
    pub system: String,
    pub user: String,
    pub temperature: Option<f32>,
    /// Detector kinds masked out of the input (`mask_secrets` scope only);
    /// shown to the user as "secrets stripped before sending".
    pub masked_kinds: Vec<String>,
}

impl std::fmt::Debug for ActionRunPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionRunPrompt")
            .field("provider_id", &self.provider_id)
            .field("system_chars", &self.system.chars().count())
            .field("user_chars", &self.user.chars().count())
            .field("masked_kinds", &self.masked_kinds)
            .finish()
    }
}

/// Phase 1 of an action run: loads the action, enforces its
/// permission scope on the input, and builds the provider call. Read-only —
/// the result the host gets back later stays pending until the user
/// confirms it (the confirm step is universal, whatever the output mode).
///
/// Scope rules (security red lines):
/// - sensitive input is refused under BOTH scopes;
/// - `normal_only` refuses any input with a suspected secret;
/// - `mask_secrets` strips suspected secrets and re-scans; if the mask
///   does not converge to zero hits the run is refused (fail-closed).
///
/// The authoritative egress gate in crates/ai still scans the
/// final prompt — this check is the use-case layer of the same defense.
pub fn ai_action_run_prompt(
    conn: &Connection,
    action_id: &str,
    input: &ActionRunInput,
) -> Result<ActionRunPrompt, IpcError> {
    let action = AiActionRepo::new(conn)
        .get(action_id)?
        .ok_or_else(|| IpcError::conflict("this action no longer exists"))?;

    let source: AiActionInputSource = input
        .source
        .parse()
        .map_err(|_| IpcError::validation("unknown input source"))?;
    if source != action.input_source {
        return Err(IpcError::validation(
            "the input does not come from this action's configured source",
        ));
    }
    if input.text.trim().is_empty() {
        return Err(IpcError::validation("there is no input text to run on"));
    }
    if input.text.chars().count() > ACTION_INPUT_MAX_CHARS {
        return Err(IpcError::validation(
            "the input is too long for an AI action — select a smaller part",
        ));
    }
    if input.is_sensitive {
        return Err(IpcError::conflict(
            "sensitive snippets never leave this device",
        ));
    }

    let (text, masked_kinds) = match action.permission_scope {
        AiActionPermissionScope::NormalOnly => {
            if !detect(&input.text).is_empty() {
                return Err(IpcError::conflict(
                    "this input looks like it contains a secret, so nothing was \
                     sent — this action refuses to run on secrets",
                ));
            }
            (input.text.clone(), Vec::new())
        }
        AiActionPermissionScope::MaskSecrets => {
            let outcome = mask_suspected_secrets(&input.text);
            // Fail-closed: masking must converge to a clean re-scan.
            if !detect(&outcome.masked).is_empty() {
                return Err(IpcError::conflict(
                    "the secrets in this input could not be stripped safely, \
                     so nothing was sent",
                ));
            }
            let kinds = outcome
                .kinds
                .iter()
                .map(|k| k.as_str().to_string())
                .collect();
            (outcome.masked, kinds)
        }
    };

    let Some(provider_id) = action.provider_id else {
        return Err(IpcError::conflict(
            "this action has no provider configured — pick one in its settings",
        ));
    };

    Ok(ActionRunPrompt {
        provider_id,
        model: action.model,
        system: action.prompt_template,
        user: text,
        temperature: action.params.temperature,
        masked_kinds,
    })
}

/// A finished action run, pending the user's confirmation. Applying the
/// output (replace / insert / copy / save as snippet) is the UI's job and
/// happens only after the user confirms — nothing here writes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResultDto {
    pub output: String,
}

/// Phase 3 of an action run: validates the model's answer (untrusted
/// external input). Action answers are plain text; an effectively empty
/// answer is a retryable business error.
pub fn ai_action_run_validate(answer: &str) -> Result<ActionResultDto, IpcError> {
    let output = answer.trim();
    if output.is_empty() {
        return Err(IpcError::conflict(
            "the AI returned an empty answer — try again",
        ));
    }
    Ok(ActionResultDto {
        output: output.to_string(),
    })
}

/// One egress-log entry as the Privacy screen shows it: when, to which
/// provider, what class of request, how many body bytes. The table this
/// reads from structurally cannot hold prompt content, response content or
/// key material, so the DTO cannot either.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEgressEntryDto {
    pub id: i64,
    pub occurred_at: i64,
    pub provider_id: String,
    pub request_class: String,
    pub request_bytes: i64,
}

/// The paged egress receipt, newest first, plus the all-time total so the
/// viewer can page and state a true count.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEgressPageDto {
    pub entries: Vec<AiEgressEntryDto>,
    pub total: i64,
}

/// Reads the un-disableable egress log. Read-only: the
/// log has no delete or edit path anywhere in the product.
pub fn ai_egress_log_list(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> Result<AiEgressPageDto, IpcError> {
    let repo = AiEgressLogRepo::new(conn);
    let entries = repo
        .list(limit, offset)?
        .into_iter()
        .map(|record| AiEgressEntryDto {
            id: record.id,
            occurred_at: record.occurred_at,
            provider_id: record.provider_id,
            request_class: record.request_class.as_str().to_string(),
            request_bytes: record.request_bytes,
        })
        .collect();
    let total = repo.count()?;
    Ok(AiEgressPageDto { entries, total })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_core::db::{migrate_to_latest, open_in_memory};

    use super::*;
    use crate::dto::{FolderCreateInput, SnippetCreateInput};
    use crate::error::IpcErrorCode;
    use crate::service::{folder_create, snippet_create, tag_create};

    fn create_folder(conn: &Connection, name: &str) -> crate::dto::FolderDto {
        folder_create(
            conn,
            FolderCreateInput {
                name: name.to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap()
    }

    fn setup() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn draft(body: &str) -> OrganizeDraftInput {
        OrganizeDraftInput {
            title: "untitled".to_string(),
            body: body.to_string(),
            description: None,
            is_sensitive: false,
        }
    }

    #[test]
    fn providers_round_trip_through_save_list_delete() {
        let conn = setup();
        let created = ai_provider_save(
            &conn,
            AiProviderSaveInput {
                id: None,
                name: "Local Ollama".to_string(),
                kind: "ollama".to_string(),
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                model: "llama3".to_string(),
                timeout_ms: 30_000,
            },
            1,
        )
        .unwrap();
        assert!(!created.has_api_key);

        let updated = ai_provider_save(
            &conn,
            AiProviderSaveInput {
                id: Some(created.id.clone()),
                name: "Ollama on this Mac".to_string(),
                kind: "ollama".to_string(),
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                model: "llama3.1".to_string(),
                timeout_ms: 45_000,
            },
            2,
        )
        .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.model, "llama3.1");

        let listed = ai_provider_list(&conn, |id| id == created.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Ollama on this Mac");
        assert!(listed[0].has_api_key);

        ai_provider_delete(&conn, &created.id).unwrap();
        assert!(ai_provider_list(&conn, |_| false).unwrap().is_empty());
        assert_eq!(
            ai_provider_delete(&conn, &created.id).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn an_unknown_kind_or_ghost_update_is_rejected() {
        let conn = setup();
        let err = ai_provider_save(
            &conn,
            AiProviderSaveInput {
                id: None,
                name: "X".to_string(),
                kind: "quantum".to_string(),
                base_url: "https://x/v1".to_string(),
                model: "m".to_string(),
                timeout_ms: 1_000,
            },
            1,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        let err = ai_provider_save(
            &conn,
            AiProviderSaveInput {
                id: Some("ghost".to_string()),
                name: "X".to_string(),
                kind: "ollama".to_string(),
                base_url: "https://x/v1".to_string(),
                model: "m".to_string(),
                timeout_ms: 1_000,
            },
            1,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::NotFound);
    }

    #[test]
    fn a_sensitive_draft_is_refused_before_anything_else() {
        let conn = setup();
        let mut input = draft("kubectl get pods");
        input.is_sensitive = true;
        let err = organize_prompt(&conn, &input).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(err.message.contains("never leave this device"));
    }

    #[test]
    fn a_secret_bearing_draft_is_refused_with_an_actionable_message() {
        let conn = setup();
        let err = organize_prompt(&conn, &draft("aws key AKIAFAKEFAKEFAKEFAKE here")).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(err.message.contains("save it as a sensitive snippet"));
        // The secret itself never rides the error (log red line).
        assert!(!err.message.contains("AKIAFAKEFAKEFAKEFAKE"));
    }

    #[test]
    fn a_blank_draft_is_a_validation_error() {
        let conn = setup();
        let err = organize_prompt(&conn, &draft("   ")).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    #[test]
    fn the_prompt_carries_the_draft_and_the_library_vocabulary() {
        let conn = setup();
        tag_create(&conn, "docker".to_string(), 1).unwrap();
        create_folder(&conn, "Work");
        let mut input = draft("docker compose up -d");
        input.title = "compose up".to_string();
        let prompt = organize_prompt(&conn, &input).unwrap();
        assert!(prompt.system.contains("docker"));
        assert!(prompt.system.contains("Work"));
        assert!(prompt.system.contains("command"));
        assert!(prompt.user.contains("docker compose up -d"));
        assert!(prompt.user.contains("compose up"));
    }

    #[test]
    fn an_overlong_body_is_cut_before_it_reaches_the_prompt() {
        let conn = setup();
        let long = "x".repeat(BODY_PROMPT_CAP + 500);
        let prompt = organize_prompt(&conn, &draft(&long)).unwrap();
        assert!(prompt.user.chars().count() < BODY_PROMPT_CAP + 100);
    }

    #[test]
    fn a_full_valid_answer_maps_onto_the_library() {
        let conn = setup();
        let tag = tag_create(&conn, "docker".to_string(), 1).unwrap();
        tag_create(&conn, "infra".to_string(), 1).unwrap();
        let folder = create_folder(&conn, "Work");
        let suggestion = organize_validate(
            &conn,
            r#"{"title":"Compose up","description":"Starts the stack.",
                "snippetType":"command","securityLevel":"normal",
                "trigger":";dcu","tags":["Docker","kubernetes"],"folder":"work"}"#,
        )
        .unwrap();
        assert_eq!(suggestion.title.as_deref(), Some("Compose up"));
        assert_eq!(suggestion.snippet_type.as_deref(), Some("command"));
        assert_eq!(suggestion.security_level.as_deref(), Some("normal"));
        assert_eq!(suggestion.trigger.as_deref(), Some(";dcu"));
        // "Docker" matched the existing tag case-insensitively;
        // "kubernetes" does not exist and was dropped.
        assert_eq!(suggestion.tags.len(), 1);
        assert_eq!(suggestion.tags[0].id, tag.id);
        assert_eq!(suggestion.folder.as_ref().unwrap().id, folder.id);
    }

    #[test]
    fn invalid_pieces_are_dropped_not_stored() {
        let conn = setup();
        snippet_create(
            &conn,
            SnippetCreateInput {
                title: "Existing".to_string(),
                body: "b".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";sig".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
            },
            1,
        )
        .unwrap();
        let suggestion = organize_validate(
            &conn,
            // Taken trigger, unknown type, unknown level, whitespace
            // trigger fallback, unknown tag/folder.
            r#"{"snippetType":"sensitive","securityLevel":"vault",
                "trigger":";sig","tags":["ghost"],"folder":"Nowhere"}"#,
        )
        .unwrap();
        assert_eq!(suggestion.snippet_type, None);
        assert_eq!(suggestion.security_level, None);
        assert_eq!(suggestion.trigger, None);
        assert!(suggestion.tags.is_empty());
        assert!(suggestion.folder.is_none());
    }

    #[test]
    fn a_fenced_answer_still_parses_and_prose_only_fails_as_business_error() {
        let conn = setup();
        let fenced = "Here you go:\n```json\n{\"title\":\"From fence\"}\n```";
        let suggestion = organize_validate(&conn, fenced).unwrap();
        assert_eq!(suggestion.title.as_deref(), Some("From fence"));

        let err = organize_validate(&conn, "I could not decide on anything.").unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
    }

    #[test]
    fn extraction_refuses_sensitive_or_secret_bearing_text() {
        let sensitive = ExtractDraftInput {
            body: "review the login module".to_string(),
            is_sensitive: true,
        };
        assert_eq!(
            extract_prompt(&sensitive).unwrap_err().code,
            IpcErrorCode::Conflict
        );
        let secret = ExtractDraftInput {
            body: "deploy with AKIAFAKEFAKEFAKEFAKE".to_string(),
            is_sensitive: false,
        };
        let err = extract_prompt(&secret).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(!err.message.contains("AKIAFAKEFAKEFAKEFAKE"));
        let blank = ExtractDraftInput {
            body: "  ".to_string(),
            is_sensitive: false,
        };
        assert_eq!(
            extract_prompt(&blank).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn valid_proposals_map_spans_that_exist_in_the_body() {
        let body = "请检查 React 项目的登录模块,重点关注安全性。";
        let prompt = extract_prompt(&ExtractDraftInput {
            body: body.to_string(),
            is_sensitive: false,
        })
        .unwrap();
        assert!(prompt.user.contains("登录模块"));

        let proposals = extract_validate(
            body,
            r#"{"variables":[
                {"name":"技术栈","original":"React","fieldType":"single_line_text","defaultValue":"React"},
                {"name":"模块名称","original":"登录模块","fieldType":"date"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(proposals.len(), 2);
        assert_eq!(proposals[0].name, "技术栈");
        assert_eq!(proposals[0].original, "React");
        assert_eq!(proposals[0].default_value.as_deref(), Some("React"));
        // Every proposal's span really is in the body (one-to-one
        // applicability), and the declared type survived validation.
        for p in &proposals {
            assert!(body.contains(&p.original));
        }
        assert_eq!(proposals[1].field_type, "date");
    }

    #[test]
    fn illegal_names_missing_spans_and_collisions_are_dropped() {
        let body = "check the {{module}} of Api in Api";
        let proposals = extract_validate(
            body,
            r#"{"variables":[
                {"name":"bad name","original":"Api"},
                {"name":"has{brace","original":"Api"},
                {"name":"ghost","original":"not-in-body"},
                {"name":"module","original":"Api"},
                {"name":"service","original":"Api","fieldType":"secret_ref"},
                {"name":"dup","original":"Api"}
            ]}"#,
        )
        .unwrap();
        // "bad name" (whitespace), "has{brace", "ghost" (span absent) and
        // "module" (collides with an existing placeholder) are dropped;
        // "service" survives but its disallowed type falls back; "dup"
        // duplicates the same span and is dropped.
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].name, "service");
        assert_eq!(proposals[0].field_type, "single_line_text");

        assert_eq!(
            extract_validate(body, "no json at all").unwrap_err().code,
            IpcErrorCode::Conflict
        );
        assert!(
            extract_validate(body, r#"{"variables":[]}"#)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn organizing_never_writes_anything() {
        let conn = setup();
        tag_create(&conn, "docker".to_string(), 1).unwrap();
        let before: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM snippet) + (SELECT COUNT(*) FROM tag) \
                 + (SELECT COUNT(*) FROM folder)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let _ = organize_prompt(&conn, &draft("docker ps"));
        let _ = organize_validate(&conn, r#"{"title":"x","tags":["docker"]}"#);
        let _ = organize_validate(&conn, "garbage");
        let after: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM snippet) + (SELECT COUNT(*) FROM tag) \
                 + (SELECT COUNT(*) FROM folder)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(before, after);
    }

    fn saved_action(conn: &Connection, scope: &str, provider: Option<&str>) -> AiActionDto {
        ai_action_save(
            conn,
            AiActionSaveInput {
                id: None,
                name: "Rewrite".to_string(),
                prompt_template: "Rewrite the text clearly.".to_string(),
                provider_id: provider.map(str::to_string),
                model: None,
                input_source: "selection".to_string(),
                output_mode: "replace".to_string(),
                permission_scope: scope.to_string(),
                temperature: Some(0.2),
            },
            1,
        )
        .unwrap()
    }

    fn run_input(text: &str) -> ActionRunInput {
        ActionRunInput {
            text: text.to_string(),
            source: "selection".to_string(),
            is_sensitive: false,
        }
    }

    #[test]
    fn listing_seeds_the_builtins_once_and_deletions_stick() {
        let mut conn = setup();
        let actions = ai_action_list(&mut conn, 1).unwrap();
        assert_eq!(actions.len(), 8);
        assert!(actions.iter().all(|a| a.is_builtin));
        assert!(actions.iter().all(|a| a.provider_id.is_none()));
        assert!(actions.iter().all(|a| a.permission_scope == "normal_only"));

        ai_action_delete(&conn, "builtin.translate").unwrap();
        let after = ai_action_list(&mut conn, 2).unwrap();
        assert_eq!(after.len(), 7);
        assert!(!after.iter().any(|a| a.id == "builtin.translate"));
    }

    #[test]
    fn action_save_round_trips_and_updates_preserve_unknown_params() {
        let conn = setup();
        let created = saved_action(&conn, "normal_only", Some("p1"));
        assert!(!created.is_builtin);
        assert_eq!(created.temperature, Some(0.2));

        // Simulate a newer build having stored a parameter key this build
        // does not model; an update through save must not drop it.
        conn.execute(
            "UPDATE ai_action SET params = '{\"temperature\":0.2,\"top_p\":0.9}' \
             WHERE id = ?1",
            [&created.id],
        )
        .unwrap();
        let updated = ai_action_save(
            &conn,
            AiActionSaveInput {
                id: Some(created.id.clone()),
                name: "Rewrite politely".to_string(),
                prompt_template: "Rewrite the text politely.".to_string(),
                provider_id: Some("p1".to_string()),
                model: Some("llama3".to_string()),
                input_source: "selection".to_string(),
                output_mode: "replace".to_string(),
                permission_scope: "normal_only".to_string(),
                temperature: Some(0.5),
            },
            2,
        )
        .unwrap();
        assert_eq!(updated.temperature, Some(0.5));
        let stored: String = conn
            .query_row(
                "SELECT params FROM ai_action WHERE id = ?1",
                [&created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(stored.contains("top_p"));
        assert!(stored.contains("0.5"));
    }

    #[test]
    fn action_save_rejects_unknown_enum_values() {
        let conn = setup();
        let mut input = AiActionSaveInput {
            id: None,
            name: "X".to_string(),
            prompt_template: "Do X.".to_string(),
            provider_id: None,
            model: None,
            input_source: "telepathy".to_string(),
            output_mode: "replace".to_string(),
            permission_scope: "normal_only".to_string(),
            temperature: None,
        };
        assert_eq!(
            ai_action_save(&conn, input.clone(), 1).unwrap_err().code,
            IpcErrorCode::Validation
        );
        input.input_source = "selection".to_string();
        input.permission_scope = "share_everything".to_string();
        assert_eq!(
            ai_action_save(&conn, input, 1).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn run_refuses_sensitive_input_under_both_scopes() {
        let conn = setup();
        for scope in ["normal_only", "mask_secrets"] {
            let action = saved_action(&conn, scope, Some("p1"));
            let mut input = run_input("my vault note");
            input.is_sensitive = true;
            let err = ai_action_run_prompt(&conn, &action.id, &input).unwrap_err();
            assert_eq!(err.code, IpcErrorCode::Conflict);
            assert!(err.message.contains("never leave"));
        }
    }

    #[test]
    fn run_normal_only_refuses_suspected_secrets_without_leaking_them() {
        let conn = setup();
        let action = saved_action(&conn, "normal_only", Some("p1"));
        let text = "deploy with AKIAFAKEFAKEFAKEFAKE now";
        // Canary: the detector really sees this input.
        assert!(!detect(text).is_empty());
        let err = ai_action_run_prompt(&conn, &action.id, &run_input(text)).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(!err.message.contains("AKIAFAKEFAKEFAKEFAKE"));
    }

    #[test]
    fn run_mask_secrets_strips_the_secret_and_reports_the_kind() {
        let conn = setup();
        let action = saved_action(&conn, "mask_secrets", Some("p1"));
        let text = "deploy key AKIAFAKEFAKEFAKEFAKE to the demo box";
        assert!(!detect(text).is_empty());
        let prompt = ai_action_run_prompt(&conn, &action.id, &run_input(text)).unwrap();
        assert!(!prompt.user.contains("AKIAFAKEFAKEFAKEFAKE"));
        assert!(detect(&prompt.user).is_empty());
        assert!(prompt.user.contains("demo box"));
        assert_eq!(prompt.masked_kinds, vec!["aws_access_key".to_string()]);
        assert_eq!(prompt.system, "Rewrite the text clearly.");
        assert_eq!(prompt.provider_id, "p1");
        assert_eq!(prompt.temperature, Some(0.2));
    }

    #[test]
    fn run_requires_a_configured_provider() {
        let conn = setup();
        let action = saved_action(&conn, "normal_only", None);
        let err = ai_action_run_prompt(&conn, &action.id, &run_input("hello there")).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        assert!(err.message.contains("no provider configured"));
    }

    #[test]
    fn run_rejects_source_mismatch_blank_and_oversized_input() {
        let conn = setup();
        let action = saved_action(&conn, "normal_only", Some("p1"));
        let mut mismatched = run_input("hello");
        mismatched.source = "clipboard".to_string();
        assert_eq!(
            ai_action_run_prompt(&conn, &action.id, &mismatched)
                .unwrap_err()
                .code,
            IpcErrorCode::Validation
        );
        assert_eq!(
            ai_action_run_prompt(&conn, &action.id, &run_input("   "))
                .unwrap_err()
                .code,
            IpcErrorCode::Validation
        );
        let oversized = "a".repeat(ACTION_INPUT_MAX_CHARS + 1);
        assert_eq!(
            ai_action_run_prompt(&conn, &action.id, &run_input(&oversized))
                .unwrap_err()
                .code,
            IpcErrorCode::Validation
        );
        assert_eq!(
            ai_action_run_prompt(&conn, "ghost", &run_input("hello"))
                .unwrap_err()
                .code,
            IpcErrorCode::Conflict
        );
    }

    #[test]
    fn run_validate_trims_and_rejects_empty_answers() {
        assert_eq!(
            ai_action_run_validate("  polished text  ").unwrap().output,
            "polished text"
        );
        assert_eq!(
            ai_action_run_validate("   ").unwrap_err().code,
            IpcErrorCode::Conflict
        );
    }

    #[test]
    fn egress_log_lists_newest_first_with_a_true_total() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let repo = AiEgressLogRepo::new(&conn);
        repo.append(
            10,
            "p1",
            typvia_core::model::AiRequestClass::Connectivity,
            0,
        )
        .unwrap();
        repo.append(
            20,
            "p1",
            typvia_core::model::AiRequestClass::Completion,
            512,
        )
        .unwrap();
        repo.append(30, "p2", typvia_core::model::AiRequestClass::Completion, 64)
            .unwrap();

        let page = ai_egress_log_list(&conn, 2, 0).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].provider_id, "p2");
        assert_eq!(page.entries[0].request_class, "completion");
        assert_eq!(page.entries[0].request_bytes, 64);
        assert_eq!(page.entries[1].occurred_at, 20);

        let rest = ai_egress_log_list(&conn, 2, 2).unwrap();
        assert_eq!(rest.entries.len(), 1);
        assert_eq!(rest.entries[0].request_class, "connectivity");
    }
}
