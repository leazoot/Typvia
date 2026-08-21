// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! System pasteboard writes for the mobile host: the one-time sensitive copy
//! and the ordinary snippet copy.
//!
//! The two are separate seams on purpose. A normal body carries no auto-clear
//! and no sensitive marker; a secret may never leave through the plain path,
//! and `copy_plain` refuses a sensitive row instead of decrypting it (the
//! vault copy below is the only way a secret reaches the pasteboard).
//!
//! The ordinary copy goes through the host at all because the frontend has no
//! working `navigator.clipboard` in the Android system WebView, and because
//! usage must be recorded where the delivery is confirmed.
//!
//! ## One-time sensitive copy
//!
//! Pasting requires switching away from Typvia, so the timed auto-clear
//! cannot be a JS timer. Each platform enforces it as strongly as the OS
//! allows:
//!
//! - iOS: the copy is written with `UIPasteboardOptionExpirationDate` (now +
//!   the shared clear delay) and `UIPasteboardOptionLocalOnly` (no Handoff
//!   for secrets); iOS wipes it on schedule even while Typvia is suspended.
//! - Android: there is no OS expiry API. The single Kotlin write method
//!   (TypviaSensitiveClipboard.kt) schedules a guarded main-looper clear
//!   before setting the clip and marks the clip sensitive
//!   (`EXTRA_IS_SENSITIVE`); the guarantees and their honest limits (the
//!   OS focus-gates clipboard clears from a backgrounded app; process death
//!   within the window) plus the resume-sweep mitigation are documented
//!   there.

use rusqlite::Connection;
use typvia_core::model::SnippetContent;
use typvia_core::repo::SnippetRepo;
use typvia_core::vault::VaultSession;
use typvia_host_service::error::IpcError;
use typvia_host_service::service;
use typvia_host_service::service::SENSITIVE_CLIPBOARD_CLEAR_MS;
use zeroize::Zeroizing;

/// Platform seam for the single pasteboard write, so the session/usage logic
/// is testable on a desktop host. The seam does not weaken the red line: the
/// only implementations that reach a real pasteboard are the iOS one (the
/// system-enforced expiry is built inside the write itself) and the Android
/// one (the Kotlin write method schedules the clear before setting the clip)
/// — there is no path that puts a secret on the pasteboard without the
/// auto-clear attached.
pub trait SecretPasteboard {
    /// Writes `secret` as the sole pasteboard item, local-only, expiring
    /// `clear_ms` from now.
    fn write_with_expiry(&self, secret: &str, clear_ms: i64) -> Result<(), IpcError>;
}

/// Platform seam for the ordinary copy. Separate from `SecretPasteboard` so
/// no caller can reach a plain, never-clearing write with a secret in hand:
/// the two traits have no shared method and `copy_plain` refuses ciphertext.
pub trait PlainPasteboard {
    /// Writes `text` as the sole pasteboard item. No expiry: a normal body is
    /// the user's own text and must stay pasteable until they replace it.
    fn write_plain(&self, text: &str) -> Result<(), IpcError>;
}

/// Converts the shared clear delay into the seconds-from-now offset the
/// pasteboard expiration date is built from (`NSDate` counts in seconds).
/// Only the iOS write path consumes it; non-iOS hosts never build an expiry.
#[cfg(any(target_os = "ios", test))]
fn expiry_seconds(clear_ms: i64) -> f64 {
    clear_ms as f64 / 1000.0
}

/// The host's pasteboard backend: `UIPasteboard` on iOS, the ClipboardManager
/// glue on Android, an honest system error elsewhere. The Android write needs
/// the app handle for the shared one-time JVM capture; other hosts are
/// stateless.
#[cfg(target_os = "android")]
pub struct PlatformPasteboard {
    pub app: tauri::AppHandle,
}

/// See the Android variant above; this shape covers iOS and the dev shell.
#[cfg(not(target_os = "android"))]
pub struct PlatformPasteboard;

