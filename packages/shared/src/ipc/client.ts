// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The typed IPC layer — the frontend's ONLY data entry point. Components
 * never call `invoke` directly (enforced by the no-restricted-imports lint
 * rule); they use these functions, so transport, command names and error
 * normalization stay in one place and tests mock exactly here or below.
 */
import { invoke } from '@tauri-apps/api/core';
import { toIpcError } from './error';
import type {
  Folder,
  FolderCreateInput,
  FolderUpdateInput,
  InsertionPause,
  LibraryCounts,
  LibraryView,
  MobileBootstrap,
  SearchHit,
  Snippet,
  SnippetCreateInput,
  SnippetUpdateInput,
  Tag,
  VaultStatus,
} from './types';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw toIpcError(raw);
  }
}

export function createSnippet(input: SnippetCreateInput): Promise<Snippet> {
  return call('snippet_create', { input });
}

export function updateSnippet(input: SnippetUpdateInput): Promise<Snippet> {
  return call('snippet_update', { input });
}

export function getSnippet(id: string): Promise<Snippet> {
  return call('snippet_get', { id });
}

export function listSnippets(limit: number, offset: number): Promise<Snippet[]> {
  return call('snippet_list', { limit, offset });
}

export function listSnippetsByFolder(
  folderId: string | null,
  limit: number,
  offset: number,
): Promise<Snippet[]> {
  return call('snippet_list_by_folder', { folderId, limit, offset });
}

/** A reader-chosen Library order, independent of the view; omit to keep the view's own. */
export type ListOrder = 'recent' | 'added' | 'used';

export function listSnippetPage(
  view: LibraryView,
  folderId: string | null,
  snippetType: string | null,
  limit: number,
  offset: number,
  order?: ListOrder,
): Promise<Snippet[]> {
  return call('snippet_list_page', {
    view,
    folderId,
    snippetType,
    order: order ?? null,
    limit,
    offset,
  });
}

export function countSnippets(
  view: LibraryView,
  folderId: string | null,
  snippetType: string | null,
): Promise<number> {
  return call('snippet_count', { view, folderId, snippetType });
}

export function libraryCounts(): Promise<LibraryCounts> {
  return call('library_counts');
}

export function batchMoveSnippets(ids: string[], folderId: string | null): Promise<void> {
  return call('snippet_batch_move', { ids, folderId });
}

export function batchTagSnippets(ids: string[], tagId: string): Promise<void> {
  return call('snippet_batch_add_tag', { ids, tagId });
}

export function batchTrashSnippets(ids: string[]): Promise<void> {
  return call('snippet_batch_trash', { ids });
}

export function trashSnippet(id: string): Promise<void> {
  return call('snippet_trash', { id });
}

export function restoreSnippet(id: string): Promise<void> {
  return call('snippet_restore', { id });
}

export function deleteSnippetForever(id: string): Promise<void> {
  return call('snippet_delete_forever', { id });
}

export function listTrash(limit: number, offset: number): Promise<Snippet[]> {
  return call('trash_list', { limit, offset });
}

export function purgeExpiredTrash(): Promise<number> {
  return call('trash_purge_expired');
}

export function createFolder(input: FolderCreateInput): Promise<Folder> {
  return call('folder_create', { input });
}

export function updateFolder(input: FolderUpdateInput): Promise<Folder> {
  return call('folder_update', { input });
}

export function deleteFolder(id: string): Promise<void> {
  return call('folder_delete', { id });
}

/**
 * Folds one folder into another in one transaction; resolves to the ids of the
 * snippets that moved, so the merge can be undone.
 */
export function mergeFolders(sourceId: string, targetId: string): Promise<string[]> {
  return call('folder_merge', { sourceId, targetId });
}

/** Writes a new folder order, first id first, all or nothing. */
export function reorderFolders(ids: string[]): Promise<void> {
  return call('folder_reorder', { ids });
}

export function listFolderChildren(parentId: string | null): Promise<Folder[]> {
  return call('folder_list_children', { parentId });
}

export function createTag(name: string): Promise<Tag> {
  return call('tag_create', { name });
}

export function listTags(): Promise<Tag[]> {
  return call('tag_list');
}

export function renameTag(id: string, name: string): Promise<void> {
  return call('tag_rename', { id, name });
}

export function deleteTag(id: string): Promise<void> {
  return call('tag_delete', { id });
}

export function searchSnippets(query: string, limit: number, offset: number): Promise<SearchHit[]> {
  return call('search_snippets', { query, limit, offset });
}

/** Ranked search returning full snippet rows for the Library list. */
export function searchLibrary(query: string, limit: number): Promise<Snippet[]> {
  return call('search_library', { query, limit });
}

