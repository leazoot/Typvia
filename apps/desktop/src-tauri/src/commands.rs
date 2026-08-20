//! Tauri command layer: parameter parsing, state locking, clock injection
//! and error mapping only — all behaviour lives in `service` / core.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use typvia_espanso_adapter::{CoexistenceChoice, EngineGate, EngineSupervisor};

use crate::espanso_cli::ResolvedEspansoCli;

use typvia_core::vault::{SecureStore, VaultSession};

use typvia_host_service::dto::{
    AppRuleCreateInput, AppRuleDto, AppRuleUpdateInput, BackupRestoreDto,
    BrowserIntegrationStatusDto, EspansoImportDto, EspansoStatusDto, EspansoSyncDto,
    FolderCreateInput, FolderDto, FolderUpdateInput, HistoryDto, ImportReportDto, LibraryCountsDto,
    OnboardingStatusDto, PanelResultsDto, SearchHitDto, SemanticStatusDto, SnippetCreateInput,
    SnippetDto, SnippetUpdateInput, TagDto, TemplateFieldDto, VaultStatusDto, VersionBodyDto,
};
use typvia_host_service::error::IpcError;
use typvia_host_service::service;

use crate::injector::{InjectionMethod, Injector};
use crate::panel;

/// Single-writer SQLite connection (WAL, database rules): one mutex, no pool.
/// The injector is stateful (owns a clipboard handle) and reused across calls.
/// The vault session holds the master key while unlocked; the secure store is
/// the platform biometric-gated backend.
pub struct AppState {
    // Shared (Arc) so the AI egress-log sink can write through the same
    // single-writer connection while no command holds the lock.
    conn: Arc<Mutex<Connection>>,
    injector: Mutex<Box<dyn Injector>>,
    vault: Mutex<VaultSession>,
    secure_store: Box<dyn SecureStore + Send + Sync>,
    // Managed espanso engine: private directories plus the daemon
    // supervisor; the user's own espanso installation is never touched.
    engine: EngineSupervisor,
}

impl AppState {
    pub fn new(
        conn: Connection,
        injector: Box<dyn Injector>,
        secure_store: Box<dyn SecureStore + Send + Sync>,
        engine: EngineSupervisor,
    ) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            injector: Mutex::new(injector),
            vault: Mutex::new(VaultSession::new()),
            secure_store,
            engine,
        }
    }

    /// The managed espanso engine supervisor (exit hook, status surface).
    pub(crate) fn engine(&self) -> &EngineSupervisor {
        &self.engine
    }

    /// Handle for components that must lock the connection on their own
    /// schedule (the AI egress sink); never call while holding `lock()`.
    pub(crate) fn conn_handle(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, IpcError> {
        // A poisoned mutex means a command panicked mid-write; surface it as
        // a system error instead of poisoning every later call with a panic.
        self.conn.lock().map_err(|_| IpcError::system())
    }

    fn lock_injector(&self) -> Result<std::sync::MutexGuard<'_, Box<dyn Injector>>, IpcError> {
        self.injector.lock().map_err(|_| IpcError::system())
    }

    pub(crate) fn lock_vault(&self) -> Result<std::sync::MutexGuard<'_, VaultSession>, IpcError> {
        self.vault.lock().map_err(|_| IpcError::system())
    }

    pub(crate) fn secure_store(&self) -> &(dyn SecureStore + Send + Sync) {
        self.secure_store.as_ref()
    }

    /// Locks the vault for a system event (sleep / screen lock):
    /// drops the master key without touching the DB connection. A poisoned
    /// mutex is ignored — there is no caller to surface it to, and the next
    /// vault command will report it.
    #[cfg(target_os = "macos")]
    pub fn lock_vault_for_system_event(&self) {
        if let Ok(mut session) = self.vault.lock() {
            session.lock();
        }
    }
}