#[cfg(target_os = "ios")]
impl SecretPasteboard for PlatformPasteboard {
    fn write_with_expiry(&self, secret: &str, clear_ms: i64) -> Result<(), IpcError> {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_foundation::{NSArray, NSDate, NSDictionary, NSNumber, NSString};
        use objc2_ui_kit::{
            UIPasteboard, UIPasteboardOption, UIPasteboardOptionExpirationDate,
            UIPasteboardOptionLocalOnly,
        };

        // One plain-text representation under the standard UTI; extra
        // flavors would only widen where the secret lands.
        let value = NSString::from_str(secret);
        let value_obj: &AnyObject = &value;
        let item_key = NSString::from_str("public.utf8-plain-text");
        let item: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[&*item_key], &[value_obj]);
        let items = NSArray::from_slice(&[&*item]);

        // The expiry is built here, from the same delay the countdown UI gets,
        // and is part of every write by construction (red line).
        let expiry = NSDate::dateWithTimeIntervalSinceNow(expiry_seconds(clear_ms));
        let expiry_obj: &AnyObject = &expiry;
        let local_only = NSNumber::new_bool(true);
        let local_obj: &AnyObject = &local_only;
        // SAFETY: reading UIKit extern NSString constants; always valid.
        let (expiry_key, local_key) = unsafe {
            (
                UIPasteboardOptionExpirationDate,
                UIPasteboardOptionLocalOnly,
            )
        };
        let options: Retained<NSDictionary<UIPasteboardOption, AnyObject>> =
            NSDictionary::from_slices(&[expiry_key, local_key], &[expiry_obj, local_obj]);

        let pasteboard = UIPasteboard::generalPasteboard();
        // SAFETY: items and options hold the documented value types (an
        // NSString item representation; NSDate / NSNumber option values).
        unsafe { pasteboard.setItems_options(&items, &options) };
        Ok(())
    }
}

#[cfg(target_os = "ios")]
impl PlainPasteboard for PlatformPasteboard {
    fn write_plain(&self, text: &str) -> Result<(), IpcError> {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_foundation::{NSArray, NSDictionary, NSString};
        use objc2_ui_kit::{UIPasteboard, UIPasteboardOption};

        // One plain-text representation under the standard UTI, like the
        // sensitive write — but with an empty options dictionary: neither an
        // expiration date nor the local-only flag belongs on ordinary text
        // the user asked to keep.
        let value = NSString::from_str(text);
        let value_obj: &AnyObject = &value;
        let item_key = NSString::from_str("public.utf8-plain-text");
        let item: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[&*item_key], &[value_obj]);
        let items = NSArray::from_slice(&[&*item]);
        let options: Retained<NSDictionary<UIPasteboardOption, AnyObject>> = NSDictionary::new();

        let pasteboard = UIPasteboard::generalPasteboard();
        // SAFETY: items holds the documented value type (an NSString item
        // representation) and the options dictionary is empty.
        unsafe { pasteboard.setItems_options(&items, &options) };
        Ok(())
    }
}

