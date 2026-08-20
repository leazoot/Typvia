//! Malicious-storage red lines for the WebDAV form: the store never sees
//! plaintext or key material at the byte level, a
//! tampered record is rejected without poisoning the round, a forged
//! directory is a hard failure, and an oversized file surfaces as an error
//! instead of a hang or panic. Rollback quietness is covered in
//! `webdav_e2e.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use common::WebdavServer;
use rusqlite::Connection;
use typvia_core::model::{
    Platform, SecurityLevel, Snippet, SnippetContent, SnippetType, SyncEntityType, TimestampMs,
};
use typvia_core::repo::{SnippetRepo, SyncStateRepo};
use typvia_core::sync_hooks::{self, EntityChange};
use typvia_crypto::{SecureStore, SecureStoreError};
use typvia_sync::{
    AdoptedAccount, CertificateSubject, DeviceCertificate, LocalDeviceConfig, NewAccount,
    OutboxSealer, RootStatement, SyncEngine, SyncKeys, WebdavClient, WebdavStore,
    ensure_local_device,
};

const NOW: TimestampMs = 1_700_000_400_000;

struct MemoryStore {
    entries: RefCell<HashMap<String, Vec<u8>>>,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
        }
    }
}

impl SecureStore for MemoryStore {
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
        self.entries
            .borrow_mut()
            .insert(entry.to_string(), secret.to_vec());
        Ok(())
    }
    fn retrieve(
        &self,
        entry: &str,
    ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, SecureStoreError> {
        Ok(self
            .entries
            .borrow()
            .get(entry)
            .cloned()
            .map(zeroize::Zeroizing::new))
    }
    fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
        self.entries.borrow_mut().remove(entry);
        Ok(())
    }
}

struct Instance {
    conn: Connection,
    store: MemoryStore,
    engine: SyncEngine,
    device_id: String,
}

impl Instance {
    fn new(dir: &Path, name: &str) -> Self {
        let db_path = dir.join(format!("{name}.db"));
        let mut conn = typvia_core::db::open(&db_path).expect("open client db");
        typvia_core::db::migrate_to_latest(&mut conn).expect("migrate client db");
        let store = MemoryStore::new();
        let device_id = format!("dav-{name}-device");
        let local = ensure_local_device(
            &conn,
            &store,
            &LocalDeviceConfig {
                device_id: &device_id,
                name,
                platform: Platform::Macos,
            },
            NOW,
        )
        .expect("ensure local device");
        let engine = SyncEngine::new_webdav(local.identity, device_id.clone());
        Self {
            conn,
            store,
            engine,
            device_id,
        }
    }

    fn identity(&self) -> typvia_sync::DeviceIdentity {
        ensure_local_device(
            &self.conn,
            &self.store,
            &LocalDeviceConfig {
                device_id: &self.device_id,
                name: "reload",
                platform: Platform::Macos,
            },
            NOW,
        )
        .expect("reload identity")
        .identity
    }

    fn attach_observer(&self) -> sync_hooks::ObserverGuard {
        let key_id = SyncStateRepo::new(&self.conn)
            .config_get()
            .expect("config")
            .sync_key_id;
        let keys = SyncKeys::load(&self.store, key_id).expect("keys");
        let sealer =
            OutboxSealer::new(self.identity(), self.device_id.clone(), &keys).expect("sealer");
        sync_hooks::register_observer(&self.conn, Arc::new(sealer))
    }

    fn write_snippet(&self, snippet: &Snippet, now: i64) {
        let tx = self.conn.unchecked_transaction().expect("tx");
        SnippetRepo::new(&tx).insert(snippet).expect("insert");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Snippet,
                entity_id: snippet.id.clone(),
                deleted_at: None,
                now,
            },
        )
        .expect("notify");
        tx.commit().expect("commit");
    }

    fn sync(&mut self, dav: &WebdavStore) -> typvia_sync::SyncReport {
        self.engine
            .sync_webdav(&self.conn, &self.store, dav, NOW + 50)
            .expect("round")
    }
}

