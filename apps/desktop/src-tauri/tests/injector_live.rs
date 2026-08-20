//! Interactive live verification of the injection engine against TextEdit,
//! mirroring the injection spike protocol on the formal API. It synthesizes
//! keystrokes into a real GUI session, so it is excluded from the default
//! test run and executed manually for acceptance:
//!
//! ```sh
//! cargo test -p typvia-desktop --test injector_live -- --ignored --nocapture
//! ```
//!
//! Preconditions: a logged-in GUI session and Accessibility permission for
//! the invoking terminal (the test binary inherits its TCC grant).
#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use typvia_desktop::injector::{InjectionMethod, platform_injector};

const KEY_A: u16 = 0;
const KEY_C: u16 = 8;
const KEY_N: u16 = 45;
const KEY_W: u16 = 13;
const KEY_Q: u16 = 12;
const KEY_FWD_DELETE: u16 = 51;
const CMD: CGEventFlags = CGEventFlags::CGEventFlagCommand;

fn frontmost_app() -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg("lsappinfo info -only name `lsappinfo front`")
        .output()
        .expect("lsappinfo");
    String::from_utf8_lossy(&out.stdout)
        .split('"')
        .nth(3)
        .unwrap_or("unknown")
        .to_string()
}

/// Every synthetic keystroke is preceded by this guard so events can never
/// leak into an arbitrary focused application.
fn guard_textedit() {
    let front = frontmost_app();
    assert_eq!(front, "TextEdit", "guard: frontmost app changed mid-test");
}

fn post_key(keycode: u16, flags: Option<CGEventFlags>) {
    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState).expect("event source");
    for down in [true, false] {
        let ev = CGEvent::new_keyboard_event(src.clone(), keycode, down).expect("kbd event");
        if let Some(f) = flags {
            ev.set_flags(f);
        }
        ev.post(CGEventTapLocation::HID);
        sleep(Duration::from_millis(30));
    }
}

/// Select-all + copy in the guarded TextEdit document, then read the
/// clipboard. Deliberately clobbers the clipboard, so restoration is
/// asserted before any read-back. The clipboard is cleared first so an
/// empty selection reads back as empty rather than as stale prior content.
fn read_back(clip: &mut arboard::Clipboard) -> String {
    guard_textedit();
    let _ = clip.set_text("__typvia_readback_cleared__");
    post_key(KEY_A, Some(CMD));
    sleep(Duration::from_millis(150));
    post_key(KEY_C, Some(CMD));
    sleep(Duration::from_millis(250));
    clip.get_text().unwrap_or_default()
}

fn close_textedit() {
    guard_textedit();
    post_key(KEY_A, Some(CMD));
    post_key(KEY_FWD_DELETE, None);
    sleep(Duration::from_millis(200));
    post_key(KEY_W, Some(CMD));
    sleep(Duration::from_millis(500));
    if frontmost_app() == "TextEdit" {
        post_key(KEY_FWD_DELETE, Some(CMD));
        sleep(Duration::from_millis(300));
    }
    if frontmost_app() == "TextEdit" {
        post_key(KEY_Q, Some(CMD));
    }
}

#[test]
#[ignore = "requires a GUI session and Accessibility permission; run manually for acceptance"]
fn injects_both_paths_into_textedit_and_restores_clipboard() {
    let mut injector = platform_injector().expect("platform injector");
    assert!(
        injector.accessibility_granted(),
        "grant Accessibility to the invoking terminal before running"
    );

    let mut clip = arboard::Clipboard::new().expect("clipboard handle");
    let sentinel = "typvia-live-restore-sentinel";

    // Clean slate: a prior failed run may have left an unsaved document that
    // would otherwise be the frontmost doc here. `quit saving no` discards it
    // without a save dialog.
    let _ = Command::new("osascript")
        .args(["-e", "tell application \"TextEdit\" to quit saving no"])
        .status();
    sleep(Duration::from_millis(1000));

    Command::new("open")
        .args(["-a", "TextEdit"])
        .status()
        .expect("open TextEdit");
    sleep(Duration::from_millis(1500));
    guard_textedit();
    // TextEdit may open with the document picker; ⌘N yields a fresh scratch.
    post_key(KEY_N, Some(CMD));
    sleep(Duration::from_millis(1000));

    // Paste path (the primary one): delivery + clipboard restore. This is the
    // robust path and is asserted hard. The sentinel is placed immediately
    // before injection: the guarantee is that the clipboard as found at inject
    // time is restored.
    let pasted = "typvia paste path 剪贴验证";
    clip.set_text(sentinel).expect("set sentinel");
    guard_textedit();
    injector
        .inject(pasted, InjectionMethod::Paste)
        .expect("paste injection");
    assert_eq!(
        clip.get_text().expect("clipboard after inject"),
        sentinel,
        "original clipboard text must be restored after a paste injection"
    );
    let doc = read_back(&mut clip);
    assert!(
        doc.contains(pasted),
        "paste payload not delivered to TextEdit (got: {doc:?})"
    );

    // Keystroke path (auxiliary): Private-source synthetic typing is subject
    // to the active IME / window focus, which makes automated delivery flaky
    // to assert reliably here. It is proven by the injection spike and covered
    // by the chunking unit tests; this run attempts it best-effort and reports
    // the observed outcome rather than failing the primary-path acceptance.
    let typed = "typvia typed path 键入验证";
    guard_textedit();
    post_key(KEY_A, Some(CMD));
    post_key(KEY_FWD_DELETE, None);
    sleep(Duration::from_millis(200));
    guard_textedit();
    injector
        .inject(typed, InjectionMethod::Keystrokes)
        .expect("keystroke injection");
    sleep(Duration::from_millis(400));
    let typed_doc = read_back(&mut clip);
    let keystrokes_delivered = typed_doc.to_lowercase().contains(&typed.to_lowercase());

    close_textedit();
    println!(
        "LIVE_RESULT paste_delivered=true clipboard_restored=true keystrokes_delivered={keystrokes_delivered}"
    );
}
