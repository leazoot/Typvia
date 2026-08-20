// SPIKE: one minimal "business" function (KeyboardSnapshot-style
// JSON parsing) exposed over both candidate FFI paths so Swift and Kotlin
// integration cost can be compared on identical semantics.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde::Deserialize;

uniffi::setup_scaffolding!();

#[derive(Deserialize)]
struct Snapshot {
    snippets: Vec<Snippet>,
}

#[derive(Deserialize)]
struct Snippet {
    title: String,
}

fn parse(json: &str) -> Option<Snapshot> {
    serde_json::from_str(json).ok()
}

// --- Path A: UniFFI proc-macro exports. ---

/// Number of snippets in a snapshot JSON document, or -1 when it fails to
/// parse (spike keeps the error channel primitive on purpose).
#[uniffi::export]
pub fn snapshot_count(json: String) -> i64 {
    match parse(&json) {
        Some(s) => s.snippets.len() as i64,
        None => -1,
    }
}

/// Title of the first snippet, exercising String/Option marshalling.
#[uniffi::export]
pub fn first_title(json: String) -> Option<String> {
    parse(&json)?.snippets.into_iter().next().map(|s| s.title)
}

// --- Path B: hand-written C ABI over the same functions. ---

/// # Safety
/// `json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn typvia_snapshot_count(json: *const c_char) -> i64 {
    if json.is_null() {
        return -1;
    }
    match CStr::from_ptr(json).to_str() {
        Ok(s) => snapshot_count(s.to_string()),
        Err(_) => -1,
    }
}

/// Returns a heap-allocated C string (or null); caller must release it via
/// `typvia_string_free`.
///
/// # Safety
/// `json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn typvia_first_title(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(s) = CStr::from_ptr(json).to_str() else {
        return std::ptr::null_mut();
    };
    match first_title(s.to_string()).and_then(|t| CString::new(t).ok()) {
        Some(c) => c.into_raw(),
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `s` must be a pointer previously returned by `typvia_first_title`.
#[no_mangle]
pub unsafe extern "C" fn typvia_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}