/**
 * Ranked search for a calling surface's search screen (mobile Search):
 * sensitive title hits are included as locked rows (`body` is null) —
 * the panel boundary, not the Library one.
 */
export function searchSnippetsAll(query: string, limit: number): Promise<Snippet[]> {
  return call('snippet_search_all', { query, limit });
}

/** How injected text reaches the target app; omit for the default (paste). */
export type InjectionMethod = 'paste' | 'keystrokes';

/**
 * Injects a snippet into the frontmost application and records one usage.
 * Rejects with an IpcError of code `permission_denied` when the OS has not
 * granted injection permission — callers fall back to {@link copySnippet}.
 */
export function injectSnippet(id: string, method?: InjectionMethod): Promise<void> {
  return call('snippet_inject', { id, method: method ?? null });
}

/**
 * Library insert: Typvia steps aside, the app the reader came from takes focus
 * again, and the snippet is injected there once its app rules allow it. Rejects
 * with `permission_denied` (no injection permission) or `rule_blocked`, and the
 * window comes back so the page can say why.
 */
export function mainInsert(id: string, method?: InjectionMethod): Promise<void> {
  return call('main_insert', { id, method: method ?? null });
}

/**
 * Library insert of a template with its variables filled in: the same
 * step-aside and rule gate as {@link mainInsert}, the values rendered host-side.
 */
export function mainInsertTemplate(
  id: string,
  values: Record<string, string>,
  method?: InjectionMethod,
): Promise<void> {
  return call('main_insert_template', { id, values, method: method ?? null });
}

/** Brings up the About window. */
export function aboutShow(): Promise<void> {
  return call('about_show');
}

/** Whether Typvia may type into other apps (the macOS Accessibility grant). */
export function accessibilityStatus(): Promise<boolean> {
  return call('accessibility_status');
}

/** Opens the system's Accessibility settings page; macOS only. */
export function openAccessibilitySettings(): Promise<void> {
  return call('open_accessibility_settings');
}

/** Copies a snippet to the clipboard and records one usage. */
export function copySnippet(id: string): Promise<void> {
  return call('snippet_copy', { id });
}

/**
 * Offline sensitive-content scan. Returns advisory pattern codes (never the
 * matched text); an empty array means nothing suspicious.
 */
export function detectSensitive(text: string): Promise<string[]> {
  return call('detect_sensitive', { text });
}

/** The template field types, stored as TEXT. */
export type TemplateFieldType =
  | 'single_line_text'
  | 'multi_line_text'
  | 'number'
  | 'date'
  | 'time'
  | 'single_select'
  | 'multi_select'
  | 'toggle'
  | 'dropdown'
  | 'dynamic_variable'
  | 'secret_ref';

/**
 * A template field. `id` is empty for a newly-authored field; the host assigns
 * one on save. `snippetId` never crosses the boundary — the command supplies it.
 */
export interface TemplateField {
  id: string;
  name: string;
  label: string;
  fieldType: TemplateFieldType;
  defaultValue: string | null;
  options: string[];
  validation: string | null;
  isRequired: boolean;
  sortOrder: number;
  platformOverrides: string | null;
}

/** Distinct `{{var}}` names in a template body, in first-seen order. */
export function templateVariables(body: string): Promise<string[]> {
  return call('template_variables', { body });
}

/** Loads a snippet's saved template fields. */
export function templateFields(snippetId: string): Promise<TemplateField[]> {
  return call('template_fields', { snippetId });
}

/** Replaces a snippet's template field set as a whole (transactional). */
export function templateSaveFields(
  snippetId: string,
  fields: TemplateField[],
): Promise<TemplateField[]> {
  return call('template_save_fields', { snippetId, fields });
}

/**
 * Renders a lenient authoring preview: unfilled fields show a placeholder and
 * secret references show a mask (never their plaintext).
 */
export function templatePreview(
  body: string,
  fields: TemplateField[],
  values: Record<string, string>,
): Promise<string> {
  return call('template_preview', { body, fields, values });
}

/**
 * Strictly renders a saved template snippet with fill-in values into final
 * text. Rejects a missing required field, an unknown variable, or a secret
 * reference (which needs the vault).
 */
export function templateRender(snippetId: string, values: Record<string, string>): Promise<string> {
  return call('template_render', { snippetId, values });
}

/**
 * Renders a filled template host-side and copies the result to the system
 * pasteboard, recording one usage (mobile template fill sheet).
 */
export function templateCopy(id: string, values: Record<string, string>): Promise<void> {
  return call('template_copy', { id, values });
}

