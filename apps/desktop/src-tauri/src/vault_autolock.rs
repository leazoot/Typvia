//! System-event vault auto-lock: drops the master key when the machine goes
//! to sleep or the screen locks, so an unlocked vault never survives a
//! lid-close or a lock shortcut. The idle timeout (5 min) still covers
//! everything these events cannot see.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2_app_kit::{NSWorkspace, NSWorkspaceWillSleepNotification};
use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSNotificationCenter};
use tauri::{AppHandle, Manager};

use crate::commands::AppState;

/// Distributed notification loginwindow posts when the screen locks. There is
/// no public constant for it; the name is stable across macOS releases.
const SCREEN_LOCKED: &str = "com.apple.screenIsLocked";

/// Registers both observers. Called once from setup, on the main thread.
pub fn register(app: &AppHandle) {
    let workspace_center = NSWorkspace::sharedWorkspace().notificationCenter();
    register_on(
        &workspace_center,
        unsafe { NSWorkspaceWillSleepNotification },
        app.clone(),
    );
    let screen_lock_name = objc2_foundation::NSString::from_str(SCREEN_LOCKED);
    let distributed_center = NSDistributedNotificationCenter::defaultCenter();
    register_on(&distributed_center, &screen_lock_name, app.clone());
}

fn register_on(center: &NSNotificationCenter, name: &objc2_foundation::NSString, app: AppHandle) {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        if let Some(state) = app.try_state::<AppState>() {
            state.lock_vault_for_system_event();
        }
    });
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    // The observer token must live as long as the app; the app lives as long
    // as the process, so the token is intentionally leaked.
    std::mem::forget(observer);
}
