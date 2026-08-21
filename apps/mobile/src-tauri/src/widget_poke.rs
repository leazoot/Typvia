// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Best-effort widget refresh after a successful KeyboardSnapshot write:
//! widgets are event-driven, so the host pokes the platform
//! widget machinery instead of running any polling timeline. A poke failure
//! is swallowed by design — the snapshot write it follows has already
//! succeeded and must stay successful either way.

/// Asks the platform to re-render home-screen widgets from the snapshot
/// just written. No-op on hosts without a widget surface.
///
/// On Android the poke rides the shared JVM/activity capture, which exists
/// only once the main webview is up — the startup snapshot write therefore
/// skips the poke there (widgets re-read on the next mutation-driven
/// refresh), while iOS pokes from startup on.
pub fn notify_widgets(app: &tauri::AppHandle) {
    let _ = app;
    #[cfg(target_os = "android")]
    android::notify(app);
    #[cfg(target_os = "ios")]
    ios::notify();
}

#[cfg(target_os = "android")]
mod android {
    /// Calls `MainActivity.typviaWidgetRefresh()` over the same JNI bridge
    /// the secure store uses (one JVM/activity capture per process).
    pub(super) fn notify(app: &tauri::AppHandle) {
        // Any failure is dropped without logging: the refresh is cosmetic
        // next to the snapshot write, and widget-adjacent errors carry no
        // information worth a logcat line.
        let Ok(host) = crate::secure_store::android::java_host(app) else {
            return;
        };
        let Ok(mut env) = host.vm.attach_current_thread() else {
            return;
        };
        let result = env.call_method(host.activity.as_obj(), "typviaWidgetRefresh", "()V", &[]);
        // A thrown Java exception must be cleared before returning to Rust,
        // or the next JNI call aborts the process.
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let _ = result;
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use objc2::msg_send;
    use objc2::runtime::AnyClass;

    /// Calls the Swift bridge class in the app target (gen/apple
    /// WidgetReload.swift). Looked up by name at runtime instead of linked
    /// eagerly because this crate is also built as a standalone cdylib
    /// (simulator dev artifact), where an app-executable symbol cannot
    /// resolve at link time; a missing class degrades to a no-op, which
    /// matches the best-effort contract.
    pub(super) fn notify() {
        let Some(class) = AnyClass::get(c"TypviaWidgetReload") else {
            return;
        };
        // SAFETY: `+[TypviaWidgetReload reload]` is our own zero-argument
        // void class method; name and signature are the Rust<->Swift
        // contract.
        let () = unsafe { msg_send![class, reload] };
    }
}