/// Maps the optional wire method name onto an [`InjectionMethod`]; absent
/// means the default, which is paste.
fn parse_method(method: Option<&str>) -> Result<InjectionMethod, IpcError> {
    match method {
        None => Ok(InjectionMethod::default()),
        Some("paste") => Ok(InjectionMethod::Paste),
        Some("keystrokes") => Ok(InjectionMethod::Keystrokes),
        Some(_) => Err(IpcError::validation("unknown injection method")),
    }
}

pub(crate) fn now_ms() -> Result<i64, IpcError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IpcError::system())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| IpcError::system())
}

#[tauri::command]
pub fn snippet_create(
    state: State<'_, AppState>,
    input: SnippetCreateInput,
) -> Result<SnippetDto, IpcError> {
    service::snippet_create(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn snippet_update(
    state: State<'_, AppState>,
    input: SnippetUpdateInput,
) -> Result<SnippetDto, IpcError> {
    service::snippet_update(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn snippet_get(state: State<'_, AppState>, id: String) -> Result<SnippetDto, IpcError> {
    service::snippet_get(&*state.lock()?, &id)
}

#[tauri::command]
pub fn snippet_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list(&*state.lock()?, limit, offset)
}

#[tauri::command]
pub fn snippet_list_by_folder(
    state: State<'_, AppState>,
    folder_id: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list_by_folder(&*state.lock()?, folder_id.as_deref(), limit, offset)
}

#[tauri::command]
pub fn snippet_list_page(
    state: State<'_, AppState>,
    view: String,
    folder_id: Option<String>,
    snippet_type: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list_page(
        &*state.lock()?,
        &view,
        folder_id.as_deref(),
        snippet_type.as_deref(),
        limit,
        offset,
    )
}

#[tauri::command]
pub fn snippet_count(
    state: State<'_, AppState>,
    view: String,
    folder_id: Option<String>,
    snippet_type: Option<String>,
) -> Result<u32, IpcError> {
    service::snippet_count(
        &*state.lock()?,
        &view,
        folder_id.as_deref(),
        snippet_type.as_deref(),
    )
}

#[tauri::command]
pub fn library_counts(state: State<'_, AppState>) -> Result<LibraryCountsDto, IpcError> {
    service::library_counts(&*state.lock()?)
}

#[tauri::command]
pub fn snippet_trash(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_trash(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn snippet_restore(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_restore(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn snippet_delete_forever(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_delete_forever(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn trash_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::trash_list(&*state.lock()?, limit, offset)
}

#[tauri::command]
pub fn trash_purge_expired(state: State<'_, AppState>) -> Result<usize, IpcError> {
    service::trash_purge_expired(&*state.lock()?, now_ms()?)
}

#[tauri::command]
pub fn snippet_batch_move(
    state: State<'_, AppState>,
    ids: Vec<String>,
    folder_id: Option<String>,
) -> Result<(), IpcError> {
    service::snippet_batch_move(&*state.lock()?, &ids, folder_id.as_deref(), now_ms()?)
}

#[tauri::command]
pub fn snippet_batch_add_tag(
    state: State<'_, AppState>,
    ids: Vec<String>,
    tag_id: String,
) -> Result<(), IpcError> {
    service::snippet_batch_add_tag(&*state.lock()?, &ids, &tag_id, now_ms()?)
}

#[tauri::command]
pub fn snippet_batch_trash(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), IpcError> {
    service::snippet_batch_trash(&*state.lock()?, &ids, now_ms()?)
}

#[tauri::command]
pub fn folder_create(
    state: State<'_, AppState>,
    input: FolderCreateInput,
) -> Result<FolderDto, IpcError> {
    service::folder_create(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn folder_update(
    state: State<'_, AppState>,
    input: FolderUpdateInput,
) -> Result<FolderDto, IpcError> {
    service::folder_update(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn folder_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::folder_delete(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn folder_list_children(
    state: State<'_, AppState>,
    parent_id: Option<String>,
) -> Result<Vec<FolderDto>, IpcError> {
    service::folder_list_children(&*state.lock()?, parent_id.as_deref())
}

#[tauri::command]
pub fn tag_create(state: State<'_, AppState>, name: String) -> Result<TagDto, IpcError> {
    service::tag_create(&*state.lock()?, name, now_ms()?)
}

#[tauri::command]
pub fn tag_list(state: State<'_, AppState>) -> Result<Vec<TagDto>, IpcError> {
    service::tag_list(&*state.lock()?)
}

#[tauri::command]
pub fn tag_rename(state: State<'_, AppState>, id: String, name: String) -> Result<(), IpcError> {
    service::tag_rename(&*state.lock()?, &id, &name, now_ms()?)
}

#[tauri::command]
pub fn tag_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::tag_delete(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn search_library(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::search_library(&*state.lock()?, &query, limit)
}

#[tauri::command]
pub fn snippet_inject(
    state: State<'_, AppState>,
    id: String,
    method: Option<String>,
) -> Result<(), IpcError> {
    let method = parse_method(method.as_deref())?;
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut injector = state.lock_injector()?;
    crate::service::snippet_inject(&conn, injector.as_mut(), &id, method, now)
}

#[tauri::command]
pub fn snippet_copy(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut injector = state.lock_injector()?;
    crate::service::snippet_copy(&conn, injector.as_mut(), &id, now)
}

/// Reads the panel's destination app and refuses the insert an app rule
/// blocks. Runs before the panel hides — the destination is still
/// set and a blocked insert keeps the panel up showing the notice.
fn gate_panel_insert(app: &AppHandle, id: &str, sensitive: bool) -> Result<(), IpcError> {
    let destination = app.state::<panel::PanelState>().destination_app();
    let state = app.state::<AppState>();
    let conn = state.lock()?;
    let platform = service::current_platform();
    if sensitive {
        service::panel_gate_insert_secret(&conn, id, platform, destination.as_deref())
    } else {
        service::panel_gate_insert(&conn, id, platform, destination.as_deref())
    }
}

/// Panel result page: recent or searched snippets minus the rows the
/// destination app's rules hide, with the hidden count for the notice line.
#[tauri::command]
pub fn panel_results(
    app: AppHandle,
    query: String,
    limit: u32,
) -> Result<PanelResultsDto, IpcError> {
    let destination = app.state::<panel::PanelState>().destination_app();
    let state = app.state::<AppState>();
    let conn = state.lock()?;
    service::panel_results(
        &conn,
        &query,
        limit,
        service::current_platform(),
        destination.as_deref(),
    )
}

/// Panel insert (⌵): hide the panel and restore the previous app on the main
/// thread, let the window server bring it frontmost, then inject into it. The
/// ordering is owned here (not the WebView) so the paste never races the focus
/// change. A `permission_denied` result lets the panel fall back to copy.
#[tauri::command]
pub async fn panel_insert(
    app: AppHandle,
    id: String,
    method: Option<String>,
) -> Result<(), IpcError> {
    let method = parse_method(method.as_deref())?;
    gate_panel_insert(&app, &id, false)?;
    let restore = app.clone();
    app.run_on_main_thread(move || panel::hide(&restore))
        .map_err(|_| IpcError::system())?;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(panel::FOCUS_SETTLE_MS));
        let now = now_ms()?;
        let state = app.state::<AppState>();
        let conn = state.lock()?;
        let mut injector = state.lock_injector()?;
        crate::service::snippet_inject(&conn, injector.as_mut(), &id, method, now)
    })
    .await
    .map_err(|_| IpcError::system())?
}

/// Panel insert of a sensitive snippet (⌵ on a vault item): the same host-owned
/// hide/settle/inject ordering as `panel_insert`, but the body is decrypted from
/// the unlocked vault and wiped after delivery — the plaintext never crosses
/// IPC. A locked vault surfaces `permission_denied` so the panel prompts unlock.
#[tauri::command]
pub async fn panel_insert_secret(
    app: AppHandle,
    id: String,
    method: Option<String>,
) -> Result<(), IpcError> {
    let method = parse_method(method.as_deref())?;
    gate_panel_insert(&app, &id, true)?;
    let restore = app.clone();
    app.run_on_main_thread(move || panel::hide(&restore))
        .map_err(|_| IpcError::system())?;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(panel::FOCUS_SETTLE_MS));
        let now = now_ms()?;
        let state = app.state::<AppState>();
        let conn = state.lock()?;
        let mut vault = state.lock_vault()?;
        let mut injector = state.lock_injector()?;
        crate::service::snippet_inject_secret(
            &conn,
            &mut vault,
            injector.as_mut(),
            &id,
            method,
            now,
        )
    })
    .await
    .map_err(|_| IpcError::system())?
}

/// Panel copy of a sensitive snippet (⇧⌵ on a vault item): decrypts from the
/// unlocked vault onto the clipboard (guarded) and schedules the timed
/// auto-clear off the command thread. Returns the clear delay in
/// ms so the panel can show the countdown. A locked vault surfaces
/// `permission_denied` for the panel to prompt unlock.
#[tauri::command]
pub fn panel_copy_secret(app: AppHandle, id: String) -> Result<i64, IpcError> {
    let now = now_ms()?;
    {
        let state = app.state::<AppState>();
        let conn = state.lock()?;
        let mut vault = state.lock_vault()?;
        let mut injector = state.lock_injector()?;
        crate::service::snippet_copy_secret(&conn, &mut vault, injector.as_mut(), &id, now)?;
    }
    // The auto-clear wipes the copy only if it is still on the clipboard when
    // the countdown elapses; a copy the user made in between is left alone.
    let delay = service::SENSITIVE_CLIPBOARD_CLEAR_MS;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(delay.unsigned_abs()));
        let state = handle.state::<AppState>();
        if let Ok(mut injector) = state.lock_injector() {
            let _ = crate::service::clipboard_clear_secret(injector.as_mut());
        }
    });
    Ok(delay)
}

#[tauri::command]
pub fn detect_sensitive(text: String) -> Vec<String> {
    service::detect_sensitive(&text)
}

#[tauri::command]
pub fn template_variables(body: String) -> Result<Vec<String>, IpcError> {
    service::template_variables(&body)
}

#[tauri::command]
pub fn template_fields(
    state: State<'_, AppState>,
    snippet_id: String,
) -> Result<Vec<TemplateFieldDto>, IpcError> {
    service::template_fields(&*state.lock()?, &snippet_id)
}

#[tauri::command]
pub fn template_save_fields(
    state: State<'_, AppState>,
    snippet_id: String,
    fields: Vec<TemplateFieldDto>,
) -> Result<Vec<TemplateFieldDto>, IpcError> {
    service::template_save_fields(&*state.lock()?, &snippet_id, fields, now_ms()?)
}

#[tauri::command]
pub fn template_preview(
    body: String,
    fields: Vec<TemplateFieldDto>,
    values: std::collections::HashMap<String, String>,
) -> Result<String, IpcError> {
    service::template_preview(&body, fields, values)
}

#[tauri::command]
pub fn template_render(
    state: State<'_, AppState>,
    snippet_id: String,
    values: std::collections::HashMap<String, String>,
) -> Result<String, IpcError> {
    service::template_render(&*state.lock()?, &snippet_id, &values)
}

/// Panel template call: render the filled template and inject it into the app
/// the panel was summoned from, then record usage — the same host-owned
/// hide/settle/inject ordering as `panel_insert`, so the paste never
/// races the focus hand-off.
#[tauri::command]
pub async fn panel_insert_template(
    app: AppHandle,
    id: String,
    values: std::collections::HashMap<String, String>,
    method: Option<String>,
) -> Result<(), IpcError> {
    let method = parse_method(method.as_deref())?;
    gate_panel_insert(&app, &id, false)?;
    let restore = app.clone();
    app.run_on_main_thread(move || panel::hide(&restore))
        .map_err(|_| IpcError::system())?;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(panel::FOCUS_SETTLE_MS));
        let now = now_ms()?;
        let state = app.state::<AppState>();
        let conn = state.lock()?;
        let mut injector = state.lock_injector()?;
        crate::service::template_inject(&conn, injector.as_mut(), &id, &values, method, now)
    })
    .await
    .map_err(|_| IpcError::system())?
}

#[tauri::command]
pub fn search_snippets(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
    offset: u32,
) -> Result<Vec<SearchHitDto>, IpcError> {
    service::search_snippets(&*state.lock()?, &query, limit, offset)
}

/// Resolves the generated match file inside the managed engine's private
/// config directory, materialising the layout first. No DB lock and
/// no espanso CLI involved.
fn espanso_config_path(state: &AppState) -> Result<std::path::PathBuf, IpcError> {
    let dirs = state.engine.dirs();
    dirs.ensure().map_err(|_| IpcError::system())?;
    Ok(dirs.typvia_match_path())
}

#[tauri::command]
pub fn espanso_status(state: State<'_, AppState>) -> Result<EspansoStatusDto, IpcError> {
    let target = espanso_config_path(&state)?;
    // Self-heal before reporting: an enabled engine whose gate is open should
    // be running by the time the state maps (status is a low-frequency call).
    engine_autostart(&state);
    let gate = typvia_espanso_adapter::engine_gate(&ResolvedEspansoCli, state.engine.dirs());
    let choice = typvia_espanso_adapter::load_choice(state.engine.dirs());
    let engine = state.engine.state();
    let enabled = !state.engine.dirs().is_disabled();
    crate::service::espanso_status(
        &*state.lock()?,
        ResolvedEspansoCli,
        &target,
        enabled,
        gate,
        engine,
        choice,
    )
}

/// Regenerates the espanso config from the current snippet set (also the
/// "enable" action). Path resolution runs before the DB lock is taken.
/// Reconcile the managed engine with the coexistence gate: start it only when
/// the gate allows (a running user-owned espanso always wins until the user
/// has decided — never a silent takeover), and *yield* — stop our
/// running engine — when the gate closes again, e.g. the user's instance came
/// back mid-session. Runs on launch, config sync and every status read.
pub(crate) fn engine_autostart(state: &AppState) {
    let gate = typvia_espanso_adapter::engine_gate(&ResolvedEspansoCli, state.engine.dirs());
    if gate == EngineGate::Start {
        // Failure lands in the supervisor state the status surface reports.
        let _ = state.engine.ensure_running();
    } else if matches!(
        state.engine.state(),
        typvia_espanso_adapter::EngineState::Running { .. }
            | typvia_espanso_adapter::EngineState::Retrying { .. }
    ) {
        state.engine.stop();
    }
}

/// Launch-time bootstrap: the engine is on by default. Unless the
/// user explicitly turned it off, regenerate the config from the current
/// snippet set and run the gated start — no click required.
pub(crate) fn engine_bootstrap(state: &AppState) {
    let dirs = state.engine.dirs();
    if dirs.is_disabled() || dirs.ensure().is_err() {
        return;
    }
    if let Ok(conn) = state.lock() {
        let _ = crate::service::espanso_regenerate(&conn, &dirs.typvia_match_path());
    }
    engine_autostart(state);
}

#[tauri::command]
pub fn espanso_sync(state: State<'_, AppState>) -> Result<EspansoSyncDto, IpcError> {
    let target = espanso_config_path(&state)?;
    // An explicit sync is also the "turn on" action: clear the off switch.
    let _ = state.engine.dirs().set_disabled(false);
    let trigger_count = crate::service::espanso_regenerate(&*state.lock()?, &target)?;
    // The config write is the command's result; the engine start is gated and
    // best-effort on top — it must not fail a sync whose file is written.
    engine_autostart(&state);
    Ok(EspansoSyncDto {
        enabled: true,
        trigger_count,
    })
}

/// Records the user's answer to the coexistence prompt ("takeover" /
/// "stand_aside") and re-runs the gated start.
#[tauri::command]
pub fn espanso_coexistence(state: State<'_, AppState>, choice: String) -> Result<(), IpcError> {
    let parsed = CoexistenceChoice::parse(&choice)
        .ok_or_else(|| IpcError::validation("unknown coexistence choice"))?;
    let dirs = state.engine.dirs();
    dirs.ensure().map_err(|_| IpcError::system())?;
    typvia_espanso_adapter::store_choice(dirs, parsed).map_err(|_| IpcError::system())?;
    engine_autostart(&state);
    Ok(())
}

/// Removes Typvia's espanso config file and stops the managed engine (turns
/// the integration off).
#[tauri::command]
pub fn espanso_disable(state: State<'_, AppState>) -> Result<EspansoSyncDto, IpcError> {
    let target = espanso_config_path(&state)?;
    crate::service::espanso_remove(&target)?;
    // Record the explicit choice so the default-on bootstrap respects it.
    let _ = state.engine.dirs().set_disabled(true);
    state.engine.stop();
    Ok(EspansoSyncDto {
        enabled: false,
        trigger_count: 0,
    })
}

/// Imports the plain trigger/replace pairs from an espanso match file's text.
#[tauri::command]
pub fn espanso_import(
    state: State<'_, AppState>,
    content: String,
) -> Result<EspansoImportDto, IpcError> {
    crate::service::espanso_import(&*state.lock()?, &content, now_ms()?)
}

/// Imports snippets from the text of a Markdown, JSON, or CSV file.
#[tauri::command]
pub fn snippets_import(
    state: State<'_, AppState>,
    format: String,
    content: String,
) -> Result<ImportReportDto, IpcError> {
    service::snippets_import(&*state.lock()?, &format, &content, now_ms()?)
}

/// Exports the whole library as an encrypted backup file's text. The
/// passphrase crosses the boundary once, derives the backup key host-side,
/// and is dropped — never stored or logged. The returned text is ciphertext
/// plus KDF parameters only.
#[tauri::command]
pub fn backup_export(state: State<'_, AppState>, passphrase: String) -> Result<String, IpcError> {
    service::backup_export(&*state.lock()?, &passphrase, now_ms()?)
}

/// Writes an encrypted backup into the user's Downloads folder and returns
/// the file name for the UI to state. The destination is host-chosen — no
/// filesystem path crosses IPC, by design — and the millisecond timestamp
/// keeps every export a distinct file.
#[tauri::command]
pub fn backup_export_file(
    app: AppHandle,
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<String, IpcError> {
    let now = now_ms()?;
    let document = service::backup_export(&*state.lock()?, &passphrase, now)?;
    let dir = app.path().download_dir().map_err(|_| IpcError::system())?;
    let name = format!("Typvia-backup-{now}.json");
    std::fs::write(dir.join(&name), document).map_err(|_| IpcError::system())?;
    Ok(name)
}

/// Restores an encrypted backup into an empty library. A wrong passphrase,
/// tampering, or damage fail with the same message.
#[tauri::command]
pub fn backup_restore(
    state: State<'_, AppState>,
    passphrase: String,
    content: String,
) -> Result<BackupRestoreDto, IpcError> {
    service::backup_restore(&*state.lock()?, &passphrase, &content, now_ms()?)
}

// --- Vault commands ---------------------------------------------------------
//
// Every vault command locks the connection first, then the vault session, in
// that order (avoiding a lock-order cycle). The master password crosses the
// boundary as a plain `String` (the user typed it in the unlock UI); it is
// used to derive the KEK and dropped — never stored or logged.

#[tauri::command]
pub fn history_list(
    state: State<'_, AppState>,
    id: String,
    limit: u32,
    offset: u32,
) -> Result<HistoryDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::history_list(&conn, &mut session, &id, limit, offset, now)
}

#[tauri::command]
pub fn history_get(
    state: State<'_, AppState>,
    id: String,
    version: u32,
) -> Result<VersionBodyDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::history_get(&conn, &mut session, &id, version, now)
}

#[tauri::command]
pub fn history_restore(
    state: State<'_, AppState>,
    id: String,
    version: u32,
) -> Result<SnippetDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::history_restore(&conn, &mut session, &id, version, now)
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> Result<VaultStatusDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_status(&conn, &mut session, now)
}

