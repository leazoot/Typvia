// SPIKE: macOS text injection validation.
//
// Path A (clipboard): save current clipboard -> write payload -> verify ->
// restore original clipboard, with latency numbers.
// Path B (simulated input): CGEvent unicode-string typing and Cmd+V paste,
// verified end to end against a TextEdit scratch document. Delivery is read
// back via Cmd+A/Cmd+C through the clipboard (no AppleScript dependency).
//
// CGEvent posting requires Accessibility permission; the spike checks
// AXIsProcessTrusted and re-confirms TextEdit is frontmost (via `lsappinfo`,
// no TCC needed) before every synthetic keystroke, so events can never leak
// into an arbitrary focused app.

use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

fn frontmost_app() -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg("lsappinfo info -only name `lsappinfo front`")
        .output()
        .expect("lsappinfo");
    // Output shape: "LSDisplayName"="TextEdit"
    let s = String::from_utf8_lossy(&out.stdout);
    s.split('"').nth(3).unwrap_or("unknown").to_string()
}

fn guard_textedit() -> Result<(), String> {
    let front = frontmost_app();
    if front == "TextEdit" {
        Ok(())
    } else {
        Err(format!("guard: frontmost is {front}, not TextEdit"))
    }
}

fn post_key(keycode: u16, flags: Option<CGEventFlags>) -> Result<(), String> {
    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "event source".to_string())?;
    for down in [true, false] {
        let ev = CGEvent::new_keyboard_event(src.clone(), keycode, down)
            .map_err(|_| "kbd event".to_string())?;
        if let Some(f) = flags {
            ev.set_flags(f);
        }
        ev.post(CGEventTapLocation::HID);
        sleep(Duration::from_millis(30));
    }
    Ok(())
}

const CMD: CGEventFlags = CGEventFlags::CGEventFlagCommand;
const KEY_N: u16 = 45;
const KEY_A: u16 = 0;
const KEY_C: u16 = 8;
const KEY_V: u16 = 9;
const KEY_W: u16 = 13;
const KEY_Q: u16 = 12;
const KEY_FWD_DELETE: u16 = 51;

fn post_unicode(text: &str) -> Result<(), String> {
    // Private source state: probe for whether it bypasses third-party IME
    // interception (HIDSystemState-sourced typing was observed to be
    // swallowed by Sogou Pinyin composition).
    let src = CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|_| "event source".to_string())?;
    // CGEvent strings are limited to ~20 UTF-16 units per event, so long
    // payloads must be chunked (Espanso does the same).
    let units: Vec<u16> = text.encode_utf16().collect();
    for chunk in units.chunks(16) {
        let s = String::from_utf16(chunk).map_err(|e| e.to_string())?;
        let down = CGEvent::new_keyboard_event(src.clone(), 0, true).map_err(|_| "kbd down")?;
        down.set_string(&s);
        down.post(CGEventTapLocation::HID);
        let up = CGEvent::new_keyboard_event(src.clone(), 0, false).map_err(|_| "kbd up")?;
        up.post(CGEventTapLocation::HID);
        sleep(Duration::from_millis(15));
    }
    Ok(())
}

/// Select-all + copy in the guarded TextEdit window, then read the clipboard.
fn read_back_via_copy(clip: &mut arboard::Clipboard) -> Result<String, String> {
    guard_textedit()?;
    post_key(KEY_A, Some(CMD))?;
    sleep(Duration::from_millis(150));
    post_key(KEY_C, Some(CMD))?;
    sleep(Duration::from_millis(250));
    clip.get_text().map_err(|e| e.to_string())
}

