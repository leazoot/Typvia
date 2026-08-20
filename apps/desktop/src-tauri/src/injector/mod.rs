//! Desktop text-injection engine: clipboard paste is the primary path,
//! Private-source keystroke synthesis the auxiliary one.
//!
//! Security posture: on the paste path the plaintext necessarily transits
//! the system pasteboard; the previous clipboard text is restored on every
//! outcome and the buffers this module owns are wiped after use. Injected
//! content must never reach any log or error message.

mod chunk;
#[cfg(target_os = "macos")]
mod macos;

use std::fmt;

/// How the text reaches the frontmost application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InjectionMethod {
    /// Write to the clipboard, synthesize a paste, restore the clipboard.
    /// Robust for long/multi-line text and immune to IME composition.
    Paste,
    /// Private-source unicode keystrokes in bounded chunks; bypasses CJK
    /// IME interception. For short text or paste-hostile targets.
    Keystrokes,
}

impl Default for InjectionMethod {
    /// Paste is the primary path.
    fn default() -> Self {
        Self::Paste
    }
}

/// Injection failures. Messages are static by design: the injected text or
/// clipboard contents must never appear in an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InjectorError {
    /// Accessibility permission is missing; callers degrade to copy.
    PermissionDenied,
    /// Clipboard read/write/restore failed.
    Clipboard,
    /// Keyboard event synthesis or posting failed.
    Synthesis,
    /// No injector exists for this platform yet.
    Unsupported,
}

impl fmt::Display for InjectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::PermissionDenied => "accessibility permission not granted",
            Self::Clipboard => "clipboard operation failed",
            Self::Synthesis => "keyboard event synthesis failed",
            Self::Unsupported => "text injection is not supported on this platform",
        };
        f.write_str(message)
    }
}

impl std::error::Error for InjectorError {}

/// Platform seam for text injection: one implementation per desktop OS;
/// service code and its tests depend on this trait only.
pub trait Injector: Send {
    /// Whether the OS currently allows synthetic keystrokes.
    fn accessibility_granted(&self) -> bool;

    /// Delivers `text` into the frontmost application. Every outcome leaves
    /// the clipboard's text content as it was found; a non-text clipboard
    /// (image, files) cannot round-trip and is cleared instead.
    fn inject(&mut self, text: &str, method: InjectionMethod) -> Result<(), InjectorError>;

    /// Puts `text` on the clipboard (also the no-permission fallback).
    fn copy(&mut self, text: &str) -> Result<(), InjectorError>;

    /// Puts `text` on the clipboard and remembers it, so a later
    /// [`Injector::clear_guarded`] can wipe it *only if the user has not copied
    /// something else since*. This is the sensitive-copy path: the
    /// secret sits on the clipboard for a bounded countdown, then auto-clears.
    fn copy_guarded(&mut self, text: &str) -> Result<(), InjectorError>;

    /// Clears the clipboard iff it still holds the last
    /// [`Injector::copy_guarded`] value, then forgets it. Returns whether it
    /// cleared. A no-op returning `false` when nothing is guarded or the
    /// clipboard has changed — a copy the user made in between is left alone.
    fn clear_guarded(&mut self) -> Result<bool, InjectorError>;
}

/// The injector for the current platform. Windows lands in a later batch;
/// until then non-macOS hosts get `Unsupported`.
pub fn platform_injector() -> Result<Box<dyn Injector>, InjectorError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacInjector::new()?))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(InjectorError::Unsupported)
    }
}

/// Stand-in when no platform injector could be built (unsupported OS, or a
/// clipboard handle that failed to open). Every operation reports
/// `Unsupported` so the app still runs and other features keep working.
pub struct NullInjector;

impl Injector for NullInjector {
    fn accessibility_granted(&self) -> bool {
        false
    }

    fn inject(&mut self, _text: &str, _method: InjectionMethod) -> Result<(), InjectorError> {
        Err(InjectorError::Unsupported)
    }

    fn copy(&mut self, _text: &str) -> Result<(), InjectorError> {
        Err(InjectorError::Unsupported)
    }

    fn copy_guarded(&mut self, _text: &str) -> Result<(), InjectorError> {
        Err(InjectorError::Unsupported)
    }

    fn clear_guarded(&mut self) -> Result<bool, InjectorError> {
        // Nothing was ever guarded on an unsupported host.
        Ok(false)
    }
}

/// The platform injector, or a [`NullInjector`] if one can't be built. Used
/// at startup so a clipboard-init failure never blocks the app from running.
pub fn platform_injector_or_null() -> Box<dyn Injector> {
    platform_injector().unwrap_or_else(|_| Box::new(NullInjector))
}

#[cfg(test)]
mod tests {
    use super::{InjectionMethod, InjectorError};

    #[test]
    fn default_method_is_paste() {
        // Clipboard paste is the primary path; keystrokes are
        // opt-in for paste-hostile targets.
        assert_eq!(InjectionMethod::default(), InjectionMethod::Paste);
    }

    #[test]
    fn error_messages_carry_no_dynamic_content() {
        // Injected text and clipboard contents must never surface in an
        // error; the Display strings are a fixed, payload-free set.
        for error in [
            InjectorError::PermissionDenied,
            InjectorError::Clipboard,
            InjectorError::Synthesis,
            InjectorError::Unsupported,
        ] {
            let message = error.to_string();
            assert!(!message.is_empty());
            assert!(
                message.is_ascii(),
                "static English messages only: {message:?}"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_hosts_report_unsupported() {
        assert_eq!(
            super::platform_injector().err(),
            Some(InjectorError::Unsupported)
        );
    }
}