/**
 * Panel template call: renders the filled template and injects it into the app
 * the panel was summoned from, recording usage. The host hides the panel and
 * restores focus first.
 */
export function panelInsertTemplate(
  id: string,
  values: Record<string, string>,
  method?: InjectionMethod,
): Promise<void> {
  return call('panel_insert_template', { id, values, method });
}

/** Hides the global command panel and returns focus to the prior frontmost app. */
export function hidePanel(): Promise<void> {
  return call('panel_hide');
}

/**
 * Panel insert: hides the panel, restores the summoning app, and injects the
 * snippet into it (usage recorded). Ordering is owned by the host so the paste
 * never races the focus change. Rejects with `permission_denied` when the OS
 * has not granted injection — the panel falls back to {@link copySnippet}.
 */
export function panelInsert(id: string, method?: InjectionMethod): Promise<void> {
  return call('panel_insert', { id, method: method ?? null });
}

/**
 * Panel insert of a sensitive snippet: same hide/settle/inject ordering as
 * {@link panelInsert}, but the body is decrypted host-side from the unlocked
 * vault and never crosses IPC. Rejects with `permission_denied` when the vault
 * is locked — the panel prompts to unlock, then retries.
 */
export function panelInsertSecret(id: string, method?: InjectionMethod): Promise<void> {
  return call('panel_insert_secret', { id, method: method ?? null });
}

/**
 * Panel copy of a sensitive snippet: decrypts it onto the clipboard (guarded)
 * and schedules a timed auto-clear. Resolves with the clear delay
 * in ms. Rejects with `permission_denied` when the vault is locked.
 */
export function panelCopySecret(id: string): Promise<number> {
  return call('panel_copy_secret', { id });
}

/** Signals the panel's first frame has painted (drives the debug latency probe). */
export function panelReady(): Promise<void> {
  return call('panel_ready');
}

/**
 * Managed expansion-engine state. The engine runs as a Typvia-owned
 * subprocess; a user-installed expander blocks it through the coexistence gate
 * (`conflict` → user must choose, `takeover_pending` → waiting for their
 * instance to stop, `standing_aside` → they kept it). `unavailable` means the
 * engine binary itself is missing from this build.
 */
export type EspansoState =
  | 'running'
  | 'retrying'
  | 'failed'
  | 'off'
  | 'conflict'
  | 'takeover_pending'
  | 'standing_aside'
  | 'unavailable';

/** The user's recorded answer to the coexistence prompt. */
export type EspansoCoexistenceChoice = 'takeover' | 'stand_aside';

/** Managed-engine status for the Settings block and Home rail. */
export interface EspansoStatus {
  state: EspansoState;
  /** Reported engine version, when the binary is present. */
  version: string | null;
  /** Absolute path of the generated config inside the private engine dirs. */
  configPath: string | null;
  /** Whether the generated config currently exists (integration on). */
  enabled: boolean;
  /** Triggers that would be written from the current snippet set. */
  triggerCount: number;
  /** Recorded coexistence answer, when one has been made. */
  coexistenceChoice: EspansoCoexistenceChoice | null;
}

/** Result of writing or clearing the engine config. */
export interface EspansoSyncResult {
  enabled: boolean;
  triggerCount: number;
}

/** Reports the managed engine's status (runs the gated autostart first). */
export function espansoStatus(): Promise<EspansoStatus> {
  return call('espanso_status');
}

/**
 * Regenerates the engine config from the current snippet set (also the
 * "turn on" action). Sensitive snippets never enter the config (red line).
 */
export function espansoSync(): Promise<EspansoSyncResult> {
  return call('espanso_sync');
}

/** Removes the generated config and stops the engine (turns it off). */
export function espansoDisable(): Promise<EspansoSyncResult> {
  return call('espanso_disable');
}

/** Records the coexistence answer and re-runs the gated engine start. */
export function espansoCoexistence(choice: EspansoCoexistenceChoice): Promise<void> {
  return call('espanso_coexistence', { choice });
}

/** Semantic-search status for the desktop Settings block. */
export interface SemanticStatus {
  /** All model files are on disk (the feature's on state). */
  modelPresent: boolean;
  downloading: boolean;
  downloadReceived: number;
  downloadTotal: number;
  downloadFailed: boolean;
  /** Vectors stored for the current model. */
  embeddedCount: number;
  /** Live snippets still waiting for a (fresh) vector. */
  pendingCount: number;
  modelId: string;
}

/** Reports the local semantic-search model and index state. */
export function semanticStatus(): Promise<SemanticStatus> {
  return call('semantic_status');
}

/**
 * Starts the checksum-pinned model download in the background (the
 * feature's only network face; idempotent while running). Progress is
 * polled via {@link semanticStatus}.
 */