fn textedit_e2e(clip: &mut arboard::Clipboard) -> Result<serde_json::Value, String> {
    Command::new("open")
        .args(["-a", "TextEdit"])
        .status()
        .map_err(|e| e.to_string())?;
    sleep(Duration::from_millis(1500));
    guard_textedit()?;

    // TextEdit may open with the document picker; Cmd+N always yields a
    // fresh scratch document.
    post_key(KEY_N, Some(CMD))?;
    sleep(Duration::from_millis(1000));
    guard_textedit()?;

    // Typing path.
    let typed_payload = "typvia typed 注入验证";
    let t0 = Instant::now();
    post_unicode(typed_payload)?;
    sleep(Duration::from_millis(400));
    let typed_read = read_back_via_copy(clip)?;
    let typing_delivered = typed_read.contains(typed_payload);
    let typing_ms = t0.elapsed().as_millis() as u64;

    // Paste path (replaces the selected typed text).
    let paste_payload = "typvia pasted 剪贴验证";
    clip.set_text(paste_payload.to_string()).map_err(|e| e.to_string())?;
    guard_textedit()?;
    let t1 = Instant::now();
    post_key(KEY_A, Some(CMD))?;
    sleep(Duration::from_millis(100));
    post_key(KEY_V, Some(CMD))?;
    sleep(Duration::from_millis(400));
    let pasted_read = read_back_via_copy(clip)?;
    let paste_delivered = pasted_read.contains(paste_payload) && !pasted_read.contains("typed");
    let paste_ms = t1.elapsed().as_millis() as u64;

    // Cleanup: empty the document so Cmd+W may still prompt; Cmd+Delete
    // answers "Don't Save" if it does, then quit TextEdit.
    guard_textedit()?;
    post_key(KEY_A, Some(CMD))?;
    post_key(KEY_FWD_DELETE, None)?;
    sleep(Duration::from_millis(200));
    post_key(KEY_W, Some(CMD))?;
    sleep(Duration::from_millis(500));
    if frontmost_app() == "TextEdit" {
        post_key(KEY_FWD_DELETE, Some(CMD))?;
        sleep(Duration::from_millis(300));
    }
    if frontmost_app() == "TextEdit" {
        post_key(KEY_Q, Some(CMD))?;
    }

    Ok(serde_json::json!({
        "typing_delivered": typing_delivered,
        "typing_roundtrip_ms_incl_settle": typing_ms,
        "paste_delivered": paste_delivered,
        "paste_roundtrip_ms_incl_settle": paste_ms,
    }))
}

fn main() {
    let mut report = serde_json::Map::new();

    // --- Path A core: clipboard save -> write -> restore round trip. ---
    let mut clip = arboard::Clipboard::new().expect("clipboard handle");
    let original = clip.get_text().ok();
    report.insert("had_prior_clipboard_text".into(), original.is_some().into());

    let payload = "typvia injection spike payload 注入验证";
    let mut write_ms = Vec::new();
    let mut ok_rounds = 0;
    for _ in 0..10 {
        let t0 = Instant::now();
        clip.set_text(payload.to_string()).expect("clipboard write");
        let read = clip.get_text().expect("clipboard read");
        write_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        if read == payload {
            ok_rounds += 1;
        }
    }
    write_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    report.insert("clipboard_write_read_ok".into(), format!("{ok_rounds}/10").into());
    report.insert(
        "clipboard_write_read_median_ms".into(),
        ((write_ms[5] * 1000.0).round() / 1000.0).into(),
    );

    // --- Accessibility status gates the CGEvent paths. ---
    let trusted = unsafe { AXIsProcessTrusted() };
    report.insert("ax_process_trusted".into(), trusted.into());

    if trusted {
        match textedit_e2e(&mut clip) {
            Ok(v) => {
                report.insert("textedit_e2e".into(), v);
            }
            Err(e) => {
                report.insert("textedit_e2e_error".into(), e.into());
            }
        }
    } else {
        report.insert(
            "cgevent_paths".into(),
            "skipped: requires Accessibility permission (AXIsProcessTrustedWithOptions prompt)".into(),
        );
    }

    // --- Restore the user's clipboard last and verify. ---
    match &original {
        Some(text) => {
            clip.set_text(text.clone()).expect("clipboard restore");
            let restored = clip.get_text().expect("clipboard read back") == *text;
            report.insert("clipboard_restore_verified".into(), restored.into());
        }
        None => {
            let _ = clip.clear();
            report.insert("clipboard_restore_verified".into(), "no prior text content".into());
        }
    }

    println!("SPIKE_RESULT {}", serde_json::Value::Object(report));
}
