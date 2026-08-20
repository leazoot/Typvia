//! Mobile host IPC commands: parse args, call the shared host-service layer,
//! map errors. The surface is the boot smoke, the read-only browse/search
//! commands, the snippet create/edit + version-list commands, and the vault
//! commands the mobile UI calls; panel and Espanso commands stay
//! desktop-only.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Manager, State};

use typvia_core::repo::{ListScope, SnippetRepo};
use typvia_core::vault::{SecureStore, VaultSession};
use typvia_host_service::dto::{
    FolderDto, HistoryDto, LibraryCountsDto, OnboardingStatusDto, SnippetCreateInput, SnippetDto,
    SnippetUpdateInput, TagDto, VaultStatusDto,
};
use typvia_host_service::error::IpcError;
use typvia_host_service::service;

/// Where keyboard snapshots are written and under which device identity.
/// Resolved once at setup: the App Group container (iOS) or the private
/// app-data snapshot directory (Android) plus the per-install device id.
pub struct SnapshotTarget {
    pub dir: PathBuf,
    pub device_id: String,
}

pub struct AppState {
    // Arc so the AI egress sink can lock it for the mandatory pre-send log
    // write while the provider round trip itself runs lock-free (mirroring
    // the desktop host).
    conn: Arc<Mutex<Connection>>,
    // The vault session holds the master key while unlocked; history
    // redaction of sensitive bodies follows from the locked state.
    vault: Mutex<VaultSession>,
    // Platform biometric-gated backend for the MK copy: the Face ID-gated
    // Keychain on iOS, unavailable elsewhere.
    secure_store: Box<dyn SecureStore + Send + Sync>,
    // `None` when this host has no snapshot directory (the desktop dev
    // shell) or the device id could not be established; refresh is then a
    // no-op. iOS and Android both resolve a real target at setup.
    snapshot: Option<SnapshotTarget>,
    // App data directory resolved once at setup; the onboarding completion
    // marker lives here (shared host-service semantics).
    data_dir: PathBuf,
    // Root the share inbox lives under (`inbox/` inside it): the App Group
    // container on iOS, the app data dir on Android where the same-UID
    // share target writes directly. `None` on hosts without a share surface
    // — the drain command then only reports counts already parked below.
    share_inbox_root: Option<PathBuf>,
    // Shares the host already ingested (setup / foreground) that the
    // frontend has not yet been told about; drained by `share_inbox_ingest`
    // so the toast can name the real number.
    pending_share_ingests: AtomicU32,
}

impl AppState {
    pub fn new(
        conn: Connection,
        secure_store: Box<dyn SecureStore + Send + Sync>,
        snapshot: Option<SnapshotTarget>,
        data_dir: PathBuf,
        share_inbox_root: Option<PathBuf>,
        ingested_at_setup: u32,
    ) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            vault: Mutex::new(VaultSession::new()),
            secure_store,
            snapshot,
            data_dir,
            share_inbox_root,
            pending_share_ingests: AtomicU32::new(ingested_at_setup),
        }
    }

    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, IpcError> {
        // A poisoned mutex means a command panicked mid-write; surface it as
        // a system error instead of poisoning every later call with a panic.
        self.conn.lock().map_err(|_| IpcError::system())
    }

    pub(crate) fn lock_vault(&self) -> Result<std::sync::MutexGuard<'_, VaultSession>, IpcError> {
        self.vault.lock().map_err(|_| IpcError::system())
    }

    pub(crate) fn secure_store(&self) -> &(dyn SecureStore + Send + Sync) {
        self.secure_store.as_ref()
    }

    /// Shared handle for code that must lock the connection on its own
    /// schedule (the AI egress sink); commands themselves use [`lock`].
    pub(crate) fn conn_handle(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

pub(crate) fn now_ms() -> Result<i64, IpcError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IpcError::system())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| IpcError::system())
}

/// Boot smoke payload: proves migrations ran and the snippet store answers
/// on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileBootstrapDto {
    pub schema_version: u32,
    pub snippet_total: u32,
}