#[tauri::command]
pub fn vault_initialize(
    state: State<'_, AppState>,
    password: String,
) -> Result<VaultStatusDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_initialize(&conn, &mut session, &password, now)
}

#[tauri::command]
pub fn vault_unlock_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<VaultStatusDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_unlock_password(&conn, &mut session, &password, now)
}

#[tauri::command]
pub fn vault_unlock_biometric(state: State<'_, AppState>) -> Result<VaultStatusDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_unlock_biometric(&conn, &mut session, state.secure_store(), now)
}

#[tauri::command]
pub fn vault_lock(state: State<'_, AppState>) -> Result<VaultStatusDto, IpcError> {
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_lock(&conn, &mut session)
}

#[tauri::command]
pub fn vault_enable_biometric(state: State<'_, AppState>) -> Result<(), IpcError> {
    let session = state.lock_vault()?;
    service::vault_enable_biometric(&session, state.secure_store())
}

#[tauri::command]
pub fn vault_disable_biometric(state: State<'_, AppState>) -> Result<(), IpcError> {
    let session = state.lock_vault()?;
    service::vault_disable_biometric(&session, state.secure_store())
}

#[tauri::command]
pub fn vault_reset_preview(state: State<'_, AppState>) -> Result<u32, IpcError> {
    service::vault_reset_preview(&*state.lock()?)
}

