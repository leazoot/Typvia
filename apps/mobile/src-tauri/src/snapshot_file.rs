// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Mobile snapshot-file plumbing: resolves where the KeyboardSnapshot
//! document lives on this host (App Group container on iOS, private
//! app-data subdirectory on Android) and re-exports the shared atomic
//! writer owned by host-service (single implementation across hosts).

use std::path::Path;
#[cfg(any(target_os = "android", test))]
use std::path::PathBuf;

use typvia_host_service::error::IpcError;

#[cfg(test)]
pub use typvia_host_service::snapshot::SNAPSHOT_FILE_NAME;
pub use typvia_host_service::snapshot::write_snapshot;

/// Per-install device identity marker; owned by host-service (one id shared
/// by the snapshot pipeline and sync). Named here only for the test that
/// asserts an existing marker is reused.
#[cfg(test)]
const DEVICE_ID_FILE_NAME: &str = "device-id";
/// Snapshot subdirectory of the app data dir on hosts without an App Group
/// container (Android). The IME service runs under the same application UID
/// as the main app, so it reads the file directly — no cross-process grant,
/// and the main database stays host-only. Gated with
/// its callers (the Android setup path and the host tests) so iOS builds
/// carry no dead code — same convention as `app_group::APP_GROUP_ID`.
#[cfg(any(target_os = "android", test))]
const DATA_DIR_SNAPSHOT_DIR_NAME: &str = "keyboard-snapshot";

/// Resolves (creating on demand) the app-data snapshot directory used where
/// no App Group container exists. Temp file and rename stay inside this one
/// directory, so the atomic-replace guarantee is unchanged.
#[cfg(any(target_os = "android", test))]
pub fn ensure_data_dir_snapshot_dir(data_dir: &Path) -> Result<PathBuf, IpcError> {
    let dir = data_dir.join(DATA_DIR_SNAPSHOT_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|_| IpcError::system())?;
    Ok(dir)
}

/// Loads the stable per-install device id, creating it on first run. The id
/// is a random UUID carrying no user data; it is stored next to the
/// database and must never be logged.
pub fn load_or_create_device_id(data_dir: &Path) -> Result<String, IpcError> {
    typvia_host_service::service::load_or_create_device_id(data_dir)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use typvia_core::model::KeyboardSnapshot;
    use typvia_host_service::dto::SnippetCreateInput;
    use typvia_host_service::service;

    fn seeded_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        service::snippet_create(
            &conn,
            SnippetCreateInput {
                title: "Tail logs".to_string(),
                body: "docker logs -f app".to_string(),
                snippet_type: "command".to_string(),
                description: None,
                folder_id: None,
                trigger: Some(";dlog".to_string()),
                trigger_mode: Some("delimiter".to_string()),
                language: None,
            },
            1_700_000_000_000,
        )
        .unwrap();
        conn
    }

    #[test]
    fn device_id_is_created_once_and_reused_afterwards() {
        let dir = tempfile::tempdir().unwrap();

        let first = load_or_create_device_id(dir.path()).unwrap();
        let second = load_or_create_device_id(dir.path()).unwrap();

        assert!(!first.is_empty());
        assert_eq!(first, second);
    }

    #[test]
    fn data_dir_snapshot_dir_is_created_once_and_reusable() {
        let data_dir = tempfile::tempdir().unwrap();

        let first = ensure_data_dir_snapshot_dir(data_dir.path()).unwrap();
        // A second resolution over the existing directory must not fail.
        let second = ensure_data_dir_snapshot_dir(data_dir.path()).unwrap();

        assert_eq!(first, data_dir.path().join(DATA_DIR_SNAPSHOT_DIR_NAME));
        assert_eq!(first, second);
        assert!(first.is_dir());
    }

    #[test]
    fn data_dir_snapshot_dir_blocked_by_a_file_is_a_system_error() {
        let data_dir = tempfile::tempdir().unwrap();
        std::fs::write(data_dir.path().join(DATA_DIR_SNAPSHOT_DIR_NAME), b"x").unwrap();

        let error = ensure_data_dir_snapshot_dir(data_dir.path()).unwrap_err();

        assert_eq!(error, IpcError::system());
    }

    #[test]
    fn snapshot_written_into_the_data_dir_subdirectory_round_trips() {
        // The Android layout: device id in the data dir, snapshot in its
        // keyboard-snapshot subdirectory — the two files must not collide.
        let conn = seeded_conn();
        let data_dir = tempfile::tempdir().unwrap();
        let dir = ensure_data_dir_snapshot_dir(data_dir.path()).unwrap();
        let device_id = load_or_create_device_id(data_dir.path()).unwrap();

        write_snapshot(&conn, &dir, &device_id, 1_700_000_000_500).unwrap();

        let bytes = std::fs::read(dir.join(SNAPSHOT_FILE_NAME)).unwrap();
        let parsed: KeyboardSnapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.validate(), Ok(()));
        assert_eq!(parsed.device_id, device_id);
        assert_eq!(parsed.snippets.len(), 1);
        assert!(!data_dir.path().join(SNAPSHOT_FILE_NAME).exists());
    }

    #[test]
    fn device_id_respects_an_existing_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DEVICE_ID_FILE_NAME), "existing-id\n").unwrap();

        assert_eq!(load_or_create_device_id(dir.path()).unwrap(), "existing-id");
    }
}
