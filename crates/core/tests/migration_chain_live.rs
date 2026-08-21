// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Full-chain migration tests on real database files.
//!
//! The in-module tests in `db::migrate` cover per-migration boundaries on
//! in-memory connections; these walk the whole 0..=latest chain stepwise on
//! disk, where WAL journals, file locks and reopened connections are real.

#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;
use typvia_core::db::{latest_version, migrate_to, migrate_to_latest, open, schema_version};

static NEXT_DB: AtomicU32 = AtomicU32::new(0);

/// On-disk database under the OS temp dir; the file trio is removed on drop.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        let unique = format!(
            "typvia-mig-{tag}-{}-{}.db",
            std::process::id(),
            NEXT_DB.fetch_add(1, Ordering::Relaxed)
        );
        Self {
            path: std::env::temp_dir().join(unique),
        }
    }

    fn open(&self) -> Connection {
        open(&self.path).expect("open on-disk database")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut name = self.path.as_os_str().to_os_string();
            name.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(name));
        }
    }
}

fn integrity_ok(conn: &Connection) -> bool {
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity_check");
    verdict == "ok"
}

fn insert_snippet(conn: &Connection, id: &str, title: &str) {
    conn.execute(
        "INSERT INTO snippet (id, workspace_id, title, content_plaintext, type, \
         security_level, platform_scope, created_at, updated_at, version) \
         VALUES (?1, 'w', ?2, 'body', 'text', 'normal', '[]', 1, 1, 1)",
        rusqlite::params![id, title],
    )
    .expect("insert snippet");
}

#[test]
fn stepwise_upgrade_walks_every_version_on_disk() {
    let db = TempDb::new("up");
    let mut conn = db.open();
    for version in 1..=latest_version() {
        migrate_to(&mut conn, version).expect("upgrade one step");
        assert_eq!(schema_version(&conn).expect("version"), version);
    }
    assert!(integrity_ok(&conn));
}

#[test]
fn stepwise_rollback_walks_every_version_down_to_zero() {
    let db = TempDb::new("down");
    let mut conn = db.open();
    migrate_to_latest(&mut conn).expect("upgrade");
    for version in (0..latest_version()).rev() {
        migrate_to(&mut conn, version).expect("downgrade one step");
        assert_eq!(schema_version(&conn).expect("version"), version);
    }
    assert!(integrity_ok(&conn));
}

#[test]
fn reupgrade_from_every_intermediate_version_is_idempotent() {
    for stop in 0..=latest_version() {
        let db = TempDb::new("idem");
        let mut conn = db.open();
        migrate_to_latest(&mut conn).expect("upgrade");
        migrate_to(&mut conn, stop).expect("downgrade to stop");
        migrate_to_latest(&mut conn).expect("re-upgrade");
        migrate_to_latest(&mut conn).expect("second run is a no-op");
        assert_eq!(schema_version(&conn).expect("version"), latest_version());
        assert!(integrity_ok(&conn), "corrupt after round trip via {stop}");
    }
}

#[test]
fn snippet_rows_survive_the_full_downgrade_upgrade_round_trip() {
    let db = TempDb::new("data");
    let mut conn = db.open();
    migrate_to_latest(&mut conn).expect("upgrade");
    insert_snippet(&conn, "s1", "Release checklist");

    for version in (1..latest_version()).rev() {
        migrate_to(&mut conn, version).expect("downgrade one step");
        let survived: i64 = conn
            .query_row("SELECT COUNT(*) FROM snippet", [], |row| row.get(0))
            .expect("count");
        assert_eq!(survived, 1, "snippet lost downgrading to {version}");
    }
    migrate_to_latest(&mut conn).expect("re-upgrade");
    let title: String = conn
        .query_row("SELECT title FROM snippet WHERE id = 's1'", [], |row| {
            row.get(0)
        })
        .expect("row back");
    assert_eq!(title, "Release checklist");
    assert!(integrity_ok(&conn));
}

#[test]
fn a_reopened_file_resumes_migration_from_its_stored_version() {
    let db = TempDb::new("reopen");
    {
        let mut conn = db.open();
        migrate_to(&mut conn, 5).expect("stop half way");
    }
    // A fresh process opening the same file continues from version 5.
    let mut conn = db.open();
    assert_eq!(schema_version(&conn).expect("version"), 5);
    migrate_to_latest(&mut conn).expect("resume");
    assert_eq!(schema_version(&conn).expect("version"), latest_version());
    assert!(integrity_ok(&conn));
}