#[tauri::command]
pub fn vault_reset(state: State<'_, AppState>) -> Result<VaultStatusDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_reset(&conn, &mut session, state.secure_store(), now)
}

#[tauri::command]
pub fn vault_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::vault_list(&*state.lock()?, limit, offset)
}

#[tauri::command]
pub fn vault_reveal(state: State<'_, AppState>, id: String) -> Result<String, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    service::vault_reveal(&conn, &mut session, &id, now)
}

#[tauri::command]
pub fn vault_create_secret(
    state: State<'_, AppState>,
    input: SnippetCreateInput,
) -> Result<SnippetDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let session = state.lock_vault()?;
    service::vault_create_secret(&conn, &session, input, now)
}

#[tauri::command]
pub fn vault_update_secret(
    state: State<'_, AppState>,
    input: SnippetUpdateInput,
) -> Result<SnippetDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let session = state.lock_vault()?;
    service::vault_update_secret(&conn, &session, input, now)
}

#[tauri::command]
pub fn snippet_convert_to_sensitive(
    state: State<'_, AppState>,
    id: String,
) -> Result<SnippetDto, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let session = state.lock_vault()?;
    service::snippet_convert_to_sensitive(&conn, &session, &id, now)
}

// ===== App rules =====

#[tauri::command]
pub fn app_rule_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<AppRuleDto>, IpcError> {
    let conn = state.lock()?;
    service::app_rule_list(&conn, service::current_platform(), limit, offset)
}