export function semanticModelDownload(): Promise<void> {
  return call('semantic_model_download');
}

/** Deletes the model and every stored vector (back to lexical-only). */
export function semanticModelDelete(): Promise<void> {
  return call('semantic_model_delete');
}

/**
 * Drains pending embeddings in the background; a no-op without a model,
 * so callers may poke unconditionally after data changes.
 */
export function semanticSync(): Promise<void> {
  return call('semantic_sync');
}

/**
 * Deep library search: lexical hits first, then semantic-only hits above
 * the score floor. Identical to {@link searchLibrary} while no model is
 * installed; a semantic branch past its time budget is absent, not late.
 */
export function searchLibraryDeep(query: string, limit: number): Promise<Snippet[]> {
  return call('search_library_deep', { query, limit });
}

/** Browser-integration status for the desktop Settings block. */
export interface BrowserIntegrationStatus {
  /** Whether the browser-snapshot data plane is switched on. */
  enabled: boolean;
  /** Whether at least one native-messaging host manifest is registered. */
  hostInstalled: boolean;
}

/** Reports whether desktop browser integration is switched on. */
export function browserIntegrationStatus(): Promise<BrowserIntegrationStatus> {
  return call('browser_integration_status');
}

/**
 * Switches browser integration on: writes the first browser snapshot (the
 * read model the native-messaging host serves) before flipping the switch.
 * Sensitive snippets enter the snapshot as opaque ciphertext only (red line).
 */
export function browserIntegrationEnable(): Promise<void> {
  return call('browser_integration_enable');
}

/** Switches browser integration off (idempotent). */
export function browserIntegrationDisable(): Promise<void> {
  return call('browser_integration_disable');
}

/**
 * Refreshes the browser snapshot after a data change; a successful no-op
 * while the integration is off, so callers may poke unconditionally.
 */
export function browserIntegrationSync(): Promise<void> {
  return call('browser_integration_sync');
}

/** A match skipped during import, with a human-readable reason. */
export interface EspansoSkipped {
  trigger: string | null;
  reason: string;
}

/** Summary of importing an Espanso match file. */
export interface EspansoImportResult {
  imported: number;
  conflicts: string[];
  skipped: EspansoSkipped[];
}

/**
 * Imports the plain trigger/replace pairs from an Espanso match file's text as
 * normal snippets. Unsupported matches and trigger conflicts are reported, not
 * imported; a parse failure rejects with a validation IpcError.
 */
export function espansoImport(content: string): Promise<EspansoImportResult> {
  return call('espanso_import', { content });
}

/** File formats the generic importer accepts. */
export type ImportFormat = 'markdown' | 'json' | 'csv' | 'masscode' | 'copyq';

/** An import entry that could not be taken in, with a human-readable reason. */
export interface ImportSkipped {
  /** Identifies the entry (title, trigger, row number…) when readable. */
  label: string | null;
  reason: string;
}

/** Summary of a Markdown / JSON / CSV import. */
export interface ImportReport {
  imported: number;
  conflicts: string[];
  skipped: ImportSkipped[];
}

/**
 * Imports snippets from the text of a Markdown, JSON, or CSV file. Trigger
 * conflicts and unimportable entries are reported, not imported; a parse
 * failure rejects with a validation IpcError.
 */
export function snippetsImport(format: ImportFormat, content: string): Promise<ImportReport> {
  return call('snippets_import', { format, content });
}

/** What a backup restore brought back (counts only, no content). */
export interface BackupRestoreReport {
  snippets: number;
  folders: number;
  tags: number;
  versions: number;
  vaultRestored: boolean;
}

/**
 * Exports the whole library as an encrypted backup file's text. The
 * passphrase derives the backup key host-side and is never stored; the
 * returned text is ciphertext plus KDF parameters only.
 */
export function backupExport(passphrase: string): Promise<string> {
  return call('backup_export', { passphrase });
}

/**
 * Writes an encrypted backup into the Downloads folder (host-chosen — no
 * path crosses IPC) and resolves with the file name for display.
 */
export function backupExportFile(passphrase: string): Promise<string> {
  return call('backup_export_file', { passphrase });
}

/**
 * Restores an encrypted backup into an empty library. A wrong passphrase,
 * tampering, or damage all reject with the same message.
 */
export function backupRestore(passphrase: string, content: string): Promise<BackupRestoreReport> {
  return call('backup_restore', { passphrase, content });
}

// --- Version history -------------------------------------------------------

/** One history row's metadata; bodies travel one at a time via historyGet. */
export interface VersionMeta {
  version: number;
  title: string;
  createdAt: number;
}

