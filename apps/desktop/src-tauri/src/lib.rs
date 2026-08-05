//! Typvia desktop shell: hosts the WebView UI and the IPC commands that
//! bridge into the Rust core (commands.rs / service.rs).

mod commands;
mod dto;
mod error;
pub mod injector;
mod service;

use tauri::Manager;

use commands::AppState;

/// Builds and runs the Tauri application; exits the process on startup
/// failure since no UI exists yet to report into.
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let mut conn = typvia_core::db::open(&data_dir.join("typvia.db"))?;
            typvia_core::db::migrate_to_latest(&mut conn)?;
            let injector = injector::platform_injector_or_null();
            app.manage(AppState::new(conn, injector));
            Ok(())
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
            commands::detect_sensitive,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("failed to start Typvia: {error}");
        std::process::exit(1);
    }
}