#[tauri::command]
pub fn app_rule_create(
    state: State<'_, AppState>,
    input: AppRuleCreateInput,
) -> Result<AppRuleDto, IpcError> {
    let conn = state.lock()?;
    service::app_rule_create(&conn, service::current_platform(), input, now_ms()?)
}

#[tauri::command]
pub fn app_rule_update(
    state: State<'_, AppState>,
    id: String,
    input: AppRuleUpdateInput,
) -> Result<AppRuleDto, IpcError> {
    let conn = state.lock()?;
    service::app_rule_update(&conn, &id, input, now_ms()?)
}

#[tauri::command]
pub fn app_rule_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    let conn = state.lock()?;
    service::app_rule_delete(&conn, &id, now_ms()?)
}

fn onboarding_dir(app: &AppHandle) -> Result<std::path::PathBuf, IpcError> {
    app.path().app_data_dir().map_err(|_| IpcError::system())
}

#[tauri::command]
pub fn semantic_status(
    app: AppHandle,
    state: State<'_, AppState>,
    semantic: State<'_, std::sync::Arc<crate::semantic::SemanticState>>,
) -> Result<SemanticStatusDto, IpcError> {
    let data_dir = onboarding_dir(&app)?;
    let (downloading, received, failed) = crate::semantic::download_progress(&semantic);
    let conn = state.lock()?;
    let repo = typvia_core::repo::EmbeddingRepo::new(&conn);
    Ok(SemanticStatusDto {
        model_present: crate::semantic::model_present(&data_dir),
        downloading,
        download_received: received,
        download_total: crate::semantic::total_download_bytes(),
        download_failed: failed,
        embedded_count: repo.count_for_model(crate::semantic::MODEL_ID)?,
        pending_count: repo.count_pending(crate::semantic::MODEL_ID)?,
        model_id: crate::semantic::MODEL_ID.to_string(),
    })
}