/** A snippet's history, newest first, plus its live version number. */
export interface SnippetHistory {
  current: number;
  entries: VersionMeta[];
}

/** One full history entry for the diff view. */
export interface VersionBody {
  version: number;
  title: string;
  body: string;
  createdAt: number;
}

/**
 * Lists a snippet's version history, newest first. For a sensitive snippet
 * this requires an unlocked vault — locked sessions are refused outright,
 * so the locked state shows no version data at all.
 */
export function historyList(id: string, limit: number, offset: number): Promise<SnippetHistory> {
  return call('history_list', { id, limit, offset });
}

/** Loads one version's full state (sensitive bodies require an unlocked vault). */
export function historyGet(id: string, version: number): Promise<VersionBody> {
  return call('history_get', { id, version });
}

/**
 * Restores a version by writing it forward: the restored state becomes a new
 * highest version and no existing entry is changed. Returns the updated
 * snippet.
 */
export function historyRestore(id: string, version: number): Promise<Snippet> {
  return call('history_restore', { id, version });
}

// --- App rules --------------------------------------------------------------

/**
 * Rule effects a snippet can carry per app. `disable_expansion`
 * exists in the data model but cannot be created in v1 (Espanso expansion is
 * one global config file), so reads may carry it while creates refuse it.
 */
export type AppRuleType =
  'show_only' | 'disable' | 'disable_expansion' | 'deny_sensitive_injection';

/** One per-snippet application rule, snippet title joined in for the list. */
export interface AppRule {
  id: string;
  snippetId: string;
  snippetTitle: string;
  platform: string;
  appIdentifier: string;
  ruleType: AppRuleType;
}

export interface AppRuleCreateInput {
  snippetId: string;
  appIdentifier: string;
  ruleType: AppRuleType;
}

export interface AppRuleUpdateInput {
  appIdentifier: string;
  ruleType: AppRuleType;
}

/** Lists this platform's app rules for the settings section. */
export function appRuleList(limit: number, offset: number): Promise<AppRule[]> {
  return call('app_rule_list', { limit, offset });
}

export function appRuleCreate(input: AppRuleCreateInput): Promise<AppRule> {
  return call('app_rule_create', { input });
}

export function appRuleUpdate(id: string, input: AppRuleUpdateInput): Promise<AppRule> {
  return call('app_rule_update', { id, input });
}

export function appRuleDelete(id: string): Promise<void> {
  return call('app_rule_delete', { id });
}

/** Panel result page: surviving rows plus how many an app rule hid. */
export interface PanelResults {
  rows: Snippet[];
  hiddenByRules: number;
}

/**
 * Panel results for the destination app the panel was summoned from: recent
 * snippets for an empty query, ranked hits otherwise, minus rows hidden by
 * app rules (counted so the panel can state the real number).
 */
export function panelResults(query: string, limit: number): Promise<PanelResults> {
  return call('panel_results', { query, limit });
}

// --- Onboarding -------------------------------------------------------------

/**
 * Packs a WebDAV username and password into the one string the transport
 * stores and reads.
 *
 * Asked for rather than composed here: the shape of that string belongs beside
 * the parser that reads it back, and a second copy of it is a copy that can
 * still be writing the old shape after the first one changes.
 *
 * @returns null for a folder that needs no credentials — which is not the same
 *   as a folder handed empty ones.
 */
export function webdavCredentials(username: string, password: string): Promise<string | null> {
  return call('webdav_credentials', { username, password });
}

/**
 * The shortest master password the vault accepts.
 *
 * Asked for rather than known: the rule belongs to the core, and a form that
 * keeps its own copy will one day refuse a password the vault would have
 * taken, or take one it would have refused.
 */
export function masterPasswordMinLength(): Promise<number> {
  return call('master_password_min_length');
}

export interface OnboardingStatus {
  completed: boolean;
}

/** Whether first-run onboarding already ran (or was skipped) on this data dir. */
export function onboardingStatus(): Promise<OnboardingStatus> {
  return call('onboarding_status');
}

/** Persists the completion marker; finishing and skipping both call this. */
export function onboardingComplete(): Promise<void> {
  return call('onboarding_complete');
}

/**
 * Clipboard text for the onboarding first-snippet prefill, already filtered
 * host-side: empty, oversized, and suspected-secret content comes back null.
 */
export function clipboardReadText(): Promise<string | null> {
  return call('clipboard_read_text');
}

// --- Vault ------------------------------------------------------------------
//
// The master password crosses this boundary as a plain string (the user just
// typed it); it is used host-side to derive the KEK and is never stored or
// logged. `vaultReveal` is the only path that returns sensitive plaintext, and
// only on an unlocked session.

