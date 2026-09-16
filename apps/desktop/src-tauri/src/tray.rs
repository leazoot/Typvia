// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Resident entry: the menu-bar (macOS) / tray (Windows) icon and the small
//! card it opens. The card is a resident hidden window like the panel. A click
//! records where the icon is and which app was frontmost (the insert target),
//! the page lays itself out and reports its height, and only then is the
//! window placed against the icon and shown — so it never flashes at a stale
//! size or position.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use typvia_host_service::error::IpcError;

use crate::main_window;
use crate::panel::frontmost;

/// Window label of the tray card (declared in tauri.conf.json).
pub const TRAY_LABEL: &str = "tray";

/// Card width in logical pixels (matches tauri.conf.json).
const CARD_WIDTH: f64 = 300.0;
/// Space between the icon and the card, in logical pixels.
const GAP: f64 = 6.0;
/// A click on the icon right after the card hid on blur is that same click:
/// it must not reopen the card it just closed.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

/// A rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Area {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// The icon's place, the app to hand focus back to, and when the card last
/// hid. Managed by Tauri as shared state.
#[derive(Default)]
pub struct TrayState {
    anchor: Mutex<Option<Area>>,
    previous: Mutex<Option<(i32, Option<String>)>>,
    hidden_at: Mutex<Option<Instant>>,
}

impl TrayState {
    /// Bundle id of the app the card inserts into, for the app-rule gate.
    pub fn destination_app(&self) -> Option<String> {
        self.previous
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().and_then(|(_, bundle)| bundle.clone()))
    }

    fn anchor(&self) -> Option<Area> {
        self.anchor.lock().ok().and_then(|guard| *guard)
    }

    fn just_hidden(&self) -> bool {
        self.hidden_at
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .is_some_and(|at| at.elapsed() < REOPEN_GUARD)
    }
}

/// Top-left of the card: centred on the icon and kept inside the screen
/// horizontally; below the icon when it sits in the top half of the screen
/// (the macOS menu bar), above it otherwise (a bottom taskbar).
fn place(icon: Area, card_width: f64, card_height: f64, screen: Area, gap: f64) -> (f64, f64) {
    let centred = icon.x + icon.width / 2.0 - card_width / 2.0;
    let right_edge = screen.x + screen.width - card_width - gap;
    let x = centred.min(right_edge).max(screen.x + gap);
    let icon_on_top = icon.y + icon.height / 2.0 < screen.y + screen.height / 2.0;
    let y = if icon_on_top {
        icon.y + icon.height + gap
    } else {
        icon.y - card_height - gap
    };
    (x, y)
}

