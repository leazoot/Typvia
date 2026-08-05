//! Global command panel host (PRD §12.3): a resident, initially hidden window
//! summoned by a global shortcut. Resident-hidden was frozen by DEC-007 /
//! TASK-005 (median 9ms show) so the field can be typed into at frame 0.
//!
//! Summoning records the app that was frontmost so focus — and the injection
//! target (TASK-039) — returns to it on hide; the panel itself never keeps the
//! foreground. Search, selection and injection are wired in TASK-039; this
//! module owns only the window lifecycle and the frontmost-app bookkeeping.

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
    /// When the panel was last shown; drives the debug-only latency probe.
    shown_at: Mutex<Option<Instant>>,
}

impl PanelState {
    fn remember(&self, pid: Option<i32>) {
        if let Ok(mut guard) = self.previous_app.lock() {
            *guard = pid;
        }
    }

    fn take_previous(&self) -> Option<i32> {
        self.previous_app.lock().ok().and_then(|mut g| g.take())
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

/// Summons the panel. Records the frontmost app first (before we steal focus),
/// then shows, focuses, and tells the WebView to reset and focus the search
/// field at frame 0.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    let state = app.state::<PanelState>();
    state.remember(frontmost::frontmost_pid());
    state.mark_shown();
    let _ = window.show();
    let _ = window.set_focus();
    // Frame-0 focus: the WebView is resident, so a fresh show must re-focus the
    // input and clear any prior query. The panel front-end listens for this.
    let _ = window.emit_to(PANEL_LABEL, "panel:show", ());
}

/// Hides the panel and returns focus to the app that was frontmost when it was
/// summoned.
pub fn hide(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };
    let _ = window.hide();
    if let Some(pid) = app.state::<PanelState>().take_previous() {
        frontmost::activate_pid(pid);
    }
}

/// Hides the panel in response to a WebView action (ESC). Kept out of the IPC
/// data layer since it only drives window lifecycle, not core state.
#[tauri::command]
pub fn panel_hide(app: AppHandle) {
    hide(&app);
}

/// Reports that the panel's first frame has painted (double-rAF from the
/// WebView). Debug builds print the summon→first-frame latency for the
/// <150ms budget (PRD §19); compiled to a no-op cost in release.
#[tauri::command]
pub fn panel_ready(state: tauri::State<'_, PanelState>) {
    let elapsed = state.shown_elapsed();
    #[cfg(debug_assertions)]
    if let Some(elapsed) = elapsed {
        // Acceptance instrumentation only (TASK-038): never present in release.
        eprintln!("PANEL_LATENCY_MS={:.1}", elapsed.as_secs_f64() * 1000.0);
    }
    #[cfg(not(debug_assertions))]
    let _ = elapsed;
}

/// Frontmost-app tracking. macOS reads/reactivates via AppKit; other platforms
/// are no-ops until their panel lands (TASK-014 leftover for Windows).
#[cfg(target_os = "macos")]
mod frontmost {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

    /// PID of the frontmost application, or `None` if unreadable / off the main
    /// thread (AppKit is main-thread-affine; callers run on the event loop).
    pub fn frontmost_pid() -> Option<i32> {
        let _mtm = MainThreadMarker::new()?;
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        Some(app.processIdentifier())
    }

    /// Re-activates the app with the given PID so it regains focus on hide.
    pub fn activate_pid(pid: i32) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return;
        };
        // `activate()` is macOS 14+, but the product supports macOS 12+
        // (DEC-007), so the pre-14 options API is the correct call here.
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

#[cfg(not(target_os = "macos"))]
mod frontmost {
    pub fn frontmost_pid() -> Option<i32> {
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
        state.remember(Some(4242));
        assert_eq!(state.take_previous(), Some(4242));
        // Taking clears it, so a second hide does not re-activate a stale app.
        assert_eq!(state.take_previous(), None);
    }

    #[test]
    fn panel_state_marks_show_time() {
        let state = PanelState::default();
        assert!(state.shown_elapsed().is_none());
        state.mark_shown();
        assert!(state.shown_elapsed().is_some());
    }
}
