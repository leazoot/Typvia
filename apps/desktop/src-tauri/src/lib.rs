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
mod panel;
pub mod secure_store;
mod semantic;
mod service;
mod sync;
#[cfg(target_os = "macos")]
mod vault_autolock;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use commands::AppState;
use panel::{PANEL_LABEL, PanelState};
use sync::{SyncHost, SyncScheduler, SyncState};

/// Builds and runs the Tauri application; exits the process on startup
/// failure since no UI exists yet to report into.
pub fn run() {
    // Global summon shortcut ⌘⇧V for the panel. Registered in setup; the
    // plugin handler toggles the resident panel window.
    let summon = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);
    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &summon && event.state() == ShortcutState::Pressed {
                        panel::toggle(app);
                    }
                })
                .build(),
        )
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
            schedule_sync(app.handle().clone(), scheduler);
            if semantic::model_present(&data_dir) {
                let conn_handle = app.state::<AppState>().inner().conn_handle();
                semantic::spawn_embed_worker(semantic_state, conn_handle, data_dir);
            }
            // The panel's 28% scrim needs the transparent window to cover
            // the screen; the card centers itself in CSS. Sized once here —
            // never per show — so summoning stays free of geometry work
            // (<150ms budget). If the monitor cannot be read the window keeps
            // its config size and the card fills it exactly (no scrim, same
            // panel as before).
            if let Some(panel_window) = app.get_webview_window(PANEL_LABEL)
                && let Ok(Some(monitor)) = panel_window.primary_monitor()
            {
                let _ = panel_window.set_position(*monitor.position());
                let _ = panel_window.set_size(*monitor.size());
            }
            // Sleep / screen-lock events drop the vault master key.
            #[cfg(target_os = "macos")]
            vault_autolock::register(app.handle());
            app.global_shortcut().register(summon)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // The panel hides when it loses focus, the same as pressing ESC.
            if window.label() == PANEL_LABEL
                && let WindowEvent::Focused(false) = event
            {
                panel::hide(window.app_handle());
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