/** Current vault-session state (no key material). Safe to poll while on-page. */
export function vaultStatus(): Promise<VaultStatus> {
  return call('vault_status');
}

/** First-run setup: sets the master password and leaves the vault unlocked. */
export function vaultInitialize(password: string): Promise<VaultStatus> {
  return call('vault_initialize', { password });
}

/**
 * Unlocks with the master password. Rejects with `permission_denied` on a wrong
 * password or while throttled — the message never says which part was wrong.
 */
export function vaultUnlockPassword(password: string): Promise<VaultStatus> {
  return call('vault_unlock_password', { password });
}

/**
 * Unlocks via the biometric-gated key copy (Touch ID). The OS enforces the
 * gate; rejects with `conflict` when biometrics were never enrolled.
 */
export function vaultUnlockBiometric(): Promise<VaultStatus> {
  return call('vault_unlock_biometric');
}

/** Locks the vault immediately (re-lock is instantaneous). */
export function vaultLock(): Promise<VaultStatus> {
  return call('vault_lock');
}

/** Enrolls a biometric-gated key copy so later unlocks can use Touch ID. */
export function vaultEnableBiometric(): Promise<void> {
  return call('vault_enable_biometric');
}

/** Removes the biometric key copy (idempotent). Master password is unaffected. */
export function vaultDisableBiometric(): Promise<void> {
  return call('vault_disable_biometric');
}

/**
 * How many secrets a vault reset would permanently destroy, trashed rows
 * included — the honest number for the confirm copy. Works while locked.
 */
export function vaultResetPreview(): Promise<number> {
  return call('vault_reset_preview');
}

/**
 * Destroys the vault for a lost master password: deletes every
 * sensitive snippet (tombstoned so other devices converge), all key material
 * and the biometric copy, and returns the fresh uninitialized status. There
 * is no undo — the UI confirms with the real count before calling this.
 */
export function vaultReset(): Promise<VaultStatus> {
  return call('vault_reset');
}

/** Lists the vault's sensitive snippets (metadata only; `body` is always null). */
export function vaultList(limit: number, offset: number): Promise<Snippet[]> {
  return call('vault_list', { limit, offset });
}

/**
 * Decrypts one sensitive snippet's content for transient display ("Reveal").
 * Requires an unlocked session; rejects with `permission_denied` when locked.
 */
export function vaultReveal(id: string): Promise<string> {
  return call('vault_reveal', { id });
}

/**
 * Mobile host: one-time sensitive copy for the iOS secure-field round trip.
 * Decrypts the secret onto the system pasteboard with an OS-enforced expiry
 * (the plaintext never crosses IPC) and resolves with the clear delay in ms
 * for the countdown. Rejects with `permission_denied` when the vault is locked.
 */
export function vaultCopySecret(id: string): Promise<number> {
  return call('vault_copy_secret', { id });
}

/** Creates a sensitive snippet; the body is encrypted host-side before storage. */
export function vaultCreateSecret(input: SnippetCreateInput): Promise<Snippet> {
  return call('vault_create_secret', { input });
}

/** Re-encrypts an edited sensitive snippet's body (already-sensitive only). */
export function vaultUpdateSecret(input: SnippetUpdateInput): Promise<Snippet> {
  return call('vault_update_secret', { input });
}

/**
 * Promotes a normal snippet to sensitive: encrypts its plaintext and rebuilds
 * the index so the former plaintext leaves index storage. Requires unlock.
 */
export function snippetConvertToSensitive(id: string): Promise<Snippet> {
  return call('snippet_convert_to_sensitive', { id });
}

/** Mobile shell boot smoke: schema version + snippet count from the device DB. */
export function mobileBootstrap(): Promise<MobileBootstrap> {
  return call('mobile_bootstrap');
}

/**
 * Mobile host: opens the system Settings app so the user can add the Typvia
 * keyboard — iOS offers no programmatic enablement.
 * Resolving only means the jump was dispatched, never that the keyboard was
 * added; on hosts without a Settings deep link this is a successful no-op.
 */
export function openKeyboardSettings(): Promise<void> {
  return call('open_keyboard_settings');
}

/**
 * Mobile host: regenerates the keyboard snapshot in the App Group container.
 * The write is atomic host-side — on failure the previous snapshot stays in
 * place — so callers treat this as best-effort after a data change.
 */
export function snapshotRefresh(): Promise<void> {
  return call('snapshot_refresh');
}

/**
 * Mobile host: drains the share-extension inbox (App Group `inbox/*.json`)
 * into the library and resolves with how many snippets were created —
 * including items the host already ingested itself at launch or on
 * foreground. Resolves 0 on hosts without an App Group container.
 */
