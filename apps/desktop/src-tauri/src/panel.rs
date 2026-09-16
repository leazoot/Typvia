// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Global command panel host: a resident, initially hidden window summoned by
//! a global shortcut. Keeping it resident and hidden (median 9ms show) is what
//! lets the field be typed into at frame 0.
//!
//! Summoning records the app that was frontmost so focus — and the injection
//! target — returns to it on hide; the panel itself never keeps the
//! foreground. This module owns only the window lifecycle and the
//! frontmost-app bookkeeping; search, selection and injection live elsewhere.

use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

/// Window label of the resident global panel (declared in tauri.conf.json).
pub const PANEL_LABEL: &str = "panel";

/// Remembers the app that was frontmost when the panel was summoned so it can
/// be re-activated on hide, and the panel's show timestamp for the latency
/// probe. Managed by Tauri as shared state.
#[derive(Default)]
pub struct PanelState {
    /// PID of the app frontmost at summon time; `None` once the panel is
    /// hidden or when the frontmost app could not be read.
    previous_app: Mutex<Option<i32>>,
    /// Bundle id of the app frontmost at summon time — the identity app rules
    /// match against. `None` while hidden or when unidentifiable.
    destination_app: Mutex<Option<String>>,
    /// When the panel was last shown; drives the debug-only latency probe.
    shown_at: Mutex<Option<Instant>>,
}

impl PanelState {
    fn remember(&self, pid: Option<i32>, bundle_id: Option<String>) {
        if let Ok(mut guard) = self.previous_app.lock() {
            *guard = pid;
        }
        if let Ok(mut guard) = self.destination_app.lock() {
            *guard = bundle_id;
        }
    }

    fn take_previous(&self) -> Option<i32> {
        self.previous_app.lock().ok().and_then(|mut g| g.take())
    }

    fn clear_destination(&self) {
        if let Ok(mut guard) = self.destination_app.lock() {
            *guard = None;
        }
    }

    /// Bundle id of the app the panel will insert into, read by the result
    /// filter and the injection gates while the panel is up.
    pub fn destination_app(&self) -> Option<String> {
        self.destination_app.lock().ok().and_then(|g| g.clone())
    }

    fn mark_shown(&self) {
        if let Ok(mut guard) = self.shown_at.lock() {
            *guard = Some(Instant::now());
        }
    }

    fn shown_elapsed(&self) -> Option<std::time::Duration> {
        self.shown_at
            .lock()
            .ok()
            .and_then(|g| g.map(|since| since.elapsed()))
    }
}

/// Toggles the panel: hide it if visible, otherwise summon it. Bound to the
/// global shortcut.
pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
    } else {
        show(app);
    }
}

/// Payload of the `panel:show` event: the app the insert will land in, named
/// in the panel header, plus its bundle id — the identity app rules match
/// against.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelShow {
    destination: Option<String>,
    destination_app_id: Option<String>,
}

/// Summons the panel. Records the frontmost app first (before we steal focus),
/// then shows, focuses, and tells the WebView to reset, focus the search field
/// at frame 0, and name the destination app.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    let (pid, name, bundle_id) = match frontmost::frontmost_app() {
        Some(front) => (Some(front.pid), Some(front.name), front.bundle_id),
        None => (None, None, None),
    };
    let state = app.state::<PanelState>();
    state.remember(pid, bundle_id.clone());
    state.mark_shown();
    let _ = window.show();
    let _ = window.set_focus();
    // Frame-0 focus: the WebView is resident, so a fresh show must re-focus the
    // input and clear any prior query. The panel front-end listens for this.
    let destination = name.filter(|n| !n.is_empty());
    let _ = window.emit_to(
        PANEL_LABEL,
        "panel:show",
        PanelShow {
            destination,
            destination_app_id: bundle_id,
        },
    );
    // Payload-free broadcast for the onboarding shortcut rehearsal:
    // the main window only learns that the panel opened, never the foreground
    // app identity (that stays on the panel-only event above).
    let _ = app.emit("panel:summoned", ());
}

/// Bundle id of the app frontmost right now, for inserts that do not go
/// through the panel. AppKit only answers on the main thread, so the read is
/// posted there; callers must be off the main thread or this would deadlock.
pub fn frontmost_bundle_id(app: &AppHandle) -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(frontmost::frontmost_app().and_then(|front| front.bundle_id));
    })
    .ok()?;
    receiver
        .recv_timeout(std::time::Duration::from_millis(500))
        .ok()
        .flatten()
}

/// Delay after restoring the previous app before the insert paints, so the
/// window server has brought it frontmost before the insert targets it.
pub const FOCUS_SETTLE_MS: u64 = 100;

/// Hides the panel and returns focus to the app that was frontmost when it was
/// summoned.
pub fn hide(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    let _ = window.hide();
    let state = app.state::<PanelState>();
    // The injection gates read the destination before the hide is scheduled
    // (commands.rs), so clearing here never races a pending insert.
    state.clear_destination();
    if let Some(pid) = state.take_previous() {
        frontmost::activate_pid(pid);
    }
}