#[tauri::command]
pub fn mobile_bootstrap(state: State<'_, AppState>) -> Result<MobileBootstrapDto, IpcError> {
    bootstrap(&*state.lock()?)
}

fn bootstrap(conn: &Connection) -> Result<MobileBootstrapDto, IpcError> {
    Ok(MobileBootstrapDto {
        schema_version: typvia_core::db::schema_version(conn)?,
        snippet_total: SnippetRepo::new(conn).count_scoped(ListScope::All, None)?,
    })
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
pub fn search_library(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::search_library(&*state.lock()?, &query, limit)
}

#[tauri::command]
pub fn template_fields(
    state: State<'_, AppState>,
    snippet_id: String,
) -> Result<Vec<typvia_host_service::dto::TemplateFieldDto>, IpcError> {
    service::template_fields(&*state.lock()?, &snippet_id)
}

#[tauri::command]
pub fn template_variables(body: String) -> Result<Vec<String>, IpcError> {
    service::template_variables(&body)
}

#[tauri::command]
pub fn template_preview(
    body: String,
    fields: Vec<typvia_host_service::dto::TemplateFieldDto>,
    values: std::collections::HashMap<String, String>,
) -> Result<String, IpcError> {
    service::template_preview(&body, fields, values)
}

// Renders a filled template and copies the result — the platform split is
// the same one `snippet_copy` documents below.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn template_copy(
    state: State<'_, AppState>,
    id: String,
    values: std::collections::HashMap<String, String>,
) -> Result<(), IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    crate::pasteboard::copy_rendered(
        &conn,
        &crate::pasteboard::PlatformPasteboard,
        &id,
        &values,
        now,
    )
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn template_copy(
    app: tauri::AppHandle,
    id: String,
    values: std::collections::HashMap<String, String>,
) -> Result<(), IpcError> {
    blocking_vault(app.clone(), move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        crate::pasteboard::copy_rendered(
            &conn,
            &crate::pasteboard::PlatformPasteboard { app },
            &id,
            &values,
            now,
        )
    })
    .await
}

#[tauri::command]
pub fn snippet_search_all(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::search_all(&*state.lock()?, &query, limit)
}

#[tauri::command]
pub fn snippet_trash(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_trash(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn snippet_get(state: State<'_, AppState>, id: String) -> Result<SnippetDto, IpcError> {
    service::snippet_get(&*state.lock()?, &id)
}

#[tauri::command]
pub fn folder_list_children(
    state: State<'_, AppState>,
    parent_id: Option<String>,
) -> Result<Vec<FolderDto>, IpcError> {
    service::folder_list_children(&*state.lock()?, parent_id.as_deref())
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
pub fn tag_list(state: State<'_, AppState>) -> Result<Vec<TagDto>, IpcError> {
    service::tag_list(&*state.lock()?)
}

// --- Vault commands (mirroring the desktop wire names) ---------------------
//
// Every vault command locks the connection first, then the vault session, in
// that order (avoiding a lock-order cycle); the biometric unlock takes the
// session lock alone first (taking a later lock without the earlier one is
// order-safe). The master password crosses the boundary as a plain `String`
// (the user typed it in the unlock UI); it is used to derive the KEK and
// dropped — never stored or logged. Time-based auto-lock lives in core's
// VaultSession: each status/reveal call passes `now`, so the idle window is
// enforced on the next vault call.
//
// Except the pasteboard copy on iOS — which must stay on the main thread
// for UIPasteboard, while its Android reading hops like the rest — every
// vault command is async and hops to the blocking pool. Two reasons stack:
//  - the KDF trio runs Argon2id, which froze the WebView when it executed
//    on the main thread (must not regress);
//  - the Android biometric unlock keeps the vault-session mutex held while
//    the system prompt is on screen (seconds to a minute). Any command that
//    waits for that mutex on the main thread would freeze the UI and risk an
//    ANR; on the blocking pool the wait only parks a worker thread. The
//    prompt itself needs a free main thread to render, which is exactly why
//    no vault command may block the main thread on these mutexes.

// Runs a vault closure on the blocking pool via an AppHandle, leaving the
// UI thread free (see the section comment for why this is mandatory).
async fn blocking_vault<R: Send + 'static>(
    app: tauri::AppHandle,
    body: impl FnOnce(&AppState) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        body(&state)
    })
    .await
    .map_err(|_| IpcError::system())?
}