fn snippet(id: &str, title: &str, body: &str, level: SecurityLevel) -> Snippet {
    Snippet {
        id: id.to_string(),
        workspace_id: "ws-default".to_string(),
        title: title.to_string(),
        content: match level {
            SecurityLevel::Normal => SnippetContent::Plaintext(body.to_string()),
            SecurityLevel::Sensitive => SnippetContent::Ciphertext(body.as_bytes().to_vec()),
        },
        snippet_type: match level {
            SecurityLevel::Normal => SnippetType::Text,
            SecurityLevel::Sensitive => SnippetType::Sensitive,
        },
        description: None,
        folder_id: None,
        trigger: None,
        trigger_mode: None,
        language: None,
        security_level: level,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: vec![],
        created_at: NOW,
        updated_at: NOW,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

fn tempdir() -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "davred-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn setup(dir: &Path, server: &WebdavServer) -> (Instance, Instance, WebdavStore) {
    let dav = WebdavStore::new(WebdavClient::new(&server.url, None).expect("client"));
    let mut a = Instance::new(dir, "alpha");
    let account_id = a
        .engine
        .create_webdav_account(
            &a.conn,
            &a.store,
            &dav,
            &NewAccount {
                device_name: "alpha",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: None,
            },
            NOW,
        )
        .expect("found");
    let mut b = Instance::new(dir, "beta");
    let key = a.store.retrieve("sync.k_sync.1").unwrap().unwrap();
    b.store.store("sync.k_sync.1", &key).unwrap();
    let b_identity = b.identity();
    let cert = DeviceCertificate::issue(
        &a.identity(),
        &a.device_id,
        CertificateSubject {
            device_id: b.device_id.clone(),
            ed25519_pub: b_identity.ed25519_public(),
            x25519_pub: b_identity.x25519_public(),
            name: "beta".to_string(),
            platform: Platform::Macos,
            created_at: NOW,
        },
        NOW,
    )
    .expect("cert");
    a.engine
        .webdav_admit_device(&dav, std::slice::from_ref(&cert))
        .expect("admit");
    let root_fingerprint = SyncStateRepo::new(&a.conn)
        .config_get()
        .unwrap()
        .root_fingerprint
        .unwrap();
    b.engine
        .adopt_webdav_account(
            &b.conn,
            &b.store,
            &AdoptedAccount {
                server_url: server.url.clone(),
                account_id,
                root_fingerprint: root_fingerprint.try_into().unwrap(),
                sync_key_id: 1,
            },
            NOW,
        )
        .expect("adopt");
    (a, b, dav)
}

#[test]
fn the_store_never_holds_plaintext_or_key_material_at_the_byte_level() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav) = setup(&dir, &server);

    let normal_marker = "TYPVIA_DAV_NORMAL_BODY_MARKER_9f31";
    let sensitive_title = "TYPVIA_DAV_SENSITIVE_TITLE_5c20";
    let sensitive_body = "TYPVIA_DAV_SENSITIVE_BODY_77aa";
    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("s-n", "Plain note", normal_marker, SecurityLevel::Normal),
            NOW + 1,
        );
        a.write_snippet(
            &snippet(
                "s-s",
                sensitive_title,
                sensitive_body,
                SecurityLevel::Sensitive,
            ),
            NOW + 2,
        );
    }
    a.sync(&dav);
    b.sync(&dav);
    assert!(SnippetRepo::new(&b.conn).get("s-n").unwrap().is_some());

    // Every byte the store holds, concatenated.
    let mut all = Vec::new();
    for (_, bytes) in server.state.snapshot() {
        all.extend_from_slice(&bytes);
    }
    let hits = |needle: &[u8]| all.windows(needle.len()).any(|w| w == needle);

    // Self-check first: the scanner does find plaintext that is legally
    // there (device ids are public metadata in record files).
    assert!(hits(b"dav-alpha-device"), "scanner self-check failed");

    // Red lines: no body plaintext, no sensitive title/body, no K_sync
    // bytes, no device seed.
    assert!(!hits(normal_marker.as_bytes()), "normal body leaked");
    assert!(!hits(sensitive_title.as_bytes()), "sensitive title leaked");
    assert!(!hits(sensitive_body.as_bytes()), "sensitive body leaked");
    let k_sync = a.store.retrieve("sync.k_sync.1").unwrap().unwrap();
    assert!(!hits(&k_sync), "K_sync bytes leaked into the store");
    let seed = a.store.retrieve("device.ed25519.seed").unwrap().unwrap();
    assert!(!hits(&seed), "device seed leaked into the store");
}

