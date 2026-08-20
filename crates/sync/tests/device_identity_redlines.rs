//! Device private-key red line (regression-mandatory).
//!
//! Proves at the byte level that device private keys never reach the
//! SQLite database: after local-device registration, neither the Ed25519
//! seed nor the X25519 secret occurs anywhere in the database file, while
//! the Ed25519 public key does (canary — the scan would catch a leak).
//! Also covers restart recovery: a second registration run restores the
//! same identity instead of minting a new device.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open};
use typvia_core::model::Platform;
use typvia_core::repo::new_id;
use typvia_crypto::{SecureStore, SecureStoreError};
use typvia_sync::{
    DEVICE_ED25519_ENTRY, DEVICE_X25519_ENTRY, EnsureLocalDeviceError, LocalDeviceConfig,
    ensure_local_device,
};
use zeroize::Zeroizing;

/// In-memory secure-store double; exposes its entries so tests can scan
/// for the exact private-key bytes it holds.
struct MemoryStore {
    entries: RefCell<HashMap<String, Vec<u8>>>,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
        }
    }

    fn entry(&self, name: &str) -> Vec<u8> {
        self.entries.borrow().get(name).cloned().unwrap()
    }
}

impl SecureStore for MemoryStore {
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
        self.entries
            .borrow_mut()
            .insert(entry.to_string(), secret.to_vec());
        Ok(())
    }

    fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
        Ok(self
            .entries
            .borrow()
            .get(entry)
            .cloned()
            .map(Zeroizing::new))
    }

    fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
        self.entries.borrow_mut().remove(entry);
        Ok(())
    }
}

/// On-disk database that cleans up after itself; file-backed so storage
/// can be scanned as raw bytes (same pattern as the FTS red line).
struct DiskDb {
    path: PathBuf,
    conn: Connection,
}

impl DiskDb {
    fn create() -> Self {
        let path = std::env::temp_dir().join(format!("typvia-sync-redline-{}.db", new_id()));
        let mut conn = open(&path).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        Self { path, conn }
    }

    /// All bytes of the main database file, WAL flushed in first.
    fn file_bytes(&self) -> Vec<u8> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        fs::read(&self.path).unwrap()
    }
}

impl Drop for DiskDb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("db-wal"));
        let _ = fs::remove_file(self.path.with_extension("db-shm"));
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn config() -> LocalDeviceConfig<'static> {
    LocalDeviceConfig {
        device_id: "11111111-2222-3333-4444-555555555555",
        name: "Redline test device",
        platform: Platform::Macos,
    }
}

#[test]
fn private_key_bytes_never_reach_the_database_file() {
    let db = DiskDb::create();
    let store = MemoryStore::new();

    let local = ensure_local_device(&db.conn, &store, &config(), 1_700_000_000_000).unwrap();

    let file = db.file_bytes();
    // Canary first: the public key IS in the file, so the same scan would
    // catch a private-key leak.
    let public = local.identity.ed25519_public();
    assert!(
        contains_bytes(&file, &public),
        "canary failed: public key not found in the database file"
    );

    // Red line: neither 32-byte secret occurs anywhere in the file.
    for entry in [DEVICE_ED25519_ENTRY, DEVICE_X25519_ENTRY] {
        let secret = store.entry(entry);
        assert_eq!(secret.len(), 32);
        assert!(
            !contains_bytes(&file, &secret),
            "private key bytes ({entry}) leaked into the database file"
        );
    }
}

#[test]
fn a_second_run_restores_the_same_identity_and_device_row() {
    let db = DiskDb::create();
    let store = MemoryStore::new();

    let first = ensure_local_device(&db.conn, &store, &config(), 1_000).unwrap();
    // Simulates a restart: same store and database, fresh call.
    let second = ensure_local_device(&db.conn, &store, &config(), 2_000).unwrap();

    assert_eq!(
        second.identity.ed25519_public(),
        first.identity.ed25519_public()
    );
    assert_eq!(second.device.id, first.device.id);
    assert_eq!(second.device.created_at, 1_000);
    assert_eq!(second.device.last_seen_at, Some(2_000));

    // Still exactly one device row.
    let count: i64 = db
        .conn
        .query_row("SELECT count(*) FROM device", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn a_device_row_from_a_different_identity_is_a_mismatch_not_a_merge() {
    let db = DiskDb::create();

    // First install registered with one identity...
    let original_store = MemoryStore::new();
    ensure_local_device(&db.conn, &original_store, &config(), 1_000).unwrap();

    // ...then the secure store is wiped (fresh keychain) while the row
    // survives. Registration must refuse to adopt the old row.
    let wiped_store = MemoryStore::new();
    let error = ensure_local_device(&db.conn, &wiped_store, &config(), 2_000).unwrap_err();
    assert!(matches!(error, EnsureLocalDeviceError::IdentityMismatch));
}
