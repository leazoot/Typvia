//! Typvia desktop shell: hosts the WebView UI and the IPC commands that
//! bridge into the Rust core (commands.rs / service.rs).

mod commands;
mod dto;
mod error;
pub mod injector;
mod panel;
mod service;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use commands::AppState;
use panel::{PANEL_LABEL, PanelState};

/// Builds and runs the Tauri application; exits the process on startup
/// failure since no UI exists yet to report into.
pub fn run() {
    // Global summon shortcut ⌘⇧V (design Phase 1 · "Panel ⌘⇧V"). Registered in
    // setup; the plugin handler toggles the resident panel window.
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
            app.manage(AppState::new(conn, injector));
            app.manage(PanelState::default());
            app.global_shortcut().register(summon)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // The panel hides when it loses focus (design: "ESC / 失焦隐藏").
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
            commands::detect_sensitive,
            panel::panel_hide,
            panel::panel_ready,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("failed to start Typvia: {error}");
        std::process::exit(1);
    }
}