export function shareInboxIngest(): Promise<number> {
  return call('share_inbox_ingest');
}

// --- Sync & devices ---------------------------------------------------------
//
// Mirrors apps/desktop/src-tauri/src/sync/. No key material, no recovery code
// and no entity content is ever persisted on this side: the recovery code is
// returned once for display, and sensitive conflict bodies arrive as `null`.

/** State of sync on this device, for the Sync page and the Settings group. */
export interface SyncStatus {
  /** False when the platform has no secure key storage: sync cannot be set up. */
  available: boolean;
  /** An account is bound, even when sync is switched off. */
  configured: boolean;
  enabled: boolean;
  serverUrl: string | null;
  accountId: string | null;
  deviceId: string;
  deviceName: string;
  /** Current K_sync generation; rises on every rotation. */
  keyGeneration: number;
  /** Records still waiting in the outbox. */
  pendingBacklog: number;
  /** Conflict copies awaiting a decision. */
  conflictCount: number;
  lastSyncAt: number | null;
  vaultReady: boolean;
  vaultUnlocked: boolean;
  recoveryExportedAt: number | null;
  /**
   * A recovery finished but its historical catch-up has not drained yet;
   * the background rounds resume it automatically.
   */
  recoveryCatchupPending: boolean;
  /** Which sync backend the account is bound to. */
  transportKind: 'server' | 'webdav';
}

/**
 * Founds an account on the user's own WebDAV endpoint. The endpoint
 * is probed for the conditional-request semantics the protocol needs and
 * refused honestly when it strips them. `credentials` is `basic:<user>:<pw>`
 * or `bearer:<token>` — it goes into the platform secure store only.
 */
export function syncEnableWebdav(baseUrl: string, credentials: string | null): Promise<SyncStatus> {
  return call('sync_enable_webdav', { baseUrl, credentials });
}

/** Starts joining an existing WebDAV account from this (new) device. */
export function pairingBeginWebdav(
  baseUrl: string,
  credentials: string | null,
): Promise<PairingStart> {
  return call('pairing_begin_webdav', { baseUrl, credentials });
}

/** Recovers a WebDAV account with the recovery code. */
export function recoveryRecoverWebdav(
  baseUrl: string,
  credentials: string | null,
  code: string,
  masterPassword: string,
): Promise<SyncStatus> {
  return call('recovery_recover_webdav', { baseUrl, credentials, code, masterPassword });
}

/** One device on the account — one node on the route drawing. */
export interface SyncDevice {
  deviceId: string;
  name: string;
  platform: string;
  createdAt: number;
  revokedAt: number | null;
  /** Its certificate chain verifies to the pinned trust root. */
  verified: boolean;
  isThisDevice: boolean;
  isRoot: boolean;
}

/** What one sync round did — counts only, never which entities. */
export interface SyncRound {
  pushed: number;
  applied: number;
  merged: number;
  conflictCopies: number;
  parked: number;
  skipped: number;
  pendingBacklog: number;
  at: number;
}

/** A started pairing session on the joining device. */
export interface PairingStart {
  /** `TYPVIA-PAIR.V1.<base64>` — the QR payload and the paste fallback. */
  code: string;
  sessionId: string;
}

/** The admitting device's view of a scanned code. */
export interface PairingSas {
  sas: string;
  deviceName: string;
  platform: string;
  vaultReady: boolean;
}

/** The joining device's view once the offer arrived. */
export interface PairingClaim {
  sas: string;
  rootFingerprint: string;
}

/** A generated recovery code, returned exactly once. */
export interface RecoveryCode {
  code: string;
  accountId: string;
  serverUrl: string;
}

/** Which body survives a conflict decision. */
export type ConflictKeep = 'source' | 'copy' | 'both';

/**
 * One snippet changed in two places: the body that stayed on the entity and
 * the copy carrying the other device's body. Sensitive pairs arrive with both
 * bodies `null` — a locked conflict is read as two locked rows.
 */
export interface ConflictPair {
  source: Snippet;
  copy: Snippet;
  sensitive: boolean;
}

export function syncStatus(): Promise<SyncStatus> {
  return call('sync_status');
}

/**
 * Creates the account on `serverUrl` with this device as its trust root and
 * starts queueing local changes.
 */
export function syncEnable(serverUrl: string): Promise<SyncStatus> {
  return call('sync_enable', { serverUrl });
}

/** Turns sync off. The queue, the keys and the account binding all stay. */
export function syncDisable(): Promise<SyncStatus> {
  return call('sync_disable');
}