/// Same blocking hop for the sync commands (sync/commands.rs), which need the
/// sync host alongside the app state. A `State` guard borrows the handle, so
/// both are resolved inside the pool thread rather than passed in.
pub(crate) async fn blocking_state<R: Send + 'static>(
    app: tauri::AppHandle,
    body: impl FnOnce(&AppState, &crate::sync::SyncState) -> Result<R, IpcError> + Send + 'static,
) -> Result<R, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sync = app.state::<crate::sync::SyncState>();
        body(&state, &sync)
    })
    .await
    .map_err(|_| IpcError::system())?
}

#[tauri::command]
pub async fn vault_initialize(
    app: tauri::AppHandle,
    password: String,
) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_initialize(&conn, &mut session, &password, now)
    })
    .await
}

#[tauri::command]
pub async fn vault_unlock_password(
    app: tauri::AppHandle,
    password: String,
) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_unlock_password(&conn, &mut session, &password, now)
    })
    .await
}

#[tauri::command]
pub async fn vault_status(app: tauri::AppHandle) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_status(&conn, &mut session, now)
    })
    .await
}

/// The gated read inside blocks on the OS auth sheet (the Face ID Keychain
/// read on iOS, the BiometricPrompt-bound Keystore decrypt on Android) — the
/// OS owns the prompt, not this host. The gate runs while holding only the
/// vault-session mutex: the DB connection must stay free for the whole
/// prompt lifetime so the rest of the app keeps answering (see the section
/// comment); the status snapshot re-locks in the canonical conn→session
/// order afterwards.
#[tauri::command]
pub async fn vault_unlock_biometric(app: tauri::AppHandle) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        {
            let mut session = state.lock_vault()?;
            session.unlock_with_biometric(state.secure_store(), now)?;
        }
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_status(&conn, &mut session, now)
    })
    .await
}

#[tauri::command]
pub async fn vault_lock(app: tauri::AppHandle) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_lock(&conn, &mut session)
    })
    .await
}

/// Enrolls the biometric MK copy. Never prompts on either platform (the iOS
/// Keychain add is silent; the Android wrap is a public-key operation), but
/// the Android side does Keystore keypair generation — blocking-pool work.
#[tauri::command]
pub async fn vault_enable_biometric(app: tauri::AppHandle) -> Result<(), IpcError> {
    blocking_vault(app, move |state| {
        let session = state.lock_vault()?;
        service::vault_enable_biometric(&session, state.secure_store())
    })
    .await
}

#[tauri::command]
pub async fn vault_disable_biometric(app: tauri::AppHandle) -> Result<(), IpcError> {
    blocking_vault(app, move |state| {
        let session = state.lock_vault()?;
        service::vault_disable_biometric(&session, state.secure_store())
    })
    .await
}

#[tauri::command]
pub async fn vault_reset_preview(app: tauri::AppHandle) -> Result<u32, IpcError> {
    blocking_vault(app, move |state| {
        service::vault_reset_preview(&*state.lock()?)
    })
    .await
}

#[tauri::command]
pub async fn vault_reset(app: tauri::AppHandle) -> Result<VaultStatusDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_reset(&conn, &mut session, state.secure_store(), now)
    })
    .await
}

#[tauri::command]
pub async fn vault_list(
    app: tauri::AppHandle,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    blocking_vault(app, move |state| {
        service::vault_list(&*state.lock()?, limit, offset)
    })
    .await
}

