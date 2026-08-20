//! Write-path interruption tests: a child process is killed
//! without warning mid-write and the parent asserts the reopened database
//! is consistent — uncommitted work vanishes completely, committed work
//! survives WAL recovery, and the file stays writable.
//!
//! The `child_*` tests are inert under a normal `cargo test` run; the
//! parent re-invokes this same test binary with `TYPVIA_CRASH_ROLE` set to
//! turn exactly one of them into the sacrificial writer.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{Connection, TransactionBehavior};
use typvia_core::db::{migrate_to_latest, open, schema_version};

const ROLE_ENV: &str = "TYPVIA_CRASH_ROLE";
const DB_ENV: &str = "TYPVIA_CRASH_DB";
const READY_ENV: &str = "TYPVIA_CRASH_READY";

static NEXT_DB: AtomicU32 = AtomicU32::new(0);

struct TempDb {
    path: PathBuf,
    ready: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        let unique = format!(
            "typvia-crash-{tag}-{}-{}",
            std::process::id(),
            NEXT_DB.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir();
        Self {
            path: dir.join(format!("{unique}.db")),
            ready: dir.join(format!("{unique}.ready")),
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
        let _ = std::fs::remove_file(&self.ready);
    }
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

fn snippet_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM snippet", [], |row| row.get(0))
        .expect("count")
}

fn integrity_ok(conn: &Connection) -> bool {
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity_check");
    verdict == "ok"
}

/// Spawns this test binary again, filtered to one `child_*` test, with the
/// role env var that arms it.
fn spawn_child(role: &str, db: &TempDb) -> Child {
    Command::new(std::env::current_exe().expect("test binary path"))
        .args([role, "--exact", "--nocapture"])
        .env(ROLE_ENV, role)
        .env(DB_ENV, &db.path)
        .env(READY_ENV, &db.ready)
        .spawn()
        .expect("spawn child writer")
}

/// Waits until the child signals it is mid-write, then kills it cold.
fn wait_then_kill(mut child: Child, ready: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "child writer never became ready");
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!("child writer exited early: {status}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    child.kill().expect("kill child");
    let _ = child.wait();
}

/// The child signals readiness and then parks until the parent kills it.
fn signal_ready_and_park() {
    let ready = std::env::var(READY_ENV).expect("ready path");
    std::fs::write(&ready, b"ready").expect("write ready marker");
    std::thread::sleep(Duration::from_secs(30));
}

fn armed(role: &str) -> Option<Connection> {
    if std::env::var(ROLE_ENV).as_deref() != Ok(role) {
        return None;
    }
    let db = std::env::var(DB_ENV).expect("db path");
    Some(open(Path::new(&db)).expect("child opens database"))
}

// ---------------------------------------------------------------- children

/// Inert unless spawned: holds an open write transaction with 50 inserted
/// rows and never commits.
#[test]
fn child_holds_an_uncommitted_write_transaction() {
    let Some(mut conn) = armed("child_holds_an_uncommitted_write_transaction") else {
        return;
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("begin immediate");
    for i in 0..50 {
        tx.execute(
            "INSERT INTO snippet (id, workspace_id, title, content_plaintext, type, \
             security_level, platform_scope, created_at, updated_at, version) \
             VALUES (?1, 'w', 'doomed', 'body', 'text', 'normal', '[]', 1, 1, 1)",
            rusqlite::params![format!("doomed-{i}")],
        )
        .expect("insert inside tx");
    }
    signal_ready_and_park();
    // Unreachable: the parent kills this process while the tx is open.
    drop(tx);
}

/// Inert unless spawned: commits one row (into the WAL) and parks without
/// closing the connection, so no checkpoint runs before the kill.
#[test]
fn child_commits_into_the_wal_then_parks() {
    let Some(conn) = armed("child_commits_into_the_wal_then_parks") else {
        return;
    };
    insert_snippet(&conn, "committed", "Survives the crash");
    signal_ready_and_park();
}

/// Inert unless spawned: holds an uncommitted schema change (DDL) open.
#[test]
fn child_holds_an_uncommitted_schema_change() {
    let Some(mut conn) = armed("child_holds_an_uncommitted_schema_change") else {
        return;
    };
    let tx = conn.transaction().expect("begin");
    tx.execute_batch(
        "CREATE TABLE half_state (id TEXT PRIMARY KEY); \
         INSERT INTO half_state (id) VALUES ('x');",
    )
    .expect("ddl inside tx");
    signal_ready_and_park();
    drop(tx);
}

// ----------------------------------------------------------------- parents

#[test]
fn killing_a_writer_mid_transaction_loses_nothing_committed() {
    let db = TempDb::new("tx");
    {
        let mut conn = db.open();
        migrate_to_latest(&mut conn).expect("prepare schema");
        insert_snippet(&conn, "baseline", "Already saved");
    }

    let child = spawn_child("child_holds_an_uncommitted_write_transaction", &db);
    wait_then_kill(child, &db.ready);

    let conn = db.open();
    assert!(integrity_ok(&conn));
    // None of the 50 uncommitted rows exist; the committed baseline does.
    assert_eq!(snippet_count(&conn), 1);
    // The dead process left no lock behind: a fresh write goes through.
    insert_snippet(&conn, "after", "Still writable");
    assert_eq!(snippet_count(&conn), 2);
}

#[test]
fn a_commit_survives_killing_the_process_before_any_checkpoint() {
    let db = TempDb::new("wal");
    {
        let mut conn = db.open();
        migrate_to_latest(&mut conn).expect("prepare schema");
    }

    let child = spawn_child("child_commits_into_the_wal_then_parks", &db);
    wait_then_kill(child, &db.ready);

    let conn = db.open();
    assert!(integrity_ok(&conn));
    let title: String = conn
        .query_row(
            "SELECT title FROM snippet WHERE id = 'committed'",
            [],
            |row| row.get(0),
        )
        .expect("WAL recovery kept the committed row");
    assert_eq!(title, "Survives the crash");
}

#[test]
fn killing_an_interrupted_schema_change_leaves_the_file_migratable() {
    let db = TempDb::new("ddl");
    {
        let mut conn = db.open();
        migrate_to_latest(&mut conn).expect("prepare schema");
    }

    let child = spawn_child("child_holds_an_uncommitted_schema_change", &db);
    wait_then_kill(child, &db.ready);

    let mut conn = db.open();
    assert!(integrity_ok(&conn));
    // The half-applied DDL rolled back completely…
    let leaked: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'half_state'",
            [],
            |row| row.get(0),
        )
        .expect("query master");
    assert_eq!(leaked, 0, "partial schema change leaked");
    // …and the stored version is still trusted by the normal startup path.
    let before = schema_version(&conn).expect("version");
    migrate_to_latest(&mut conn).expect("startup migration still runs");
    assert_eq!(schema_version(&conn).expect("version"), before);
}
