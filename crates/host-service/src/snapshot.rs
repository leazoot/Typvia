// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Atomic writer for the KeyboardSnapshot document that extension surfaces
//! read: the iOS keyboard (App Group container), the Android IME (private
//! app-data subdirectory), and the desktop browser-integration host
//! (browser-snapshot directory). The write is temp file + rename inside
//! the same directory (one volume, atomic), so a failure at any
//! point leaves the previous snapshot file untouched. Extension readers
//! only ever open the final file name.

use std::path::Path;

use rusqlite::Connection;

use crate::error::IpcError;

/// File name every snapshot consumer reads (read-only consumers).
pub const SNAPSHOT_FILE_NAME: &str = "snapshot.json";
/// Same-directory temp name so the final rename never crosses volumes.
const SNAPSHOT_TMP_NAME: &str = "snapshot.json.tmp";

/// Generates the snapshot from current database state and atomically
/// replaces `snapshot.json` in `dir`. Any failure (generation,
/// serialization, IO) leaves the existing snapshot file untouched.
pub fn write_snapshot(
    conn: &Connection,
    dir: &Path,
    device_id: &str,
    now: i64,
) -> Result<(), IpcError> {
    let snapshot = typvia_core::snapshot::generate(conn, device_id, now)?;
    // A serialization failure cannot be described without echoing content;
    // collapse to the generic system error (log red line).
    let json = serde_json::to_vec(&snapshot).map_err(|_| IpcError::system())?;
    let tmp = dir.join(SNAPSHOT_TMP_NAME);
    std::fs::write(&tmp, json).map_err(|_| IpcError::system())?;
    std::fs::rename(&tmp, dir.join(SNAPSHOT_FILE_NAME)).map_err(|_| {
        // Best-effort cleanup of the stranded temp file; the previous
        // snapshot is still in place either way.
        let _ = std::fs::remove_file(&tmp);
        IpcError::system()
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dto::SnippetCreateInput;
    use crate::service;
    use typvia_core::model::KeyboardSnapshot;

    fn create_input(title: &str, body: &str, trigger: &str) -> SnippetCreateInput {
        SnippetCreateInput {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: Some(trigger.to_string()),
            trigger_mode: Some("delimiter".to_string()),
            language: None,
        }
    }

    fn seeded_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        service::snippet_create(
            &conn,
            create_input("Tail logs", "docker logs -f app", ";dlog"),
            1_700_000_000_000,
        )
        .unwrap();
        conn
    }

    fn read_snapshot(dir: &Path) -> KeyboardSnapshot {
        let bytes = std::fs::read(dir.join(SNAPSHOT_FILE_NAME)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn writes_a_snapshot_that_parses_back_and_validates() {
        let conn = seeded_conn();
        let dir = tempfile::tempdir().unwrap();

        write_snapshot(&conn, dir.path(), "device-1", 1_700_000_000_500).unwrap();

        let parsed = read_snapshot(dir.path());
        assert_eq!(parsed.validate(), Ok(()));
        assert_eq!(parsed.device_id, "device-1");
        assert_eq!(parsed.generated_at, 1_700_000_000_500);
        assert_eq!(parsed.snippets.len(), 1);
        // A successful write leaves no temp file behind.
        assert!(!dir.path().join(SNAPSHOT_TMP_NAME).exists());
    }

    #[test]
    fn a_second_write_after_a_data_change_replaces_the_snapshot() {
        let conn = seeded_conn();
        let dir = tempfile::tempdir().unwrap();
        write_snapshot(&conn, dir.path(), "device-1", 1_700_000_000_500).unwrap();

        service::snippet_create(
            &conn,
            create_input("Pod logs", "kubectl logs app", ";klog"),
            1_700_000_001_000,
        )
        .unwrap();
        write_snapshot(&conn, dir.path(), "device-1", 1_700_000_001_500).unwrap();

        let parsed = read_snapshot(dir.path());
        assert_eq!(parsed.generated_at, 1_700_000_001_500);
        assert_eq!(parsed.snippets.len(), 2);
    }

    #[test]
    fn a_failed_write_keeps_the_old_snapshot_bytes_intact() {
        let conn = seeded_conn();
        let dir = tempfile::tempdir().unwrap();
        let old = br#"{"snapshot_version":1,"device_id":"old"}"#;
        std::fs::write(dir.path().join(SNAPSHOT_FILE_NAME), old).unwrap();
        // Occupy the temp name with a directory so the temp write fails
        // before anything can touch the live file.
        std::fs::create_dir(dir.path().join(SNAPSHOT_TMP_NAME)).unwrap();

        let error = write_snapshot(&conn, dir.path(), "device-1", 2).unwrap_err();

        assert_eq!(error, IpcError::system());
        assert_eq!(
            std::fs::read(dir.path().join(SNAPSHOT_FILE_NAME)).unwrap(),
            old
        );
    }
}