#[tauri::command]
pub async fn vault_reveal(app: tauri::AppHandle, id: String) -> Result<String, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        service::vault_reveal(&conn, &mut session, &id, now)
    })
    .await
}

/// One-time sensitive copy for the secure-field round trip: the
/// decrypted body goes onto the system pasteboard with its auto-clear
/// attached and never crosses IPC. Returns the clear delay in ms for the row
/// countdown. A plain decrypt on an unlocked session is fast; this is the
/// one vault command kept synchronous because the UIPasteboard write must
/// stay on the main thread (UIKit). Remaining non-Android hosts (the desktop
/// dev shell) refuse the copy honestly inside the pasteboard seam. A locked
/// vault surfaces `permission_denied`.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn vault_copy_secret(state: State<'_, AppState>, id: String) -> Result<i64, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let mut session = state.lock_vault()?;
    crate::pasteboard::copy_secret(
        &conn,
        &mut session,
        &crate::pasteboard::PlatformPasteboard,
        &id,
        now,
    )
}

/// Android reading of the same command: identical wire name,
/// return value and delivery-order semantics, but async + blocking pool like
/// the other Android vault commands — the Kotlin glue posts the
/// ClipboardManager write to the main looper and the calling thread parks on
/// its completion, so the command must never run on the main thread itself
/// (the posted write would deadlock behind it).
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn vault_copy_secret(app: tauri::AppHandle, id: String) -> Result<i64, IpcError> {
    blocking_vault(app.clone(), move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let mut session = state.lock_vault()?;
        crate::pasteboard::copy_secret(
            &conn,
            &mut session,
            &crate::pasteboard::PlatformPasteboard { app },
            &id,
            now,
        )
    })
    .await
}

#[tauri::command]
pub async fn vault_create_secret(
    app: tauri::AppHandle,
    input: SnippetCreateInput,
) -> Result<SnippetDto, IpcError> {
    blocking_vault(app, move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        let session = state.lock_vault()?;
        service::vault_create_secret(&conn, &session, input, now)
    })
    .await
}

