// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Typvia desktop shell: hosts the WebView UI and the IPC commands that
//! bridge into the Rust core (commands.rs / service.rs).

mod ai;
mod browser_integration;
mod commands;
mod espanso_cli;
pub mod injector;
mod main_window;
mod panel;
mod pause;
pub mod secure_store;
mod semantic;
mod service;
mod sync;
mod tray;
#[cfg(target_os = "macos")]
mod vault_autolock;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use commands::AppState;
use panel::{PANEL_LABEL, PanelState};
use sync::{SyncHost, SyncScheduler, SyncState};
use tray::{TRAY_LABEL, TrayState};

/// Keys tried for the global summon, most wanted first, each with the words
/// the UI shows for it. Windows keeps most Win combinations for the shell and
/// refuses to hand them out, so the app takes the first one the system grants.
fn summon_candidates() -> Vec<(Shortcut, &'static str)> {
    let mut keys = Vec::new();
    #[cfg(target_os = "macos")]
    keys.push((
        Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV),
        "⌘⇧V",
    ));
    #[cfg(not(target_os = "macos"))]
    {
        keys.push((
            Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV),
            "Win Shift V",
        ));
        keys.push((
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV),
            "Ctrl Alt V",
        ));
    }
    keys
}

/// Builds and runs the Tauri application; exits the process on startup
/// failure since no UI exists yet to report into.
pub fn run() {
    // Which summon keys the system grants is only known at registration time,
    // so the handler matches against whatever setup managed to take.
    let granted: std::sync::Arc<std::sync::Mutex<Option<Shortcut>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let pressed = std::sync::Arc::clone(&granted);
    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && pressed
                            .lock()
                            .is_ok_and(|current| current.as_ref() == Some(shortcut))
                    {
                        panel::toggle(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let mut conn = typvia_core::db::open(&data_dir.join("typvia.db"))?;
            typvia_core::db::migrate_to_latest(&mut conn)?;
            let injector = injector::platform_injector_or_null();
            let secure_store = secure_store::platform_secure_store_or_unavailable();
            // The sync host registers this install's device identity and the
            // seal-at-write observer against the one connection the app owns,
            // before that connection moves into the shared state.
            let device_id = typvia_host_service::service::load_or_create_device_id(&data_dir)
                .map_err(|_| "could not establish a device id")?;
            let scheduler = std::sync::Arc::new(SyncScheduler::new());
            let sync_host = SyncHost::start(
                &conn,
                secure_store.as_ref(),
                device_id,
                typvia_host_service::service::current_platform(),
                commands::now_ms().map_err(|_| "system clock is unusable")?,
                std::sync::Arc::clone(&scheduler),
            );
            // Managed espanso engine: private dirs + daemon supervisor,
            // started through the coexistence gate below.
            let engine = typvia_espanso_adapter::EngineSupervisor::new(
                typvia_espanso_adapter::ManagedDirs::new(&data_dir),
                espanso_cli::resolved_program(),
            );
            app.manage(AppState::new(conn, injector, secure_store, engine));
            // Off the setup path: retire the legacy typvia.yml from the
            // user's espanso directory (their instance must not keep expanding
            // a stale trigger set), then run the default-on bootstrap
            // (generate config + gated start; no click required).
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    typvia_espanso_adapter::retire_legacy_config(espanso_cli::ResolvedEspansoCli);
                    commands::engine_bootstrap(&handle.state::<AppState>());
                });
            }
            // Semantic search: resident state + a startup drain so pending
            // embeddings catch up when a model is installed.
            let semantic_state = std::sync::Arc::new(semantic::SemanticState::default());
            app.manage(std::sync::Arc::clone(&semantic_state));
            app.manage(SyncState::new(sync_host, data_dir.clone()));
            app.manage(PanelState::default());
            app.manage(TrayState::default());
            schedule_sync(app.handle().clone(), scheduler);
            if semantic::model_present(&data_dir) {
                let conn_handle = app.state::<AppState>().inner().conn_handle();
                semantic::spawn_embed_worker(semantic_state, conn_handle, data_dir);
            }
            // The Quick Bar window is the card itself, under the system's
            // frosted material, so the desktop blurs through it. It sits
            // centred, 22% down the primary screen. Placed once here — never
            // per show — so summoning stays free of geometry work (<150ms
            // budget); if the monitor cannot be read it stays centred.
            if let Some(panel_window) = app.get_webview_window(PANEL_LABEL)
                && let Ok(Some(monitor)) = panel_window.primary_monitor()
                && let Ok(size) = panel_window.outer_size()
            {
                let area = monitor.size();
                let origin = monitor.position();
                let x = origin.x
                    + i32::try_from(area.width.saturating_sub(size.width) / 2).unwrap_or(0);
                let y = origin.y + i32::try_from(area.height / 100 * 22).unwrap_or(0);
                let _ = panel_window.set_position(tauri::PhysicalPosition::new(x, y));
            }
            // Sleep / screen-lock events drop the vault master key.
            #[cfg(target_os = "macos")]
            vault_autolock::register(app.handle());
            // On Windows the page draws the title bar itself: the mark, the
            // menus and the three window buttons.
            #[cfg(target_os = "windows")]
            if let Some(main) = app.get_webview_window(main_window::MAIN_LABEL) {
                let _ = main.set_decorations(false);
            }
            // A refused shortcut costs the shortcut, never the launch: the
            // panel still opens from the tray card and the window.
            let summon = panel::SummonShortcut::default();
            if let Some((shortcut, words)) = summon_candidates()
                .into_iter()
                .find(|(candidate, _)| app.global_shortcut().register(*candidate).is_ok())
            {
                if let Ok(mut current) = granted.lock() {
                    *current = Some(shortcut);
                }
                summon.set(words);
            }
            app.manage(summon);
            tray::install(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // The panel hides when it loses focus, the same as pressing ESC.
            if window.label() == PANEL_LABEL
                && let WindowEvent::Focused(false) = event
            {
                panel::hide(window.app_handle());
            }
            // The tray card closes when it loses focus, as a menu does.
            if window.label() == TRAY_LABEL
                && let WindowEvent::Focused(false) = event
            {
                tray::hide(window.app_handle(), false);
            }
            // Closing About only puts it away; the menu brings the same window back.
            if window.label() == main_window::ABOUT_LABEL
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::snippet_create,
            commands::snippet_update,
            commands::snippet_get,
            commands::snippet_list,
            commands::snippet_list_by_folder,
            commands::snippet_list_page,
            commands::snippet_count,
            commands::library_counts,
            commands::snippet_trash,
            commands::snippet_batch_move,
            commands::snippet_batch_add_tag,
            commands::snippet_batch_trash,
            commands::snippet_restore,
            commands::snippet_delete_forever,
            commands::trash_list,
            commands::trash_purge_expired,
            commands::folder_create,
            commands::folder_update,
            commands::folder_delete,
            commands::folder_merge,
            commands::folder_reorder,
            commands::folder_list_children,
            commands::tag_create,
            commands::tag_list,
            commands::tag_rename,
            commands::tag_delete,
            commands::search_snippets,
            commands::search_library,
            commands::snippet_inject,
            commands::snippet_copy,
            commands::panel_insert,
            commands::main_insert,
            commands::main_insert_template,
            commands::about_show,
            commands::accessibility_status,
            commands::open_accessibility_settings,
            commands::panel_insert_secret,
            commands::panel_copy_secret,
            commands::detect_sensitive,
            commands::template_variables,
            commands::template_fields,
            commands::template_save_fields,
            commands::template_preview,
            commands::template_render,
            commands::panel_insert_template,
            commands::semantic_status,
            commands::semantic_model_download,
            commands::semantic_model_delete,
            commands::semantic_sync,
            commands::search_library_deep,
            commands::browser_integration_status,
            commands::browser_integration_enable,
            commands::browser_integration_disable,
            commands::browser_integration_sync,
            commands::espanso_status,
            commands::espanso_sync,
            commands::espanso_disable,
            commands::espanso_coexistence,
            commands::espanso_import,
            commands::snippets_import,
            commands::backup_export,
            commands::backup_export_file,
            commands::backup_restore,
            commands::history_list,
            commands::history_get,
            commands::history_restore,
            commands::vault_status,
            commands::master_password_min_length,
            commands::webdav_credentials,
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
            commands::vault_create_secret,
            commands::vault_update_secret,
            commands::snippet_convert_to_sensitive,
            commands::panel_results,
            commands::app_rule_list,
            commands::app_rule_create,
            commands::app_rule_update,
            commands::app_rule_delete,
            commands::onboarding_status,
            commands::onboarding_complete,
            commands::clipboard_read_text,
            ai::ai_organize,
            ai::ai_extract_variables,
            ai::ai_provider_list,
            ai::ai_provider_save,
            ai::ai_provider_delete,
            ai::ai_api_key_set,
            ai::ai_api_key_clear,
            ai::ai_check_connectivity,
            ai::ai_action_list,
            ai::ai_action_save,
            ai::ai_action_delete,
            ai::ai_action_run,
            ai::ai_egress_log_list,
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
            panel::panel_hide,
            panel::panel_ready,
            panel::summon_shortcut,
            commands::tray_results,
            commands::tray_insert,
            commands::insertion_pause_status,
            commands::insertion_pause,
            commands::insertion_resume,
            tray::tray_present,
            tray::tray_hide,
            tray::tray_open_library,
            tray::app_quit,
            tray::autostart_status,
            tray::autostart_set,
        ])
        .build(tauri::generate_context!());
    match result {
        // The exit hook stops the managed espanso engine so no daemon
        // outlives the app (orphan-daemon red line).
        Ok(app) => app.run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app_handle.state::<AppState>().engine().stop();
            }
        }),
        Err(error) => {
            eprintln!("failed to start Typvia: {error}");
            std::process::exit(1);
        }
    }
}

