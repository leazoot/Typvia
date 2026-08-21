// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Typvia browser-extension native-messaging host: launched by the browser
//! per the registered host manifest, speaks length-prefixed JSON frames over
//! stdio, and serves search/list/render from the read-only browser snapshot
//! the desktop app maintains. It never opens the main database and never
//! writes anywhere; it also never logs — every failure is an in-band stable
//! error code.

mod protocol;
mod service;
mod store;

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use protocol::{ErrorResponse, Frame, Request, read_frame, write_frame};
use store::SnapshotStore;

/// Test/advanced override for where the snapshot directory lives.
const SNAPSHOT_DIR_ENV: &str = "TYPVIA_BROWSER_SNAPSHOT_DIR";
/// Desktop app identifier — the platform data dir segment Tauri uses
/// (apps/desktop/src-tauri/tauri.conf.json); the host mirrors that
/// resolution because browsers pass no arguments through the manifest.
const APP_IDENTIFIER: &str = "dev.typvia.desktop";
/// Snapshot subdirectory written by the desktop app (browser_integration.rs).
const SNAPSHOT_SUBDIR: &str = "browser-snapshot";
const SNAPSHOT_FILE: &str = "snapshot.json";

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut store = SnapshotStore::new(snapshot_path());
    // IO errors on either pipe mean the browser is gone: exit quietly.
    let _ = serve(&mut reader, &mut writer, &mut store);
}

/// Frame loop: one response per inbound frame, in order, until EOF.
fn serve<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    store: &mut SnapshotStore,
) -> std::io::Result<()> {
    loop {
        let payload = match read_frame(reader)? {
            Frame::Eof => return Ok(()),
            Frame::Oversized => {
                respond(writer, &ErrorResponse::code("oversized_frame"))?;
                continue;
            }
            Frame::Payload(payload) => payload,
        };
        match serde_json::from_slice::<Request>(&payload) {
            Ok(request) => {
                let value = service::handle(&request, store);
                let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| fallback_error());
                write_frame(writer, &bytes)?;
            }
            Err(_) => respond(writer, &ErrorResponse::code("unknown_message"))?,
        }
    }
}

fn respond<W: Write>(writer: &mut W, error: &ErrorResponse) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(error).unwrap_or_else(|_| fallback_error());
    write_frame(writer, &bytes)
}

fn fallback_error() -> Vec<u8> {
    br#"{"ok":false,"code":"internal"}"#.to_vec()
}

/// Resolves the snapshot file location: env override first (tests), then
/// the per-OS user data directory the desktop app writes into.
fn snapshot_path() -> PathBuf {
    if let Ok(dir) = std::env::var(SNAPSHOT_DIR_ENV) {
        return PathBuf::from(dir).join(SNAPSHOT_FILE);
    }
    platform_data_dir()
        .join(APP_IDENTIFIER)
        .join(SNAPSHOT_SUBDIR)
        .join(SNAPSHOT_FILE)
}

#[cfg(target_os = "macos")]
fn platform_data_dir() -> PathBuf {
    home().join("Library/Application Support")
}

#[cfg(target_os = "windows")]
fn platform_data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map_or_else(|| home().join("AppData/Roaming"), PathBuf::from)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map_or_else(|| home().join(".local/share"), PathBuf::from)
}

fn home() -> PathBuf {
    #[cfg(target_os = "windows")]
    let var = "USERPROFILE";
    #[cfg(not(target_os = "windows"))]
    let var = "HOME";
    // An empty path only degrades to "snapshot unavailable" downstream —
    // this host has no user to prompt and must not log.
    std::env::var_os(var).map_or_else(PathBuf::new, PathBuf::from)
}