/** Switches sync back on for the already-bound account. */
export function syncResume(): Promise<SyncStatus> {
  return call('sync_resume');
}

/**
 * Runs one push → pull → merge round now. Rejects with `unavailable` when the
 * server could not be reached — that is the offline state, not a failure.
 */
export function syncNow(): Promise<SyncRound> {
  return call('sync_now');
}

/** The account's devices, pinned and verified against the trust root. */
export function syncDevices(): Promise<SyncDevice[]> {
  return call('sync_devices');
}

/**
 * Revokes another device and rotates the sync key in the same call — a
 * revoked device still holds the old generation. Resolves with the new one.
 */
export function syncRevokeDevice(deviceId: string): Promise<number> {
  return call('sync_revoke_device', { deviceId });
}

/** Rotates the sync key on demand. Resolves with the new generation. */
export function syncRotateKey(): Promise<number> {
  return call('sync_rotate_key');
}

/** Starts pairing on this (joining) device and returns the code to display. */
export function pairingBegin(serverUrl: string, accountId: string): Promise<PairingStart> {
  return call('pairing_begin', { serverUrl, accountId });
}

/**
 * Polls for the other device's offer. Resolves `null` while it has not
 * confirmed. A ready offer returns the short code for the user's check —
 * nothing is installed until {@link pairingFinalize}.
 */
export function pairingPoll(): Promise<PairingClaim | null> {
  return call('pairing_poll');
}

/**
 * Completes pairing after the user compared the short code on both screens.
 * `masterPassword` is required when the offer carries vault material.
 */
export function pairingFinalize(masterPassword: string | null): Promise<SyncStatus> {
  return call('pairing_finalize', { masterPassword });
}

/** Abandons an in-flight pairing session on this device. */
export function pairingCancel(): Promise<void> {
  return call('pairing_cancel');
}

/** Reads a pairing code and returns the short code to compare. Admits nothing. */
export function pairingSas(code: string): Promise<PairingSas> {
  return call('pairing_sas', { code });
}

/**
 * Admits the device after the user compared the short code on both screens.
 * `allowVault` decides whether the vault keys travel with the bundle.
 */
export function pairingApprove(code: string, allowVault: boolean): Promise<void> {
  return call('pairing_approve', { code, allowVault });
}

/**
 * Generates a recovery code and uploads its wrapped blob. The code is shown
 * once and never stored — a second call replaces it and voids the previous
 * one. Requires an unlocked vault.
 */
export function recoveryPublish(): Promise<RecoveryCode> {
  return call('recovery_publish');
}

/**
 * Recovers an account on this device: the trust root is replaced, every
 * previous device is revoked, and the sync key is rotated. `masterPassword`
 * is the one this device will use for its own vault afterwards.
 */
export function recoveryRecover(
  serverUrl: string,
  accountId: string,
  code: string,
  masterPassword: string,
): Promise<SyncStatus> {
  return call('recovery_recover', { serverUrl, accountId, code, masterPassword });
}

/** The conflict pairs awaiting a decision. */
export function syncConflicts(): Promise<ConflictPair[]> {
  return call('sync_conflicts');
}

/**
 * Records the decision. The version not kept moves to the Trash (or stays as
 * its own snippet for `both`); nothing is destroyed.
 */
export function syncConflictResolve(copyId: string, keep: ConflictKeep): Promise<void> {
  return call('sync_conflict_resolve', { copyId, keep });
}

/** The tray card's shortlist: recent snippets it can insert as they are. */
export function trayResults(limit: number): Promise<Snippet[]> {
  return call('tray_results', { limit });
}

/** Inserts from the tray card into the app that was frontmost when it opened. */
export function trayInsert(id: string, method?: InjectionMethod): Promise<void> {
  return call('tray_insert', { id, method: method ?? null });
}

/** The tray card has laid out at `height` logical pixels; place and show it. */
export function trayPresent(height: number): Promise<void> {
  return call('tray_present', { height });
}

export function hideTray(): Promise<void> {
  return call('tray_hide');
}

export function trayOpenLibrary(): Promise<void> {
  return call('tray_open_library');
}

export function quitApp(): Promise<void> {
  return call('app_quit');
}

export function insertionPauseStatus(): Promise<InsertionPause> {
  return call('insertion_pause_status');
}

export function pauseInsertion(minutes: number): Promise<InsertionPause> {
  return call('insertion_pause', { minutes });
}

export function resumeInsertion(): Promise<InsertionPause> {
  return call('insertion_resume');
}

export function autostartStatus(): Promise<boolean> {
  return call('autostart_status');
}

export function setAutostart(enabled: boolean): Promise<boolean> {
  return call('autostart_set', { enabled });
}
