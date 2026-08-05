//! Typvia desktop shell: hosts the WebView UI and the IPC commands that
//! bridge into the Rust core (commands.rs / service.rs).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod error;
mod service;

use tauri::Manager;

use commands::AppState;

fn main() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let mut conn = typvia_core::db::open(&data_dir.join("typvia.db"))?;
            typvia_core::db::migrate_to_latest(&mut conn)?;
            app.manage(AppState::new(conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snippet_create,
            commands::snippet_update,
            commands::snippet_get,
            commands::snippet_list,
            commands::snippet_list_by_folder,
            commands::snippet_trash,
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
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        // Startup failures happen before any UI exists to report into.
        eprintln!("failed to start Typvia: {error}");
        std::process::exit(1);
    }
}
