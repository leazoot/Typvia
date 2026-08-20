//! Typvia mobile shell (iOS first): hosts the WebView UI and the minimal
//! IPC surface that bridges into the Rust core (commands.rs).

mod ai;
mod app_group;
mod background;
mod commands;
mod pasteboard;
mod secure_store;
mod share_inbox;
mod snapshot_file;
mod sync;
mod web_chrome;
mod widget_poke;

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tauri::Manager;

use commands::{AppState, SnapshotTarget};
use sync::{SyncHost, SyncState};

/// Delay before the first sync round, so the first frame paints before the
/// connection attempt takes the database lock.
const SYNC_START_DELAY_MS: u64 = 3_000;

/// Builds and runs the Tauri application; exits the process on startup
/// failure since no UI exists yet to report into.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let built = tauri::Builder::default()
        // Deep-link channel: delivers typvia:// opens to the
        // WebView router; scheme registration lives in the platform
        // manifests (CFBundleURLTypes / intent-filter).
        .plugin(tauri_plugin_deep_link::init())
        // Every page load re-checks: the WKContentView can be recreated
        // after a webview process swap, and the re-class is idempotent.
        .on_page_load(|webview, _| {
            web_chrome::hide_form_accessory_bar(webview);
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let mut conn = typvia_core::db::open(&data_dir.join("typvia.db"))?;
            typvia_core::db::migrate_to_latest(&mut conn)?;
            let container = app_group::container_dir();
            let share_root = share_inbox_root(container.as_deref(), &data_dir);
            // Inbox drain before the first snapshot write, so text shared
            // while the app was closed is in the library (and the keyboard
            // snapshot) from the first frame. The count is parked in
            // AppState for the frontend's drain command (toast).
            let ingested = ingest_share_inbox_at_startup(&conn, share_root.as_deref());
            let snapshot = snapshot_target(container.as_deref(), &data_dir);
            if let Some(target) = &snapshot {
                refresh_snapshot_at_startup(app.handle(), &conn, target);
            }
            let store = secure_store::platform_secure_store_or_unavailable(app.handle());
            // The sync host registers this install's device identity and the
            // seal-at-write observer against the one connection the app owns,
            // before that connection moves into the shared state.
            let device_id = typvia_host_service::service::load_or_create_device_id(&data_dir)
                .map_err(|_| "could not establish a device id")?;
            let sync_host = SyncHost::start(
                &conn,
                store.as_ref(),
                device_id,
                typvia_host_service::service::current_platform(),
                commands::now_ms().map_err(|_| "system clock is unusable")?,
            );
            app.manage(SyncState::new(sync_host, data_dir.clone()));
            app.manage(AppState::new(
                conn, store, snapshot, data_dir, share_root, ingested,
            ));
            schedule_startup_sync(app.handle().clone());
            #[cfg(target_os = "ios")]
            background::ios::install(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::mobile_bootstrap,
            commands::snippet_list_page,
            commands::snippet_count,
            commands::library_counts,
            commands::search_library,
            commands::snippet_search_all,
            commands::snippet_trash,
            commands::template_fields,
            commands::template_variables,
            commands::template_preview,
            commands::template_copy,
            commands::snippet_get,
            commands::folder_list_children,
            commands::tag_list,
            commands::snippet_create,
            commands::snippet_update,
            commands::history_list,
            commands::snapshot_refresh,
            commands::share_inbox_ingest,
            commands::onboarding_status,
            commands::onboarding_complete,
            commands::open_keyboard_settings,
            commands::vault_status,
            commands::vault_initialize,
            commands::vault_unlock_password,
            commands::vault_unlock_biometric,
            commands::vault_lock,
            commands::vault_enable_biometric,
            commands::vault_disable_biometric,
            commands::vault_reset_preview,
            commands::vault_reset,
            commands::vault_list,
            commands::vault_reveal,
            commands::vault_copy_secret,
            commands::vault_create_secret,
            commands::snippet_copy,
            sync::sync_status,
            sync::sync_enable,
            sync::sync_enable_webdav,
            sync::pairing_begin_webdav,
            sync::recovery_recover_webdav,
            sync::sync_disable,
            sync::sync_resume,
            sync::sync_now,
            sync::sync_devices,
            sync::sync_revoke_device,
            sync::sync_rotate_key,
            sync::pairing_begin,
            sync::pairing_poll,
            sync::pairing_finalize,
            sync::pairing_cancel,
            sync::pairing_sas,
            sync::pairing_approve,
            sync::recovery_publish,
            sync::recovery_recover,
            sync::sync_conflicts,
            sync::sync_conflict_resolve,
            ai::ai_provider_list,
            ai::ai_provider_save,
            ai::ai_provider_delete,
            ai::ai_api_key_set,
            ai::ai_api_key_clear,
            ai::ai_organize,
            ai::ai_action_list,
            ai::ai_action_run,
            ai::ai_egress_log_list,
        ])
        .build(tauri::generate_context!());
    match built {
        Ok(app) => app.run(handle_run_event),
        Err(error) => {
            eprintln!("failed to start Typvia mobile: {error}");
            std::process::exit(1);
        }
    }
}

