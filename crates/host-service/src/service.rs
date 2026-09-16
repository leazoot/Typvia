// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Host-side use-case composition: repository writes paired with search
//! index maintenance. This lives in the host (not core) because the crate
//! dependency direction is search → core — only the application can see
//! both sides. No SQL here; repositories stay the only SQL location.

use std::path::Path;
use std::str::FromStr;

use rusqlite::Connection;
use typvia_core::app_rules::{self, AppRuleDecision};
use typvia_core::import as generic_import;
use typvia_core::model::{
    AppRule, AppRuleType, Folder, Platform, SecurityLevel, Snippet, SnippetContent, SnippetType,
    SnippetVersion, SyncEntityType, Tag, TemplateField, TemplateFieldType, TriggerMode,
};
use typvia_core::repo::{
    AppRuleRepo, FolderRepo, ListOrder, ListScope, RepoError, SnippetRepo, TRASH_RETENTION_MS,
    TagRepo, TemplateFieldRepo, VaultKeyRepo, VersionRepo, new_id,
};
use typvia_core::sync_hooks::{self, EntityChange};
use typvia_core::vault::{SecureStore, UnlockStatus, VaultSession};
use typvia_search::{SearchIndex, Searcher};

use crate::dto::{
    AppRuleCreateInput, AppRuleDto, AppRuleUpdateInput, BackupRestoreDto, ConflictKeep,
    ConflictPairDto, FolderCountDto, FolderCreateInput, FolderDto, FolderUpdateInput, HistoryDto,
    ImportReportDto, ImportSkippedDto, LibraryCountsDto, PanelResultsDto, SearchHitDto,
    SnippetCreateInput, SnippetDto, SnippetUpdateInput, TagDto, TemplateFieldDto, VaultStatusDto,
    VersionBodyDto, VersionMetaDto,
};
use crate::error::IpcError;

/// v1.0 has a single implicit workspace.
const WORKSPACE_ID: &str = "default";

/// Per-install device identity marker kept in the app data dir; the sync
/// device row and the keyboard snapshot share this one id.
const DEVICE_ID_FILE_NAME: &str = "device-id";

/// List queries stay bounded: reject silly page sizes
/// instead of silently clamping them.
const MAX_PAGE_LIMIT: u32 = 500;

fn check_limit(limit: u32) -> Result<(), IpcError> {
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        return Err(IpcError::validation("limit must be between 1 and 500"));
    }
    Ok(())
}

/// Announces a synced-entity change through the core sync_hooks seam,
/// inside the caller's write transaction so sealing joins the write
/// atomically. Without a registered observer
/// (sync disabled) this is a no-op; a hook failure aborts the write —
/// a change that cannot be queued must not silently diverge.
fn notify_sync(
    conn: &Connection,
    entity_type: SyncEntityType,
    entity_id: &str,
    deleted_at: Option<i64>,
    now: i64,
) -> Result<(), IpcError> {
    sync_hooks::notify_change(
        conn,
        &EntityChange {
            entity_type,
            entity_id: entity_id.to_string(),
            deleted_at,
            now,
        },
    )
    .map_err(|_| IpcError::system())
}

fn parse_snippet_type(value: &str) -> Result<SnippetType, IpcError> {
    SnippetType::from_str(value).map_err(|_| IpcError::validation("unknown snippet type"))
}

/// A trigger word as it will be stored, and the mode that must travel with it.
///
/// An empty field is not an empty trigger — it is no trigger, and the mode has
/// nothing left to describe. Hosts were normalising this on their own side
/// (iOS did; Android was about to have to), which is how two platforms come to
/// disagree about a field the reader left alone.
///
/// Trimming stops there: which characters a trigger may contain is the model's
/// rule, and a caller that sends `;de ploy` is told so rather than quietly
/// getting something else stored.
fn snippet_trigger(
    trigger: Option<String>,
    mode: Option<String>,
) -> (Option<String>, Option<String>) {
    match trigger
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        // A trigger with no stated mode is a delimiter trigger. That default
        // was written down five times across the two hosts and the share
        // intake; it is a product decision, and it belongs where the decision
        // can only be made once.
        Some(trigger) => (
            Some(trigger),
            Some(mode.unwrap_or_else(|| DEFAULT_TRIGGER_MODE.to_string())),
        ),
        None => (None, None),
    }
}

/// What a trigger means when nobody said: type it, then a delimiter.
const DEFAULT_TRIGGER_MODE: &str = "delimiter";

/// The name a snippet is filed under.
///
/// A snippet must have a title — that is the data model's rule and it stays.
/// What this adds is the product's answer to a reader who did not want to name
/// anything: the first line of what they wrote. The alternative is a library
/// full of rows called "Untitled", which is a library nobody can scan.
///
/// It lives here rather than in a screen because both hosts were about to have
/// their own copy of it, and a convenience that differs per platform is not a
/// convenience — it is two products.
fn snippet_title(given: &str, body: &str) -> String {
    let given = given.trim();
    if !given.is_empty() {
        return given.to_string();
    }
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    first.chars().take(TITLE_FROM_BODY_MAX_CHARS).collect()
}

/// How much of the first line becomes a name. Long enough to recognise, short
/// enough to scan a column of them.
const TITLE_FROM_BODY_MAX_CHARS: usize = 60;

/// The kind an ordinary save may file a snippet under.
///
/// Every kind but one. A secret's body is encrypted before it reaches storage
/// and its row carries no plaintext beside it; letting this door accept the
/// secret kind would produce the exact opposite — a row that calls itself a
/// secret while holding the body in the clear, in the full-text index, and in
/// every snapshot the extensions can open. The vault has its own door for
/// that, and an existing snippet is promoted through `convert_to_sensitive`.
///
/// The refusal is here rather than in the hosts because otherwise every host
/// needs its own copy of it, and the one that forgets is the one that leaks.
fn parse_editable_snippet_type(value: &str) -> Result<SnippetType, IpcError> {
    match parse_snippet_type(value)? {
        SnippetType::Sensitive => Err(IpcError::validation(
            "a secret is written through the vault, not through an ordinary save",
        )),
        kind => Ok(kind),
    }
}

fn parse_trigger_mode(value: Option<&str>) -> Result<Option<TriggerMode>, IpcError> {
    value
        .map(|v| TriggerMode::from_str(v).map_err(|_| IpcError::validation("unknown trigger mode")))
        .transpose()
}

fn check_trigger_free(
    repo: &SnippetRepo<'_>,
    trigger: Option<&str>,
    exclude_id: Option<&str>,
) -> Result<(), IpcError> {
    if let Some(trigger) = trigger
        && repo.find_trigger_conflict(trigger, exclude_id)?.is_some()
    {
        return Err(IpcError::conflict("trigger already in use"));
    }
    Ok(())
}

pub fn snippet_create(
    conn: &Connection,
    input: SnippetCreateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let title = snippet_title(&input.title, &input.body);
    let (trigger, trigger_mode) = snippet_trigger(input.trigger, input.trigger_mode);
    let snippet = Snippet {
        id: new_id(),
        workspace_id: WORKSPACE_ID.to_string(),
        title,
        content: SnippetContent::Plaintext(input.body),
        snippet_type: parse_editable_snippet_type(&input.snippet_type)?,
        description: input.description,
        folder_id: input.folder_id,
        trigger,
        trigger_mode: parse_trigger_mode(trigger_mode.as_deref())?,
        language: input.language,
        security_level: SecurityLevel::Normal,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: Vec::new(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    };
    check_trigger_free(&SnippetRepo::new(conn), snippet.trigger.as_deref(), None)?;
    // Insert and its v1 history entry land atomically (fail-closed rule).
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).insert(&snippet)?;
    VersionRepo::new(&tx).append(&history_entry(&snippet, now))?;
    notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Snapshot of a snippet's current state as a history row.
fn history_entry(snippet: &Snippet, now: i64) -> SnippetVersion {
    SnippetVersion {
        id: new_id(),
        snippet_id: snippet.id.clone(),
        version: snippet.version,
        title: snippet.title.clone(),
        content: snippet.content.clone(),
        created_at: now,
    }
}

pub fn snippet_update(
    conn: &Connection,
    input: SnippetUpdateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    let mut snippet = repo.get(&input.id)?.ok_or_else(IpcError::not_found)?;
    if snippet.security_level == SecurityLevel::Sensitive {
        return Err(IpcError::conflict(
            "sensitive snippets are edited through the vault flow",
        ));
    }
    let content_before = (
        snippet.title.clone(),
        snippet.content.clone(),
        snippet.snippet_type,
    );
    snippet.title = input.title;
    snippet.content = SnippetContent::Plaintext(input.body);
    snippet.snippet_type = parse_editable_snippet_type(&input.snippet_type)?;
    snippet.description = input.description;
    snippet.folder_id = input.folder_id;
    let (trigger, trigger_mode) = snippet_trigger(input.trigger, input.trigger_mode);
    snippet.trigger = trigger;
    snippet.trigger_mode = parse_trigger_mode(trigger_mode.as_deref())?;
    snippet.language = input.language;
    snippet.is_favorite = input.is_favorite;
    snippet.is_pinned = input.is_pinned;
    snippet.is_enabled = input.is_enabled;
    snippet.updated_at = now;
    // Version history is append-on-write: a content change bumps the version
    // and records the new state. Metadata-only edits (folder, trigger,
    // toggles) do not create versions. Retention is applied in the same
    // transaction as the append.
    let content_changed = content_before
        != (
            snippet.title.clone(),
            snippet.content.clone(),
            snippet.snippet_type,
        );
    if content_changed {
        snippet.version += 1;
    }
    check_trigger_free(&repo, snippet.trigger.as_deref(), Some(&snippet.id))?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).update(&snippet)?;
    if content_changed {
        let versions = VersionRepo::new(&tx);
        versions.append(&history_entry(&snippet, now))?;
        versions.apply_retention(&snippet.id, now)?;
    }
    notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Gate for history access on a sensitive snippet: history is only visible
/// on an unlocked vault (a red line — the locked state shows no
/// version data at all, not even titles or timestamps).
fn check_history_access(
    snippet: &Snippet,
    session: &mut VaultSession,
    now: i64,
) -> Result<(), IpcError> {
    if snippet.security_level == SecurityLevel::Sensitive {
        session.enforce_idle_timeout(now, VAULT_IDLE_TIMEOUT_MS);
        if !matches!(session.status(), UnlockStatus::Unlocked { .. }) {
            return Err(IpcError::permission_denied("unlock the vault first"));
        }
        session.note_activity(now);
    }
    Ok(())
}

/// Lists a snippet's version history, newest first (metadata only — no
/// bodies, so listing a sensitive snippet's history decrypts nothing).
pub fn history_list(
    conn: &Connection,
    session: &mut VaultSession,
    id: &str,
    limit: u32,
    offset: u32,
    now: i64,
) -> Result<HistoryDto, IpcError> {
    check_limit(limit)?;
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    check_history_access(&snippet, session, now)?;
    let entries = VersionRepo::new(conn)
        .list(id, limit, offset)?
        .into_iter()
        .map(|v| VersionMetaDto {
            version: v.version,
            title: v.title,
            created_at: v.created_at,
        })
        .collect();
    Ok(HistoryDto {
        current: snippet.version,
        entries,
    })
}

/// Loads one history entry's full state for the diff view. Sensitive entries
/// are decrypted host-side and require an unlocked session.
pub fn history_get(
    conn: &Connection,
    session: &mut VaultSession,
    id: &str,
    version: u32,
    now: i64,
) -> Result<VersionBodyDto, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    check_history_access(&snippet, session, now)?;
    let entry = VersionRepo::new(conn)
        .get(id, version)?
        .ok_or_else(IpcError::not_found)?;
    let body = match entry.content {
        SnippetContent::Plaintext(text) => text,
        SnippetContent::Ciphertext(bytes) => {
            let plaintext = session.decrypt_content(conn, id, &bytes)?;
            String::from_utf8(plaintext.to_vec()).map_err(|_| IpcError::system())?
        }
    };
    Ok(VersionBodyDto {
        version: entry.version,
        title: entry.title,
        body,
        created_at: entry.created_at,
    })
}

/// Restores an old version by writing it forward — restoring v9 writes a
/// new v10 and v9 is never lost: the restored state becomes the snippet's
/// new highest version and existing history entries are untouched beyond
/// the retention pass.
pub fn history_restore(
    conn: &Connection,
    session: &mut VaultSession,
    id: &str,
    version: u32,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    check_history_access(&snippet, session, now)?;
    if version == snippet.version {
        return Err(IpcError::conflict("already the current version"));
    }
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    VersionRepo::new(&tx).restore_version(id, version, &new_id(), now)?;
    notify_sync(&tx, SyncEntityType::Snippet, id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    // The restore may change the title/body, so the index row is stale.
    SearchIndex::new(conn).sync_snippet(id)?;
    snippet_get(conn, id)
}

/// Ranked search that returns full row data for the Library list. Runs the
/// Searcher, then loads each hit's snippet in ranked order — one
/// lock, at most `limit` point lookups (candidate pool is already capped).
pub fn search_library(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    search_rows(conn, query, limit, false)
}

/// Ranked search for a calling surface's search screen (mobile Search):
/// sensitive title hits ARE included — as locked rows whose `body` is
/// already `None` on the DTO — mirroring the panel boundary, while the
/// Library search above keeps hiding them.
pub fn search_all(conn: &Connection, query: &str, limit: u32) -> Result<Vec<SnippetDto>, IpcError> {
    search_rows(conn, query, limit, true)
}

/// Shared ranked-search tail of the Library search and the panel search.
/// The index knows sensitive titles, but the Library never lists them — the
/// vault page is their only Library-side list. The
/// panel DOES surface them (`include_sensitive`): it owns the
/// verify-then-insert flow, and a title hit there is a locked row, never
/// content.
fn search_rows(
    conn: &Connection,
    query: &str,
    limit: u32,
    include_sensitive: bool,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let hits = Searcher::new(conn).search(query, limit, 0)?;
    let repo = SnippetRepo::new(conn);
    let mut rows = Vec::with_capacity(hits.len());
    for hit in hits {
        // A hit can race a deletion; skipping is correct, not an error.
        if let Some(snippet) = repo.get(&hit.snippet_id)? {
            if !include_sensitive && snippet.security_level == SecurityLevel::Sensitive {
                continue;
            }
            rows.push(snippet.into());
        }
    }
    Ok(rows)
}

/// Offline sensitive-content scan: advisory kinds only, never
/// matched text — safe to cross the IPC boundary and to show in the editor.
pub fn detect_sensitive(text: &str) -> Vec<String> {
    typvia_core::sensitive::detect(text)
        .into_iter()
        .map(|kind| kind.as_str().to_string())
        .collect()
}

/// Distinct `{{var}}` names in a template body, in first-seen order.
/// Business logic (parsing) stays in Rust; the Builder calls this rather than
/// scanning the body itself.
pub fn template_variables(body: &str) -> Result<Vec<String>, IpcError> {
    typvia_template::variables(body).map_err(|e| IpcError::validation(e.to_string()))
}

/// Loads a snippet's saved template fields.
pub fn template_fields(
    conn: &Connection,
    snippet_id: &str,
) -> Result<Vec<TemplateFieldDto>, IpcError> {
    let fields = TemplateFieldRepo::new(conn).list_by_snippet(snippet_id)?;
    Ok(fields.into_iter().map(TemplateFieldDto::from).collect())
}

/// Replaces a snippet's template fields as a whole set (transactional).
pub fn template_save_fields(
    conn: &Connection,
    snippet_id: &str,
    fields: Vec<TemplateFieldDto>,
    now: i64,
) -> Result<Vec<TemplateFieldDto>, IpcError> {
    let models = fields
        .iter()
        .map(|dto| dto_to_field(dto, snippet_id))
        .collect::<Result<Vec<_>, _>>()?;
    let before = TemplateFieldRepo::new(conn).list_by_snippet(snippet_id)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let stored = TemplateFieldRepo::new(&tx).replace_for_snippet(snippet_id, &models)?;
    // Replace-all writes: every surviving field is a content change, every
    // removed field a tombstone (fields are their own sync entities).
    for field in &stored {
        notify_sync(&tx, SyncEntityType::TemplateField, &field.id, None, now)?;
    }
    for old in &before {
        if !stored.iter().any(|f| f.id == old.id) {
            notify_sync(&tx, SyncEntityType::TemplateField, &old.id, Some(now), now)?;
        }
    }
    tx.commit().map_err(RepoError::from)?;
    Ok(stored.into_iter().map(TemplateFieldDto::from).collect())
}

/// Lenient authoring preview: resolves fill-in values against `fields`, showing
/// placeholders for unfilled fields and a fixed mask for secret references
/// (never their plaintext).
pub fn template_preview(
    body: &str,
    fields: Vec<TemplateFieldDto>,
    values: std::collections::HashMap<String, String>,
) -> Result<String, IpcError> {
    let models = fields
        .iter()
        .map(|dto| dto_to_field(dto, "preview"))
        .collect::<Result<Vec<_>, _>>()?;
    typvia_template::preview(body, &models, &values)
        .map_err(|e| IpcError::validation(e.to_string()))
}

/// Strictly renders a template snippet with the supplied fill-in values into
/// final text, ready for injection. Unlike the authoring preview this is
/// exact: a missing required field, an undefined variable, or an unresolved
/// secret reference is a business error. Secret references need the vault,
/// so a template using them cannot be injected yet.
pub fn template_render(
    conn: &Connection,
    id: &str,
    values: &std::collections::HashMap<String, String>,
) -> Result<String, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    let body = match snippet.content {
        SnippetContent::Plaintext(text) => text,
        SnippetContent::Ciphertext(_) => {
            return Err(IpcError::validation(
                "sensitive snippets cannot be injected yet",
            ));
        }
    };
    let fields = TemplateFieldRepo::new(conn).list_by_snippet(id)?;
    let rendered = typvia_template::render(&body, &fields, values).map_err(render_error)?;
    rendered.into_text().map_err(render_error)
}

/// Maps a template render/parse failure to a business error. Messages carry
/// only the field name (metadata), never a fill-in value or body (log red line).
fn render_error(e: typvia_template::RenderError) -> IpcError {
    use typvia_template::RenderError;
    match e {
        RenderError::MissingRequired { field } => {
            IpcError::validation(format!("fill in the required field: {field}"))
        }
        RenderError::UnknownVariable { .. } => {
            IpcError::validation("template references an undefined field")
        }
        RenderError::UnresolvedSecret { .. } => {
            IpcError::validation("secret fields are filled after unlocking the vault")
        }
        RenderError::Parse(_) => IpcError::validation("template could not be parsed"),
    }
}

/// Maps a field DTO to the core model, parsing the field-type enum. The
/// `snippet_id` is supplied by the caller, never trusted from the DTO.
fn dto_to_field(dto: &TemplateFieldDto, snippet_id: &str) -> Result<TemplateField, IpcError> {
    let field_type = TemplateFieldType::from_str(&dto.field_type)
        .map_err(|_| IpcError::validation("unknown template field type"))?;
    Ok(TemplateField {
        id: dto.id.clone(),
        snippet_id: snippet_id.to_string(),
        name: dto.name.clone(),
        label: dto.label.clone(),
        field_type,
        default_value: dto.default_value.clone(),
        options: dto.options.clone(),
        validation: dto.validation.clone(),
        is_required: dto.is_required,
        sort_order: dto.sort_order,
        platform_overrides: dto.platform_overrides.clone(),
    })
}