/// Runs sync rounds on the scheduler's cadence: a debounced round
/// after each local change, exponential backoff after failures, the idle poll
/// otherwise. Failures are swallowed on purpose: a scheduled round that
/// cannot reach the server is the offline state, not an event worth
/// interrupting the user for, and the Sync page reports the backlog either
/// way. Nothing about the failure is printed — the log red line covers this
/// path too.
fn schedule_sync(app: tauri::AppHandle, scheduler: std::sync::Arc<SyncScheduler>) {
    tauri::async_runtime::spawn_blocking(move || {
        loop {
            scheduler.wait_until_due();
            let started = scheduler.begin_round();
            let ok = run_scheduled_round(&app);
            scheduler.note_round(started, ok);
        }
    });
}

/// One scheduled round; the return value feeds the backoff. "Sync is not set
/// up" reports as an idle success — an unconfigured host polls quietly at the
/// idle interval instead of climbing the retry ladder.
fn run_scheduled_round(app: &tauri::AppHandle) -> bool {
    let Ok(now) = commands::now_ms() else {
        return false;
    };
    let state = app.state::<AppState>();
    // The periodic tick doubles as the temporary-snippet expiry sweep:
    // expired rows follow the exact trash flow, so a failure here never
    // blocks the sync round below.
    if let Ok(conn) = state.lock() {
        let _ = typvia_host_service::service::temporary_expire(&conn, now);
    }
    let sync_state = app.state::<SyncState>();
    // Lock order sync → conn: the round takes the connection
    // per phase through the mutex, never across network I/O.
    let Ok(mut host) = sync_state.lock() else {
        return false;
    };
    match sync::run_round(&state.conn_handle(), state.secure_store(), &mut host, now) {
        Ok(_) => true,
        Err(error) => error.code == typvia_host_service::error::IpcErrorCode::Conflict,
    }
}