/// Both mobile platforms surface a return to the foreground as the
/// mobile-only `WindowEvent::Resumed` — iOS from
/// applicationWillEnterForeground, Android from the activity's onResume
/// through tao's ndk_glue (verified on the emulator via logcat). Drain the
/// share inbox there so text shared while Typvia was
/// backgrounded lands without waiting for the WebView's own visibility
/// drain. The same event is the mobile sync schedule's main trigger: coming
/// back to the app is when a round is worth running.
#[cfg(mobile)]
fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::WindowEvent {
        event: tauri::WindowEvent::Resumed,
        ..
    } = event
    {
        commands::ingest_share_inbox_on_foreground(&app.state::<AppState>());
        // Off the main thread: this handler runs on it, and a round does
        // network + SQLite work (Android additionally reaches SecureStore
        // JNI, which must not run on the main thread).
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || run_sync_round(&handle));
    }
}

/// Desktop dev shell: no mobile lifecycle events to react to.
#[cfg(not(mobile))]
fn handle_run_event(_app: &tauri::AppHandle, _event: tauri::RunEvent) {}

/// Runs one sync round shortly after launch, once. The mobile schedule is
/// deliberately this plus the foreground `Resumed` trigger, the frontend's
/// visibilitychange call and the manual "Sync now" — there is no periodic
/// timer like the desktop host's, because a phone spends most of its time
/// backgrounded (where a timer would be throttled or killed anyway) and
/// background tasks are out of scope here.
fn schedule_startup_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(SYNC_START_DELAY_MS));
        run_sync_round(&app);
    });
}

/// One round, failures swallowed on purpose: a round that cannot reach the
/// server is the offline state, not an event worth interrupting the user
/// for, and the Sync page reports the backlog either way. Nothing about the
/// failure is printed — the log red line covers this path too. Guards are
/// taken in the host-wide `conn` → `sync` order.
pub(crate) fn run_sync_round(app: &tauri::AppHandle) {
    // Foreground return and startup double as the temporary-snippet expiry
    // sweep, mirroring the desktop tick.
    if let (Ok(now), state) = (commands::now_ms(), app.state::<AppState>())
        && let Ok(conn) = state.lock()
    {
        let _ = typvia_host_service::service::temporary_expire(&conn, now);
    }
    let Ok(now) = commands::now_ms() else {
        return;
    };
    let state = app.state::<AppState>();
    let sync_state = app.state::<SyncState>();
    // Lock order sync → conn: the round takes the connection
    // per phase through the mutex, never across network I/O.
    let Ok(mut host) = sync_state.lock() else {
        return;
    };
    let _ = sync::run_round(&state.conn_handle(), state.secure_store(), &mut host, now);
}

/// Best-effort setup drain of the share inbox. A missing inbox root or
/// clock failure disables the pipeline for this pass without failing setup;
/// the inbox files stay put for the next drain.
fn ingest_share_inbox_at_startup(conn: &Connection, share_root: Option<&Path>) -> u32 {
    let Some(dir) = share_root else {
        return 0;
    };
    let Ok(now) = commands::now_ms() else {
        return 0;
    };
    share_inbox::ingest(conn, dir, now)
}

/// Android: the share-target Activity runs in the same APK/UID and writes
/// `inbox/` directly under the app data dir (`Context.dataDir`, which is
/// what Tauri's `app_data_dir()` resolves to on Android),
/// mirroring the iOS relative layout under the App Group root. Same cfg
/// dispatch shape as `snapshot_dir` below.
#[cfg(target_os = "android")]
fn share_inbox_root(_container: Option<&Path>, data_dir: &Path) -> Option<PathBuf> {
    Some(data_dir.to_path_buf())
}

/// Everywhere else the share inbox lives under the iOS App Group container;
/// hosts without one (the desktop dev shell) have no share surface feeding
/// an inbox.
#[cfg(not(target_os = "android"))]
fn share_inbox_root(container: Option<&Path>, _data_dir: &Path) -> Option<PathBuf> {
    container.map(Path::to_path_buf)
}

/// Resolves where keyboard snapshots go on this host. `None` (no snapshot
/// directory, or no persistable device id) disables the pipeline without
/// failing setup — the app must stay usable without a keyboard extension.
fn snapshot_target(container: Option<&Path>, data_dir: &Path) -> Option<SnapshotTarget> {
    let dir = snapshot_dir(container, data_dir)?;
    let device_id = snapshot_file::load_or_create_device_id(data_dir).ok()?;
    Some(SnapshotTarget { dir, device_id })
}

/// Android: snapshots live in a private app-data subdirectory — the IME
/// service shares the application UID and reads the file directly, so no
/// App Group equivalent is needed.
#[cfg(target_os = "android")]
fn snapshot_dir(_container: Option<&Path>, data_dir: &Path) -> Option<PathBuf> {
    snapshot_file::ensure_data_dir_snapshot_dir(data_dir).ok()
}

/// Everywhere else the snapshot directory is the iOS App Group container;
/// hosts without one (the desktop dev shell) have no extension to feed.
#[cfg(not(target_os = "android"))]
fn snapshot_dir(container: Option<&Path>, _data_dir: &Path) -> Option<PathBuf> {
    container.map(Path::to_path_buf)
}

/// Best-effort snapshot write at startup: a failure must not abort the app,
/// and the previous snapshot file stays in place for the keyboard.
fn refresh_snapshot_at_startup(app: &tauri::AppHandle, conn: &Connection, target: &SnapshotTarget) {
    let written = commands::now_ms()
        .and_then(|now| snapshot_file::write_snapshot(conn, &target.dir, &target.device_id, now));
    match written {
        Ok(()) => widget_poke::notify_widgets(app),
        Err(_) => {
            // Static line only: no paths or content may reach logs.
            eprintln!("keyboard snapshot refresh failed at startup; previous snapshot kept");
        }
    }
}