pub fn snippet_get(conn: &Connection, id: &str) -> Result<SnippetDto, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    Ok(snippet.into())
}

pub fn snippet_list(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list(limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn snippet_list_by_folder(
    conn: &Connection,
    folder_id: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list_by_folder(folder_id, limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Maps the wire view name + optional folder id onto a repo scope. The view
/// vocabulary is the Library rail: all | recent | used | starred | unsorted |
/// folder.
fn parse_scope<'a>(view: &str, folder_id: Option<&'a str>) -> Result<ListScope<'a>, IpcError> {
    match (view, folder_id) {
        ("folder", Some(id)) => Ok(ListScope::Folder(id)),
        ("folder", None) => Err(IpcError::validation("folder view requires folderId")),
        (_, Some(_)) => Err(IpcError::validation(
            "folderId only applies to the folder view",
        )),
        ("all", None) => Ok(ListScope::All),
        ("recent", None) => Ok(ListScope::Recent),
        ("used", None) => Ok(ListScope::Used),
        ("starred", None) => Ok(ListScope::Starred),
        ("unsorted", None) => Ok(ListScope::Unsorted),
        _ => Err(IpcError::validation("unknown library view")),
    }
}

/// Maps the wire order name onto a repo order; `None` keeps the view's own
/// order (the mobile chapters rely on that).
fn parse_order(order: Option<&str>) -> Result<Option<ListOrder>, IpcError> {
    match order {
        None => Ok(None),
        Some("recent") => Ok(Some(ListOrder::LastUsed)),
        Some("added") => Ok(Some(ListOrder::Created)),
        Some("used") => Ok(Some(ListOrder::UsageCount)),
        Some(_) => Err(IpcError::validation("unknown list order")),
    }
}

pub fn snippet_list_page(
    conn: &Connection,
    view: &str,
    folder_id: Option<&str>,
    snippet_type: Option<&str>,
    order: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let scope = parse_scope(view, folder_id)?;
    let type_filter = snippet_type.map(parse_snippet_type).transpose()?;
    let repo = SnippetRepo::new(conn);
    let rows = match parse_order(order)? {
        Some(order) => repo.list_scoped_ordered(scope, type_filter, order, limit, offset)?,
        None => repo.list_scoped(scope, type_filter, limit, offset)?,
    };
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Exact row count behind one Library page query — the virtual list sizes
/// its scroll range from this, so it honours the same type filter.
pub fn snippet_count(
    conn: &Connection,
    view: &str,
    folder_id: Option<&str>,
    snippet_type: Option<&str>,
) -> Result<u32, IpcError> {
    let scope = parse_scope(view, folder_id)?;
    let type_filter = snippet_type.map(parse_snippet_type).transpose()?;
    Ok(SnippetRepo::new(conn).count_scoped(scope, type_filter)?)
}

pub fn library_counts(conn: &Connection) -> Result<LibraryCountsDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    Ok(LibraryCountsDto {
        total: repo.count_scoped(ListScope::All, None)?,
        recent: repo.count_scoped(ListScope::Recent, None)?,
        starred: repo.count_scoped(ListScope::Starred, None)?,
        unsorted: repo.count_scoped(ListScope::Unsorted, None)?,
        trash: repo.count_trashed()?,
        folders: repo
            .count_by_folder()?
            .into_iter()
            .map(|(folder_id, count)| FolderCountDto { folder_id, count })
            .collect(),
    })
}

pub fn snippet_trash(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).soft_delete(id, now)?;
    // Entering the recycle bin propagates as a tombstone.
    notify_sync(&tx, SyncEntityType::Snippet, id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

/// Records one use of a snippet, after a host has actually delivered it.
///
/// Delivery itself is the host's own act — a clipboard here, an injector
/// there, a text field somewhere else — so this layer cannot do it and does
/// not pretend to. What it owns is the rule: **a use is counted only after the
/// words have gone somewhere**, and it is counted with one light statement of
/// its own rather than inside a content transaction it would have to compete
/// with.
///
/// It exists because the rule was living in one host's private code: the
/// desktop counted uses, the two mobile hosts did not, and their recall
/// shelves were empty forever — not because nobody used anything, but because
/// nobody was writing it down.
pub fn snippet_record_use(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

pub fn snippet_restore(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let repo = SnippetRepo::new(&tx);
    repo.restore_from_trash(id)?;
    // A restored temporary snippet restarts its expiry window — without
    // this it would expire again on the next sweep.
    if let Some(snippet) = repo.get(id)?
        && snippet.snippet_type == SnippetType::Temporary
    {
        repo.touch_updated_at(id, now)?;
    }
    // Restoring is an explicit user action: a fresh content record is the
    // legitimate revival path past an applied tombstone.
    notify_sync(&tx, SyncEntityType::Snippet, id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

/// How long a temporary snippet lives after its last edit before the sweep
/// moves it to the recycle bin: 48 hours, fixed in v1.
pub const TEMPORARY_TTL_MS: i64 = 48 * 60 * 60 * 1000;

/// Moves expired temporary snippets into the recycle bin: each
/// one follows the exact trash flow — soft delete plus a sync tombstone in
/// the same transaction — so peers converge and the 30-day bin retention
/// still applies. Returns how many expired. Runs from the hosts' periodic
/// tick; only `temporary` rows are ever considered.
pub fn temporary_expire(conn: &Connection, now: i64) -> Result<usize, IpcError> {
    let ids = SnippetRepo::new(conn).list_expired_temporary(now, TEMPORARY_TTL_MS)?;
    for id in &ids {
        snippet_trash(conn, id, now)?;
    }
    Ok(ids.len())
}

pub fn snippet_delete_forever(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).delete(id)?;
    // Deleting from an active state still needs a tombstone; purging an
    // already-tombstoned trash row re-asserts it harmlessly (head + 1).
    notify_sync(&tx, SyncEntityType::Snippet, id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(id)?;
    Ok(())
}

pub fn trash_list(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list_trashed(limit, offset)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn trash_purge_expired(conn: &Connection, now: i64) -> Result<usize, IpcError> {
    // Trashed rows are never in the search index, so no index work here.
    Ok(SnippetRepo::new(conn).purge_expired_trash(now, TRASH_RETENTION_MS)?)
}

fn check_ids(ids: &[String]) -> Result<(), IpcError> {
    if ids.is_empty() {
        return Err(IpcError::validation("no snippets selected"));
    }
    if ids.len() > MAX_PAGE_LIMIT as usize {
        return Err(IpcError::validation("too many snippets in one batch"));
    }
    Ok(())
}

/// Moves a batch into a folder (`None` = unfiled) atomically, then re-indexes
/// each row — the folder name is part of the search index.
pub fn snippet_batch_move(
    conn: &Connection,
    ids: &[String],
    folder_id: Option<&str>,
    now: i64,
) -> Result<(), IpcError> {
    check_ids(ids)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).batch_move(ids, folder_id)?;
    for id in ids {
        notify_sync(&tx, SyncEntityType::Snippet, id, None, now)?;
    }
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Tags a batch atomically, then re-indexes (tags are searchable).
pub fn snippet_batch_add_tag(
    conn: &Connection,
    ids: &[String],
    tag_id: &str,
    now: i64,
) -> Result<(), IpcError> {
    check_ids(ids)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).batch_add_tag(ids, tag_id)?;
    for id in ids {
        notify_sync(
            &tx,
            SyncEntityType::SnippetTag,
            &format!("{id}/{tag_id}"),
            None,
            now,
        )?;
    }
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Moves a batch into the recycle bin atomically; trashed rows leave the
/// search index.
pub fn snippet_batch_trash(conn: &Connection, ids: &[String], now: i64) -> Result<(), IpcError> {
    check_ids(ids)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let repo = SnippetRepo::new(&tx);
    for id in ids {
        repo.soft_delete(id, now)?;
        notify_sync(&tx, SyncEntityType::Snippet, id, Some(now), now)?;
    }
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    for id in ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Every snippet id inside a folder subtree (the folder itself plus all
/// descendants), collected via the repositories — no SQL in the host.
fn snippet_ids_in_subtree(conn: &Connection, folder_id: &str) -> Result<Vec<String>, IpcError> {
    let folders = FolderRepo::new(conn);
    let snippets = SnippetRepo::new(conn);
    let mut queue = vec![folder_id.to_string()];
    let mut ids = Vec::new();
    while let Some(current) = queue.pop() {
        for child in folders.list_children(Some(&current))? {
            queue.push(child.id);
        }
        let mut offset = 0;
        loop {
            let page = snippets.list_by_folder(Some(&current), MAX_PAGE_LIMIT, offset)?;
            let page_len = page.len();
            ids.extend(page.into_iter().map(|s| s.id));
            if page_len < MAX_PAGE_LIMIT as usize {
                break;
            }
            offset += MAX_PAGE_LIMIT;
        }
    }
    Ok(ids)
}

/// A folder's name, as it will be stored.
///
/// Trimmed, because "上线" and "上线 " are one folder to a reader and two to a
/// database; refused when there is nothing left, because a nameless folder
/// renders as a chip with no word on it — unselectable, unrenameable, and
/// impossible to tell from a rendering fault.
fn folder_name(raw: &str) -> Result<String, IpcError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IpcError::validation("a folder needs a name"));
    }
    Ok(trimmed.to_string())
}

pub fn folder_create(
    conn: &Connection,
    input: FolderCreateInput,
    now: i64,
) -> Result<FolderDto, IpcError> {
    let folder = Folder {
        id: new_id(),
        parent_id: input.parent_id,
        name: folder_name(&input.name)?,
        sort_order: input.sort_order,
        created_at: now,
        updated_at: now,
    };
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    FolderRepo::new(&tx).insert(&folder)?;
    notify_sync(&tx, SyncEntityType::Folder, &folder.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(folder.into())
}

pub fn folder_update(
    conn: &Connection,
    input: FolderUpdateInput,
    now: i64,
) -> Result<FolderDto, IpcError> {
    let repo = FolderRepo::new(conn);
    let mut folder = repo.get(&input.id)?.ok_or_else(IpcError::not_found)?;
    let name = folder_name(&input.name)?;
    let renamed = folder.name != name;
    folder.name = name;
    folder.parent_id = input.parent_id;
    folder.sort_order = input.sort_order;
    folder.updated_at = now;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    FolderRepo::new(&tx).update(&folder)?;
    notify_sync(&tx, SyncEntityType::Folder, &folder.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    // The folder name is indexed with each snippet: a rename must re-index
    // the folder's direct contents.
    if renamed {
        let index = SearchIndex::new(conn);
        let mut offset = 0;
        loop {
            let page =
                SnippetRepo::new(conn).list_by_folder(Some(&folder.id), MAX_PAGE_LIMIT, offset)?;
            let page_len = page.len();
            for snippet in page {
                index.sync_snippet(&snippet.id)?;
            }
            if page_len < MAX_PAGE_LIMIT as usize {
                break;
            }
            offset += MAX_PAGE_LIMIT;
        }
    }
    Ok(folder.into())
}

/// Deletes a folder: child folders cascade away and contents fall back to
/// unfiled (schema rules), so the affected snippets are re-indexed after.
pub fn folder_delete(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let affected = snippet_ids_in_subtree(conn, id)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    FolderRepo::new(&tx).delete(id)?;
    // One tombstone for the subtree root: both sides run the same cascade
    // (child folders away, contents to unfiled).
    notify_sync(&tx, SyncEntityType::Folder, id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    for snippet_id in &affected {
        index.sync_snippet(snippet_id)?;
    }
    Ok(())
}

/// Folds one folder into another in a single transaction: the source's own
/// snippets move to the target, folders nested under the source move up to
/// the source's parent so nothing cascades away, and the source is deleted.
/// Returns the ids that moved, which is what an undo needs to put them back.
pub fn folder_merge(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    now: i64,
) -> Result<Vec<String>, IpcError> {
    if source_id == target_id {
        return Err(IpcError::validation("a folder cannot merge into itself"));
    }
    let folders = FolderRepo::new(conn);
    let source = folders.get(source_id)?.ok_or_else(IpcError::not_found)?;
    folders.get(target_id)?.ok_or_else(IpcError::not_found)?;

    let snippets = SnippetRepo::new(conn);
    let mut moved = Vec::new();
    let mut offset = 0;
    loop {
        let page = snippets.list_by_folder(Some(source_id), MAX_PAGE_LIMIT, offset)?;
        let page_len = page.len();
        moved.extend(page.into_iter().map(|s| s.id));
        if page_len < MAX_PAGE_LIMIT as usize {
            break;
        }
        offset += MAX_PAGE_LIMIT;
    }
    let children = folders.list_children(Some(source_id))?;

    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    if !moved.is_empty() {
        SnippetRepo::new(&tx).batch_move(&moved, Some(target_id))?;
        for id in &moved {
            notify_sync(&tx, SyncEntityType::Snippet, id, None, now)?;
        }
    }
    let repo = FolderRepo::new(&tx);
    for mut child in children {
        child.parent_id.clone_from(&source.parent_id);
        child.updated_at = now;
        repo.update(&child)?;
        notify_sync(&tx, SyncEntityType::Folder, &child.id, None, now)?;
    }
    repo.delete(source_id)?;
    notify_sync(&tx, SyncEntityType::Folder, source_id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;

    // The folder name is indexed with each snippet.
    let index = SearchIndex::new(conn);
    for id in &moved {
        index.sync_snippet(id)?;
    }
    Ok(moved)
}

/// Writes a new order for the given folders, first id first, all or nothing:
/// a half-applied order would show a list the reader never arranged.
pub fn folder_reorder(conn: &Connection, ids: &[String], now: i64) -> Result<(), IpcError> {
    if ids.is_empty() {
        return Err(IpcError::validation("no folders to order"));
    }
    if ids.len() > MAX_PAGE_LIMIT as usize {
        return Err(IpcError::validation("too many folders in one order"));
    }
    let distinct: std::collections::HashSet<&String> = ids.iter().collect();
    if distinct.len() != ids.len() {
        return Err(IpcError::validation("a folder appears twice in the order"));
    }
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let repo = FolderRepo::new(&tx);
    for (position, id) in ids.iter().enumerate() {
        let mut folder = repo.get(id)?.ok_or_else(IpcError::not_found)?;
        folder.sort_order = i32::try_from(position).map_err(|_| IpcError::system())?;
        folder.updated_at = now;
        repo.update(&folder)?;
        notify_sync(&tx, SyncEntityType::Folder, id, None, now)?;
    }
    tx.commit().map_err(RepoError::from)?;
    Ok(())
}

pub fn folder_list_children(
    conn: &Connection,
    parent_id: Option<&str>,
) -> Result<Vec<FolderDto>, IpcError> {
    let rows = FolderRepo::new(conn).list_children(parent_id)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn tag_create(conn: &Connection, name: String, now: i64) -> Result<TagDto, IpcError> {
    let tag = Tag {
        id: new_id(),
        name,
        created_at: now,
    };
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    TagRepo::new(&tx).insert(&tag)?;
    notify_sync(&tx, SyncEntityType::Tag, &tag.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(tag.into())
}

pub fn tag_list(conn: &Connection) -> Result<Vec<TagDto>, IpcError> {
    let rows = TagRepo::new(conn).list_all()?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub fn tag_rename(conn: &Connection, id: &str, name: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    TagRepo::new(&tx).rename(id, name)?;
    notify_sync(&tx, SyncEntityType::Tag, id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(())
}

pub fn tag_delete(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    TagRepo::new(&tx).delete(id)?;
    notify_sync(&tx, SyncEntityType::Tag, id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(())
}

pub fn search_snippets(
    conn: &Connection,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<SearchHitDto>, IpcError> {
    check_limit(limit)?;
    let hits = Searcher::new(conn).search(query, limit, offset)?;
    Ok(hits.into_iter().map(Into::into).collect())
}

// ===== App rules =====

/// The platform variant this build runs on; `None` where the model has no
/// matching variant. Two callers read it: app rules, which then default-allow
/// — they are a convenience filter, not a security boundary — and the sync
/// host, which cannot register this install's device row without it and
/// stays unavailable instead.
pub fn current_platform() -> Option<Platform> {
    if cfg!(target_os = "macos") {
        Some(Platform::Macos)
    } else if cfg!(target_os = "windows") {
        Some(Platform::Windows)
    } else if cfg!(target_os = "ios") {
        Some(Platform::Ios)
    } else if cfg!(target_os = "android") {
        Some(Platform::Android)
    } else {
        None
    }
}

fn delivery_decision(
    conn: &Connection,
    id: &str,
    platform: Option<Platform>,
    destination: Option<&str>,
) -> Result<AppRuleDecision, IpcError> {
    let (Some(platform), Some(destination)) = (platform, destination) else {
        return Ok(AppRuleDecision::ALLOW_ALL);
    };
    let rules = AppRuleRepo::new(conn).list_for_snippet(id)?;
    Ok(app_rules::evaluate(&rules, platform, Some(destination)))
}

/// Panel insert gate: refuses delivery of a snippet the destination app's
/// rules hide. Commands run this before hiding the panel, so a blocked
/// insert keeps the panel up with the notice.
pub fn panel_gate_insert(
    conn: &Connection,
    id: &str,
    platform: Option<Platform>,
    destination: Option<&str>,
) -> Result<(), IpcError> {
    if !delivery_decision(conn, id, platform, destination)?.visible {
        return Err(IpcError::rule_blocked(
            "an app rule hides this snippet in the destination app",
        ));
    }
    Ok(())
}

/// Sensitive panel insert gate: a hidden snippet or a matched
/// `deny_sensitive_injection` rule both refuse. The vault stays the actual
/// security boundary — this only closes the convenience path into apps the
/// user ruled out.
pub fn panel_gate_insert_secret(
    conn: &Connection,
    id: &str,
    platform: Option<Platform>,
    destination: Option<&str>,
) -> Result<(), IpcError> {
    let decision = delivery_decision(conn, id, platform, destination)?;
    if !decision.visible {
        return Err(IpcError::rule_blocked(
            "an app rule hides this snippet in the destination app",
        ));
    }
    if !decision.sensitive_injection_allowed {
        return Err(IpcError::rule_blocked(
            "an app rule blocks sensitive insert into the destination app",
        ));
    }
    Ok(())
}

/// A `/xx` type-quick-filter parsed off the front of a panel query.
/// The tokens are the product's two-letter type marks;
/// `/sc` filters by security level (the sensitive bucket), the rest by
/// snippet type. An unrecognized token is treated as literal search text.
#[derive(Debug, PartialEq)]
enum PanelTypeFilter {
    SnippetType(&'static str),
    Sensitive,
}

/// Splits a panel query into an optional type filter and the remaining text.
fn parse_panel_filter(query: &str) -> (Option<PanelTypeFilter>, String) {
    let trimmed = query.trim_start();
    let Some(token) = trimmed.strip_prefix('/') else {
        return (None, query.to_string());
    };
    // Char-based split: a multi-byte char after the slash must not panic.
    let mark: String = token.chars().take(2).collect();
    let rest = &token[mark.len()..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return (None, query.to_string());
    }
    let filter = match mark.to_ascii_lowercase().as_str() {
        "tx" => PanelTypeFilter::SnippetType("text"),
        "cd" => PanelTypeFilter::SnippetType("code"),
        "cm" => PanelTypeFilter::SnippetType("command"),
        "pr" => PanelTypeFilter::SnippetType("prompt"),
        "tp" => PanelTypeFilter::SnippetType("template"),
        "ai" => PanelTypeFilter::SnippetType("ai_action"),
        "lk" => PanelTypeFilter::SnippetType("link"),
        "sc" => PanelTypeFilter::Sensitive,
        _ => return (None, query.to_string()),
    };
    (Some(filter), rest.trim_start().to_string())
}

fn matches_panel_filter(row: &SnippetDto, filter: &PanelTypeFilter) -> bool {
    match filter {
        PanelTypeFilter::SnippetType(snippet_type) => row.snippet_type == *snippet_type,
        PanelTypeFilter::Sensitive => row.security_level != "normal",
    }
}

/// Panel result page: recent snippets for an empty query, ranked search hits
/// otherwise, minus the rows the destination app's rules hide. The hidden
/// count is returned so the panel states the real number instead of rows
/// vanishing silently. A leading `/xx` type mark narrows either mode to one
/// type bucket before the rule filter runs.
pub fn panel_results(
    conn: &Connection,
    query: &str,
    limit: u32,
    platform: Option<Platform>,
    destination: Option<&str>,
) -> Result<PanelResultsDto, IpcError> {
    check_limit(limit)?;
    let (filter, text) = parse_panel_filter(query);
    // Unlike the Library, the panel keeps sensitive rows reachable (recents
    // and title search): it is the calling surface that owns the
    // verify-then-insert flow, and a listed row carries no content.
    let mut rows: Vec<SnippetDto> = if text.trim().is_empty() {
        let rows = SnippetRepo::new(conn).list_recent_including_sensitive(limit)?;
        rows.into_iter().map(Into::into).collect()
    } else {
        search_rows(conn, &text, limit, true)?
    };
    // Disabled snippets stay findable in the Library but never enter the
    // calling surface ("archive" means disable) — same rule the
    // trigger set and the keyboard snapshot already apply.
    rows.retain(|row| row.is_enabled);
    if let Some(filter) = &filter {
        rows.retain(|row| matches_panel_filter(row, filter));
    }
    let (Some(platform), Some(destination)) = (platform, destination) else {
        return Ok(PanelResultsDto {
            rows,
            hidden_by_rules: 0,
        });
    };
    let rule_repo = AppRuleRepo::new(conn);
    let mut kept = Vec::with_capacity(rows.len());
    let mut hidden = 0u32;
    for row in rows {
        let rules = rule_repo.list_for_snippet(&row.id)?;
        if app_rules::evaluate(&rules, platform, Some(destination)).visible {
            kept.push(row);
        } else {
            hidden += 1;
        }
    }
    Ok(PanelResultsDto {
        rows: kept,
        hidden_by_rules: hidden,
    })
}

fn parse_rule_type(value: &str) -> Result<AppRuleType, IpcError> {
    let rule_type =
        AppRuleType::from_str(value).map_err(|_| IpcError::validation("unknown rule type"))?;
    if rule_type == AppRuleType::DisableExpansion {
        // Espanso expansion is one global config file in v1, so a per-app
        // expansion rule would be stored but never enforced. Refused rather
        // than left silently inert.
        return Err(IpcError::validation(
            "per-app expansion control is not available yet",
        ));
    }
    Ok(rule_type)
}

fn rule_dto(rule: AppRule, snippet_title: String) -> AppRuleDto {
    AppRuleDto {
        id: rule.id,
        snippet_id: rule.snippet_id,
        snippet_title,
        platform: rule.platform.as_str().to_string(),
        app_identifier: rule.app_identifier,
        rule_type: rule.rule_type.as_str().to_string(),
    }
}

fn is_duplicate_rule(
    repo: &AppRuleRepo<'_>,
    snippet_id: &str,
    platform: Platform,
    app_identifier: &str,
    rule_type: AppRuleType,
    exclude_id: Option<&str>,
) -> Result<bool, IpcError> {
    Ok(repo.list_for_snippet(snippet_id)?.into_iter().any(|r| {
        exclude_id != Some(r.id.as_str())
            && r.platform == platform
            && r.rule_type == rule_type
            && r.app_identifier.eq_ignore_ascii_case(app_identifier)
    }))
}

/// This platform's rules for the settings list, snippet titles joined in.
pub fn app_rule_list(
    conn: &Connection,
    platform: Option<Platform>,
    limit: u32,
    offset: u32,
) -> Result<Vec<AppRuleDto>, IpcError> {
    check_limit(limit)?;
    let Some(platform) = platform else {
        return Ok(Vec::new());
    };
    let rules = AppRuleRepo::new(conn).list_for_platform(platform, limit, offset)?;
    let snippets = SnippetRepo::new(conn);
    let mut dtos = Vec::with_capacity(rules.len());
    for rule in rules {
        // CASCADE keeps rules snippet-backed; a racing delete just skips.
        let Some(snippet) = snippets.get(&rule.snippet_id)? else {
            continue;
        };
        dtos.push(rule_dto(rule, snippet.title));
    }
    Ok(dtos)
}

pub fn app_rule_create(
    conn: &Connection,
    platform: Option<Platform>,
    input: AppRuleCreateInput,
    now: i64,
) -> Result<AppRuleDto, IpcError> {
    let Some(platform) = platform else {
        return Err(IpcError::validation(
            "app rules are not supported on this platform",
        ));
    };
    let rule_type = parse_rule_type(&input.rule_type)?;
    let app_identifier = input.app_identifier.trim().to_string();
    let snippet = SnippetRepo::new(conn)
        .get(&input.snippet_id)?
        .ok_or_else(IpcError::not_found)?;
    let repo = AppRuleRepo::new(conn);
    if is_duplicate_rule(
        &repo,
        &snippet.id,
        platform,
        &app_identifier,
        rule_type,
        None,
    )? {
        return Err(IpcError::conflict("this rule already exists"));
    }
    let rule = AppRule {
        id: new_id(),
        snippet_id: snippet.id.clone(),
        platform,
        app_identifier,
        rule_type,
        // v1 never collects window titles (security red line), so no rule
        // may carry a pattern that would match them.
        window_title_pattern: None,
    };
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    AppRuleRepo::new(&tx).insert(&rule)?;
    notify_sync(&tx, SyncEntityType::AppRule, &rule.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(rule_dto(rule, snippet.title))
}

pub fn app_rule_update(
    conn: &Connection,
    id: &str,
    input: AppRuleUpdateInput,
    now: i64,
) -> Result<AppRuleDto, IpcError> {
    let rule_type = parse_rule_type(&input.rule_type)?;
    let app_identifier = input.app_identifier.trim().to_string();
    let repo = AppRuleRepo::new(conn);
    let mut rule = repo.get(id)?.ok_or_else(IpcError::not_found)?;
    if is_duplicate_rule(
        &repo,
        &rule.snippet_id,
        rule.platform,
        &app_identifier,
        rule_type,
        Some(&rule.id),
    )? {
        return Err(IpcError::conflict("this rule already exists"));
    }
    rule.app_identifier = app_identifier;
    rule.rule_type = rule_type;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    AppRuleRepo::new(&tx).update(&rule)?;
    notify_sync(&tx, SyncEntityType::AppRule, &rule.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    let title = SnippetRepo::new(conn)
        .get(&rule.snippet_id)?
        .map(|s| s.title)
        .unwrap_or_default();
    Ok(rule_dto(rule, title))
}

pub fn app_rule_delete(conn: &Connection, id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    AppRuleRepo::new(&tx).delete(id)?;
    notify_sync(&tx, SyncEntityType::AppRule, id, Some(now), now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(())
}

// ===== Onboarding =====

/// Marker file in the app data directory recording that first-run onboarding
/// finished (or was skipped). A fresh data directory has no marker, so a new
/// install always enters the flow; deleting the directory re-runs it.
const ONBOARDING_MARKER: &str = "onboarding-complete";

pub fn onboarding_completed(data_dir: &Path) -> bool {
    data_dir.join(ONBOARDING_MARKER).is_file()
}

pub fn mark_onboarding_complete(data_dir: &Path) -> Result<(), IpcError> {
    std::fs::create_dir_all(data_dir).map_err(|_| IpcError::system())?;
    std::fs::write(data_dir.join(ONBOARDING_MARKER), b"done\n").map_err(|_| IpcError::system())
}

/// Longest clipboard text the onboarding step-3 prefill accepts, in chars.
/// Longer content is almost never a snippet the user meant to keep, and the
/// prefill card is not a place to scroll a document.
pub const CLIPBOARD_SEED_MAX_CHARS: usize = 10_000;

/// Filters raw clipboard text down to what the onboarding first-snippet card
/// may prefill. Returns `None` for empty/oversized content and — clipboard
/// content that looks like a secret is ignored by default — for anything
/// the offline sensitive scan flags, so a copied token never lands in a
/// plain snippet form. The text itself is never logged.
pub fn clipboard_seed(text: Option<String>) -> Option<String> {
    let text = text?;
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > CLIPBOARD_SEED_MAX_CHARS {
        return None;
    }
    if !typvia_core::sensitive::detect(trimmed).is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

/// Imports the text of a Markdown / JSON / CSV file as normal snippets, using
/// the same transactional pattern as `espanso_import`: parse failures are
/// business errors, unimportable entries and trigger conflicts are reported.
pub fn snippets_import(
    conn: &Connection,
    format: &str,
    text: &str,
    now: i64,
) -> Result<ImportReportDto, IpcError> {
    let format = generic_import::ImportFormat::from_str(format)
        .map_err(|_| IpcError::validation("unknown import format"))?;
    let parsed = generic_import::parse(format, text)
        .map_err(|error| IpcError::validation(error.message()))?;

    let snippets = parsed
        .entries
        .iter()
        .map(|entry| entry_snippet(entry, now))
        .collect();
    let (imported_ids, conflicts) = commit_imported(conn, snippets, now)?;

    Ok(ImportReportDto {
        imported: imported_ids.len(),
        conflicts,
        skipped: parsed
            .skipped
            .into_iter()
            .map(|skip| ImportSkippedDto {
                label: skip.label,
                reason: skip.reason,
            })
            .collect(),
    })
}

/// Inserts import candidates in one transaction: a trigger already used by an
/// existing snippet — or by an earlier entry in the same batch (visible inside
/// the transaction) — is a reported conflict, not an import. A failure rolls
/// the whole batch back (no half-imported state); the search index is synced
/// only for committed rows. Returns (imported ids, conflicting triggers).
pub fn commit_imported(
    conn: &Connection,
    snippets: Vec<Snippet>,
    now: i64,
) -> Result<(Vec<String>, Vec<String>), IpcError> {
    let mut imported_ids = Vec::new();
    let mut conflicts = Vec::new();
    {
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        let repo = SnippetRepo::new(&tx);
        let versions = VersionRepo::new(&tx);
        for snippet in snippets {
            if let Some(trigger) = snippet.trigger.as_deref()
                && repo.find_trigger_conflict(trigger, None)?.is_some()
            {
                conflicts.push(trigger.to_string());
                continue;
            }
            repo.insert(&snippet)?;
            versions.append(&history_entry(&snippet, now))?;
            notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, None, now)?;
            imported_ids.push(snippet.id.clone());
        }
        tx.commit().map_err(RepoError::from)?;
    }

    for id in &imported_ids {
        SearchIndex::new(conn).sync_snippet(id)?;
    }
    Ok((imported_ids, conflicts))
}

// --- Encrypted backup --------------------------------------------------------
//
// The backup document is assembled from the repositories (bounded pages) and
// sealed in core::backup under a key derived from the user's backup
// passphrase. Sensitive bodies stay in their vault envelopes inside the
// document; nothing here decrypts anything.

const BACKUP_PAGE: u32 = 500;

/// Exhausts a paged repository query (bounded pages, per database rules).
fn drain_pages<T, F>(mut fetch: F) -> Result<Vec<T>, RepoError>
where
    F: FnMut(u32, u32) -> Result<Vec<T>, RepoError>,
{
    let mut all = Vec::new();
    let mut offset = 0;
    loop {
        let page = fetch(BACKUP_PAGE, offset)?;
        let done = page.len() < BACKUP_PAGE as usize;
        all.extend(page);
        if done {
            return Ok(all);
        }
        offset += BACKUP_PAGE;
    }
}

fn backup_error(error: typvia_core::backup::BackupError) -> IpcError {
    use typvia_core::backup::BackupError;
    match error {
        BackupError::CannotOpen => IpcError::permission_denied(error.message()),
        _ => IpcError::validation(error.message()),
    }
}

/// Exports the whole library (snippets incl. trash, folders, tags, links,
/// template fields, version history, wrapped vault keys) as an encrypted
/// backup file's text. The passphrase is used to derive the backup key and
/// dropped — never stored or logged.
pub fn backup_export(conn: &Connection, passphrase: &str, now: i64) -> Result<String, IpcError> {
    if passphrase.trim().is_empty() {
        return Err(IpcError::validation(
            "the backup passphrase must not be empty",
        ));
    }
    use typvia_core::backup as bk;

    let repo = SnippetRepo::new(conn);
    let mut snippets = drain_pages(|limit, offset| repo.list(limit, offset))?;
    snippets.extend(drain_pages(|limit, offset| {
        repo.list_trashed(limit, offset)
    })?);

    let mut document = bk::BackupDocument {
        exported_at: now,
        folders: FolderRepo::new(conn)
            .list_all_parents_first()?
            .iter()
            .map(bk::FolderRecord::from_model)
            .collect(),
        tags: TagRepo::new(conn)
            .list_all()?
            .iter()
            .map(bk::TagRecord::from_model)
            .collect(),
        ..bk::BackupDocument::default()
    };

    let fields = TemplateFieldRepo::new(conn);
    let versions = VersionRepo::new(conn);
    for snippet in &snippets {
        for tag_id in repo.tag_ids_of(&snippet.id)? {
            document.tag_links.push(bk::TagLinkRecord {
                snippet_id: snippet.id.clone(),
                tag_id,
            });
        }
        document.template_fields.extend(
            fields
                .list_by_snippet(&snippet.id)?
                .iter()
                .map(bk::TemplateFieldRecord::from_model),
        );
        let mut offset = 0;
        loop {
            let page = versions.list(&snippet.id, BACKUP_PAGE, offset)?;
            let done = page.len() < BACKUP_PAGE as usize;
            document
                .versions
                .extend(page.iter().map(bk::VersionRecord::from_model));
            if done {
                break;
            }
            offset += BACKUP_PAGE;
        }
        document
            .snippets
            .push(bk::SnippetRecord::from_model(snippet));
    }

    let keys = VaultKeyRepo::new(conn);
    if let Some(header) = keys.load_header()? {
        document.vault = Some(bk::VaultRecord {
            header: bk::KeyHeaderRecord::from_model(&header),
            domain_keys: keys
                .list_domain_keys()?
                .iter()
                .map(bk::DomainKeyRecord::from_model)
                .collect(),
        });
    }

    bk::export(&document, passphrase).map_err(backup_error)
}

/// Restores a backup into an empty library: everything lands in one
/// transaction (a failure leaves the library untouched), then the search
/// index is rebuilt for the restored snippets. Requires that no snippets
/// exist and the vault is uninitialized — restore never merges.
pub fn backup_restore(
    conn: &Connection,
    passphrase: &str,
    text: &str,
    now: i64,
) -> Result<BackupRestoreDto, IpcError> {
    use typvia_core::backup as bk;

    let repo = SnippetRepo::new(conn);
    let occupied = !repo.list(1, 0)?.is_empty()
        || !repo.list_trashed(1, 0)?.is_empty()
        || VaultKeyRepo::new(conn).load_header()?.is_some();
    if occupied {
        return Err(IpcError::conflict(
            "restore needs an empty library — this one already has content",
        ));
    }

    let document = bk::import(text, passphrase).map_err(backup_error)?;

    let mut active_ids = Vec::new();
    {
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        let folders = FolderRepo::new(&tx);
        for record in &document.folders {
            let folder = record.to_model();
            folders.insert(&folder)?;
            notify_sync(&tx, SyncEntityType::Folder, &folder.id, None, now)?;
        }
        let tags = TagRepo::new(&tx);
        for record in &document.tags {
            let tag = record.to_model();
            tags.insert(&tag)?;
            notify_sync(&tx, SyncEntityType::Tag, &tag.id, None, now)?;
        }
        let snippets = SnippetRepo::new(&tx);
        for record in &document.snippets {
            let snippet = record.to_model().map_err(backup_error)?;
            snippets.insert(&snippet)?;
            // Active rows sync as content; restored trash rows propagate as
            // tombstones so every device agrees they sit in the bin.
            let tombstone = snippet.deleted_at;
            notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, tombstone, now)?;
            if snippet.deleted_at.is_none() {
                active_ids.push(snippet.id.clone());
            }
        }
        for link in &document.tag_links {
            snippets.add_tag(&link.snippet_id, &link.tag_id)?;
            notify_sync(
                &tx,
                SyncEntityType::SnippetTag,
                &format!("{}/{}", link.snippet_id, link.tag_id),
                None,
                now,
            )?;
        }
        let fields = TemplateFieldRepo::new(&tx);
        let mut by_snippet: Vec<(String, Vec<TemplateField>)> = Vec::new();
        for record in &document.template_fields {
            let field = record.to_model().map_err(backup_error)?;
            match by_snippet
                .iter_mut()
                .find(|(id, _)| *id == field.snippet_id)
            {
                Some((_, group)) => group.push(field),
                None => by_snippet.push((field.snippet_id.clone(), vec![field])),
            }
        }
        for (snippet_id, group) in &by_snippet {
            for field in fields.replace_for_snippet(snippet_id, group)? {
                notify_sync(&tx, SyncEntityType::TemplateField, &field.id, None, now)?;
            }
        }
        let versions = VersionRepo::new(&tx);
        for record in &document.versions {
            versions.append(&record.to_model().map_err(backup_error)?)?;
        }
        if let Some(vault) = &document.vault {
            let keys = VaultKeyRepo::new(&tx);
            keys.put_header(&vault.header.to_model().map_err(backup_error)?)?;
            for key in &vault.domain_keys {
                keys.put_domain_key(&key.to_model().map_err(backup_error)?)?;
            }
        }
        tx.commit().map_err(RepoError::from)?;
    }

    for id in &active_ids {
        SearchIndex::new(conn).sync_snippet(id)?;
    }

    Ok(BackupRestoreDto {
        snippets: document.snippets.len(),
        folders: document.folders.len(),
        tags: document.tags.len(),
        versions: document.versions.len(),
        vault_restored: document.vault.is_some(),
    })
}

/// Builds a normal snippet from a unified import entry. A missing title is
/// derived from the content (first non-empty line, bounded) so the row is
/// legible; a trigger mode exists only when a trigger does.
pub fn entry_snippet(entry: &generic_import::ImportedEntry, now: i64) -> Snippet {
    let mode = entry.trigger.as_ref().map(|_| {
        if entry.word {
            TriggerMode::WordBoundary
        } else {
            TriggerMode::Immediate
        }
    });
    let title = entry
        .title
        .clone()
        .unwrap_or_else(|| import_title(&entry.content, entry.trigger.as_deref()));
    Snippet {
        id: new_id(),
        workspace_id: WORKSPACE_ID.to_string(),
        title,
        content: SnippetContent::Plaintext(entry.content.clone()),
        snippet_type: SnippetType::Text,
        description: entry.description.clone(),
        folder_id: None,
        trigger: entry.trigger.clone(),
        trigger_mode: mode,
        language: None,
        security_level: SecurityLevel::Normal,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: Vec::new(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

fn import_title(content: &str, trigger: Option<&str>) -> String {
    let first_line = content.lines().map(str::trim).find(|line| !line.is_empty());
    let base = first_line.or(trigger).unwrap_or("Imported snippet");
    base.chars().take(60).collect()
}

// --- Vault use-cases ---------------------------------------------------------
//
// Sensitive content is created/edited/read only through these functions on an
// unlocked `VaultSession`. Plaintext is encrypted host-side into a versioned
// envelope before it reaches storage (content_ciphertext, plaintext NULL), and
// is only ever returned to the WebView by an explicit `vault_reveal`.

/// Idle window before the vault auto-locks: five minutes idle. The UI
/// renders the re-lock countdown from this.
pub const VAULT_IDLE_TIMEOUT_MS: i64 = 5 * 60 * 1000;

/// Builds the observable vault status (no key material) for the Vault page.
fn vault_status_dto(conn: &Connection, session: &VaultSession) -> Result<VaultStatusDto, IpcError> {
    let initialized = VaultSession::is_initialized(conn).map_err(IpcError::from)?;
    let (unlocked, unlocked_at, last_activity_at) = match session.status() {
        UnlockStatus::Locked => (false, None, None),
        UnlockStatus::Unlocked {
            since,
            last_activity,
        } => (true, Some(since), Some(last_activity)),
    };
    Ok(VaultStatusDto {
        initialized,
        unlocked,
        unlocked_at,
        last_activity_at,
        idle_timeout_ms: VAULT_IDLE_TIMEOUT_MS,
    })
}

/// Reports vault status, first applying the idle timeout so a stale session
/// auto-locks on the next status poll (the frontend polls while on the page).
pub fn vault_status(
    conn: &Connection,
    session: &mut VaultSession,
    now: i64,
) -> Result<VaultStatusDto, IpcError> {
    session.enforce_idle_timeout(now, VAULT_IDLE_TIMEOUT_MS);
    vault_status_dto(conn, session)
}

/// First-run setup: sets the master password and leaves the vault unlocked.
pub fn vault_initialize(
    conn: &Connection,
    session: &mut VaultSession,
    password: &str,
    now: i64,
) -> Result<VaultStatusDto, IpcError> {
    session.initialize(conn, password.as_bytes(), now)?;
    vault_status_dto(conn, session)
}

/// Unlocks with the master password.
pub fn vault_unlock_password(
    conn: &Connection,
    session: &mut VaultSession,
    password: &str,
    now: i64,
) -> Result<VaultStatusDto, IpcError> {
    session.unlock_with_password(conn, password.as_bytes(), now)?;
    vault_status_dto(conn, session)
}

/// Unlocks via the biometric-gated MK copy (Touch ID). The OS enforces the
/// gate inside `retrieve`.
pub fn vault_unlock_biometric(
    conn: &Connection,
    session: &mut VaultSession,
    store: &dyn SecureStore,
    now: i64,
) -> Result<VaultStatusDto, IpcError> {
    session.unlock_with_biometric(store, now)?;
    vault_status_dto(conn, session)
}

/// Locks the vault immediately; re-locking is instantaneous.
pub fn vault_lock(
    conn: &Connection,
    session: &mut VaultSession,
) -> Result<VaultStatusDto, IpcError> {
    session.lock();
    vault_status_dto(conn, session)
}

/// Enrolls a biometric-gated MK copy so later unlocks can use Touch ID.
/// Requires an unlocked session.
pub fn vault_enable_biometric(
    session: &VaultSession,
    store: &dyn SecureStore,
) -> Result<(), IpcError> {
    session.enable_biometric(store)?;
    Ok(())
}

/// Removes the biometric MK copy (idempotent). The master-password path is
/// unaffected — biometrics are a gate, not the key source.
pub fn vault_disable_biometric(
    session: &VaultSession,
    store: &dyn SecureStore,
) -> Result<(), IpcError> {
    session.disable_biometric(store)?;
    Ok(())
}

/// How many sensitive snippets a vault reset would permanently destroy,
/// trashed rows included — the honest number for the confirm copy.
/// Works while locked: only a count crosses this boundary.
pub fn vault_reset_preview(conn: &Connection) -> Result<u32, IpcError> {
    let ids = SnippetRepo::new(conn).list_sensitive_ids()?;
    u32::try_from(ids.len()).map_err(|_| IpcError::system())
}

/// Destroys the vault for a user who lost the master password.
///
/// Order is deliberate: the biometric MK copy leaves the secure store first —
/// if the platform store refuses, nothing else has changed — then one
/// transaction deletes every sensitive snippet (each with a tombstone so
/// peers converge on the deletion) and all stored key material. After commit
/// the database is VACUUMed and the WAL truncated so the destroyed ciphertext
/// and wrapped keys do not linger in free pages. Works while locked: losing
/// the password is exactly the state this exists for.
pub fn vault_reset(
    conn: &Connection,
    session: &mut VaultSession,
    store: &dyn SecureStore,
    now: i64,
) -> Result<VaultStatusDto, IpcError> {
    if !VaultSession::is_initialized(conn).map_err(IpcError::from)? {
        return Err(IpcError::conflict("vault is not set up"));
    }
    let ids = SnippetRepo::new(conn).list_sensitive_ids()?;
    session.disable_biometric(store).map_err(IpcError::from)?;

    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    for id in &ids {
        SnippetRepo::new(&tx).delete(id)?;
        notify_sync(&tx, SyncEntityType::Snippet, id, Some(now), now)?;
    }
    VaultKeyRepo::new(&tx).delete_all_key_material()?;
    tx.commit().map_err(RepoError::from)?;

    session.lock();
    for id in &ids {
        SearchIndex::new(conn).sync_snippet(id)?;
    }
    purge_free_pages(conn)?;
    vault_status_dto(conn, session)
}

/// Rewrites the database and truncates the WAL, so bytes that were logically
/// removed stop existing on disk.
///
/// Deleting a row does not erase it: the old page keeps its contents until
/// something reuses it, and the write-ahead log keeps its own copy of the
/// page as it was. Both files sit next to each other in the data directory,
/// and "it will be overwritten eventually" is not a promise anybody made to
/// the user. Called on the two paths where plaintext stops being allowed to
/// exist — resetting the vault, and turning an ordinary snippet into a
/// sensitive one.
///
/// Expensive by nature. Both callers are deliberate, rare user actions, and
/// on both of them being thorough is worth more than being quick.
fn purge_free_pages(conn: &Connection) -> Result<(), IpcError> {
    conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(RepoError::from)?;
    Ok(())
}

/// Lists the vault's sensitive snippets (metadata only; `body` is `None`).
/// Contents are never returned here — only `vault_reveal` decrypts one.
pub fn vault_list(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<SnippetDto>, IpcError> {
    check_limit(limit)?;
    let rows = SnippetRepo::new(conn).list_scoped(
        ListScope::All,
        Some(SnippetType::Sensitive),
        limit,
        offset,
    )?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Decrypts one sensitive snippet's content for transient display in the
/// unlocked UI ("Reveal"). Requires an unlocked session; injection and
/// clipboard delivery of sensitive content go through their own paths.
pub fn vault_reveal(
    conn: &Connection,
    session: &mut VaultSession,
    id: &str,
    now: i64,
) -> Result<String, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    let envelope = match snippet.content {
        SnippetContent::Ciphertext(bytes) => bytes,
        SnippetContent::Plaintext(_) => {
            return Err(IpcError::conflict("snippet is not sensitive"));
        }
    };
    let plaintext = session.decrypt_content(conn, id, &envelope)?;
    // Revealing is user activity: defer the idle auto-lock.
    session.note_activity(now);
    // The plaintext must be valid UTF-8 text (snippets are text). The Zeroizing
    // buffer wipes on drop; the returned String is the caller's to hold briefly.
    String::from_utf8(plaintext.to_vec()).map_err(|_| IpcError::system())
}

/// How long a sensitive value stays on the clipboard after a copy before the
/// timed auto-clear wipes it. The product's supported range is 15–60
/// seconds; 30s sits mid-range and the host schedules the clear from this.
pub const SENSITIVE_CLIPBOARD_CLEAR_MS: i64 = 30 * 1000;

/// Creates a sensitive snippet: the supplied plaintext body is encrypted under
/// the vault key before it reaches storage (ciphertext-only, plaintext NULL).
/// Requires an unlocked session.
pub fn vault_create_secret(
    conn: &Connection,
    session: &VaultSession,
    input: SnippetCreateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let id = new_id();
    let ciphertext = session.encrypt_content(conn, &id, input.body.as_bytes())?;
    let (trigger, trigger_mode) = snippet_trigger(input.trigger, input.trigger_mode);
    let snippet = Snippet {
        id,
        workspace_id: WORKSPACE_ID.to_string(),
        title: input.title,
        content: SnippetContent::Ciphertext(ciphertext),
        snippet_type: SnippetType::Sensitive,
        description: input.description,
        folder_id: input.folder_id,
        trigger,
        trigger_mode: parse_trigger_mode(trigger_mode.as_deref())?,
        language: input.language,
        security_level: SecurityLevel::Sensitive,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: Vec::new(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    };
    check_trigger_free(&SnippetRepo::new(conn), snippet.trigger.as_deref(), None)?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).insert(&snippet)?;
    VersionRepo::new(&tx).append(&history_entry(&snippet, now))?;
    // Sensitive snippets sync as their vault envelope (double envelope);
    // the hook seals metadata plus ciphertext, never plaintext.
    notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    // A sensitive snippet contributes only title/tags to the index, never body.
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Re-encrypts an edited sensitive snippet's body. Requires an unlocked
/// session and an already-sensitive snippet (normal edits use `snippet_update`;
/// promoting a normal snippet uses `snippet_convert_to_sensitive`).
pub fn vault_update_secret(
    conn: &Connection,
    session: &VaultSession,
    input: SnippetUpdateInput,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    let mut snippet = repo.get(&input.id)?.ok_or_else(IpcError::not_found)?;
    if snippet.security_level != SecurityLevel::Sensitive {
        return Err(IpcError::conflict("snippet is not sensitive"));
    }
    let ciphertext = session.encrypt_content(conn, &snippet.id, input.body.as_bytes())?;
    snippet.title = input.title;
    snippet.content = SnippetContent::Ciphertext(ciphertext);
    snippet.description = input.description;
    snippet.folder_id = input.folder_id;
    let (trigger, trigger_mode) = snippet_trigger(input.trigger, input.trigger_mode);
    snippet.trigger = trigger;
    snippet.trigger_mode = parse_trigger_mode(trigger_mode.as_deref())?;
    snippet.language = input.language;
    snippet.is_favorite = input.is_favorite;
    snippet.is_pinned = input.is_pinned;
    snippet.is_enabled = input.is_enabled;
    snippet.updated_at = now;
    // Re-encryption always produces a fresh envelope, so every save is a
    // content change and bumps the version.
    snippet.version += 1;
    check_trigger_free(&repo, snippet.trigger.as_deref(), Some(&snippet.id))?;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).update(&snippet)?;
    let versions = VersionRepo::new(&tx);
    versions.append(&history_entry(&snippet, now))?;
    // Retention runs in the same transaction as the write, exactly like the
    // plain-snippet update path above.
    versions.apply_retention(&snippet.id, now)?;
    notify_sync(&tx, SyncEntityType::Snippet, &snippet.id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(&snippet.id)?;
    Ok(snippet.into())
}

/// Promotes a normal snippet to sensitive: encrypts its current plaintext,
/// stores ciphertext-only, and rebuilds the search index so the former
/// plaintext leaves index storage (`optimize`, red line). Prior plaintext
/// version rows are pruned in the same transaction — a secret must not leave
/// its cleartext history behind (secure_delete=ON zeros the freed pages).
/// Requires an unlocked session.
pub fn snippet_convert_to_sensitive(
    conn: &Connection,
    session: &VaultSession,
    id: &str,
    now: i64,
) -> Result<SnippetDto, IpcError> {
    let repo = SnippetRepo::new(conn);
    let mut snippet = repo.get(id)?.ok_or_else(IpcError::not_found)?;
    let plaintext = match &snippet.content {
        SnippetContent::Plaintext(text) => text.clone(),
        SnippetContent::Ciphertext(_) => {
            return Err(IpcError::conflict("snippet is already sensitive"));
        }
    };
    let ciphertext = session.encrypt_content(conn, id, plaintext.as_bytes())?;
    snippet.content = SnippetContent::Ciphertext(ciphertext);
    snippet.snippet_type = SnippetType::Sensitive;
    snippet.security_level = SecurityLevel::Sensitive;
    snippet.updated_at = now;
    snippet.version += 1;
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).update(&snippet)?;
    let versions = VersionRepo::new(&tx);
    versions.append(&history_entry(&snippet, now))?;
    // Keep only the newest (now ciphertext) version; drop the plaintext history.
    versions.prune_versions(id, 1)?;
    notify_sync(&tx, SyncEntityType::Snippet, id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    let index = SearchIndex::new(conn);
    index.sync_snippet(id)?;
    index.optimize()?;
    // The row, its history and its index entry no longer hold the plaintext —
    // but the pages they used to live on still do, and so does the WAL. The
    // reader was promised the plaintext is gone; "gone" has to mean gone from
    // the files, not gone from the queries.
    purge_free_pages(conn)?;
    Ok(snippet.into())
}

// ===== Sync conflict adjudication =====

/// Lists the conflict pairs still awaiting a decision: every live conflict
/// copy together with the snippet it points at. A copy whose source is gone
/// (the user deleted it meanwhile) is not a decision any more — the marker
/// is cleared so the copy becomes an ordinary snippet instead of a row that
/// can never be resolved.
pub fn sync_conflict_list(conn: &Connection, now: i64) -> Result<Vec<ConflictPairDto>, IpcError> {
    let repo = SnippetRepo::new(conn);
    let mut pairs = Vec::new();
    for copy in repo.list_conflict_copies()? {
        let Some(source_id) = copy.conflict_of.clone() else {
            continue;
        };
        match repo.get(&source_id)? {
            Some(source) if source.deleted_at.is_none() => {
                let sensitive = source.security_level == SecurityLevel::Sensitive
                    || copy.security_level == SecurityLevel::Sensitive;
                pairs.push(ConflictPairDto {
                    source: source.into(),
                    copy: copy.into(),
                    sensitive,
                });
            }
            _ => clear_conflict_marker(conn, &copy.id, now)?,
        }
    }
    Ok(pairs)
}

/// Clears the marker and lets the change reach the account, so the other
/// devices stop offering the same decision.
fn clear_conflict_marker(conn: &Connection, copy_id: &str, now: i64) -> Result<(), IpcError> {
    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    SnippetRepo::new(&tx).clear_conflict_of(copy_id)?;
    notify_sync(&tx, SyncEntityType::Snippet, copy_id, None, now)?;
    tx.commit().map_err(RepoError::from)?;
    Ok(())
}

/// Resolves one conflict pair. The losing body is never destroyed: keeping
/// one side moves the other into the recycle bin (restorable, and propagated
/// as a tombstone), and `Both` simply drops the marker.
///
/// A sensitive pair resolved in favour of the copy needs an unlocked vault:
/// the copy's envelope is bound to the copy's record id, so its body must be
/// opened and re-sealed under the source's id rather than moved as bytes.
pub fn sync_conflict_resolve(
    conn: &Connection,
    session: &VaultSession,
    copy_id: &str,
    keep: ConflictKeep,
    now: i64,
) -> Result<(), IpcError> {
    let repo = SnippetRepo::new(conn);
    let copy = repo.get(copy_id)?.ok_or_else(IpcError::not_found)?;
    let source_id = copy
        .conflict_of
        .clone()
        .ok_or_else(|| IpcError::conflict("snippet is not a conflict copy"))?;
    let source = repo.get(&source_id)?.ok_or_else(IpcError::not_found)?;

    if keep == ConflictKeep::Copy {
        let content = match &copy.content {
            SnippetContent::Plaintext(text) => SnippetContent::Plaintext(text.clone()),
            SnippetContent::Ciphertext(envelope) => {
                let plaintext = session.decrypt_content(conn, &copy.id, envelope)?;
                SnippetContent::Ciphertext(session.encrypt_content(
                    conn,
                    &source.id,
                    plaintext.as_slice(),
                )?)
            }
        };
        let winner = Snippet {
            content,
            updated_at: now,
            version: source.version.saturating_add(1),
            ..source
        };
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        SnippetRepo::new(&tx).update(&winner)?;
        let versions = VersionRepo::new(&tx);
        versions.append(&history_entry(&winner, now))?;
        versions.apply_retention(&winner.id, now)?;
        notify_sync(&tx, SyncEntityType::Snippet, &winner.id, None, now)?;
        tx.commit().map_err(RepoError::from)?;
        SearchIndex::new(conn).sync_snippet(&winner.id)?;
    }

    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
    let repo = SnippetRepo::new(&tx);
    repo.clear_conflict_of(copy_id)?;
    match keep {
        ConflictKeep::Both => {
            notify_sync(&tx, SyncEntityType::Snippet, copy_id, None, now)?;
        }
        ConflictKeep::Source | ConflictKeep::Copy => {
            repo.soft_delete(copy_id, now)?;
            notify_sync(&tx, SyncEntityType::Snippet, copy_id, Some(now), now)?;
        }
    }
    tx.commit().map_err(RepoError::from)?;
    SearchIndex::new(conn).sync_snippet(copy_id)?;
    Ok(())
}

/// Re-indexes the snippets a pulled sync round touched. The search index
/// lives above `crates/sync` in the dependency order, so applying records
/// cannot maintain it — the host does, right after the round.
pub fn reindex_snippets(conn: &Connection, snippet_ids: &[String]) -> Result<(), IpcError> {
    let index = SearchIndex::new(conn);
    for id in snippet_ids {
        index.sync_snippet(id)?;
    }
    Ok(())
}

/// Loads the stable per-install device id, creating it on first run. The id
/// is a random UUID carrying no user data; it lives next to the database and
/// must never be logged.
pub fn load_or_create_device_id(data_dir: &Path) -> Result<String, IpcError> {
    let path = data_dir.join(DEVICE_ID_FILE_NAME);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let id = existing.trim();
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    let id = new_id();
    std::fs::write(&path, &id).map_err(|_| IpcError::system())?;
    Ok(id)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::IpcErrorCode;

    fn espanso_snippet(
        title: &str,
        trigger: &str,
        level: SecurityLevel,
        content: SnippetContent,
    ) -> Snippet {
        Snippet {
            id: new_id(),
            workspace_id: WORKSPACE_ID.to_string(),
            title: title.to_string(),
            content,
            snippet_type: match level {
                SecurityLevel::Sensitive => SnippetType::Sensitive,
                SecurityLevel::Normal => SnippetType::Text,
            },
            description: None,
            folder_id: None,
            trigger: Some(trigger.to_string()),
            trigger_mode: Some(TriggerMode::Immediate),
            language: None,
            security_level: level,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 1,
            updated_at: 1,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        }
    }
    #[test]
    fn markdown_import_creates_snippets_with_section_titles() {
        let conn = test_conn();
        let text = "## Greeting\nHello there\n\n## Address\n1 Main St\n";
        let report = snippets_import(&conn, "markdown", text, 1_000).unwrap();

        assert_eq!(report.imported, 2);
        assert!(report.conflicts.is_empty());
        assert!(report.skipped.is_empty());
        let titles: Vec<String> = SnippetRepo::new(&conn)
            .list(10, 0)
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert!(titles.contains(&"Greeting".to_string()));
        assert!(titles.contains(&"Address".to_string()));
    }

    #[test]
    fn json_import_reports_conflicts_and_skips() {
        let conn = test_conn();
        SnippetRepo::new(&conn)
            .insert(&espanso_snippet(
                "Existing",
                ":taken",
                SecurityLevel::Normal,
                SnippetContent::Plaintext("old".to_string()),
            ))
            .unwrap();

        let text = r#"[
            {"title": "Sig", "content": "Best regards", "trigger": ":sig", "word": true},
            {"content": "conflict body", "trigger": ":taken"},
            {"title": "NoBody"}
        ]"#;
        let report = snippets_import(&conn, "json", text, 1_000).unwrap();

        assert_eq!(report.imported, 1);
        assert_eq!(report.conflicts, vec![":taken".to_string()]);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].label.as_deref(), Some("NoBody"));

        // The imported snippet is real, keeps its word mode, and is indexed.
        let imported = SnippetRepo::new(&conn)
            .list_triggered_active()
            .unwrap()
            .into_iter()
            .find(|s| s.trigger.as_deref() == Some(":sig"))
            .expect("imported snippet exists");
        assert_eq!(imported.title, "Sig");
        assert_eq!(imported.trigger_mode, Some(TriggerMode::WordBoundary));
    }

    #[test]
    fn csv_import_parses_quoted_fields_and_derives_titles() {
        let conn = test_conn();
        let text = "content,trigger\n\"line one\nline two\",:multi\n";
        let report = snippets_import(&conn, "csv", text, 1_000).unwrap();

        assert_eq!(report.imported, 1);
        let imported = SnippetRepo::new(&conn)
            .list_triggered_active()
            .unwrap()
            .into_iter()
            .find(|s| s.trigger.as_deref() == Some(":multi"))
            .expect("imported snippet exists");
        // No title column: the title falls back to the first content line.
        assert_eq!(imported.title, "line one");
        assert_eq!(
            imported.content,
            SnippetContent::Plaintext("line one\nline two".to_string())
        );
    }

    #[test]
    fn masscode_import_creates_titled_snippets_and_reports_trash() {
        let conn = test_conn();
        let text = r#"{"snippets": [
            {"name": "Greeting", "content": [{"label": "F1", "value": "Hello"}]},
            {"name": "Old", "content": [{"value": "gone"}], "isDeleted": true}
        ]}"#;
        let report = snippets_import(&conn, "masscode", text, 1_000).unwrap();

        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, "in massCode trash");
        let titles: Vec<String> = SnippetRepo::new(&conn)
            .list(10, 0)
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(titles, vec!["Greeting".to_string()]);
    }

    #[test]
    fn copyq_import_derives_titles_from_item_text() {
        let conn = test_conn();
        let report = snippets_import(&conn, "copyq", r#"["ssh admin@host", ""]"#, 1_000).unwrap();

        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped.len(), 1);
        let titles: Vec<String> = SnippetRepo::new(&conn)
            .list(10, 0)
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(titles, vec!["ssh admin@host".to_string()]);
    }

    #[test]
    fn backup_round_trips_a_full_library_into_a_fresh_db() {
        let conn = test_conn();
        // Library: a triggered snippet with an edit (2 history entries), a
        // trashed snippet, a tag link, and a vault secret.
        let kept = snippet_create(&conn, create_input("Greeting", Some(";g")), 1).unwrap();
        snippet_update(
            &conn,
            SnippetUpdateInput {
                id: kept.id.clone(),
                title: "Greeting".to_string(),
                body: "hello v2".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";g".to_string()),
                trigger_mode: Some("immediate".to_string()),
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
            },
            2,
        )
        .unwrap();
        let trashed = snippet_create(&conn, create_input("Old note", None), 3).unwrap();
        SnippetRepo::new(&conn).soft_delete(&trashed.id, 4).unwrap();
        let tag = Tag {
            id: "t1".to_string(),
            name: "work".to_string(),
            created_at: 1,
        };
        TagRepo::new(&conn).insert(&tag).unwrap();
        SnippetRepo::new(&conn)
            .batch_add_tag(std::slice::from_ref(&kept.id), &tag.id)
            .unwrap();
        let session = unlocked_session(&conn);
        let marker = "AKIA_FAKE_BACKUP_MARKER_9Q7";
        let secret =
            vault_create_secret(&conn, &session, secret_input("Cloud key", marker), 5).unwrap();

        let text = backup_export(&conn, "backup passphrase", 9_000).unwrap();

        // Sensitive red line: the marker never appears in the backup, neither
        // in the file text nor in the decoded sealed bytes.
        assert!(!text.contains(marker), "sensitive plaintext leaked");
        assert!(!text.contains("Greeting"), "document is not sealed");

        let dest = test_conn();
        let report = backup_restore(&dest, "backup passphrase", &text, 9_000).unwrap();
        assert_eq!(report.snippets, 3);
        assert_eq!(report.tags, 1);
        assert!(report.vault_restored);

        // Fidelity: live rows, trash, tag link, history, and search index.
        let dest_repo = SnippetRepo::new(&dest);
        assert_eq!(dest_repo.list(10, 0).unwrap().len(), 2);
        assert_eq!(dest_repo.list_trashed(10, 0).unwrap().len(), 1);
        assert_eq!(dest_repo.tag_ids_of(&kept.id).unwrap(), vec![tag.id]);
        assert_eq!(
            VersionRepo::new(&dest).list(&kept.id, 10, 0).unwrap().len(),
            2
        );
        assert_eq!(search_snippets(&dest, "greeting", 10, 0).unwrap().len(), 1);

        // The vault survives end to end: the original master password unlocks
        // the restored library and decrypts the secret.
        let mut restored_session = VaultSession::new();
        vault_unlock_password(
            &dest,
            &mut restored_session,
            "correct horse battery staple",
            VAULT_NOW,
        )
        .unwrap();
        assert_eq!(
            vault_reveal(&dest, &mut restored_session, &secret.id, VAULT_NOW).unwrap(),
            marker
        );
    }

    #[test]
    fn backup_restore_rejects_a_wrong_passphrase_without_saying_why() {
        let conn = test_conn();
        snippet_create(&conn, create_input("Doc", None), 1).unwrap();
        let text = backup_export(&conn, "right", 2).unwrap();

        let dest = test_conn();
        let err = backup_restore(&dest, "wrong", &text, 9_000).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::PermissionDenied);
        // Nothing landed in the target library.
        assert!(SnippetRepo::new(&dest).list(10, 0).unwrap().is_empty());
    }

    #[test]
    fn backup_restore_requires_an_empty_library() {
        let conn = test_conn();
        snippet_create(&conn, create_input("Doc", None), 1).unwrap();
        let text = backup_export(&conn, "pw", 2).unwrap();

        let err = backup_restore(&conn, "pw", &text, 9_000).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
    }

    #[test]
    fn snippets_import_rejects_unknown_formats_and_malformed_input() {
        let conn = test_conn();
        let unknown = snippets_import(&conn, "yaml", "matches: []", 1).unwrap_err();
        assert_eq!(unknown.code, IpcErrorCode::Validation);
        let malformed = snippets_import(&conn, "json", "{not json", 1).unwrap_err();
        assert_eq!(malformed.code, IpcErrorCode::Validation);
        // Nothing was written by the failed attempts.
        assert!(SnippetRepo::new(&conn).list(10, 0).unwrap().is_empty());
    }

    fn template_snippet(conn: &Connection, id: &str, body: &str) {
        SnippetRepo::new(conn)
            .insert(&Snippet {
                id: id.to_string(),
                workspace_id: WORKSPACE_ID.to_string(),
                title: "Bug report".to_string(),
                content: SnippetContent::Plaintext(body.to_string()),
                snippet_type: SnippetType::Template,
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                security_level: SecurityLevel::Normal,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
                platform_scope: vec![],
                created_at: 1,
                updated_at: 1,
                last_used_at: None,
                usage_count: 0,
                version: 1,
                deleted_at: None,
                conflict_of: None,
            })
            .unwrap();
    }

    fn field_dto(name: &str, field_type: &str) -> TemplateFieldDto {
        TemplateFieldDto {
            id: String::new(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: field_type.to_string(),
            default_value: None,
            options: vec![],
            validation: None,
            is_required: false,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    #[test]
    fn template_variables_lists_body_placeholders_in_order() {
        assert_eq!(
            template_variables("### {{module}} · {{severity}}").unwrap(),
            vec!["module".to_string(), "severity".to_string()]
        );
    }

    #[test]
    fn template_save_and_load_round_trips_fields() {
        let conn = test_conn();
        template_snippet(&conn, "s1", "### {{module}}");
        let mut module = field_dto("module", "single_line_text");
        module.default_value = Some("auth-service".to_string());
        module.is_required = true;

        let saved = template_save_fields(&conn, "s1", vec![module], 9_000).unwrap();
        assert_eq!(saved.len(), 1);
        assert!(!saved[0].id.is_empty(), "repo assigns an id");

        let loaded = template_fields(&conn, "s1").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "module");
        assert_eq!(loaded[0].default_value, Some("auth-service".to_string()));
    }

    #[test]
    fn template_save_rejects_a_choice_field_without_options() {
        let conn = test_conn();
        template_snippet(&conn, "s1", "{{severity}}");
        let choice = field_dto("severity", "single_select");
        let err = template_save_fields(&conn, "s1", vec![choice], 9_000).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    #[test]
    fn template_preview_masks_a_secret_reference_and_never_its_value() {
        let secret = field_dto("api_key", "secret_ref");
        let mut values = std::collections::HashMap::new();
        values.insert("api_key".to_string(), "vault-ref-42".to_string());

        let out = template_preview("key: {{api_key}}", vec![secret], values).unwrap();
        assert!(out.contains('•'), "secret is masked: {out}");
        assert!(
            !out.contains("vault-ref-42"),
            "preview leaked a secret reference: {out}"
        );
    }

    #[test]
    fn template_preview_shows_defaults_and_placeholders() {
        let mut with_default = field_dto("env", "single_line_text");
        with_default.default_value = Some("prod".to_string());
        let bare = field_dto("module", "single_line_text");

        let out = template_preview(
            "{{env}}/{{module}}",
            vec![with_default, bare],
            std::collections::HashMap::new(),
        )
        .unwrap();
        assert_eq!(out, "prod/‹module›");
    }

    fn fill(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn template_render_rejects_a_missing_required_field() {
        let conn = test_conn();
        template_snippet(&conn, "s1", "{{module}}");
        let mut module = field_dto("module", "single_line_text");
        module.is_required = true;
        template_save_fields(&conn, "s1", vec![module], 9_000).unwrap();
        let err = template_render(&conn, "s1", &fill(&[])).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    #[test]
    fn template_render_defers_secret_fields_to_the_vault() {
        let conn = test_conn();
        template_snippet(&conn, "s1", "{{api_key}}");
        template_save_fields(&conn, "s1", vec![field_dto("api_key", "secret_ref")], 9_000).unwrap();
        let err = template_render(&conn, "s1", &fill(&[("api_key", "ref-1")])).unwrap_err();
        // Secret dereference needs the vault; the template cannot be injected yet.
        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    fn test_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    /// Records every sync change announcement, asserting it arrives inside
    /// the write transaction (seal-at-write).
    struct RecordingObserver {
        seen: std::sync::Mutex<Vec<(SyncEntityType, String, Option<i64>)>>,
    }

    impl typvia_core::sync_hooks::ChangeObserver for RecordingObserver {
        fn entity_changed(
            &self,
            conn: &Connection,
            change: &EntityChange,
        ) -> Result<(), typvia_core::sync_hooks::HookError> {
            assert!(
                !conn.is_autocommit(),
                "sync hook must run inside the write transaction"
            );
            self.seen.lock().unwrap().push((
                change.entity_type,
                change.entity_id.clone(),
                change.deleted_at,
            ));
            Ok(())
        }
    }

    #[test]
    fn write_paths_announce_sync_changes_inside_their_transactions() {
        let conn = test_conn();
        let observer = std::sync::Arc::new(RecordingObserver {
            seen: std::sync::Mutex::new(Vec::new()),
        });
        let _guard = typvia_core::sync_hooks::register_observer(&conn, observer.clone());

        let created = snippet_create(&conn, create_input("Sync me", None), 1_000).unwrap();
        snippet_trash(&conn, &created.id, 2_000).unwrap();
        snippet_restore(&conn, &created.id, 3_000).unwrap();
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                parent_id: None,
                name: "Work".to_string(),
                sort_order: 0,
            },
            4_000,
        )
        .unwrap();
        folder_delete(&conn, &folder.id, 5_000).unwrap();
        let tag = tag_create(&conn, "urgent".to_string(), 6_000).unwrap();
        snippet_batch_add_tag(&conn, std::slice::from_ref(&created.id), &tag.id, 7_000).unwrap();

        let seen = observer.seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            vec![
                (SyncEntityType::Snippet, created.id.clone(), None),
                (SyncEntityType::Snippet, created.id.clone(), Some(2_000)),
                (SyncEntityType::Snippet, created.id.clone(), None),
                (SyncEntityType::Folder, folder.id.clone(), None),
                (SyncEntityType::Folder, folder.id.clone(), Some(5_000)),
                (SyncEntityType::Tag, tag.id.clone(), None),
                (
                    SyncEntityType::SnippetTag,
                    format!("{}/{}", created.id, tag.id),
                    None
                ),
            ]
        );
    }

    #[test]
    fn a_failing_sync_hook_aborts_the_write() {
        struct Failing;
        impl typvia_core::sync_hooks::ChangeObserver for Failing {
            fn entity_changed(
                &self,
                _conn: &Connection,
                _change: &EntityChange,
            ) -> Result<(), typvia_core::sync_hooks::HookError> {
                Err(typvia_core::sync_hooks::HookError::new(
                    std::io::Error::other("outbox unavailable"),
                ))
            }
        }
        let conn = test_conn();
        let _guard =
            typvia_core::sync_hooks::register_observer(&conn, std::sync::Arc::new(Failing));
        let err = snippet_create(&conn, create_input("Doomed", None), 1_000).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::System);
        // Fail-closed: the write rolled back with the failed enqueue.
        assert!(SnippetRepo::new(&conn).list(10, 0).unwrap().is_empty());
    }

    const VAULT_NOW: i64 = 1_700_000_000_000;

    fn unlocked_session(conn: &Connection) -> VaultSession {
        let mut session = VaultSession::new();
        session
            .initialize(conn, b"correct horse battery staple", VAULT_NOW)
            .unwrap();
        session
    }

    fn secret_input(title: &str, body: &str) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        }
    }

    #[test]
    fn vault_create_secret_stores_ciphertext_only_and_hides_the_body() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        let dto = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod read replica", "postgres://readonly@10.4.2.19:5432"),
            VAULT_NOW,
        )
        .unwrap();
        // The DTO never carries a sensitive body across IPC.
        assert_eq!(dto.body, None);
        assert_eq!(dto.security_level, "sensitive");
        // At rest: ciphertext column set, plaintext column NULL (the CHECK also
        // enforces this, but assert the write layer honoured it).
        let (plaintext, has_cipher): (Option<String>, bool) = conn
            .query_row(
                "SELECT content_plaintext, content_ciphertext IS NOT NULL FROM snippet WHERE id = ?1",
                [&dto.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(plaintext, None);
        assert!(has_cipher);
    }

    #[test]
    fn vault_reveal_round_trips_the_plaintext_only_when_unlocked() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let secret = "sk_test_51NfEXAMPLEONLY";
        let dto = vault_create_secret(&conn, &session, secret_input("Stripe", secret), VAULT_NOW)
            .unwrap();
        assert_eq!(
            vault_reveal(&conn, &mut session, &dto.id, VAULT_NOW).unwrap(),
            secret
        );
        // Once locked, revealing is refused (permission denied).
        session.lock();
        let err = vault_reveal(&conn, &mut session, &dto.id, VAULT_NOW).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::PermissionDenied);
    }

    #[test]
    fn vault_create_secret_requires_an_unlocked_session() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        session.lock();
        let err = vault_create_secret(&conn, &session, secret_input("Nope", "secret"), VAULT_NOW)
            .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::PermissionDenied);
    }

    #[test]
    fn vault_list_returns_only_sensitive_snippets() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        snippet_create(
            &conn,
            SnippetCreateInput {
                title: "Normal".to_string(),
                body: "hello".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            VAULT_NOW,
        )
        .unwrap();
        vault_create_secret(&conn, &session, secret_input("Secret", "shh"), VAULT_NOW).unwrap();

        let listed = vault_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Secret");
        assert_eq!(listed[0].security_level, "sensitive");
        assert_eq!(listed[0].body, None);
    }

    #[test]
    fn library_list_count_and_search_exclude_sensitive_snippets() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        snippet_create(
            &conn,
            SnippetCreateInput {
                title: "Email".to_string(),
                body: "hello".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            VAULT_NOW,
        )
        .unwrap();
        vault_create_secret(&conn, &session, secret_input("APIKEY", "shh"), VAULT_NOW).unwrap();

        // Library browse, count and search never surface the secret…
        let page = snippet_list_page(&conn, "all", None, None, None, 50, 0).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].title, "Email");
        assert_eq!(snippet_count(&conn, "all", None, None).unwrap(), 1);
        assert!(search_library(&conn, "APIKEY", 50).unwrap().is_empty());

        // …while the panel's search path still does (that is how secrets
        // are called), and the vault list remains intact.
        let panel_hits = search_snippets(&conn, "APIKEY", 50, 0).unwrap();
        assert_eq!(panel_hits.len(), 1);
        assert_eq!(vault_list(&conn, 50, 0).unwrap().len(), 1);
    }

    #[test]
    fn panel_results_keeps_vault_rows_reachable_where_the_library_hides_them() {
        // Regression: panel_results reused the Library search and
        // silently lost vault rows, leaving the panel's verify-then-insert
        // flow unreachable. Both panel modes must list the locked row.
        let conn = test_conn();
        let session = unlocked_session(&conn);
        let secret =
            vault_create_secret(&conn, &session, secret_input("APIKEY", "shh"), VAULT_NOW).unwrap();

        // Title search reaches the locked row; the row carries no content.
        let hits = panel_results(&conn, "APIKEY", 50, None, None).unwrap();
        assert_eq!(hits.rows.len(), 1);
        assert_eq!(hits.rows[0].title, "APIKEY");
        assert_eq!(hits.rows[0].security_level, "sensitive");
        assert_eq!(hits.rows[0].body, None);

        // Once used, the secret shows up in the empty-query recent list too.
        SnippetRepo::new(&conn)
            .record_usage(&secret.id, VAULT_NOW + 1)
            .unwrap();
        let recents = panel_results(&conn, "", 50, None, None).unwrap();
        assert!(recents.rows.iter().any(|row| row.id == secret.id));

        // The Library search stays sensitive-free.
        assert!(search_library(&conn, "APIKEY", 50).unwrap().is_empty());
    }

    #[test]
    fn search_all_lists_locked_vault_titles_without_content() {
        // Mobile Search screen: vault titles surface as locked
        // rows — body None — where search_library keeps hiding the row.
        let conn = test_conn();
        let session = unlocked_session(&conn);
        vault_create_secret(&conn, &session, secret_input("APIKEY", "shh"), VAULT_NOW).unwrap();

        let hits = search_all(&conn, "APIKEY", 50).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "APIKEY");
        assert_eq!(hits[0].security_level, "sensitive");
        assert_eq!(hits[0].body, None);
        assert!(search_library(&conn, "APIKEY", 50).unwrap().is_empty());
    }

    /// In-memory secure store for the reset tests: no biometric gate, just a
    /// map (the real gate is the host live test).
    #[derive(Default)]
    struct MapStore {
        entries: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
    }

    impl SecureStore for MapStore {
        fn store(
            &self,
            entry: &str,
            secret: &[u8],
        ) -> Result<(), typvia_core::vault::SecureStoreError> {
            self.entries
                .lock()
                .unwrap()
                .insert(entry.to_string(), secret.to_vec());
            Ok(())
        }

        fn retrieve(
            &self,
            entry: &str,
        ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, typvia_core::vault::SecureStoreError>
        {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .get(entry)
                .map(|v| zeroize::Zeroizing::new(v.clone())))
        }

        fn remove(&self, entry: &str) -> Result<(), typvia_core::vault::SecureStoreError> {
            self.entries.lock().unwrap().remove(entry);
            Ok(())
        }
    }

    #[test]
    fn vault_reset_destroys_secrets_keys_and_biometric_copy_with_tombstones() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let store = MapStore::default();
        vault_enable_biometric(&session, &store).unwrap();

        let kept = snippet_create(&conn, create_input("Normal note", None), VAULT_NOW).unwrap();
        let active =
            vault_create_secret(&conn, &session, secret_input("APIKEY", "shh-1"), VAULT_NOW)
                .unwrap();
        let trashed = vault_create_secret(
            &conn,
            &session,
            secret_input("Old token", "shh-2"),
            VAULT_NOW,
        )
        .unwrap();
        snippet_trash(&conn, &trashed.id, VAULT_NOW).unwrap();

        // The preview counts trashed secrets too — the confirm copy must be
        // the whole truth.
        assert_eq!(vault_reset_preview(&conn).unwrap(), 2);

        let observer = std::sync::Arc::new(RecordingObserver {
            seen: std::sync::Mutex::new(Vec::new()),
        });
        let _guard = typvia_core::sync_hooks::register_observer(&conn, observer.clone());

        let now = VAULT_NOW + 10;
        let status = vault_reset(&conn, &mut session, &store, now).unwrap();
        assert!(!status.initialized);
        assert!(!status.unlocked);
        assert!(!session.is_unlocked());

        // Both secrets tombstoned so peers converge on the deletion.
        let mut seen = observer.seen.lock().unwrap().clone();
        seen.sort_by(|a, b| a.1.cmp(&b.1));
        let mut expected = vec![
            (SyncEntityType::Snippet, active.id.clone(), Some(now)),
            (SyncEntityType::Snippet, trashed.id.clone(), Some(now)),
        ];
        expected.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(seen, expected);

        // Rows, key material and the biometric copy are all gone; the normal
        // snippet is untouched.
        let secrets: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM snippet WHERE security_level = 'sensitive'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(secrets, 0);
        let headers: i64 = conn
            .query_row("SELECT COUNT(*) FROM key_header", [], |r| r.get(0))
            .unwrap();
        assert_eq!(headers, 0);
        let domain_keys: i64 = conn
            .query_row("SELECT COUNT(*) FROM domain_key", [], |r| r.get(0))
            .unwrap();
        assert_eq!(domain_keys, 0);
        assert!(
            store
                .retrieve(typvia_core::vault::BIOMETRIC_MK_ENTRY)
                .unwrap()
                .is_none()
        );
        assert_eq!(snippet_get(&conn, &kept.id).unwrap().title, "Normal note");

        // Resetting twice is a conflict; setting up fresh works.
        let err = vault_reset(&conn, &mut session, &store, now + 1).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        vault_initialize(&conn, &mut session, "a brand new password", now + 2).unwrap();
        assert!(session.is_unlocked());
    }

    #[test]
    fn vault_reset_leaves_no_ciphertext_or_wrapped_key_bytes_in_the_db_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault-reset.db");
        let conn = {
            let mut conn = typvia_core::db::open(&path).unwrap();
            typvia_core::db::migrate_to_latest(&mut conn).unwrap();
            conn
        };
        let mut session = unlocked_session(&conn);
        let store = MapStore::default();
        let dto = vault_create_secret(
            &conn,
            &session,
            secret_input("Canary", "shh-canary"),
            VAULT_NOW,
        )
        .unwrap();

        // 32-byte windows of the at-rest ciphertext and the wrapped MK: if any
        // survive the reset in the file bytes, the free pages leaked them.
        let cipher: Vec<u8> = conn
            .query_row(
                "SELECT content_ciphertext FROM snippet WHERE id = ?1",
                [&dto.id],
                |r| r.get(0),
            )
            .unwrap();
        let wrapped_mk: Vec<u8> = conn
            .query_row("SELECT wrapped_mk FROM key_header", [], |r| r.get(0))
            .unwrap();
        let windows: Vec<&[u8]> = vec![&cipher[..32], &wrapped_mk[..32]];

        vault_reset(&conn, &mut session, &store, VAULT_NOW + 10).unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        for suffix in ["-wal", "-shm"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            if let Ok(more) = std::fs::read(std::path::PathBuf::from(name)) {
                bytes.extend_from_slice(&more);
            }
        }
        for window in windows {
            assert!(
                !bytes.windows(window.len()).any(|w| w == window),
                "destroyed key material or ciphertext still present in the database file"
            );
        }
    }

    #[test]
    fn disabled_snippets_stay_in_the_library_but_leave_the_calling_surface() {
        let conn = test_conn();
        let kept = snippet_create(&conn, create_input("Enabled note", None), 1_000).unwrap();
        let parked = snippet_create(&conn, create_input("Archived note", None), 1_000).unwrap();
        snippet_update(
            &conn,
            SnippetUpdateInput {
                id: parked.id.clone(),
                title: "Archived note".to_string(),
                body: "still here".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: false,
            },
            2_000,
        )
        .unwrap();

        // The Library keeps it findable (an archived snippet must stay
        // recoverable)…
        let listed = snippet_list_page(&conn, "all", None, None, None, 50, 0).unwrap();
        assert!(listed.iter().any(|s| s.id == parked.id));
        assert!(
            search_library(&conn, "Archived", 50)
                .unwrap()
                .iter()
                .any(|s| s.id == parked.id)
        );

        // …while the panel never offers it for insertion.
        let panel = panel_results(&conn, "note", 50, None, None).unwrap();
        assert!(panel.rows.iter().any(|r| r.id == kept.id));
        assert!(
            !panel.rows.iter().any(|r| r.id == parked.id),
            "disabled snippets must not enter the calling surface"
        );
    }

    #[test]
    fn temporary_snippets_expire_into_the_trash_with_tombstones() {
        let conn = test_conn();
        // A temporary snippet edited long ago, a fresh one, and a normal one.
        let mut stale = espanso_snippet(
            "Stale scratch",
            ":stale",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("old".to_string()),
        );
        stale.snippet_type = SnippetType::Temporary;
        stale.updated_at = 1_000;
        SnippetRepo::new(&conn).insert(&stale).unwrap();
        let mut fresh = espanso_snippet(
            "Fresh scratch",
            ":fresh",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("new".to_string()),
        );
        fresh.snippet_type = SnippetType::Temporary;
        fresh.id = new_id();
        fresh.updated_at = 1_000 + TEMPORARY_TTL_MS;
        SnippetRepo::new(&conn).insert(&fresh).unwrap();
        let normal = snippet_create(&conn, create_input("Keeper", None), 1_000).unwrap();

        let observer = std::sync::Arc::new(RecordingObserver {
            seen: std::sync::Mutex::new(Vec::new()),
        });
        let _guard = typvia_core::sync_hooks::register_observer(&conn, observer.clone());

        let now = 1_000 + TEMPORARY_TTL_MS + 1;
        assert_eq!(temporary_expire(&conn, now).unwrap(), 1);

        // Only the stale temporary moved; it is in the bin with a tombstone.
        assert!(
            SnippetRepo::new(&conn)
                .get(&stale.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_some()
        );
        assert!(
            SnippetRepo::new(&conn)
                .get(&fresh.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
        assert!(
            SnippetRepo::new(&conn)
                .get(&normal.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
        let seen = observer.seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            vec![(SyncEntityType::Snippet, stale.id.clone(), Some(now))]
        );

        // Restoring restarts the expiry window: the next sweep
        // leaves it alone.
        snippet_restore(&conn, &stale.id, now + 10).unwrap();
        assert_eq!(temporary_expire(&conn, now + 20).unwrap(), 0);
        assert!(
            SnippetRepo::new(&conn)
                .get(&stale.id)
                .unwrap()
                .unwrap()
                .deleted_at
                .is_none()
        );
    }

    #[test]
    fn converting_to_sensitive_encrypts_and_purges_plaintext_from_index_and_history() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        // A normal snippet whose body carries a token unique to the content.
        let token = "correcthorsebatterystaplexyz";
        let created = snippet_create(
            &conn,
            SnippetCreateInput {
                title: "API key".to_string(),
                body: format!("value {token}"),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            VAULT_NOW,
        )
        .unwrap();
        // Editing it once so there is a plaintext history row to purge.
        snippet_update(
            &conn,
            SnippetUpdateInput {
                id: created.id.clone(),
                title: "API key".to_string(),
                body: format!("value {token} v2"),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
            },
            VAULT_NOW,
        )
        .unwrap();

        let converted =
            snippet_convert_to_sensitive(&conn, &session, &created.id, VAULT_NOW).unwrap();
        assert_eq!(converted.security_level, "sensitive");
        assert_eq!(converted.body, None);

        // The former plaintext is no longer reachable through content search.
        let hits = search_snippets(&conn, token, 20, 0).unwrap();
        assert!(
            hits.iter().all(|h| h.snippet_id != created.id),
            "converted snippet still searchable by its old plaintext"
        );

        // No version row retains the plaintext token (history was pruned).
        let plaintext_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM snippet_version \
                 WHERE snippet_id = ?1 AND content_plaintext IS NOT NULL",
                [&created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plaintext_rows, 0, "plaintext version history survived");

        // And it now reads back through the vault.
        let mut session = session;
        assert_eq!(
            vault_reveal(&conn, &mut session, &created.id, VAULT_NOW).unwrap(),
            format!("value {token} v2")
        );
    }

    fn create_input(title: &str, trigger: Option<&str>) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: format!("{title} body"),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: trigger.map(str::to_string),
            trigger_mode: trigger.map(|_| "delimiter".to_string()),
            language: None,
        }
    }

    /// A host that delivered something says so, and the library remembers.
    ///
    /// This is what the recall shelf is built from. Two hosts had no way to
    /// say it at all, so their shelves were empty forever.
    #[test]
    fn a_delivered_snippet_is_counted_and_dated() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Docker tail logs", None), 1).unwrap();
        assert_eq!(created.usage_count, 0);
        assert_eq!(created.last_used_at, None);

        snippet_record_use(&conn, &created.id, 1_700_000_000_000).unwrap();
        snippet_record_use(&conn, &created.id, 1_700_000_060_000).unwrap();

        let seen = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(seen.usage_count, 2);
        // The latest use, not the first: "last used" is a fact about the most
        // recent time, and a shelf ordered by a stale one is ordered wrongly.
        assert_eq!(seen.last_used_at, Some(1_700_000_060_000));
    }

    #[test]
    fn a_use_of_something_that_is_not_there_is_refused_rather_than_ignored() {
        let conn = test_conn();

        let refused = snippet_record_use(&conn, "no-such-id", 1_700_000_000_000);

        assert!(refused.is_err());
    }

    /// Counting a use must not disturb what the snippet is. The two writes
    /// compete for the same row, and the light one must never be the reason a
    /// body changed.
    #[test]
    fn counting_a_use_changes_nothing_else_about_the_snippet() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Rollback", Some(";rb")), 1).unwrap();

        snippet_record_use(&conn, &created.id, 1_700_000_000_000).unwrap();

        let seen = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(seen.title, created.title);
        assert_eq!(seen.body, created.body);
        assert_eq!(seen.trigger, created.trigger);
        assert_eq!(seen.version, created.version);
    }

    #[test]
    fn create_then_query_roundtrip_via_list_get_and_search() {
        let conn = test_conn();
        let created =
            snippet_create(&conn, create_input("Docker tail logs", Some(";dl")), 1).unwrap();
        assert_eq!(created.security_level, "normal");
        assert_eq!(created.body.as_deref(), Some("Docker tail logs body"));

        let fetched = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(fetched.title, "Docker tail logs");

        let listed = snippet_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 1);

        // The create path must have indexed the snippet: search finds it.
        let hits = search_snippets(&conn, "docker", 10, 0).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet_id, created.id);
        assert!(!hits[0].is_sensitive);
    }

    /// A folder is named or it is not made. The mobile editor now creates
    /// them inline, so whatever was typed arrives here — including nothing.
    #[test]
    fn a_folder_is_named_and_the_name_is_trimmed() {
        let conn = test_conn();

        let blank = folder_create(
            &conn,
            FolderCreateInput {
                name: "   ".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap_err();
        assert_eq!(blank.code, IpcErrorCode::Validation);

        let made = folder_create(
            &conn,
            FolderCreateInput {
                name: "  上线  ".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();
        // One folder to a reader is one folder in the database.
        assert_eq!(made.name, "上线");
        assert_eq!(folder_list_children(&conn, None).unwrap().len(), 1);
    }

    #[test]
    fn renaming_a_folder_to_nothing_leaves_its_name_alone() {
        let conn = test_conn();
        let made = folder_create(
            &conn,
            FolderCreateInput {
                name: "上线".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();

        let err = folder_update(
            &conn,
            FolderUpdateInput {
                id: made.id.clone(),
                name: String::new(),
                parent_id: None,
                sort_order: 0,
            },
            2,
        )
        .unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Validation);
        assert_eq!(folder_list_children(&conn, None).unwrap()[0].name, "上线");
    }

    fn folder_named(conn: &Connection, name: &str, parent: Option<&str>) -> FolderDto {
        folder_create(
            conn,
            FolderCreateInput {
                name: name.to_string(),
                parent_id: parent.map(str::to_string),
                sort_order: 0,
            },
            1,
        )
        .unwrap()
    }

    /// Merging moves the reader's snippets and nothing else about them: the
    /// triggers stay, the source is gone, and a folder that lived inside the
    /// source is not swept away with it.
    #[test]
    fn merging_a_folder_moves_its_snippets_and_keeps_what_was_inside() {
        let conn = test_conn();
        let replies = folder_named(&conn, "客服回复", None);
        let mail = folder_named(&conn, "邮件", None);
        let nested = folder_named(&conn, "英文", Some(&replies.id));
        let mut input = create_input("Order shipped", Some(";ship"));
        input.folder_id = Some(replies.id.clone());
        let moving = snippet_create(&conn, input, 1).unwrap();
        let mut stays = create_input("Refund", None);
        stays.folder_id = Some(nested.id.clone());
        let nested_snippet = snippet_create(&conn, stays, 1).unwrap();

        let moved = folder_merge(&conn, &replies.id, &mail.id, 2).unwrap();

        assert_eq!(moved, vec![moving.id.clone()]);
        let after = snippet_get(&conn, &moving.id).unwrap();
        assert_eq!(after.folder_id.as_deref(), Some(mail.id.as_str()));
        assert_eq!(after.trigger.as_deref(), Some(";ship"));
        let top: Vec<String> = folder_list_children(&conn, None)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(top, vec!["英文".to_string(), "邮件".to_string()]);
        assert_eq!(
            snippet_get(&conn, &nested_snippet.id)
                .unwrap()
                .folder_id
                .as_deref(),
            Some(nested.id.as_str())
        );
    }

    #[test]
    fn a_folder_merged_into_itself_or_nowhere_is_refused_and_left_alone() {
        let conn = test_conn();
        let mail = folder_named(&conn, "邮件", None);

        let itself = folder_merge(&conn, &mail.id, &mail.id, 2).unwrap_err();
        assert_eq!(itself.code, IpcErrorCode::Validation);
        let nowhere = folder_merge(&conn, &mail.id, "missing", 2).unwrap_err();
        assert_eq!(nowhere.code, IpcErrorCode::NotFound);

        assert_eq!(folder_list_children(&conn, None).unwrap().len(), 1);
    }

    #[test]
    fn reordering_folders_writes_the_order_given() {
        let conn = test_conn();
        let a = folder_named(&conn, "A", None);
        let b = folder_named(&conn, "B", None);
        let c = folder_named(&conn, "C", None);

        folder_reorder(&conn, &[c.id.clone(), a.id.clone(), b.id.clone()], 2).unwrap();

        let names: Vec<String> = folder_list_children(&conn, None)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(
            names,
            vec!["C".to_string(), "A".to_string(), "B".to_string()]
        );
    }

    /// An order that names a folder that is not there changes nothing at all.
    #[test]
    fn an_order_with_an_unknown_folder_leaves_the_old_order() {
        let conn = test_conn();
        let a = folder_named(&conn, "A", None);
        let b = folder_named(&conn, "B", None);
        folder_reorder(&conn, &[a.id.clone(), b.id.clone()], 2).unwrap();

        let err = folder_reorder(&conn, &[b.id.clone(), "missing".to_string()], 3).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::NotFound);
        let twice = folder_reorder(&conn, &[a.id.clone(), a.id.clone()], 3).unwrap_err();
        assert_eq!(twice.code, IpcErrorCode::Validation);

        let names: Vec<String> = folder_list_children(&conn, None)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, vec!["A".to_string(), "B".to_string()]);
    }

    /// The default was written down in five places across two hosts. Here it
    /// is one place, and a host that has no opinion no longer needs one.
    #[test]
    fn a_trigger_with_no_stated_mode_is_a_delimiter_trigger() {
        let conn = test_conn();
        let mut input = create_input("Greeting", None);
        input.trigger = Some(";g".to_string());
        input.trigger_mode = None;

        let created = snippet_create(&conn, input, 1).unwrap();

        assert_eq!(created.trigger_mode.as_deref(), Some("delimiter"));
    }

    #[test]
    fn a_stated_mode_is_kept() {
        let conn = test_conn();
        let mut input = create_input("Greeting", None);
        input.trigger = Some(";g".to_string());
        input.trigger_mode = Some("immediate".to_string());

        let created = snippet_create(&conn, input, 1).unwrap();

        assert_eq!(created.trigger_mode.as_deref(), Some("immediate"));
    }

    /// A field the reader left alone is no trigger — and the mode, which only
    /// exists to describe one, goes with it. Both hosts were about to hold
    /// their own version of this.
    #[test]
    fn an_empty_trigger_field_is_no_trigger_rather_than_a_blank_one() {
        let conn = test_conn();
        let mut input = create_input("Greeting", None);
        input.trigger = Some("   ".to_string());
        input.trigger_mode = Some("delimiter".to_string());

        let created = snippet_create(&conn, input, 1).unwrap();

        assert_eq!(created.trigger, None);
        assert_eq!(created.trigger_mode, None);
    }

    #[test]
    fn a_trigger_is_stored_trimmed_and_keeps_its_mode() {
        let conn = test_conn();
        let mut input = create_input("Greeting", None);
        input.trigger = Some("  ;deploy  ".to_string());
        input.trigger_mode = Some("delimiter".to_string());

        let created = snippet_create(&conn, input, 1).unwrap();

        assert_eq!(created.trigger.as_deref(), Some(";deploy"));
        assert_eq!(created.trigger_mode.as_deref(), Some("delimiter"));
    }

    /// Trimming is not silent repair: a trigger with a space inside it is
    /// still refused, because which characters are allowed is the model's rule
    /// and quietly storing something else would be answering a question the
    /// caller did not ask.
    #[test]
    fn a_trigger_with_a_space_inside_is_still_refused() {
        let conn = test_conn();
        let mut input = create_input("Greeting", None);
        input.trigger = Some(";de ploy".to_string());
        input.trigger_mode = Some("delimiter".to_string());

        let err = snippet_create(&conn, input, 1).unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    /// A reader who did not want to name anything gets the first line of what
    /// they wrote. Both hosts were about to hold their own copy of this rule.
    #[test]
    fn a_snippet_with_no_name_is_filed_under_its_first_line() {
        let conn = test_conn();
        let mut input = create_input("ignored", None);
        input.title = "   ".to_string();
        input.body = "\n\nkubectl get pods\n--all-namespaces".to_string();

        let created = snippet_create(&conn, input, 1).unwrap();

        // Leading blank lines are not the name; the first line with something
        // on it is.
        assert_eq!(created.title, "kubectl get pods");
    }

    #[test]
    fn a_given_name_is_kept_and_trimmed() {
        let conn = test_conn();
        let mut input = create_input("ignored", None);
        input.title = "  Rollback  ".to_string();

        let created = snippet_create(&conn, input, 1).unwrap();

        assert_eq!(created.title, "Rollback");
    }

    /// A body with nothing in it leaves nothing to be named after, and the
    /// model's own rule then applies: a snippet has a title.
    #[test]
    fn nothing_to_write_and_nothing_to_call_it_is_still_refused() {
        let conn = test_conn();
        let mut input = create_input("ignored", None);
        input.title = String::new();
        input.body = "   ".to_string();

        let err = snippet_create(&conn, input, 1).unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Validation);
    }

    /// The kind row in the mobile editor offers all eight marks, and the
    /// secret one must not be writable through this call: it would store the
    /// body in the clear under a name that says otherwise.
    #[test]
    fn an_ordinary_save_refuses_to_file_something_as_a_secret() {
        let conn = test_conn();
        let mut input = create_input("Deploy key", None);
        input.snippet_type = "sensitive".to_string();
        input.body = "AKIA_FAKE_NOT_A_SECRET_deploy_value".to_string();

        let err = snippet_create(&conn, input, 1).unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Validation);
        // Refused means nothing was written — not a row, and not an index
        // entry carrying the body the caller tried to file in the clear.
        assert!(snippet_list(&conn, 50, 0).unwrap().is_empty());
        assert!(
            search_snippets(&conn, "AKIA_FAKE_NOT_A_SECRET_deploy_value", 10, 0)
                .unwrap()
                .is_empty()
        );
    }

    /// The same door from the other side: an ordinary snippet cannot become a
    /// secret by having its kind rewritten. Promotion re-encrypts and drops
    /// the plaintext history, which is what `snippet_convert_to_sensitive` is.
    #[test]
    fn an_ordinary_edit_refuses_to_turn_a_snippet_into_a_secret() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Cluster login", None), 1).unwrap();

        let err = snippet_update(
            &conn,
            SnippetUpdateInput {
                id: created.id.clone(),
                title: "Cluster login".to_string(),
                body: "AKIA_FAKE_NOT_A_SECRET_cluster_value".to_string(),
                snippet_type: "sensitive".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
            },
            2,
        )
        .unwrap_err();

        assert_eq!(err.code, IpcErrorCode::Validation);
        let kept = snippet_get(&conn, &created.id).unwrap();
        assert_eq!(kept.security_level, "normal");
        assert_eq!(kept.body.as_deref(), Some("Cluster login body"));
    }

    #[test]
    fn content_updates_apply_the_version_retention_policy() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Doc", None), 1).unwrap();

        // Enough content edits to overflow the retention count cap.
        for round in 0..typvia_core::repo::VERSION_KEEP_MAX + 5 {
            snippet_update(
                &conn,
                SnippetUpdateInput {
                    id: created.id.clone(),
                    title: "Doc".to_string(),
                    body: format!("body {round}"),
                    snippet_type: "text".to_string(),
                    description: None,
                    folder_id: None,
                    trigger: None,
                    trigger_mode: None,
                    language: None,
                    is_favorite: false,
                    is_pinned: false,
                    is_enabled: true,
                },
                i64::from(round) + 10,
            )
            .unwrap();
        }

        let history = VersionRepo::new(&conn).list(&created.id, 100, 0).unwrap();
        assert_eq!(
            history.len(),
            typvia_core::repo::VERSION_KEEP_MAX as usize,
            "history is capped in the update write path"
        );
    }

    fn update_body(id: &str, title: &str, body: &str) -> SnippetUpdateInput {
        SnippetUpdateInput {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "text".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
        }
    }

    #[test]
    fn history_restore_writes_forward_and_keeps_existing_versions() {
        let conn = test_conn();
        let mut session = VaultSession::new();
        let created = snippet_create(&conn, create_input("Doc", None), 1).unwrap();
        snippet_update(&conn, update_body(&created.id, "Doc", "second body"), 2).unwrap();
        snippet_update(&conn, update_body(&created.id, "Doc", "third body"), 3).unwrap();

        let history = history_list(&conn, &mut session, &created.id, 100, 0, 4).unwrap();
        assert_eq!(history.current, 3);
        let listed: Vec<u32> = history.entries.iter().map(|e| e.version).collect();
        assert_eq!(listed, vec![3, 2, 1]);

        let v1 = history_get(&conn, &mut session, &created.id, 1, 4).unwrap();
        assert_eq!(v1.body, "Doc body");

        // Restore writes forward: v1's state becomes v4; v1..v3 stay intact.
        let restored = history_restore(&conn, &mut session, &created.id, 1, 5).unwrap();
        assert_eq!(restored.version, 4);
        assert_eq!(restored.body.as_deref(), Some("Doc body"));
        let after = history_list(&conn, &mut session, &created.id, 100, 0, 6).unwrap();
        let versions: Vec<u32> = after.entries.iter().map(|e| e.version).collect();
        assert_eq!(versions, vec![4, 3, 2, 1]);
        assert_eq!(
            history_get(&conn, &mut session, &created.id, 3, 6)
                .unwrap()
                .body,
            "third body",
            "the pre-restore current version is never lost"
        );

        // Restoring the live version is a no-op request, refused as a conflict.
        let err = history_restore(&conn, &mut session, &created.id, 4, 7).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
    }

    #[test]
    fn secret_updates_are_bounded_by_the_retention_policy() {
        // vault_update_secret must clean history in the same transaction,
        // exactly like the plain update path.
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let dto = vault_create_secret(
            &conn,
            &session,
            secret_input("Key", "AKIA_FAKE_RETENTION_0"),
            VAULT_NOW,
        )
        .unwrap();

        let rounds = typvia_core::repo::VERSION_KEEP_MAX + 5;
        for round in 0..rounds {
            vault_update_secret(
                &conn,
                &session,
                update_body(
                    &dto.id,
                    "Key",
                    &format!("AKIA_FAKE_RETENTION_{}", round + 1),
                ),
                VAULT_NOW + i64::from(round) + 1,
            )
            .unwrap();
        }

        let listed = history_list(
            &conn,
            &mut session,
            &dto.id,
            rounds + 10,
            0,
            VAULT_NOW + i64::from(rounds) + 2,
        )
        .unwrap();
        assert_eq!(
            listed.entries.len(),
            typvia_core::repo::VERSION_KEEP_MAX as usize
        );
    }

    #[test]
    fn sensitive_history_is_invisible_while_locked() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let secret = "AKIA_FAKE_HISTORY_MARKER_3T8";
        let dto =
            vault_create_secret(&conn, &session, secret_input("Key", secret), VAULT_NOW).unwrap();
        vault_update_secret(
            &conn,
            &session,
            update_body(&dto.id, "Key", "AKIA_FAKE_HISTORY_MARKER_NEW"),
            VAULT_NOW + 1,
        )
        .unwrap();

        // Unlocked: metadata lists, and the diff view can decrypt one entry.
        let listed = history_list(&conn, &mut session, &dto.id, 100, 0, VAULT_NOW + 2).unwrap();
        assert_eq!(listed.entries.len(), 2);
        let v1 = history_get(&conn, &mut session, &dto.id, 1, VAULT_NOW + 2).unwrap();
        assert_eq!(v1.body, secret);

        // Locked: no history surface at all — list, get and restore all refuse.
        session.lock();
        for err in [
            history_list(&conn, &mut session, &dto.id, 100, 0, VAULT_NOW + 3).unwrap_err(),
            history_get(&conn, &mut session, &dto.id, 1, VAULT_NOW + 3).unwrap_err(),
            history_restore(&conn, &mut session, &dto.id, 1, VAULT_NOW + 3).unwrap_err(),
        ] {
            assert_eq!(err.code, IpcErrorCode::PermissionDenied);
        }
    }

    #[test]
    fn update_reindexes_and_respects_trigger_conflicts() {
        let conn = test_conn();
        let a = snippet_create(&conn, create_input("First", Some(";a")), 1).unwrap();
        let b = snippet_create(&conn, create_input("Second", Some(";b")), 2).unwrap();

        // Renaming updates the index.
        let updated = snippet_update(
            &conn,
            SnippetUpdateInput {
                id: b.id.clone(),
                title: "Kubernetes restart".to_string(),
                body: "kubectl rollout restart".to_string(),
                snippet_type: "command".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";b".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
                is_favorite: true,
                is_pinned: false,
                is_enabled: true,
            },
            5,
        )
        .unwrap();
        assert!(updated.is_favorite);
        let hits = search_snippets(&conn, "kubernetes", 10, 0).unwrap();
        assert_eq!(hits.len(), 1);

        // Stealing another snippet's trigger is a business conflict.
        let err = snippet_update(
            &conn,
            SnippetUpdateInput {
                id: b.id,
                title: "Second".to_string(),
                body: "body".to_string(),
                snippet_type: "command".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";a".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
                is_favorite: false,
                is_pinned: false,
                is_enabled: true,
            },
            6,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        drop(a);
    }

    #[test]
    fn trash_flow_removes_from_search_and_restores() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Trash me", None), 1).unwrap();

        snippet_trash(&conn, &created.id, 10).unwrap();
        assert!(search_snippets(&conn, "trash", 10, 0).unwrap().is_empty());
        assert_eq!(trash_list(&conn, 50, 0).unwrap().len(), 1);

        snippet_restore(&conn, &created.id, 9_000).unwrap();
        assert_eq!(search_snippets(&conn, "trash", 10, 0).unwrap().len(), 1);

        snippet_trash(&conn, &created.id, 20).unwrap();
        // Not yet expired: retention window keeps it.
        assert_eq!(trash_purge_expired(&conn, 21).unwrap(), 0);
        assert_eq!(
            trash_purge_expired(&conn, 20 + TRASH_RETENTION_MS).unwrap(),
            1
        );
        assert_eq!(
            snippet_get(&conn, &created.id).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn error_mapping_covers_the_three_classes() {
        let conn = test_conn();

        // User error: nothing to write and nothing to call it. A blank title
        // on its own is no longer one — it is filed under the body's first
        // line — so the case that still fails is the one with neither.
        let mut empty = create_input("", None);
        empty.body = String::new();
        let err = snippet_create(&conn, empty, 1).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        // User error: unknown enum input.
        let mut bad = create_input("Ok", None);
        bad.snippet_type = "nonsense".to_string();
        assert_eq!(
            snippet_create(&conn, bad, 1).unwrap_err().code,
            IpcErrorCode::Validation
        );

        // Business error: missing row.
        assert_eq!(
            snippet_get(&conn, "missing").unwrap_err().code,
            IpcErrorCode::NotFound
        );

        // Business error: duplicate trigger at creation.
        snippet_create(&conn, create_input("One", Some(";x")), 1).unwrap();
        let err = snippet_create(&conn, create_input("Two", Some(";x")), 2).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);

        // Oversized page limits are rejected, not clamped.
        assert_eq!(
            snippet_list(&conn, 0, 0).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn save_flow_appends_versions_only_on_content_change() {
        let conn = test_conn();
        let created = snippet_create(&conn, create_input("Draft", None), 1).unwrap();
        assert_eq!(created.version, 1);
        let history_count = |id: &str| {
            VersionRepo::new(&conn)
                .list(id, 100, 0)
                .map(|rows| rows.len())
                .unwrap()
        };
        // Creation recorded v1.
        assert_eq!(history_count(&created.id), 1);

        let mut input = SnippetUpdateInput {
            id: created.id.clone(),
            title: "Draft".to_string(),
            body: "Draft body".to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
        };

        // Metadata-only change: no bump, no new history row.
        input.is_favorite = true;
        let updated = snippet_update(&conn, input.clone(), 2).unwrap();
        assert_eq!(updated.version, 1);
        assert_eq!(history_count(&created.id), 1);

        // Content change: bump + snapshot of the new state.
        input.body = "Draft body, revised".to_string();
        let updated = snippet_update(&conn, input.clone(), 3).unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(history_count(&created.id), 2);

        input.title = "Draft renamed".to_string();
        let updated = snippet_update(&conn, input, 4).unwrap();
        assert_eq!(updated.version, 3);
        assert_eq!(history_count(&created.id), 3);
    }

    #[test]
    fn detect_sensitive_maps_kinds_to_stable_codes() {
        let kinds = detect_sensitive(
            "password = hunter2-not-real\n-----BEGIN RSA PRIVATE KEY-----\nFAKE\n-----END RSA PRIVATE KEY-----",
        );
        assert!(kinds.contains(&"pem_private_key".to_string()));
        assert!(kinds.contains(&"password_field".to_string()));
        assert!(detect_sensitive("just a plain sentence").is_empty());
    }

    #[test]
    fn search_library_returns_full_rows_in_rank_order() {
        let conn = test_conn();
        // Title-exact should outrank a content match.
        let mut content_hit = create_input("Restart notes", None);
        content_hit.body = "docker restart tips".to_string();
        snippet_create(&conn, content_hit, 1).unwrap();
        let title_hit = snippet_create(&conn, create_input("Docker", Some(";dk")), 2).unwrap();

        let rows = search_library(&conn, "docker", 50).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, title_hit.id);
        assert_eq!(rows[0].trigger.as_deref(), Some(";dk"));
        assert!(rows[0].body.is_some(), "full row data, not just hits");

        // Queries with nothing indexable return empty, not an error.
        assert!(search_library(&conn, "   ", 50).unwrap().is_empty());
        assert_eq!(
            search_library(&conn, "docker", 0).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn library_page_and_counts_reflect_scopes() {
        let conn = test_conn();
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Infra".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();

        let mut filed = create_input("Filed one", None);
        filed.folder_id = Some(folder.id.clone());
        snippet_create(&conn, filed, 1).unwrap();
        let mut text_kind = create_input("Loose text", None);
        text_kind.snippet_type = "text".to_string();
        snippet_create(&conn, text_kind, 2).unwrap();
        let starred = snippet_create(&conn, create_input("Starred one", None), 3).unwrap();
        let mut make_fav = SnippetUpdateInput {
            id: starred.id.clone(),
            title: starred.title.clone(),
            body: "Starred one body".to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            is_favorite: true,
            is_pinned: false,
            is_enabled: true,
        };
        snippet_update(&conn, make_fav.clone(), 4).unwrap();

        let counts = library_counts(&conn).unwrap();
        assert_eq!(
            (
                counts.total,
                counts.recent,
                counts.starred,
                counts.unsorted,
                counts.trash
            ),
            (3, 0, 1, 2, 0)
        );
        assert_eq!(counts.folders.len(), 1);
        assert_eq!(counts.folders[0].folder_id, folder.id);
        assert_eq!(counts.folders[0].count, 1);

        let page = snippet_list_page(&conn, "folder", Some(&folder.id), None, None, 50, 0).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].title, "Filed one");

        let texts = snippet_list_page(&conn, "unsorted", None, Some("text"), None, 50, 0).unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].title, "Loose text");

        // The type-filtered count matches the type-filtered page query.
        assert_eq!(
            snippet_count(&conn, "unsorted", None, Some("text")).unwrap(),
            1
        );
        assert_eq!(snippet_count(&conn, "all", None, None).unwrap(), 3);

        // Validation: unknown view, folder view without id, stray folderId.
        for (view, folder_id) in [("nonsense", None), ("folder", None), ("all", Some("x"))] {
            let err = snippet_list_page(&conn, view, folder_id, None, None, 50, 0).unwrap_err();
            assert_eq!(err.code, IpcErrorCode::Validation);
        }
        let err = snippet_list_page(&conn, "all", None, Some("nonsense"), None, 50, 0).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);
        let err = snippet_list_page(&conn, "all", None, None, Some("sideways"), 50, 0).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        // A reader-chosen order reorders a page without changing what is in it:
        // "recently added" puts the last one created first, whatever the view.
        let added = snippet_list_page(&conn, "all", None, None, Some("added"), 50, 0).unwrap();
        assert_eq!(
            added.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
            ["Starred one", "Loose text", "Filed one"]
        );

        // A trashed snippet drops out of pages and counts.
        make_fav.is_favorite = false;
        snippet_update(&conn, make_fav, 5).unwrap();
        snippet_trash(&conn, &starred.id, 6).unwrap();
        let counts = library_counts(&conn).unwrap();
        assert_eq!(counts.total, 2);
        assert_eq!(counts.trash, 1);
    }

    #[test]
    fn batch_operations_keep_the_search_index_in_step() {
        let conn = test_conn();
        let a = snippet_create(&conn, create_input("Alpha", None), 1).unwrap();
        let b = snippet_create(&conn, create_input("Beta", None), 2).unwrap();
        let ids = vec![a.id.clone(), b.id.clone()];
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Runbooks".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            3,
        )
        .unwrap();

        // Move: the folder name becomes searchable for both rows.
        snippet_batch_move(&conn, &ids, Some(&folder.id), 9_000).unwrap();
        assert_eq!(search_snippets(&conn, "runbooks", 10, 0).unwrap().len(), 2);

        // Tag: the tag name becomes searchable.
        let tag = tag_create(&conn, "prod".to_string(), 4).unwrap();
        snippet_batch_add_tag(&conn, &ids, &tag.id, 9_000).unwrap();
        assert_eq!(search_snippets(&conn, "prod", 10, 0).unwrap().len(), 2);

        // Folder rename: contents re-index under the new name.
        folder_update(
            &conn,
            FolderUpdateInput {
                id: folder.id.clone(),
                name: "Playbooks".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            5,
        )
        .unwrap();
        assert_eq!(search_snippets(&conn, "playbooks", 10, 0).unwrap().len(), 2);
        assert!(
            search_snippets(&conn, "runbooks", 10, 0)
                .unwrap()
                .is_empty()
        );

        // Folder delete: contents move out (unfiled) and stay alive.
        folder_delete(&conn, &folder.id, 9_000).unwrap();
        let listed = snippet_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|s| s.folder_id.is_none()));
        assert!(
            search_snippets(&conn, "playbooks", 10, 0)
                .unwrap()
                .is_empty()
        );
        assert_eq!(search_snippets(&conn, "alpha", 10, 0).unwrap().len(), 1);

        // Batch trash: rows leave lists, counts and the index together.
        snippet_batch_trash(&conn, &ids, 10).unwrap();
        assert!(snippet_list(&conn, 50, 0).unwrap().is_empty());
        assert!(search_snippets(&conn, "alpha", 10, 0).unwrap().is_empty());
        assert_eq!(library_counts(&conn).unwrap().total, 0);
        assert_eq!(trash_list(&conn, 50, 0).unwrap().len(), 2);

        // Empty batches are user errors.
        assert_eq!(
            snippet_batch_trash(&conn, &[], 11).unwrap_err().code,
            IpcErrorCode::Validation
        );
    }

    #[test]
    fn folder_and_tag_crud_roundtrip() {
        let conn = test_conn();
        let folder = folder_create(
            &conn,
            FolderCreateInput {
                name: "Infra".to_string(),
                parent_id: None,
                sort_order: 0,
            },
            1,
        )
        .unwrap();
        let child = folder_create(
            &conn,
            FolderCreateInput {
                name: "K8s".to_string(),
                parent_id: Some(folder.id.clone()),
                sort_order: 0,
            },
            2,
        )
        .unwrap();
        assert_eq!(
            folder_list_children(&conn, Some(&folder.id)).unwrap().len(),
            1
        );

        let renamed = folder_update(
            &conn,
            FolderUpdateInput {
                id: child.id.clone(),
                name: "Kubernetes".to_string(),
                parent_id: Some(folder.id.clone()),
                sort_order: 1,
            },
            3,
        )
        .unwrap();
        assert_eq!(renamed.name, "Kubernetes");
        folder_delete(&conn, &child.id, 9_000).unwrap();

        let tag = tag_create(&conn, "prod".to_string(), 1).unwrap();
        assert_eq!(
            tag_create(&conn, "prod".to_string(), 2).unwrap_err().code,
            IpcErrorCode::Conflict
        );
        tag_rename(&conn, &tag.id, "production", 9_000).unwrap();
        assert_eq!(tag_list(&conn).unwrap().len(), 1);
        tag_delete(&conn, &tag.id, 9_000).unwrap();
        assert!(tag_list(&conn).unwrap().is_empty());
    }

    // ===== App rules =====

    const MACOS: Option<Platform> = Some(Platform::Macos);

    fn rule_input(snippet_id: &str, app: &str, rule_type: &str) -> AppRuleCreateInput {
        AppRuleCreateInput {
            snippet_id: snippet_id.to_string(),
            app_identifier: app.to_string(),
            rule_type: rule_type.to_string(),
        }
    }

    #[test]
    fn panel_results_hides_ruled_out_rows_and_counts_them() {
        let conn = test_conn();
        let repo = SnippetRepo::new(&conn);
        // The empty query lists recently *used* snippets (ListScope::Recent).
        let mut alpha = espanso_snippet(
            "Alpha",
            ":alpha",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("alpha body".to_string()),
        );
        alpha.last_used_at = Some(2);
        repo.insert(&alpha).unwrap();
        let mut beta = espanso_snippet(
            "Beta",
            ":beta",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("beta body".to_string()),
        );
        beta.last_used_at = Some(1);
        let beta_id = beta.id.clone();
        repo.insert(&beta).unwrap();
        app_rule_create(
            &conn,
            MACOS,
            rule_input(&beta_id, "com.google.Chrome", "disable"),
            9_000,
        )
        .unwrap();

        // In the ruled-out app: Beta is dropped and counted.
        let page = panel_results(&conn, "", 20, MACOS, Some("com.google.Chrome")).unwrap();
        let titles: Vec<&str> = page.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Alpha"]);
        assert_eq!(page.hidden_by_rules, 1);

        // Elsewhere, and with no identifiable destination: default allow.
        let page = panel_results(&conn, "", 20, MACOS, Some("com.apple.Terminal")).unwrap();
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.hidden_by_rules, 0);
        let page = panel_results(&conn, "", 20, MACOS, None).unwrap();
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.hidden_by_rules, 0);
    }

    #[test]
    fn panel_results_applies_show_only_to_search_hits() {
        let conn = test_conn();
        let repo = SnippetRepo::new(&conn);
        let alpha = espanso_snippet(
            "Deploy checklist",
            ":deploy",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("steps".to_string()),
        );
        let alpha_id = alpha.id.clone();
        repo.insert(&alpha).unwrap();
        SearchIndex::new(&conn).sync_snippet(&alpha_id).unwrap();
        app_rule_create(
            &conn,
            MACOS,
            rule_input(&alpha_id, "com.apple.Terminal", "show_only"),
            9_000,
        )
        .unwrap();

        let page = panel_results(&conn, "deploy", 20, MACOS, Some("com.apple.Terminal")).unwrap();
        assert_eq!(page.rows.len(), 1);
        let page = panel_results(&conn, "deploy", 20, MACOS, Some("com.google.Chrome")).unwrap();
        assert!(page.rows.is_empty());
        assert_eq!(page.hidden_by_rules, 1);
    }

    #[test]
    fn panel_insert_gate_refuses_a_ruled_out_destination() {
        let conn = test_conn();
        let snippet = espanso_snippet(
            "Sig",
            ":sig",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("Best".to_string()),
        );
        let id = snippet.id.clone();
        SnippetRepo::new(&conn).insert(&snippet).unwrap();
        app_rule_create(
            &conn,
            MACOS,
            rule_input(&id, "com.google.Chrome", "disable"),
            9_000,
        )
        .unwrap();

        let err = panel_gate_insert(&conn, &id, MACOS, Some("com.google.Chrome")).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::RuleBlocked);
        panel_gate_insert(&conn, &id, MACOS, Some("com.apple.Terminal")).unwrap();
        panel_gate_insert(&conn, &id, MACOS, None).unwrap();
    }

    #[test]
    fn sensitive_insert_gate_honours_deny_sensitive_injection() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        let dto = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod token", "AKIA_FAKE_GATE_062"),
            VAULT_NOW,
        )
        .unwrap();
        app_rule_create(
            &conn,
            MACOS,
            rule_input(&dto.id, "com.google.Chrome", "deny_sensitive_injection"),
            9_000,
        )
        .unwrap();

        let err =
            panel_gate_insert_secret(&conn, &dto.id, MACOS, Some("com.google.Chrome")).unwrap_err();
        assert_eq!(err.code, IpcErrorCode::RuleBlocked);
        // The snippet stays visible there — only the sensitive delivery is out.
        panel_gate_insert(&conn, &dto.id, MACOS, Some("com.google.Chrome")).unwrap();
        panel_gate_insert_secret(&conn, &dto.id, MACOS, Some("com.apple.Terminal")).unwrap();
    }

    #[test]
    fn app_rule_crud_round_trips_and_rejects_duplicates_and_expansion() {
        let conn = test_conn();
        let snippet = espanso_snippet(
            "Sig",
            ":sig",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("Best".to_string()),
        );
        let id = snippet.id.clone();
        SnippetRepo::new(&conn).insert(&snippet).unwrap();

        let dto = app_rule_create(
            &conn,
            MACOS,
            rule_input(&id, " com.google.Chrome ", "disable"),
            9_000,
        )
        .unwrap();
        assert_eq!(dto.snippet_title, "Sig");
        assert_eq!(dto.app_identifier, "com.google.Chrome");
        assert_eq!(dto.rule_type, "disable");

        // Duplicates match case-insensitively, like the evaluation does.
        let err = app_rule_create(
            &conn,
            MACOS,
            rule_input(&id, "COM.GOOGLE.CHROME", "disable"),
            9_000,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Conflict);
        // Per-app expansion control has no enforcement path in v1: refused,
        // never stored inert.
        let err = app_rule_create(
            &conn,
            MACOS,
            rule_input(&id, "com.apple.Terminal", "disable_expansion"),
            9_000,
        )
        .unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Validation);

        let listed = app_rule_list(&conn, MACOS, 50, 0).unwrap();
        assert_eq!(listed.len(), 1);

        let updated = app_rule_update(
            &conn,
            &dto.id,
            AppRuleUpdateInput {
                app_identifier: "com.apple.Safari".to_string(),
                rule_type: "show_only".to_string(),
            },
            9_000,
        )
        .unwrap();
        assert_eq!(updated.app_identifier, "com.apple.Safari");
        assert_eq!(updated.rule_type, "show_only");

        app_rule_delete(&conn, &dto.id, 9_000).unwrap();
        assert!(app_rule_list(&conn, MACOS, 50, 0).unwrap().is_empty());
        assert_eq!(
            app_rule_delete(&conn, &dto.id, 9_000).unwrap_err().code,
            IpcErrorCode::NotFound
        );
    }

    #[test]
    fn panel_filter_parses_marks_and_ignores_unknown_tokens() {
        assert_eq!(
            parse_panel_filter("/tp"),
            (
                Some(PanelTypeFilter::SnippetType("template")),
                String::new()
            )
        );
        assert_eq!(
            parse_panel_filter("/TP docker"),
            (
                Some(PanelTypeFilter::SnippetType("template")),
                "docker".to_string()
            )
        );
        assert_eq!(
            parse_panel_filter("/sc"),
            (Some(PanelTypeFilter::Sensitive), String::new())
        );
        // Unknown or non-token slashes stay literal search text.
        assert_eq!(parse_panel_filter("/zz query").0, None);
        assert_eq!(parse_panel_filter("/tpx").0, None);
        assert_eq!(parse_panel_filter("plain").0, None);
        // Multi-byte input after the slash must not panic.
        assert_eq!(parse_panel_filter("/中文").0, None);
    }

    #[test]
    fn panel_results_narrows_to_the_filtered_type_bucket() {
        let conn = test_conn();
        let repo = SnippetRepo::new(&conn);
        let index = SearchIndex::new(&conn);
        let mut note = espanso_snippet(
            "Plain note",
            ":note",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("note body".to_string()),
        );
        note.last_used_at = Some(2);
        repo.insert(&note).unwrap();
        index.sync_snippet(&note.id).unwrap();
        let mut deploy = espanso_snippet(
            "Deploy note",
            ":deploy",
            SecurityLevel::Normal,
            SnippetContent::Plaintext("deploy body".to_string()),
        );
        deploy.snippet_type = SnippetType::Template;
        deploy.last_used_at = Some(1);
        repo.insert(&deploy).unwrap();
        index.sync_snippet(&deploy.id).unwrap();

        // Recent mode: /tp keeps only the template bucket.
        let page = panel_results(&conn, "/tp", 20, None, None).unwrap();
        let titles: Vec<&str> = page.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["Deploy note"]);

        // Search mode: the text after the token searches within the bucket.
        let page = panel_results(&conn, "/tp note", 20, None, None).unwrap();
        let titles: Vec<&str> = page.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["Deploy note"]);

        // No token: both buckets rank as before.
        let page = panel_results(&conn, "note", 20, None, None).unwrap();
        assert_eq!(page.rows.len(), 2);
    }

    #[test]
    fn onboarding_marker_round_trips_and_starts_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!onboarding_completed(dir.path()));
        mark_onboarding_complete(dir.path()).unwrap();
        assert!(onboarding_completed(dir.path()));
        // Marking twice stays completed (idempotent).
        mark_onboarding_complete(dir.path()).unwrap();
        assert!(onboarding_completed(dir.path()));
    }

    #[test]
    fn clipboard_seed_trims_and_passes_ordinary_text() {
        let seed = clipboard_seed(Some("  docker logs -f api  ".to_string()));
        assert_eq!(seed.as_deref(), Some("docker logs -f api"));
    }

    #[test]
    fn clipboard_seed_rejects_empty_oversized_and_suspected_secrets() {
        assert_eq!(clipboard_seed(None), None);
        assert_eq!(clipboard_seed(Some("   \n".to_string())), None);
        let oversized = "x".repeat(CLIPBOARD_SEED_MAX_CHARS + 1);
        assert_eq!(clipboard_seed(Some(oversized)), None);
        // Suspected secret on the clipboard must never prefill a plain
        // snippet form: suspected secrets on the clipboard are ignored by
        // default. Fake value shaped to trip the AWS access-key rule.
        assert_eq!(
            clipboard_seed(Some("AKIAFAKEFAKEFAKEFAKE".to_string())),
            None
        );
    }

    // ===== Sync conflict adjudication =====

    const CONFLICT_NOW: i64 = 1_700_000_100_000;

    /// Builds a resolved-by-merge pair: `source` holds the body that won the
    /// entity, `copy` the parked one, exactly as the merge writes them.
    fn conflict_pair(conn: &Connection) -> (String, String) {
        let source =
            snippet_create(conn, create_input("Weekly update", Some(";weekly")), 1).unwrap();
        let copy = snippet_create(
            conn,
            create_input("Weekly update (conflict on Pixel 8)", None),
            2,
        )
        .unwrap();
        let repo = SnippetRepo::new(conn);
        let mut parked = repo.get(&copy.id).unwrap().unwrap();
        parked.conflict_of = Some(source.id.clone());
        parked.content = SnippetContent::Plaintext("the other device's body".to_string());
        repo.update(&parked).unwrap();
        (source.id, copy.id)
    }

    #[test]
    fn conflict_list_pairs_each_copy_with_the_snippet_it_points_at() {
        let conn = test_conn();
        let (source_id, copy_id) = conflict_pair(&conn);

        let pairs = sync_conflict_list(&conn, CONFLICT_NOW).unwrap();

        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].source.id, source_id);
        assert_eq!(pairs[0].copy.id, copy_id);
        assert!(!pairs[0].sensitive);
        assert_eq!(
            pairs[0].copy.body.as_deref(),
            Some("the other device's body")
        );
    }

    #[test]
    fn conflict_list_clears_a_marker_whose_source_is_gone() {
        let conn = test_conn();
        let (source_id, copy_id) = conflict_pair(&conn);
        snippet_delete_forever(&conn, &source_id, CONFLICT_NOW).unwrap();

        assert_eq!(sync_conflict_list(&conn, CONFLICT_NOW).unwrap().len(), 0);
        let copy = SnippetRepo::new(&conn).get(&copy_id).unwrap().unwrap();
        assert_eq!(copy.conflict_of, None);
        assert_eq!(copy.deleted_at, None);
    }

    #[test]
    fn keeping_the_source_sends_the_copy_to_the_recycle_bin() {
        let conn = test_conn();
        let (source_id, copy_id) = conflict_pair(&conn);
        let session = VaultSession::new();

        sync_conflict_resolve(
            &conn,
            &session,
            &copy_id,
            ConflictKeep::Source,
            CONFLICT_NOW,
        )
        .unwrap();

        let repo = SnippetRepo::new(&conn);
        let source = repo.get(&source_id).unwrap().unwrap();
        assert_eq!(
            source.content,
            SnippetContent::Plaintext("Weekly update body".to_string())
        );
        let copy = repo.get(&copy_id).unwrap().unwrap();
        assert_eq!(copy.deleted_at, Some(CONFLICT_NOW));
        assert_eq!(copy.conflict_of, None);
        assert_eq!(sync_conflict_list(&conn, CONFLICT_NOW).unwrap().len(), 0);
    }

    #[test]
    fn keeping_the_copy_moves_its_body_onto_the_snippet_as_a_new_version() {
        let conn = test_conn();
        let (source_id, copy_id) = conflict_pair(&conn);
        let session = VaultSession::new();

        sync_conflict_resolve(&conn, &session, &copy_id, ConflictKeep::Copy, CONFLICT_NOW).unwrap();

        let repo = SnippetRepo::new(&conn);
        let source = repo.get(&source_id).unwrap().unwrap();
        assert_eq!(
            source.content,
            SnippetContent::Plaintext("the other device's body".to_string())
        );
        assert_eq!(source.version, 2);
        // The trigger stays with the entity that owned it.
        assert_eq!(source.trigger.as_deref(), Some(";weekly"));
        assert_eq!(
            VersionRepo::new(&conn).latest_version(&source_id).unwrap(),
            2
        );
        assert_eq!(
            repo.get(&copy_id).unwrap().unwrap().deleted_at,
            Some(CONFLICT_NOW)
        );
    }

    #[test]
    fn keeping_both_leaves_two_live_snippets_and_no_decision() {
        let conn = test_conn();
        let (source_id, copy_id) = conflict_pair(&conn);
        let session = VaultSession::new();

        sync_conflict_resolve(&conn, &session, &copy_id, ConflictKeep::Both, CONFLICT_NOW).unwrap();

        let repo = SnippetRepo::new(&conn);
        assert_eq!(repo.get(&source_id).unwrap().unwrap().deleted_at, None);
        let copy = repo.get(&copy_id).unwrap().unwrap();
        assert_eq!(copy.deleted_at, None);
        assert_eq!(copy.conflict_of, None);
        assert_eq!(sync_conflict_list(&conn, CONFLICT_NOW).unwrap().len(), 0);
    }

    #[test]
    fn resolving_a_snippet_that_is_not_a_conflict_copy_is_refused() {
        let conn = test_conn();
        let (source_id, _) = conflict_pair(&conn);
        let session = VaultSession::new();

        let error = sync_conflict_resolve(
            &conn,
            &session,
            &source_id,
            ConflictKeep::Source,
            CONFLICT_NOW,
        )
        .unwrap_err();

        assert_eq!(error.code, IpcErrorCode::Conflict);
    }

    #[test]
    fn a_sensitive_pair_reports_locked_bodies_and_re_seals_under_the_kept_id() {
        let conn = test_conn();
        let mut session = unlocked_session(&conn);
        let source = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod token", "the live secret"),
            VAULT_NOW,
        )
        .unwrap();
        let copy = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod token (conflict on Pixel 8)", "the other secret"),
            VAULT_NOW,
        )
        .unwrap();
        let repo = SnippetRepo::new(&conn);
        let mut parked = repo.get(&copy.id).unwrap().unwrap();
        parked.conflict_of = Some(source.id.clone());
        repo.update(&parked).unwrap();

        let pairs = sync_conflict_list(&conn, CONFLICT_NOW).unwrap();
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].sensitive);
        // Neither side's plaintext crosses the boundary for the comparison.
        assert_eq!(pairs[0].source.body, None);
        assert_eq!(pairs[0].copy.body, None);

        sync_conflict_resolve(&conn, &session, &copy.id, ConflictKeep::Copy, CONFLICT_NOW).unwrap();

        // Re-sealed under the surviving id: decrypting with that id returns
        // the kept body, which a moved envelope (bound to the copy's id and
        // therefore to a different AAD) could never do.
        assert_eq!(
            vault_reveal(&conn, &mut session, &source.id, CONFLICT_NOW).unwrap(),
            "the other secret"
        );
    }

    #[test]
    fn a_sensitive_pair_kept_by_copy_is_refused_while_the_vault_is_locked() {
        let conn = test_conn();
        let session = unlocked_session(&conn);
        let source = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod token", "the live secret"),
            VAULT_NOW,
        )
        .unwrap();
        let copy = vault_create_secret(
            &conn,
            &session,
            secret_input("Prod token (conflict on Pixel 8)", "the other secret"),
            VAULT_NOW,
        )
        .unwrap();
        let repo = SnippetRepo::new(&conn);
        let mut parked = repo.get(&copy.id).unwrap().unwrap();
        parked.conflict_of = Some(source.id.clone());
        repo.update(&parked).unwrap();
        let locked = VaultSession::new();

        let error =
            sync_conflict_resolve(&conn, &locked, &copy.id, ConflictKeep::Copy, CONFLICT_NOW)
                .unwrap_err();
        assert_eq!(error.code, IpcErrorCode::PermissionDenied);
        // Keeping the source needs no key at all.
        sync_conflict_resolve(&conn, &locked, &copy.id, ConflictKeep::Source, CONFLICT_NOW)
            .unwrap();
        assert_eq!(sync_conflict_list(&conn, CONFLICT_NOW).unwrap().len(), 0);
    }
}