/// Hides the panel in response to a WebView action (ESC). Kept out of the IPC
/// data layer since it only drives window lifecycle, not core state.
/// The summon keys the system actually granted, in the words the UI shows.
/// Empty when every candidate was refused, so the UI can say the summon key is
/// unavailable rather than name one that does nothing.
#[derive(Default)]
pub struct SummonShortcut {
    words: std::sync::Mutex<Option<String>>,
}

impl SummonShortcut {
    pub fn set(&self, words: &str) {
        if let Ok(mut guard) = self.words.lock() {
            *guard = Some(words.to_owned());
        }
    }

    fn words(&self) -> Option<String> {
        self.words.lock().ok().and_then(|guard| guard.clone())
    }
}

/// Which keys summon the panel on this machine.
#[tauri::command]
pub fn summon_shortcut(state: tauri::State<'_, SummonShortcut>) -> Option<String> {
    state.words()
}

#[tauri::command]
pub fn panel_hide(app: AppHandle) {
    hide(&app);
}

/// Reports that the panel's first frame has painted (double-rAF from the
/// WebView). Debug builds print the summon→first-frame latency for the
/// <150ms budget; compiled to a no-op cost in release.
#[tauri::command]
pub fn panel_ready(state: tauri::State<'_, PanelState>) {
    let elapsed = state.shown_elapsed();
    #[cfg(debug_assertions)]
    if let Some(elapsed) = elapsed {
        // Acceptance instrumentation only: never present in release.
        eprintln!("PANEL_LATENCY_MS={:.1}", elapsed.as_secs_f64() * 1000.0);
    }
    #[cfg(not(debug_assertions))]
    let _ = elapsed;
}

/// The identity Typvia records about the frontmost app: process id (focus
/// return), display name (panel header) and bundle id (app-rule matching).
/// Deliberately nothing more — no window titles, no window content: that is a
/// security red line.
pub struct FrontmostApp {
    pub pid: i32,
    pub name: String,
    /// Reverse-DNS bundle identifier; `None` for unbundled processes.
    pub bundle_id: Option<String>,
}

/// Frontmost-app tracking. macOS reads/reactivates via AppKit; other platforms
/// are no-ops until their panel lands.
#[cfg(target_os = "macos")]
pub(crate) mod frontmost {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

    use super::FrontmostApp;

    /// Identity of the frontmost application, or `None` if unreadable / off
    /// the main thread (AppKit is main-thread-affine; callers run on the
    /// event loop).
    pub fn frontmost_app() -> Option<FrontmostApp> {
        let _mtm = MainThreadMarker::new()?;
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        let name = app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_default();
        Some(FrontmostApp {
            pid: app.processIdentifier(),
            name,
            bundle_id: app.bundleIdentifier().map(|b| b.to_string()),
        })
    }

    /// Re-activates the app with the given PID so it regains focus on hide.
    pub fn activate_pid(pid: i32) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return;
        };
        // `activate()` is macOS 14+, but the product supports macOS 12+,
        // so the pre-14 options API is the correct call here.
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) mod frontmost {
    use super::FrontmostApp;

    pub fn frontmost_app() -> Option<FrontmostApp> {
        None
    }

    pub fn activate_pid(_pid: i32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_state_remembers_and_takes_pid_once() {
        let state = PanelState::default();
        state.remember(Some(4242), None);
        assert_eq!(state.take_previous(), Some(4242));
        // Taking clears it, so a second hide does not re-activate a stale app.
        assert_eq!(state.take_previous(), None);
    }

    #[test]
    fn panel_state_holds_the_destination_until_cleared() {
        let state = PanelState::default();
        assert_eq!(state.destination_app(), None);
        state.remember(Some(1), Some("com.apple.Terminal".to_string()));
        assert_eq!(
            state.destination_app(),
            Some("com.apple.Terminal".to_string())
        );
        // Hide clears it so a stale identity never feeds the rule gates.
        state.clear_destination();
        assert_eq!(state.destination_app(), None);
    }

    #[test]
    fn panel_show_payload_carries_only_app_identity() {
        // The wire shape the panel WebView receives: display name and bundle
        // id, nothing else — no window titles, no window content. The
        // security red line lives in this contract.
        let payload = PanelShow {
            destination: Some("Terminal".to_string()),
            destination_app_id: Some("com.apple.Terminal".to_string()),
        };
        let json = serde_json::to_value(&payload).unwrap();
        let object = json.as_object().unwrap();
        assert_eq!(object.len(), 2);
        assert_eq!(json["destination"], "Terminal");
        assert_eq!(json["destinationAppId"], "com.apple.Terminal");
    }

    #[test]
    fn panel_state_marks_show_time() {
        let state = PanelState::default();
        assert!(state.shown_elapsed().is_none());
        state.mark_shown();
        assert!(state.shown_elapsed().is_some());
    }
}