/// Adds the icon. Any mouse button opens the card, as menu-bar items do.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    // The macOS menu bar tints template images to match every other item
    // (light, dark, highlighted); the Windows taskbar tints nothing, so the
    // coral mascot stays readable there.
    #[cfg(target_os = "macos")]
    let builder = TrayIconBuilder::with_id("typvia")
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = TrayIconBuilder::with_id("typvia").icon(tauri::include_image!("icons/tray.png"));
    builder
        .tooltip("Typvia")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                rect,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle(app: &AppHandle, rect: tauri::Rect) {
    let Some(window) = app.get_webview_window(TRAY_LABEL) else {
        return;
    };
    let state = app.state::<TrayState>();
    if window.is_visible().unwrap_or(false) {
        hide(app, false);
        return;
    }
    if state.just_hidden() {
        return;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let position = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    if let Ok(mut guard) = state.anchor.lock() {
        *guard = Some(Area {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        });
    }
    // Clicking a status item does not activate Typvia, so the frontmost app
    // is still the one the reader was typing in.
    if let Ok(mut guard) = state.previous.lock() {
        *guard = frontmost::frontmost_app().map(|front| (front.pid, front.bundle_id));
    }
    let _ = window.emit_to(TRAY_LABEL, "tray:show", ());
}

/// Hides the card; `restore_focus` hands focus back to the app that was
/// frontmost when the icon was clicked. Main thread only.
pub fn hide(app: &AppHandle, restore_focus: bool) {
    let Some(window) = app.get_webview_window(TRAY_LABEL) else {
        return;
    };
    let _ = window.hide();
    let state = app.state::<TrayState>();
    if let Ok(mut guard) = state.hidden_at.lock() {
        *guard = Some(Instant::now());
    }
    let previous = state
        .previous
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if restore_focus && let Some((pid, _)) = previous {
        frontmost::activate_pid(pid);
    }
}

/// The page has laid the card out at `height` logical pixels: size the
/// window to it, place it against the icon, show it.
#[tauri::command]
pub fn tray_present(app: AppHandle, window: WebviewWindow, height: f64) -> Result<(), IpcError> {
    if window.label() != TRAY_LABEL {
        return Err(IpcError::validation("only the tray card presents itself"));
    }
    if !(height.is_finite() && (60.0..=800.0).contains(&height)) {
        return Err(IpcError::validation("card height out of range"));
    }
    let Some(icon) = app.state::<TrayState>().anchor() else {
        return Ok(());
    };
    let scale = window.scale_factor().map_err(|_| IpcError::system())?;
    let monitor = window
        .monitor_from_point(icon.x, icon.y)
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Err(IpcError::system());
    };
    let screen = Area {
        x: f64::from(monitor.position().x),
        y: f64::from(monitor.position().y),
        width: f64::from(monitor.size().width),
        height: f64::from(monitor.size().height),
    };
    let (x, y) = place(
        icon,
        CARD_WIDTH * scale,
        height * scale,
        screen,
        GAP * scale,
    );
    window
        .set_size(LogicalSize::new(CARD_WIDTH, height))
        .map_err(|_| IpcError::system())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|_| IpcError::system())?;
    window.show().map_err(|_| IpcError::system())?;
    let _ = window.set_focus();
    Ok(())
}

/// Esc: put the card away and give focus back.
#[tauri::command]
pub fn tray_hide(app: AppHandle) {
    hide(&app, true);
}

/// Opens the main window on the Library.
#[tauri::command]
pub fn tray_open_library(app: AppHandle) {
    hide(&app, false);
    main_window::step_back(&app);
    let _ = app.emit_to(main_window::MAIN_LABEL, "tray:library", ());
}

/// Quits Typvia; the exit hook stops the managed engine.
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    app.exit(0);
}

/// Whether Typvia starts at login.
#[tauri::command]
pub fn autostart_status(app: AppHandle) -> Result<bool, IpcError> {
    app.autolaunch()
        .is_enabled()
        .map_err(|_| IpcError::system())
}

/// Turns start-at-login on or off and reports what the system now holds.
#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<bool, IpcError> {
    let launcher = app.autolaunch();
    let changed = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    changed.map_err(|_| IpcError::system())?;
    launcher.is_enabled().map_err(|_| IpcError::system())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Area = Area {
        x: 0.0,
        y: 0.0,
        width: 3024.0,
        height: 1964.0,
    };

    #[test]
    fn opens_below_a_menu_bar_icon_centred_on_it() {
        let icon = Area {
            x: 2000.0,
            y: 0.0,
            width: 44.0,
            height: 48.0,
        };
        assert_eq!(place(icon, 600.0, 700.0, SCREEN, 12.0), (1722.0, 60.0));
    }

    #[test]
    fn opens_above_a_taskbar_icon() {
        let icon = Area {
            x: 1500.0,
            y: 1900.0,
            width: 48.0,
            height: 48.0,
        };
        assert_eq!(place(icon, 600.0, 700.0, SCREEN, 12.0), (1224.0, 1188.0));
    }

    #[test]
    fn stays_on_screen_next_to_an_icon_at_the_right_edge() {
        let icon = Area {
            x: 2990.0,
            y: 0.0,
            width: 30.0,
            height: 48.0,
        };
        assert_eq!(place(icon, 600.0, 700.0, SCREEN, 12.0), (2412.0, 60.0));
    }
}