#[cfg(target_os = "android")]
impl PlainPasteboard for PlatformPasteboard {
    /// Forwards to the Kotlin ordinary-copy method
    /// (`TypviaClipboard.copy`), which posts the ClipboardManager write to
    /// the main looper and reports its outcome; status 0 is the Kotlin OK and
    /// anything else must not count as a delivery. Same attach/exception
    /// handling as the sensitive write above, and the same threading rule:
    /// this runs on a blocking-pool thread, never the main thread, because
    /// the Kotlin side parks the caller on the posted write.
    fn write_plain(&self, text: &str) -> Result<(), IpcError> {
        let host =
            crate::secure_store::android::java_host(&self.app).map_err(|_| IpcError::system())?;
        let mut env = crate::secure_store::android::attach(host).map_err(|_| IpcError::system())?;
        let status = (|| -> jni::errors::Result<i32> {
            let value = env.new_string(text)?;
            env.call_method(
                host.activity.as_obj(),
                "typviaClipboardCopy",
                "(Ljava/lang/String;)I",
                &[(&value).into()],
            )?
            .i()
        })();
        match status {
            Ok(0) => Ok(()),
            Ok(_) => Err(IpcError::system()),
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                Err(IpcError::system())
            }
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl PlainPasteboard for PlatformPasteboard {
    /// Remaining hosts (the desktop dev shell) have no pasteboard binding in
    /// this crate; refusing is the honest answer rather than reporting a copy
    /// that never happened.
    fn write_plain(&self, _text: &str) -> Result<(), IpcError> {
        Err(IpcError::system())
    }
}

#[cfg(target_os = "android")]
impl SecretPasteboard for PlatformPasteboard {
    /// Android has no OS clipboard-expiry API, so the auto-clear is enforced
    /// by the single Kotlin write method this calls
    /// (`TypviaSensitiveClipboard.copy`): it schedules the guarded
    /// main-looper clear *before* setting the clip, so by construction no
    /// path puts a secret on the clipboard without its clear (red line).
    /// This side only forwards text + the shared delay over JNI and maps the
    /// status — there is no set-without-clear entry point to call. Runs on a
    /// blocking-pool thread; the Kotlin side posts the ClipboardManager work
    /// to the main looper and this thread parks briefly on its completion.
    fn write_with_expiry(&self, secret: &str, clear_ms: i64) -> Result<(), IpcError> {
        let host =
            crate::secure_store::android::java_host(&self.app).map_err(|_| IpcError::system())?;
        let mut env = crate::secure_store::android::attach(host).map_err(|_| IpcError::system())?;
        let status = (|| -> jni::errors::Result<i32> {
            // The secret crosses as a Java String because ClipData stores a
            // CharSequence — an unwipeable JVM copy exists either way once
            // it is on the clipboard; Rust-side lifetime minimization stays
            // with the Zeroizing buffer in `copy_secret`.
            let text = env.new_string(secret)?;
            env.call_method(
                host.activity.as_obj(),
                "typviaSensitiveClipboardCopy",
                "(Ljava/lang/String;J)I",
                &[(&text).into(), clear_ms.into()],
            )?
            .i()
        })();
        match status {
            // Status 0 is the Kotlin OK; any other outcome must not count as
            // a delivery (the caller records usage only on success).
            Ok(0) => Ok(()),
            Ok(_) => Err(IpcError::system()),
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                Err(IpcError::system())
            }
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl SecretPasteboard for PlatformPasteboard {
    /// Remaining hosts (the desktop dev shell) cannot attach an auto-clear,
    /// so a sensitive copy is refused outright rather than delivered without
    /// it (sensitive copy MUST carry the clear — security red line).
    fn write_with_expiry(&self, _secret: &str, _clear_ms: i64) -> Result<(), IpcError> {
        Err(IpcError::system())
    }
}

/// Copies one sensitive snippet onto the system pasteboard for the
/// secure-field round trip (open Typvia → copy once → come back and paste).
/// Mirrors the desktop `snippet_copy_secret` order:
/// decrypt from the unlocked session (locked → `permission_denied`), note
/// activity, deliver, and record usage only after a successful delivery.
/// Returns the clear delay in ms so the row countdown renders from the wire
/// value, not a duplicated constant.
pub fn copy_secret(
    conn: &Connection,
    session: &mut VaultSession,
    pasteboard: &dyn SecretPasteboard,
    id: &str,
    now: i64,
) -> Result<i64, IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    let envelope = match snippet.content {
        SnippetContent::Ciphertext(bytes) => bytes,
        SnippetContent::Plaintext(_) => {
            return Err(IpcError::conflict("snippet is not sensitive"));
        }
    };
    let plaintext = session.decrypt_content(conn, id, &envelope)?;
    // Copying is user activity: defer the idle auto-lock.
    session.note_activity(now);
    let text =
        Zeroizing::new(String::from_utf8(plaintext.to_vec()).map_err(|_| IpcError::system())?);
    pasteboard.write_with_expiry(&text, SENSITIVE_CLIPBOARD_CLEAR_MS)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(SENSITIVE_CLIPBOARD_CLEAR_MS)
}

/// Copies one ordinary snippet onto the system pasteboard, mirroring the
/// desktop `snippet_copy` order: read the body, deliver, and record usage
/// only after a confirmed write. A sensitive row is refused rather than
/// decrypted — its body is ciphertext and only `copy_secret` may deliver it,
/// because only that path carries the auto-clear.
pub fn copy_plain(
    conn: &Connection,
    pasteboard: &dyn PlainPasteboard,
    id: &str,
    now: i64,
) -> Result<(), IpcError> {
    let snippet = SnippetRepo::new(conn)
        .get(id)?
        .ok_or_else(IpcError::not_found)?;
    let body = match snippet.content {
        SnippetContent::Plaintext(text) => text,
        SnippetContent::Ciphertext(_) => {
            return Err(IpcError::conflict("snippet is sensitive"));
        }
    };
    pasteboard.write_plain(&body)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

/// Renders a template with the given field values and copies the result
/// (template fill sheet). Same order and guarantees as
/// `copy_plain`: deliver first, record usage only after a confirmed write.
/// `template_render` itself refuses a sensitive body, so no ciphertext can
/// reach the pasteboard through this path either.
pub fn copy_rendered(
    conn: &Connection,
    pasteboard: &dyn PlainPasteboard,
    id: &str,
    values: &std::collections::HashMap<String, String>,
    now: i64,
) -> Result<(), IpcError> {
    let rendered = service::template_render(conn, id, values)?;
    pasteboard.write_plain(&rendered)?;
    SnippetRepo::new(conn).record_usage(id, now)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use typvia_host_service::dto::SnippetCreateInput;
    use typvia_host_service::error::IpcErrorCode;
    use typvia_host_service::service;

    /// Records each write's clear delay; never a real pasteboard.
    struct FakePasteboard {
        writes: RefCell<Vec<(String, i64)>>,
    }

    impl FakePasteboard {
        fn new() -> Self {
            Self {
                writes: RefCell::new(Vec::new()),
            }
        }
    }

    impl SecretPasteboard for FakePasteboard {
        fn write_with_expiry(&self, secret: &str, clear_ms: i64) -> Result<(), IpcError> {
            self.writes
                .borrow_mut()
                .push((secret.to_string(), clear_ms));
            Ok(())
        }
    }

    /// Records each ordinary write; never a real pasteboard.
    struct FakePlainPasteboard {
        writes: RefCell<Vec<String>>,
    }

    impl FakePlainPasteboard {
        fn new() -> Self {
            Self {
                writes: RefCell::new(Vec::new()),
            }
        }
    }

    impl PlainPasteboard for FakePlainPasteboard {
        fn write_plain(&self, text: &str) -> Result<(), IpcError> {
            self.writes.borrow_mut().push(text.to_string());
            Ok(())
        }
    }

    fn migrated_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn text_input(title: &str, body: &str) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "text".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        }
    }

    fn secret_input(title: &str, body: &str) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        }
    }