// --- Ordinary snippet copy --------------------------------------------------
//
// Same wire name and signature as the desktop command. It exists on mobile
// because the frontend cannot copy for itself: the Android system WebView
// serves the app from a non-secure origin, where `navigator.clipboard` is
// unavailable — and a host-side copy is also the only place usage can be
// recorded on a confirmed write. Sensitive snippets are refused here and go
// through `vault_copy_secret`, which carries the auto-clear.
//
// The iOS / Android split is the same one `vault_copy_secret` documents
// above: UIPasteboard must be touched on the main thread, while the Android
// glue posts its write to the main looper and parks the caller on it.

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn snippet_copy(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    crate::pasteboard::copy_plain(&conn, &crate::pasteboard::PlatformPasteboard, &id, now)
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn snippet_copy(app: tauri::AppHandle, id: String) -> Result<(), IpcError> {
    blocking_vault(app.clone(), move |state| {
        let now = now_ms()?;
        let conn = state.lock()?;
        crate::pasteboard::copy_plain(
            &conn,
            &crate::pasteboard::PlatformPasteboard { app },
            &id,
            now,
        )
    })
    .await
}

/// Drains the share inbox: ingests any documents shared
/// since the last pass, adds the count the host already ingested itself
/// (setup / foreground), and returns the total so the frontend can toast
/// and refresh. Hosts without a share inbox root only report the parked
/// count (always 0 there in practice).
#[tauri::command]
pub fn share_inbox_ingest(state: State<'_, AppState>) -> Result<u32, IpcError> {
    let parked = state.pending_share_ingests.swap(0, Ordering::AcqRel);
    let Some(root) = &state.share_inbox_root else {
        return Ok(parked);
    };
    let now = now_ms()?;
    let conn = state.lock()?;
    Ok(parked + crate::share_inbox::ingest(&conn, root, now))
}

/// Host-side foreground drain (RunEvent path, mobile only): ingests
/// immediately so shared text is in the library even if the WebView never
/// asks, and parks the count for the frontend's next `share_inbox_ingest`.
/// Best-effort: a failure leaves the inbox files for the next pass.
#[cfg(mobile)]
pub fn ingest_share_inbox_on_foreground(state: &AppState) {
    let Some(root) = &state.share_inbox_root else {
        return;
    };
    let Ok(now) = now_ms() else {
        return;
    };
    let Ok(conn) = state.lock() else {
        return;
    };
    let ingested = crate::share_inbox::ingest(&conn, root, now);
    if ingested > 0 {
        state
            .pending_share_ingests
            .fetch_add(ingested, Ordering::AcqRel);
    }
}

/// Regenerates the keyboard snapshot in the resolved snapshot directory
/// (App Group container on iOS, private app-data directory on Android). On a
/// host without a snapshot target there is no keyboard extension to feed, so
/// refresh is deliberately a successful no-op rather than an error.
#[tauri::command]
pub fn snapshot_refresh(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), IpcError> {
    let Some(target) = &state.snapshot else {
        return Ok(());
    };
    crate::snapshot_file::write_snapshot(
        &*state.lock()?,
        &target.dir,
        &target.device_id,
        now_ms()?,
    )?;
    // Widgets re-render from the file just written; a poke failure never
    // fails the refresh (best-effort by contract).
    crate::widget_poke::notify_widgets(&app);
    Ok(())
}

/// First-run gate for the mobile shell (mirrors the desktop wrapper): reads
/// the completion marker from the app data directory resolved at setup.
#[tauri::command]
pub fn onboarding_status(state: State<'_, AppState>) -> Result<OnboardingStatusDto, IpcError> {
    Ok(onboarding_status_dto(&state.data_dir))
}

fn onboarding_status_dto(data_dir: &std::path::Path) -> OnboardingStatusDto {
    OnboardingStatusDto {
        completed: service::onboarding_completed(data_dir),
    }
}

/// Persists the completion marker; finishing and skipping both call this.
#[tauri::command]
pub fn onboarding_complete(state: State<'_, AppState>) -> Result<(), IpcError> {
    service::mark_onboarding_complete(&state.data_dir)
}

/// Onboarding keyboard step: jumps to the system Settings app, the only path
/// to enabling the keyboard (enablement is not programmable). Success means
/// the jump was dispatched, never that the keyboard was added; the UI's
/// "Nothing happened?" path covers a silent failure.
#[tauri::command]
pub fn open_keyboard_settings(app: tauri::AppHandle) -> Result<(), IpcError> {
    open_settings_app(&app)
}

#[cfg(target_os = "ios")]
fn open_settings_app(app: &tauri::AppHandle) -> Result<(), IpcError> {
    // UIApplication is main-thread-only; hop there rather than assuming the
    // command's thread. The open call itself is fire-and-forget: iOS reports
    // the outcome asynchronously and the flow never claims Settings opened.
    app.run_on_main_thread(|| {
        use objc2_foundation::{MainThreadMarker, NSDictionary, NSURL};
        use objc2_ui_kit::{UIApplication, UIApplicationOpenSettingsURLString};

        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // SAFETY: reading a UIKit extern NSString constant; always valid.
        let url_string = unsafe { UIApplicationOpenSettingsURLString };
        let Some(url) = NSURL::URLWithString(url_string) else {
            return;
        };
        let application = UIApplication::sharedApplication(mtm);
        // SAFETY: documented UIKit call on the main thread; empty options and
        // no completion handler are the documented defaults.
        unsafe {
            application.openURL_options_completionHandler(&url, &NSDictionary::new(), None);
        }
    })
    .map_err(|_| IpcError::system())
}

/// Android equivalent of the Settings jump: the system input-method screen
/// (`Settings.ACTION_INPUT_METHOD_SETTINGS`), the only place a user can
/// enable an IME. JNI runs on the WebView thread via wry's main pipe and is
/// fire-and-forget like the iOS path — success only means the jump was
/// dispatched; the UI's "Nothing happened?" path covers silent failure. JNI
/// errors clear the pending exception so later calls on that thread stay
/// usable, and give up without claiming success.
#[cfg(target_os = "android")]
fn open_settings_app(app: &tauri::AppHandle) -> Result<(), IpcError> {
    let webview = app
        .get_webview_window("main")
        .ok_or_else(IpcError::system)?;
    webview
        .with_webview(|platform_webview| {
            platform_webview
                .jni_handle()
                .exec(|env, activity, _webview| {
                    let Ok(action) = env.new_string("android.settings.INPUT_METHOD_SETTINGS")
                    else {
                        return;
                    };
                    let Ok(intent) = env.new_object(
                        "android/content/Intent",
                        "(Ljava/lang/String;)V",
                        &[(&*action).into()],
                    ) else {
                        let _ = env.exception_clear();
                        return;
                    };
                    if env
                        .call_method(
                            activity,
                            "startActivity",
                            "(Landroid/content/Intent;)V",
                            &[(&intent).into()],
                        )
                        .is_err()
                    {
                        let _ = env.exception_clear();
                    }
                });
        })
        .map_err(|_| IpcError::system())
}

/// Remaining hosts (the desktop dev shell, unit tests) have no keyboard
/// Settings screen; succeeding keeps the onboarding flow identical.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn open_settings_app(_app: &tauri::AppHandle) -> Result<(), IpcError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_status_flips_only_after_the_marker_is_written() {
        let dir = tempfile::tempdir().unwrap();

        assert!(!onboarding_status_dto(dir.path()).completed);
        service::mark_onboarding_complete(dir.path()).unwrap();
        assert!(onboarding_status_dto(dir.path()).completed);
    }

    #[test]
    fn bootstrap_reports_latest_schema_and_zero_snippets() {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();

        let info = bootstrap(&conn).unwrap();
        assert_eq!(info.schema_version, typvia_core::db::latest_version());
        assert_eq!(info.snippet_total, 0);
    }

    #[test]
    fn bootstrap_on_unmigrated_db_is_a_system_error() {
        let conn = typvia_core::db::open_in_memory().unwrap();

        let error = bootstrap(&conn).unwrap_err();
        assert_eq!(error, IpcError::system());
    }

    fn migrated_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    #[test]
    fn vault_status_on_a_fresh_database_is_uninitialized_and_locked() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();

        let status = service::vault_status(&conn, &mut session, 1_000).unwrap();
        assert!(!status.initialized);
        assert!(!status.unlocked);
        assert_eq!(status.unlocked_at, None);
    }

    #[test]
    fn initialize_unlocks_and_reveal_round_trips_a_secret() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();

        let status =
            service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        assert!(status.initialized);
        assert!(status.unlocked);

        let input = SnippetCreateInput {
            title: "Prod read replica".to_string(),
            body: "postgres://readonly@10.4.2.19".to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        };
        let created = service::vault_create_secret(&conn, &session, input, 1_001).unwrap();
        // The list is metadata-only; plaintext comes back only from reveal.
        let listed = service::vault_list(&conn, 50, 0).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body, None);
        let revealed = service::vault_reveal(&conn, &mut session, &created.id, 1_002).unwrap();
        assert_eq!(revealed, "postgres://readonly@10.4.2.19");
    }

    #[test]
    fn biometric_unlock_without_a_platform_backend_is_a_system_error() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();
        service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        session.lock();

        // The dev-shell store reports Unavailable; the session must stay
        // locked and the password path must remain usable.
        let store = crate::secure_store::UnavailableStore;
        let error =
            service::vault_unlock_biometric(&conn, &mut session, &store, 2_000).unwrap_err();
        assert_eq!(error, IpcError::system());
        let status = service::vault_status(&conn, &mut session, 2_001).unwrap();
        assert!(!status.unlocked);
        let after =
            service::vault_unlock_password(&conn, &mut session, "correct horse", 2_002).unwrap();
        assert!(after.unlocked);
    }
}