#[tauri::command]
pub fn semantic_model_download(
    app: AppHandle,
    semantic: State<'_, std::sync::Arc<crate::semantic::SemanticState>>,
) -> Result<(), IpcError> {
    crate::semantic::start_download(std::sync::Arc::clone(&semantic), onboarding_dir(&app)?);
    Ok(())
}

#[tauri::command]
pub fn semantic_model_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    semantic: State<'_, std::sync::Arc<crate::semantic::SemanticState>>,
) -> Result<(), IpcError> {
    crate::semantic::delete_model(&semantic, &*state.lock()?, &onboarding_dir(&app)?)
}

/// Drains the pending-embedding queue in the background; a no-op without a
/// model. Poked from the same debounced mutation signal as the other
/// derived surfaces.
#[tauri::command]
pub fn semantic_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    semantic: State<'_, std::sync::Arc<crate::semantic::SemanticState>>,
) -> Result<(), IpcError> {
    crate::semantic::spawn_embed_worker(
        std::sync::Arc::clone(&semantic),
        state.conn_handle(),
        onboarding_dir(&app)?,
    );
    Ok(())
}

/// Deep library search: lexical hits first, then semantic-only hits above
/// the score floor. Identical to lexical search while no model is on disk.
#[tauri::command]
pub fn search_library_deep(
    app: AppHandle,
    state: State<'_, AppState>,
    semantic: State<'_, std::sync::Arc<crate::semantic::SemanticState>>,
    query: String,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    crate::semantic::search_deep(
        &semantic,
        &*state.lock()?,
        &onboarding_dir(&app)?,
        &query,
        limit,
    )
}

