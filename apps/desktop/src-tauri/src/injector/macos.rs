//! macOS injector: arboard clipboard + CGEvent keystroke synthesis, the
//! exact combination validated by the TASK-007 spike (docs/spikes/injection.md).

use std::thread::sleep;
use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use zeroize::Zeroizing;

use super::chunk::chunk_utf16;
use super::{InjectionMethod, Injector, InjectorError};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// kVK_ANSI_V, for the synthesized ⌘V.
const KEY_V: u16 = 9;
/// Pause between the key-down and key-up posts of the paste shortcut.
const KEY_PHASE_GAP: Duration = Duration::from_millis(30);
/// Time the target app gets to consume the paste before the clipboard is
/// restored underneath it.
const PASTE_SETTLE: Duration = Duration::from_millis(300);
/// A CGEvent string carries ~20 UTF-16 units; 16 leaves headroom.
const TYPE_CHUNK_UNITS: usize = 16;
/// Pacing between typed chunks so the target app keeps up.
const TYPE_CHUNK_GAP: Duration = Duration::from_millis(15);

pub(crate) struct MacInjector {
    clipboard: arboard::Clipboard,
}

impl MacInjector {
    pub(crate) fn new() -> Result<Self, InjectorError> {
        let clipboard = arboard::Clipboard::new().map_err(|_| InjectorError::Clipboard)?;
        Ok(Self { clipboard })
    }

    fn paste(&mut self, text: &str) -> Result<(), InjectorError> {
        // Only text content can round-trip; a non-text clipboard reads as
        // None here and is cleared after the paste (recorded platform limit).
        let original = self.clipboard.get_text().ok().map(Zeroizing::new);
        self.clipboard
            .set_text(text)
            .map_err(|_| InjectorError::Clipboard)?;
        let posted = post_cmd_v();
        if posted.is_ok() {
            sleep(PASTE_SETTLE);
        }
        // Restoration runs on every outcome so a failed paste still leaves
        // the clipboard as it was found.
        let restored = match original {
            Some(previous) => self
                .clipboard
                .set_text(previous.as_str())
                .map_err(|_| InjectorError::Clipboard),
            None => self.clipboard.clear().map_err(|_| InjectorError::Clipboard),
        };
        posted.and(restored)
    }

    fn keystrokes(&self, text: &str) -> Result<(), InjectorError> {
        // Private event-source state bypasses CJK IME composition, which
        // swallows HID-sourced synthetic typing (spike finding #1).
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|()| InjectorError::Synthesis)?;
        for chunk in chunk_utf16(text, TYPE_CHUNK_UNITS) {
            let chunk = Zeroizing::new(chunk);
            for down in [true, false] {
                let event = CGEvent::new_keyboard_event(source.clone(), 0, down)
                    .map_err(|()| InjectorError::Synthesis)?;
                if down {
                    event.set_string(&chunk);
                }
                event.post(CGEventTapLocation::HID);
            }
            sleep(TYPE_CHUNK_GAP);
        }
        Ok(())
    }
}

impl Injector for MacInjector {
    fn accessibility_granted(&self) -> bool {
        // Read-only TCC query; prompting is a UI concern handled upstream.
        unsafe { AXIsProcessTrusted() }
    }

    fn inject(&mut self, text: &str, method: InjectionMethod) -> Result<(), InjectorError> {
        // Both paths post keyboard events, so gate before touching anything.
        if !self.accessibility_granted() {
            return Err(InjectorError::PermissionDenied);
        }
        match method {
            InjectionMethod::Paste => self.paste(text),
            InjectionMethod::Keystrokes => self.keystrokes(text),
        }
    }

    fn copy(&mut self, text: &str) -> Result<(), InjectorError> {
        self.clipboard
            .set_text(text)
            .map_err(|_| InjectorError::Clipboard)
    }
}

fn post_cmd_v() -> Result<(), InjectorError> {
    // Command-flagged shortcuts are not routed through IME composition, so
    // the regular HID source suffices here (unlike the typing path).
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| InjectorError::Synthesis)?;
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), KEY_V, down)
            .map_err(|()| InjectorError::Synthesis)?;
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        event.post(CGEventTapLocation::HID);
        sleep(KEY_PHASE_GAP);
    }
    Ok(())
}