    #[test]
    fn expiry_seconds_matches_the_shared_clear_delay() {
        assert_eq!(expiry_seconds(SENSITIVE_CLIPBOARD_CLEAR_MS), 30.0);
        assert_eq!(expiry_seconds(1_500), 1.5);
    }

    #[test]
    fn locked_session_copy_is_permission_denied_and_records_no_usage() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();
        service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        let created = service::vault_create_secret(
            &conn,
            &session,
            secret_input("Prod key", "AKIA_FAKE_EXAMPLE"),
            1_001,
        )
        .unwrap();
        session.lock();

        let pasteboard = FakePasteboard::new();
        let error = copy_secret(&conn, &mut session, &pasteboard, &created.id, 1_002).unwrap_err();
        assert_eq!(error.code, IpcErrorCode::PermissionDenied);
        assert!(pasteboard.writes.borrow().is_empty());
        let row = SnippetRepo::new(&conn).get(&created.id).unwrap().unwrap();
        assert_eq!(row.usage_count, 0);
        assert_eq!(row.last_used_at, None);
    }

    #[test]
    fn unlocked_copy_delivers_with_the_shared_delay_and_records_one_usage() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();
        service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        let created = service::vault_create_secret(
            &conn,
            &session,
            secret_input("Prod key", "AKIA_FAKE_EXAMPLE"),
            1_001,
        )
        .unwrap();

        let pasteboard = FakePasteboard::new();
        let delay = copy_secret(&conn, &mut session, &pasteboard, &created.id, 1_002).unwrap();
        assert_eq!(delay, SENSITIVE_CLIPBOARD_CLEAR_MS);
        assert_eq!(
            *pasteboard.writes.borrow(),
            vec![(
                "AKIA_FAKE_EXAMPLE".to_string(),
                SENSITIVE_CLIPBOARD_CLEAR_MS
            )]
        );
        let row = SnippetRepo::new(&conn).get(&created.id).unwrap().unwrap();
        assert_eq!(row.usage_count, 1);
        assert_eq!(row.last_used_at, Some(1_002));
    }

    #[test]
    fn copy_of_a_normal_snippet_is_a_conflict() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();
        service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        let normal = service::snippet_create(
            &conn,
            SnippetCreateInput {
                title: "Greeting".to_string(),
                body: "hello".to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            1_001,
        )
        .unwrap();

        let pasteboard = FakePasteboard::new();
        let error = copy_secret(&conn, &mut session, &pasteboard, &normal.id, 1_002).unwrap_err();
        assert_eq!(error.code, IpcErrorCode::Conflict);
        assert!(pasteboard.writes.borrow().is_empty());
    }

    #[test]
    fn plain_copy_of_a_normal_snippet_writes_the_body_and_records_one_usage() {
        let conn = migrated_conn();
        let created =
            service::snippet_create(&conn, text_input("Greeting", "hello there"), 1_000).unwrap();

        let pasteboard = FakePlainPasteboard::new();
        copy_plain(&conn, &pasteboard, &created.id, 1_001).unwrap();

        assert_eq!(*pasteboard.writes.borrow(), vec!["hello there".to_string()]);
        let row = SnippetRepo::new(&conn).get(&created.id).unwrap().unwrap();
        assert_eq!(row.usage_count, 1);
        assert_eq!(row.last_used_at, Some(1_001));
    }

    #[test]
    fn plain_copy_of_a_sensitive_snippet_is_refused_and_writes_nothing() {
        let conn = migrated_conn();
        let mut session = VaultSession::new();
        service::vault_initialize(&conn, &mut session, "correct horse", 1_000).unwrap();
        let created = service::vault_create_secret(
            &conn,
            &session,
            secret_input("Prod key", "AKIA_FAKE_EXAMPLE"),
            1_001,
        )
        .unwrap();

        let pasteboard = FakePlainPasteboard::new();
        let error = copy_plain(&conn, &pasteboard, &created.id, 1_002).unwrap_err();

        assert_eq!(error.code, IpcErrorCode::Conflict);
        assert!(pasteboard.writes.borrow().is_empty());
        let row = SnippetRepo::new(&conn).get(&created.id).unwrap().unwrap();
        assert_eq!(row.usage_count, 0);
        assert_eq!(row.last_used_at, None);
    }

    #[test]
    fn plain_copy_of_an_unknown_id_is_not_found() {
        let conn = migrated_conn();

        let pasteboard = FakePlainPasteboard::new();
        let error = copy_plain(&conn, &pasteboard, "no-such-snippet", 1_000).unwrap_err();

        assert_eq!(error, IpcError::not_found());
        assert!(pasteboard.writes.borrow().is_empty());
    }

    #[test]
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    fn platform_pasteboard_on_unsupported_hosts_refuses_rather_than_copying_without_a_clear() {
        let error = PlatformPasteboard
            .write_with_expiry("AKIA_FAKE_EXAMPLE", SENSITIVE_CLIPBOARD_CLEAR_MS)
            .unwrap_err();
        assert_eq!(error, IpcError::system());
    }
}