#[test]
fn a_tampered_record_is_skipped_without_poisoning_the_round() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav) = setup(&dir, &server);

    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("s-t", "Tamper me", "victim", SecurityLevel::Normal),
            NOW + 1,
        );
        a.write_snippet(
            &snippet("s-ok", "Untouched", "fine", SecurityLevel::Normal),
            NOW + 2,
        );
    }
    a.sync(&dav);

    // Corrupt the first record file's bytes on the store.
    let path = "typvia-sync/devices/dav-alpha-device/records/0000000001.json";
    server.state.tamper(path, |bytes| {
        if let Some(byte) = bytes.iter_mut().rfind(|b| **b == b'A') {
            *byte = b'B';
        } else if let Some(byte) = bytes.last_mut() {
            *byte = byte.wrapping_add(1);
        }
    });

    let report = b.sync(&dav);
    // The tampered record is refused; the intact one still lands.
    assert!(report.skipped >= 1, "tampered record must be skipped");
    assert!(SnippetRepo::new(&b.conn).get("s-ok").unwrap().is_some());
    assert!(SnippetRepo::new(&b.conn).get("s-t").unwrap().is_none());
}

#[test]
fn a_forged_directory_is_a_hard_failure_and_changes_nothing() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav) = setup(&dir, &server);

    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("s-1", "Before forgery", "x", SecurityLevel::Normal),
            NOW + 1,
        );
    }
    a.sync(&dav);
    b.sync(&dav);

    // A forged directory rooted at an attacker identity replaces the file.
    let attacker = Instance::new(&dir, "mallory");
    let attacker_identity = attacker.identity();
    let forged_root = RootStatement::create(
        &attacker_identity,
        CertificateSubject {
            device_id: attacker.device_id.clone(),
            ed25519_pub: attacker_identity.ed25519_public(),
            x25519_pub: attacker_identity.x25519_public(),
            name: "mallory".to_string(),
            platform: Platform::Macos,
            created_at: NOW,
        },
    )
    .expect("forged root");
    let forged = typvia_sync::DeviceDirectory {
        root_statement_json: typvia_sync::root_statement_to_json(&forged_root).expect("json"),
        devices: Vec::new(),
        revocations: Vec::new(),
    };
    dav.write_directory_overwrite(&forged).expect("overwrite");

    let before: usize = SnippetRepo::new(&b.conn).list(100, 0).unwrap().len();
    let err = b
        .engine
        .sync_webdav(&b.conn, &b.store, &dav, NOW + 60)
        .expect_err("forged directory must hard-fail");
    let rendered = format!("{err}");
    assert!(
        !rendered.contains("Before forgery"),
        "errors never carry content"
    );
    assert_eq!(
        SnippetRepo::new(&b.conn).list(100, 0).unwrap().len(),
        before,
        "a forged directory changes nothing"
    );
}

#[test]
fn an_oversized_file_surfaces_as_an_error_not_a_hang() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (_a, mut b, dav) = setup(&dir, &server);

    // A 9 MB blob where the directory should be.
    let huge = vec![b'x'; 9 * 1024 * 1024];
    server.state.restore({
        let mut files = server.state.snapshot();
        files.insert("typvia-sync/directory.json".to_string(), huge);
        files
    });
    assert!(
        b.engine
            .sync_webdav(&b.conn, &b.store, &dav, NOW + 70)
            .is_err(),
        "oversized files are refused"
    );
}
