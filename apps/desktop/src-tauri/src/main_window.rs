// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The main window stepping out of the way so an insert lands in the app the
//! reader came from — and stepping back when the insert could not happen.

use tauri::{AppHandle, Manager};

/// Label of the main window (must match tauri.conf.json).
pub const MAIN_LABEL: &str = "main";

/// Hands focus back to the previously frontmost app. On macOS hiding the
/// application reactivates it; elsewhere minimising the window does.
/// Main thread only.
pub fn step_aside(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.hide();
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.minimize();
    }
}

/// Brings the window back so the reader sees why nothing was inserted,
/// instead of an app that silently vanished. Main thread only.
pub fn step_back(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.show();
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Label of the About window (must match tauri.conf.json).
pub const ABOUT_LABEL: &str = "about";
