//! Typvia desktop shell: hosts the WebView UI. IPC commands that bridge
//! into the Rust core are registered here from TASK-028 on.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = tauri::Builder::default().run(tauri::generate_context!()) {
        // Startup failures happen before any UI exists to report into.
        eprintln!("failed to start Typvia: {error}");
        std::process::exit(1);
    }
}