#[tauri::command]
pub fn browser_integration_status(app: AppHandle) -> Result<BrowserIntegrationStatusDto, IpcError> {
    Ok(BrowserIntegrationStatusDto {
        enabled: crate::browser_integration::is_enabled(&onboarding_dir(&app)?),
        host_installed: crate::browser_integration::host_installed(),
    })
}

#[tauri::command]
pub fn browser_integration_enable(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    crate::browser_integration::enable(&*state.lock()?, &onboarding_dir(&app)?, now_ms()?)
}

#[tauri::command]
pub fn browser_integration_disable(app: AppHandle) -> Result<(), IpcError> {
    crate::browser_integration::disable(&onboarding_dir(&app)?)
}

/// Debounced post-mutation refresh of the browser snapshot; a successful
/// no-op while integration is off (mirrors the espanso sync contract).
#[tauri::command]
pub fn browser_integration_sync(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    crate::browser_integration::sync(&*state.lock()?, &onboarding_dir(&app)?, now_ms()?)
}

#[tauri::command]
pub fn onboarding_status(app: AppHandle) -> Result<OnboardingStatusDto, IpcError> {
    Ok(OnboardingStatusDto {
        completed: service::onboarding_completed(&onboarding_dir(&app)?),
    })
}

#[tauri::command]
pub fn onboarding_complete(app: AppHandle) -> Result<(), IpcError> {
    service::mark_onboarding_complete(&onboarding_dir(&app)?)
}

/// Reads the clipboard for the onboarding first-snippet prefill. Suspected
/// secrets, empty and oversized content come back as `None` (service filter);
/// the raw text never leaves this call in any other form.
#[tauri::command]
pub fn clipboard_read_text() -> Result<Option<String>, IpcError> {
    Ok(service::clipboard_seed(read_clipboard_text()))
}

#[cfg(target_os = "macos")]
fn read_clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

#[cfg(not(target_os = "macos"))]
fn read_clipboard_text() -> Option<String> {
    None
}
