// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Two-instance end-to-end sync against the real Go server, plus the full
//! pairing, recovery, revocation, and rotation flows.
//!
//! Each test builds the server binary with `go build` (a build failure
//! fails the test — never a silent skip), starts it on a random loopback
//! port with a temporary database, and runs full client instances:
//! separate SQLite files, secure-store doubles, and device identities.
//! Devices join through the real pairing flow — begin → code → SAS on both
//! ends → offer (server-side registration) → claim → finalize — the same
//! path the product takes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use typvia_core::model::{
    DomainKey, KeyDomain, Platform, SecurityLevel, Snippet, SnippetContent, SnippetType,
    SyncEntityType, VaultKeyHeader,
};
use typvia_core::repo::{
    FolderRepo, SnippetRepo, SyncOutboxRepo, SyncStateRepo, TagRepo, VaultKeyRepo, VersionRepo,
    new_id,
};
use typvia_core::sync_hooks::{self, EntityChange};
use typvia_core::vault::VaultSession;
use typvia_crypto::{
    KdfParams, SecureStore, SecureStoreError, SymmetricKey, aad_domain_key, aad_master_key,
    derive_kek, wrap_key,
};
use typvia_sync::{
    AdoptedAccount, CertificateSubject, DeviceCertificate, DeviceIdentity, HttpTransport,
    LocalDeviceConfig, OutboxSealer, PairingCode, RecoveryCode, SyncEngine, SyncError, SyncKeys,
    SyncTransport, TransportError, ensure_local_device, install_key,
};

// ----- secure-store double ------------------------------------------------

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

// ----- server harness -----------------------------------------------------

fn server_binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/sync-server");
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("typvia-sync-server-e2e");
        let status = Command::new("go")
            .args(["build", "-o"])
            .arg(&out)
            .arg("./cmd/typvia-sync-server")
            .current_dir(&source_dir)
            .status()
            .expect("go must be on PATH to build the sync server");
        assert!(status.success(), "go build of the sync server failed");
        out
    })
}

struct TestServer {
    child: Option<Child>,
    url: String,
    listen: String,
    db_path: PathBuf,
}

impl TestServer {
    fn start(dir: &Path) -> Self {
        let db_path = dir.join("server.db");
        let mut server = Self {
            child: None,
            url: String::new(),
            listen: String::new(),
            db_path,
        };
        server.launch();
        server
    }

    /// Starts (or restarts) the server. The first launch picks a fresh
    /// ephemeral port and retries on the pick-then-bind race that parallel
    /// tests can hit; a restart reuses the established port so clients
    /// keep their configured URL.
    fn launch(&mut self) {
        let attempts = if self.listen.is_empty() { 5 } else { 1 };
        for attempt in 0.. {
            if self.listen.is_empty() {
                let listener = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
                let listen = listener.local_addr().expect("local addr").to_string();
                drop(listener);
                self.url = format!("http://{listen}");
                self.listen = listen;
            }
            let child = Command::new(server_binary())
                .args(["-listen", &self.listen, "-db"])
                .arg(&self.db_path)
                .spawn()
                .expect("spawn sync server");
            self.child = Some(child);
            if self.wait_healthy() {
                return;
            }
            self.stop();
            assert!(attempt + 1 < attempts, "sync server never became healthy");
            // The chosen port was taken meanwhile; pick another.
            self.listen.clear();
        }
    }

    fn wait_healthy(&mut self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(30);
        let url = format!("{}/healthz", self.url);
        loop {
            if let Ok(mut response) = ureq::get(&url).call()
                && response.status().as_u16() == 200
                && response.body_mut().read_to_string().is_ok()
            {
                return true;
            }
            // A dead child (port already taken) will never become healthy.
            if let Some(child) = &mut self.child
                && matches!(child.try_wait(), Ok(Some(_)))
            {
                return false;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Simulates going offline: the server process stops, the database
    /// stays for a later restart.
    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn restart(&mut self) {
        self.stop();
        self.launch();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ----- client instance ----------------------------------------------------

struct Instance {
    conn: Connection,
    store: MemoryStore,
    engine: SyncEngine,
    device_id: String,
    db_path: PathBuf,
}

impl Instance {
    fn new(dir: &Path, name: &str, server_url: &str) -> Self {
        let db_path = dir.join(format!("{name}.db"));
        let mut conn = typvia_core::db::open(&db_path).expect("open client db");
        typvia_core::db::migrate_to_latest(&mut conn).expect("migrate client db");
        let store = MemoryStore::new();
        let device_id = format!("e2e-{name}-device");
        let local = ensure_local_device(
            &conn,
            &store,
            &LocalDeviceConfig {
                device_id: &device_id,
                name,
                platform: Platform::Macos,
            },
            1_700_000_000_000,
        )
        .expect("ensure local device");
        let transport = HttpTransport::new(server_url).expect("transport");
        let engine = SyncEngine::new(Box::new(transport), local.identity, device_id.clone());
        Self {
            conn,
            store,
            engine,
            device_id,
            db_path,
        }
    }

    fn identity(&self) -> DeviceIdentity {
        DeviceIdentity::load(&self.store)
            .expect("load identity")
            .expect("identity exists")
    }

    /// Registers the seal-at-write observer, mirroring the host wiring.
    fn attach_observer(&self) -> sync_hooks::ObserverGuard {
        let config = SyncStateRepo::new(&self.conn).config_get().expect("config");
        let keys = SyncKeys::load(&self.store, config.sync_key_id).expect("keys");
        let sealer =
            OutboxSealer::new(self.identity(), self.device_id.clone(), &keys).expect("sealer");
        sync_hooks::register_observer(&self.conn, std::sync::Arc::new(sealer))
    }

    /// A local entity write through the notification seam, the way host
    /// write use cases run it: repo write + notify in one transaction.
    fn write_snippet(&self, snippet: &Snippet, now: i64) {
        let tx = self.conn.unchecked_transaction().expect("tx");
        let repo = SnippetRepo::new(&tx);
        if repo.get(&snippet.id).expect("get").is_some() {
            repo.update(snippet).expect("update");
        } else {
            repo.insert(snippet).expect("insert");
        }
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

    fn trash_snippet(&self, id: &str, now: i64) {
        let tx = self.conn.unchecked_transaction().expect("tx");
        SnippetRepo::new(&tx).soft_delete(id, now).expect("trash");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Snippet,
                entity_id: id.to_string(),
                deleted_at: Some(now),
                now,
            },
        )
        .expect("notify");
        tx.commit().expect("commit");
    }

    /// Restores a snippet from the recycle bin the way the host use case
    /// does: an explicit user action producing a new content version.
    fn restore_snippet(&self, id: &str, now: i64) {
        let tx = self.conn.unchecked_transaction().expect("tx");
        SnippetRepo::new(&tx)
            .restore_from_trash(id)
            .expect("restore");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Snippet,
                entity_id: id.to_string(),
                deleted_at: None,
                now,
            },
        )
        .expect("notify");
        tx.commit().expect("commit");
    }

    fn snippet(&self, id: &str) -> Option<Snippet> {
        SnippetRepo::new(&self.conn).get(id).expect("get")
    }

    /// Flips the persisted switch back on, mirroring `sync_resume` before
    /// it re-registers the observer.
    fn enable_sync(&self, now: i64) {
        let repo = SyncStateRepo::new(&self.conn);
        let mut config = repo.config_get().expect("config");
        config.enabled = true;
        config.updated_at = now;
        repo.config_put(&config).expect("config put");
    }

    /// The catch-up the host runs after re-registering the observer;
    /// returns how many records it queued.
    fn reconcile(&self, now: i64) -> usize {
        let config = SyncStateRepo::new(&self.conn).config_get().expect("config");
        let keys = SyncKeys::load(&self.store, config.sync_key_id).expect("keys");
        self.engine
            .reconcile(&self.conn, &keys, now)
            .expect("reconcile")
    }

    /// The consumed key-update cursor.
    fn key_update_cursor(&self) -> i64 {
        SyncStateRepo::new(&self.conn)
            .config_get()
            .expect("config")
            .key_update_seq
    }

    fn pending_count(&self) -> u64 {
        SyncOutboxRepo::new(&self.conn)
            .pending_count()
            .expect("pending count")
    }

    /// All snippets marked as conflict copies of `source_id`.
    fn conflict_copies_of(&self, source_id: &str) -> Vec<Snippet> {
        SnippetRepo::new(&self.conn)
            .list(500, 0)
            .expect("list")
            .into_iter()
            .filter(|s| s.conflict_of.as_deref() == Some(source_id))
            .collect()
    }
}

fn snippet(id: &str, title: &str, body: &str, now: i64) -> Snippet {
    Snippet {
        id: id.to_string(),
        workspace_id: "default".to_string(),
        title: title.to_string(),
        content: SnippetContent::Plaintext(body.to_string()),
        snippet_type: SnippetType::Text,
        description: None,
        folder_id: None,
        trigger: None,
        trigger_mode: None,
        language: None,
        security_level: SecurityLevel::Normal,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: Vec::new(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

/// Creates a vault with a known MK on an instance so pairing/recovery can
/// exercise the vault-material path (the product path builds the same
/// header/domain-key rows through `VaultSession::initialize`; the test
/// needs to hold the MK, so it wraps the rows itself).
fn init_vault(conn: &Connection, mk: &SymmetricKey, password: &[u8], now: i64) -> SymmetricKey {
    let params = KdfParams::v1();
    let kek = derive_kek(password, &params).expect("derive kek");
    let wrapped_mk = wrap_key(&kek, 1, &aad_master_key(), mk).expect("wrap mk");
    let repo = VaultKeyRepo::new(conn);
    repo.put_header(&VaultKeyHeader {
        id: new_id(),
        kdf: params,
        wrapped_mk,
        created_at: now,
        updated_at: now,
    })
    .expect("put header");
    let k_vault = SymmetricKey::generate();
    let wrapped = wrap_key(mk, 1, &aad_domain_key(KeyDomain::Vault.as_str()), &k_vault)
        .expect("wrap k_vault");
    repo.put_domain_key(&DomainKey {
        domain: KeyDomain::Vault,
        key_id: 1,
        wrapped_key: wrapped,
        created_at: now,
    })
    .expect("put domain key");
    k_vault
}

/// Runs the real pairing flow between a trusted instance `t` and a
/// new instance `n`: begin → code → SAS on both ends (asserted equal) →
/// approve/offer → claim → finalize. Returns the adoption facts.
#[allow(clippy::too_many_arguments)]
fn pair_via_protocol(
    server: &TestServer,
    t: &mut Instance,
    n: &mut Instance,
    account_id: &str,
    allow_vault: bool,
    master_key: Option<&SymmetricKey>,
    master_password: Option<&[u8]>,
    now: i64,
) -> AdoptedAccount {
    let handle = n
        .engine
        .begin_pairing(&n.conn, &server.url, account_id)
        .expect("begin pairing");
    // The joining screen prints how long the code is good for, so the window
    // has to come from the server that will stop honouring it — not from a
    // constant on the screen's own side.
    assert!(
        handle
            .expires_in_seconds()
            .is_some_and(|seconds| seconds > 0),
        "a server session must say how long it lasts"
    );
    let code = PairingCode::decode(handle.code()).expect("decode pairing code");
    let sas_on_trusted = t.engine.pairing_sas(&t.conn, &code).expect("SAS on T");
    // The user confirmed the SAS on T: T offers.
    t.engine
        .approve_pairing(&t.conn, &t.store, &code, allow_vault, master_key, now)
        .expect("approve pairing");
    let claimed = n
        .engine
        .poll_pairing(&handle)
        .expect("poll pairing")
        .expect("offer must be ready after approval");
    assert_eq!(
        claimed.sas(),
        sas_on_trusted,
        "both ends must independently compute the same SAS"
    );
    n.engine
        .finalize_pairing(&n.conn, &n.store, &handle, &claimed, master_password, now)
        .expect("finalize pairing")
}

/// Builds the paired pair through the real pairing flow: A creates the
/// account (root device), B runs begin/code/SAS/claim (no vault involved).
fn paired_instances(dir: &Path, server: &TestServer) -> (Instance, Instance, String) {
    let mut a = Instance::new(dir, "alpha", &server.url);
    let account_id = a
        .engine
        .create_account(
            &a.conn,
            &a.store,
            &typvia_sync::NewAccount {
                device_name: "E2E first device",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: None,
            },
            1_700_000_001_000,
        )
        .expect("create account");

    let mut b = Instance::new(dir, "beta", &server.url);
    pair_via_protocol(
        server,
        &mut a,
        &mut b,
        &account_id,
        false,
        None,
        None,
        1_700_000_002_000,
    );
    (a, b, account_id)
}

// ----- scenarios ----------------------------------------------------------

/// Marker plaintext for the sensitive snippet: must never appear in wire
/// bytes, the server database, or either client's K_sync-visible payloads.
const SENSITIVE_MARKER: &[u8] = b"AKIA_FAKE_E2E_MARKER";

#[test]
fn two_instances_converge_through_the_real_server() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;

    // A writes a folder, a tag, and two snippets — one sensitive whose
    // body is a pre-sealed vault envelope (double envelope: this test
    // treats the bytes as opaque, exactly like the product).
    let now = 1_700_000_010_000;
    {
        let guard = a.attach_observer();
        let tx = a.conn.unchecked_transaction().expect("tx");
        FolderRepo::new(&tx)
            .insert(&typvia_core::model::Folder {
                id: "folder-1".to_string(),
                parent_id: None,
                name: "Work".to_string(),
                sort_order: 0,
                created_at: now,
                updated_at: now,
            })
            .expect("folder");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Folder,
                entity_id: "folder-1".to_string(),
                deleted_at: None,
                now,
            },
        )
        .expect("notify folder");
        TagRepo::new(&tx)
            .insert(&typvia_core::model::Tag {
                id: "tag-1".to_string(),
                name: "urgent".to_string(),
                created_at: now,
            })
            .expect("tag");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Tag,
                entity_id: "tag-1".to_string(),
                deleted_at: None,
                now,
            },
        )
        .expect("notify tag");
        tx.commit().expect("commit");

        let mut normal = snippet("snip-1", "Greeting", "Hello from alpha", now);
        normal.folder_id = Some("folder-1".to_string());
        a.write_snippet(&normal, now);

        // A real inner K_vault envelope around a marker body: the sync
        // layer must move these bytes opaquely (double envelope).
        let vault_key = SymmetricKey::generate();
        let inner_envelope = typvia_crypto::seal(
            &vault_key,
            1,
            &typvia_crypto::aad_record("snip-2"),
            SENSITIVE_MARKER,
        )
        .expect("seal inner envelope");
        let mut sensitive = snippet("snip-2", "Prod secret", "", now);
        sensitive.snippet_type = SnippetType::Sensitive;
        sensitive.security_level = SecurityLevel::Sensitive;
        sensitive.content = SnippetContent::Ciphertext(inner_envelope);
        a.write_snippet(&sensitive, now);
        drop(guard);
    }

    let report = a
        .engine
        .sync(&a.conn, &a.store, now + 1_000)
        .expect("A sync");
    assert_eq!(report.pushed, 4);
    assert!(report.conflicts.is_empty());
    assert_eq!(report.pending_backlog, 0);

    // B pulls and restores everything byte-for-byte.
    let report = b
        .engine
        .sync(&b.conn, &b.store, now + 2_000)
        .expect("B sync");
    assert_eq!(report.applied, 4);
    assert_eq!(report.skipped, 0);
    let restored = b.snippet("snip-1").expect("snip-1 on B");
    assert_eq!(restored, a.snippet("snip-1").expect("snip-1 on A"));
    let restored_secret = b.snippet("snip-2").expect("snip-2 on B");
    assert_eq!(restored_secret, a.snippet("snip-2").expect("snip-2 on A"));
    assert_eq!(
        FolderRepo::new(&b.conn)
            .get("folder-1")
            .expect("get")
            .expect("folder on B")
            .name,
        "Work"
    );

    // B edits; A pulls incrementally — only the new record travels.
    let mut edited = b.snippet("snip-1").expect("snip-1");
    edited.title = "Greeting (from beta)".to_string();
    edited.updated_at = now + 3_000;
    edited.version = 2;
    {
        let _guard = b.attach_observer();
        b.write_snippet(&edited, now + 3_000);
    }
    let report = b
        .engine
        .sync(&b.conn, &b.store, now + 4_000)
        .expect("B push");
    assert_eq!(report.pushed, 1);
    let report = a
        .engine
        .sync(&a.conn, &a.store, now + 5_000)
        .expect("A pull");
    assert_eq!(report.applied, 1);
    assert_eq!(report.skipped_own, 0, "cursor already past A's own records");
    assert_eq!(
        a.snippet("snip-1").expect("snip-1").title,
        "Greeting (from beta)"
    );

    // Tombstone propagation: A trashes, B applies a soft delete.
    {
        let _guard = a.attach_observer();
        a.trash_snippet("snip-1", now + 6_000);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 7_000)
        .expect("A push tombstone");
    let report = b
        .engine
        .sync(&b.conn, &b.store, now + 8_000)
        .expect("B pull tombstone");
    assert_eq!(report.applied, 1);
    let trashed = b.snippet("snip-1").expect("still present");
    assert_eq!(trashed.deleted_at, Some(now + 6_000));
    // The shadow head survives the tombstone, guarding against revival.
    let shadow = SyncStateRepo::new(&b.conn)
        .shadow_get(SyncEntityType::Snippet, "snip-1")
        .expect("shadow")
        .expect("head kept");
    assert_eq!(shadow.document, None);
    assert!(shadow.version >= 3);

    // Red lines: the sensitive marker plaintext must not exist anywhere
    // at rest, and the raw K_sync bytes must never leave
    // the secure store — checked at the byte level on all three databases.
    let k_sync = a
        .store
        .retrieve("sync.k_sync.1")
        .expect("retrieve")
        .expect("k_sync");
    for instance in [&a, &b] {
        instance
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint");
    }
    for db in [&a.db_path, &b.db_path, &server.db_path] {
        let bytes = db_bytes(db);
        assert!(
            !contains(&bytes, SENSITIVE_MARKER),
            "sensitive plaintext leaked into {db:?}"
        );
        assert!(
            !contains(&bytes, &k_sync),
            "raw K_sync bytes leaked into {db:?}"
        );
    }
    // Self-check that the scan sees plaintext at all: A's normal snippet
    // body is legitimately present in A's database.
    assert!(contains(&db_bytes(&a.db_path), b"Hello from alpha"));
}

/// Offline backlog: multiple queued versions of one entity survive a dead
/// server and catch up in order (head + 1 contract) once it returns.
#[test]
fn offline_backlog_catches_up_in_order_after_restart() {
    let dir = tempdir();
    let mut server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_020_000;

    server.stop();
    {
        let _guard = a.attach_observer();
        for (round, title) in ["draft one", "draft two", "final"].iter().enumerate() {
            let mut row = snippet("snip-off", title, &format!("body {title}"), now);
            row.version = (round + 1) as u32;
            row.updated_at = now + round as i64;
            a.write_snippet(&row, now + round as i64);
        }
    }
    assert_eq!(
        SyncOutboxRepo::new(&a.conn).pending_count().expect("count"),
        3
    );
    // The server is down: the round fails, the backlog stays untouched.
    let err = a
        .engine
        .sync(&a.conn, &a.store, now + 10)
        .expect_err("offline");
    assert!(matches!(
        err,
        SyncError::Transport(TransportError::Network(_))
    ));
    assert_eq!(
        SyncOutboxRepo::new(&a.conn).pending_count().expect("count"),
        3
    );
    // The failure armed the backoff; an early retry never hits the wire.
    assert!(matches!(
        a.engine.sync(&a.conn, &a.store, now + 11),
        Err(SyncError::BackedOff { .. })
    ));

    server.restart();
    let report = a
        .engine
        .sync(&a.conn, &a.store, now + 60_000)
        .expect("catch up");
    assert_eq!(report.pushed, 3);
    assert_eq!(
        SyncOutboxRepo::new(&a.conn).pending_count().expect("count"),
        0
    );

    let report = b
        .engine
        .sync(&b.conn, &b.store, now + 61_000)
        .expect("B pull");
    assert_eq!(report.applied, 3);
    assert_eq!(b.snippet("snip-off").expect("row").title, "final");
}

/// Idempotent re-push: a record resent after a lost acknowledgment gets
/// the same server_seq back and creates no second copy.
#[test]
fn a_repushed_record_is_deduplicated_by_the_server() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_030_000;

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-dup", "Once", "only once", now), now);
    }
    let report = a.engine.sync(&a.conn, &a.store, now + 1).expect("push");
    assert_eq!(report.pushed, 1);

    // Model a lost acknowledgment: the record goes back to pending and is
    // resent unchanged (same record id, same version).
    a.conn
        .execute(
            "UPDATE sync_record SET state = 'pending', server_seq = NULL \
             WHERE entity_id = 'snip-dup'",
            [],
        )
        .expect("reset to pending");
    let report = a.engine.sync(&a.conn, &a.store, now + 2).expect("re-push");
    assert_eq!(report.pushed, 1);
    assert_eq!(report.pending_backlog, 0);

    // B sees exactly one record for the entity.
    let report = b.engine.sync(&b.conn, &b.store, now + 3).expect("pull");
    assert_eq!(report.applied, 1);
    assert_eq!(b.snippet("snip-dup").expect("row").title, "Once");
}

/// Concurrent edits from the same base: the loser's push is rejected whole
/// (409), its records are parked as conflicted — never dropped, never
/// faked as pushed — and the same call resolves them by three-way merge
/// and re-pushes the merged state.
#[test]
fn a_version_conflict_parks_the_losing_records() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_040_000;

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-c", "Base", "base body", now), now);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 1)
        .expect("A push base");
    b.engine
        .sync(&b.conn, &b.store, now + 2)
        .expect("B pull base");

    // Both edit from the same base; A wins the push race.
    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-c").expect("row");
        mine.title = "A's edit".to_string();
        mine.updated_at = now + 3;
        a.write_snippet(&mine, now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");

    {
        let _guard = b.attach_observer();
        let mut mine = b.snippet("snip-c").expect("row");
        mine.title = "B's edit".to_string();
        mine.updated_at = now + 5;
        b.write_snippet(&mine, now + 5);
    }
    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B sync");
    assert_eq!(report.conflicts.len(), 1, "the 409 head was reported");
    assert_eq!(report.conflicts[0].entity_id, "snip-c");
    assert_eq!(report.conflicts[0].head_version, 2);
    // The same call resolves the conflict — three-way merge, then the
    // merged record goes out past the server head.
    assert_eq!(report.merged, 1);
    assert_eq!(report.pushed, 1, "the merged record was re-pushed in-call");
    assert!(
        SyncOutboxRepo::new(&b.conn)
            .list_conflicted()
            .expect("list")
            .is_empty(),
        "nothing stays parked once the merge resolved it"
    );
    // Title changed on both sides: rule 2 LWW — B's later clock wins.
    assert_eq!(b.snippet("snip-c").expect("row").title, "B's edit");

    // A second round has nothing left to do.
    let report = b.engine.sync(&b.conn, &b.store, now + 7).expect("B again");
    assert_eq!(report.pushed, 0);
    assert!(report.conflicts.is_empty());
    assert_eq!(report.merged, 0);

    // A pulls the merged head and both ends converge.
    let report = a.engine.sync(&a.conn, &a.store, now + 8).expect("A pull");
    assert_eq!(report.applied, 1);
    assert_eq!(a.snippet("snip-c"), b.snippet("snip-c"));
}

/// A malicious (or corrupted) pull response with a non-monotonic cursor
/// aborts the round: nothing is applied, the watermark stays, and a clean
/// retry applies everything (fail keeps prior state; re-run is idempotent).
#[test]
fn a_poisoned_pull_applies_nothing_and_a_clean_retry_recovers() {
    struct PoisonOnce {
        inner: HttpTransport,
        poisoned: std::cell::Cell<bool>,
    }

    impl typvia_sync::SyncTransport for PoisonOnce {
        fn handshake(&self) -> Result<typvia_sync::HandshakeInfo, TransportError> {
            self.inner.handshake()
        }
        fn create_account(
            &self,
            root: &typvia_sync::RootStatement,
        ) -> Result<String, TransportError> {
            self.inner.create_account(root)
        }
        fn auth_challenge(&self, device_id: &str) -> Result<Vec<u8>, TransportError> {
            self.inner.auth_challenge(device_id)
        }
        fn auth_session(
            &self,
            device_id: &str,
            signature: &[u8; 64],
        ) -> Result<typvia_sync::SessionToken, TransportError> {
            self.inner.auth_session(device_id, signature)
        }
        fn push_records(
            &self,
            token: &typvia_sync::SessionToken,
            records: &[typvia_sync::WireRecord],
        ) -> Result<typvia_sync::PushOutcome, TransportError> {
            self.inner.push_records(token, records)
        }
        fn pull_records(
            &self,
            token: &typvia_sync::SessionToken,
            since: u64,
            limit: u32,
        ) -> Result<typvia_sync::PullPage, TransportError> {
            let mut page = self.inner.pull_records(token, since, limit)?;
            if !self.poisoned.get() && page.records.len() > 1 {
                // A compromised server replays the page out of order.
                page.records.reverse();
                self.poisoned.set(true);
            }
            Ok(page)
        }
        fn device_directory(
            &self,
            token: &typvia_sync::SessionToken,
        ) -> Result<typvia_sync::DeviceDirectory, TransportError> {
            self.inner.device_directory(token)
        }
        fn revoke_device(
            &self,
            token: &typvia_sync::SessionToken,
            device_id: &str,
            revoked_at: i64,
            signature: &[u8; 64],
        ) -> Result<(), TransportError> {
            self.inner
                .revoke_device(token, device_id, revoked_at, signature)
        }
        fn pair_begin(
            &self,
            account_id: &str,
        ) -> Result<typvia_sync::PairingSession, TransportError> {
            self.inner.pair_begin(account_id)
        }
        fn pair_offer(
            &self,
            token: &typvia_sync::SessionToken,
            session_id: &str,
            certificate: &DeviceCertificate,
            sealed_bundle: &[u8],
            bundle_signature: &[u8; 64],
        ) -> Result<(), TransportError> {
            self.inner.pair_offer(
                token,
                session_id,
                certificate,
                sealed_bundle,
                bundle_signature,
            )
        }
        fn pair_claim(&self, session_id: &str) -> Result<typvia_sync::PairClaim, TransportError> {
            self.inner.pair_claim(session_id)
        }
        fn put_key_update(
            &self,
            token: &typvia_sync::SessionToken,
            target_device_id: &str,
            payload: &[u8],
        ) -> Result<i64, TransportError> {
            self.inner.put_key_update(token, target_device_id, payload)
        }
        fn list_key_updates(
            &self,
            token: &typvia_sync::SessionToken,
            since: i64,
        ) -> Result<Vec<typvia_sync::KeyUpdate>, TransportError> {
            self.inner.list_key_updates(token, since)
        }
        fn put_recovery_blob(
            &self,
            token: &typvia_sync::SessionToken,
            blob: &[u8],
            rootproof_pub: &[u8],
        ) -> Result<(), TransportError> {
            self.inner.put_recovery_blob(token, blob, rootproof_pub)
        }
        fn get_recovery_blob(&self, account_id: &str) -> Result<Vec<u8>, TransportError> {
            self.inner.get_recovery_blob(account_id)
        }
        fn re_root(
            &self,
            account_id: &str,
            proof_signature: Option<&[u8; 64]>,
            new_root: Option<&typvia_sync::RootStatement>,
        ) -> Result<typvia_sync::ReRootOutcome, TransportError> {
            self.inner.re_root(account_id, proof_signature, new_root)
        }
    }

    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let now = 1_700_000_050_000;

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-p1", "One", "body one", now), now);
        a.write_snippet(&snippet("snip-p2", "Two", "body two", now), now);
    }
    a.engine.sync(&a.conn, &a.store, now + 1).expect("A push");

    // B syncs through the poisoning transport: the round aborts whole.
    let mut poisoned_engine = SyncEngine::new(
        Box::new(PoisonOnce {
            inner: HttpTransport::new(&server.url).expect("transport"),
            poisoned: std::cell::Cell::new(false),
        }),
        b.identity(),
        b.device_id.clone(),
    );
    let err = poisoned_engine
        .sync(&b.conn, &b.store, now + 2)
        .expect_err("poisoned pull must abort");
    assert!(matches!(err, SyncError::CursorViolation));
    // Nothing was applied and the watermark did not move.
    assert_eq!(b.snippet("snip-p1"), None);
    assert_eq!(b.snippet("snip-p2"), None);
    assert_eq!(
        SyncStateRepo::new(&b.conn)
            .config_get()
            .expect("config")
            .applied_server_seq,
        0
    );

    // A clean retry applies everything — the re-run is idempotent.
    let mut b = b;
    let report = b
        .engine
        .sync(&b.conn, &b.store, now + 3)
        .expect("clean retry");
    assert_eq!(report.applied, 2);
    assert_eq!(b.snippet("snip-p1").expect("row").title, "One");
    assert_eq!(b.snippet("snip-p2").expect("row").title, "Two");
}

/// A record sealed under a K_sync generation B does not hold is parked —
/// persisted, not dropped — and applies on the next round after the key
/// arrives.
#[test]
fn an_unknown_key_generation_parks_then_applies_after_key_arrival() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_060_000;

    // A rotates to generation 2 locally (distribution is covered
    // elsewhere; here the generation simply exists only on A).
    let generation_two = SymmetricKey::generate();
    install_key(&a.store, 2, &generation_two).expect("install gen 2");
    let state = SyncStateRepo::new(&a.conn);
    let mut config = state.config_get().expect("config");
    config.sync_key_id = 2;
    state.config_put(&config).expect("config put");

    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("snip-k", "Rotated", "sealed under gen 2", now),
            now,
        );
    }
    a.engine.sync(&a.conn, &a.store, now + 1).expect("A push");

    // B pulls: the record parks, the watermark still advances.
    let report = b.engine.sync(&b.conn, &b.store, now + 2).expect("B sync");
    assert_eq!(report.parked, 1);
    assert_eq!(report.applied, 0);
    assert_eq!(b.snippet("snip-k"), None);
    assert_eq!(
        SyncStateRepo::new(&b.conn)
            .pending_list()
            .expect("parked")
            .len(),
        1
    );

    // The key arrives (pairing/key-update equivalent): the parked record
    // applies on the next round without any re-pull.
    install_key(&b.store, 2, &generation_two).expect("install gen 2 on B");
    let state = SyncStateRepo::new(&b.conn);
    let mut config = state.config_get().expect("config");
    config.sync_key_id = 2;
    state.config_put(&config).expect("config put");

    let report = b.engine.sync(&b.conn, &b.store, now + 3).expect("B retry");
    assert_eq!(report.applied, 1);
    assert_eq!(b.snippet("snip-k").expect("row").title, "Rotated");
    assert!(
        SyncStateRepo::new(&b.conn)
            .pending_list()
            .expect("parked")
            .is_empty()
    );
}

// ----- conflict resolution scenarios --------------------------------------

/// Seeds one base snippet through the server so both instances share the
/// same shadow base before the conflicting edits.
fn seed_base(a: &mut Instance, b: &mut Instance, row: &Snippet, now: i64) {
    {
        let _guard = a.attach_observer();
        a.write_snippet(row, now);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 1)
        .expect("A push base");
    b.engine
        .sync(&b.conn, &b.store, now + 2)
        .expect("B pull base");
    assert_eq!(a.snippet(&row.id), b.snippet(&row.id));
}

/// Merge rule 1: concurrent edits of different fields (title on A, folder
/// on B) auto-merge, and both devices converge on an identical row.
#[test]
fn concurrent_edits_of_different_fields_converge_identically() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_070_000;

    // A shared folder reaches both sides along with the base snippet.
    {
        let _guard = a.attach_observer();
        let tx = a.conn.unchecked_transaction().expect("tx");
        FolderRepo::new(&tx)
            .insert(&typvia_core::model::Folder {
                id: "folder-m".to_string(),
                parent_id: None,
                name: "Merged".to_string(),
                sort_order: 0,
                created_at: now,
                updated_at: now,
            })
            .expect("folder");
        sync_hooks::notify_change(
            &tx,
            &EntityChange {
                entity_type: SyncEntityType::Folder,
                entity_id: "folder-m".to_string(),
                deleted_at: None,
                now,
            },
        )
        .expect("notify folder");
        tx.commit().expect("commit");
    }
    seed_base(
        &mut a,
        &mut b,
        &snippet("snip-m", "Base", "base body", now),
        now,
    );

    // A retitles and wins the push race; B moves it into the folder.
    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-m").expect("row");
        mine.title = "Retitled on alpha".to_string();
        mine.updated_at = now + 3;
        mine.version = 2;
        a.write_snippet(&mine, now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");
    {
        let _guard = b.attach_observer();
        let mut mine = b.snippet("snip-m").expect("row");
        mine.folder_id = Some("folder-m".to_string());
        mine.updated_at = now + 5;
        mine.version = 2;
        b.write_snippet(&mine, now + 5);
    }

    // One sync call on B runs the whole cycle: 409 → merge → re-push.
    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B merge");
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.merged, 1);
    assert!(report.conflict_copy_ids.is_empty(), "no body conflict here");
    assert_eq!(report.pushed, 1, "the merged record went out in-call");
    assert_eq!(report.pending_backlog, 0);
    assert!(
        SyncOutboxRepo::new(&b.conn)
            .list_conflicted()
            .expect("list")
            .is_empty(),
        "the conflict parking is cleared by the merge"
    );

    let report = a.engine.sync(&a.conn, &a.store, now + 7).expect("A pull");
    assert_eq!(report.applied, 1);

    let on_a = a.snippet("snip-m").expect("on A");
    let on_b = b.snippet("snip-m").expect("on B");
    assert_eq!(on_a, on_b, "both ends must hold the identical merged row");
    assert_eq!(on_a.title, "Retitled on alpha");
    assert_eq!(on_a.folder_id.as_deref(), Some("folder-m"));
    assert_eq!(
        on_a.content,
        SnippetContent::Plaintext("base body".to_string())
    );
}

/// Merge rule 3: concurrent body edits keep both versions — the remote
/// body enters the entity, the local body becomes a conflict copy, and the
/// copy syncs so both devices see the same pending pair.
#[test]
fn concurrent_body_edits_keep_both_versions_on_both_devices() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_080_000;
    seed_base(
        &mut a,
        &mut b,
        &snippet("snip-b", "Notes", "base body", now),
        now,
    );

    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-b").expect("row");
        mine.content = SnippetContent::Plaintext("alpha body".to_string());
        mine.updated_at = now + 3;
        mine.version = 2;
        a.write_snippet(&mine, now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");
    {
        let _guard = b.attach_observer();
        let mut mine = b.snippet("snip-b").expect("row");
        mine.content = SnippetContent::Plaintext("beta body".to_string());
        mine.updated_at = now + 5;
        mine.version = 2;
        b.write_snippet(&mine, now + 5);
    }

    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B merge");
    assert_eq!(report.merged, 1);
    assert_eq!(report.conflict_copy_ids.len(), 1);
    let copy_id = report.conflict_copy_ids[0].clone();
    assert_eq!(report.pushed, 2, "merged record plus the conflict copy");

    let report = a.engine.sync(&a.conn, &a.store, now + 7).expect("A pull");
    assert_eq!(report.applied, 2, "merged head and the conflict copy");

    for instance in [&a, &b] {
        let entity = instance.snippet("snip-b").expect("entity");
        assert_eq!(
            entity.content,
            SnippetContent::Plaintext("alpha body".to_string()),
            "the server-head body won the entity"
        );
        let copies = instance.conflict_copies_of("snip-b");
        assert_eq!(copies.len(), 1, "exactly one pending pair per device");
        assert_eq!(copies[0].id, copy_id);
        assert_eq!(
            copies[0].content,
            SnippetContent::Plaintext("beta body".to_string())
        );
        assert_eq!(copies[0].title, "Notes (conflict on beta)");
        assert_eq!(copies[0].trigger, None);
    }
    assert_eq!(a.snippet("snip-b"), b.snippet("snip-b"));
    assert_eq!(a.snippet(&copy_id), b.snippet(&copy_id));
}

/// Merge rule 2: with equal clocks the lexicographically larger device id
/// wins — a deterministic tie-break both ends compute identically.
#[test]
fn equal_clocks_resolve_deterministically_by_device_id() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_090_000;
    seed_base(&mut a, &mut b, &snippet("snip-t", "Base", "body", now), now);

    // Both retitle with the exact same clock value.
    let same_clock = now + 3;
    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-t").expect("row");
        mine.title = "Alpha title".to_string();
        mine.updated_at = same_clock;
        mine.version = 2;
        a.write_snippet(&mine, same_clock);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");
    {
        let _guard = b.attach_observer();
        let mut mine = b.snippet("snip-t").expect("row");
        mine.title = "Beta title".to_string();
        mine.updated_at = same_clock;
        mine.version = 2;
        b.write_snippet(&mine, same_clock);
    }
    let report = b.engine.sync(&b.conn, &b.store, now + 5).expect("B merge");
    assert_eq!(report.merged, 1);
    a.engine.sync(&a.conn, &a.store, now + 6).expect("A pull");

    // "e2e-beta-device" > "e2e-alpha-device": beta's value must win on
    // both ends, no copy involved (title is not the body).
    assert_eq!(a.snippet("snip-t").expect("row").title, "Beta title");
    assert_eq!(a.snippet("snip-t"), b.snippet("snip-t"));
    assert!(a.conflict_copies_of("snip-t").is_empty());
    assert!(b.conflict_copies_of("snip-t").is_empty());
}

/// A concurrent delete and edit resolves to the tombstone. The
/// edited content survives in the version history and both recycle bins
/// end up holding the same row.
#[test]
fn a_concurrent_delete_and_edit_resolves_to_the_tombstone() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_100_000;
    seed_base(
        &mut a,
        &mut b,
        &snippet("snip-d", "Base", "base body", now),
        now,
    );

    // A edits and wins the push race; B deletes concurrently.
    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-d").expect("row");
        mine.title = "Edited on alpha".to_string();
        mine.content = SnippetContent::Plaintext("edited body".to_string());
        mine.updated_at = now + 3;
        mine.version = 2;
        a.write_snippet(&mine, now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");
    {
        let _guard = b.attach_observer();
        b.trash_snippet("snip-d", now + 5);
    }

    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B merge");
    assert_eq!(report.merged, 1);
    assert_eq!(
        report.pushed, 1,
        "the winning tombstone re-queued past the head"
    );

    // B: the edit landed in the version history, the entity sits in the
    // bin carrying the edited state.
    let history = VersionRepo::new(&b.conn)
        .list("snip-d", 10, 0)
        .expect("history");
    assert!(
        history.iter().any(|v| v.title == "Edited on alpha"
            && v.content == SnippetContent::Plaintext("edited body".to_string())),
        "the edited content must survive in the version history"
    );
    let on_b = b.snippet("snip-d").expect("bin row");
    assert_eq!(on_b.deleted_at, Some(now + 5));
    assert_eq!(
        on_b.content,
        SnippetContent::Plaintext("edited body".to_string())
    );

    // A pulls the winning tombstone: both bins converge.
    let report = a.engine.sync(&a.conn, &a.store, now + 7).expect("A pull");
    assert_eq!(report.applied, 1);
    let on_a = a.snippet("snip-d").expect("bin row");
    assert_eq!(on_a.deleted_at, Some(now + 5));
    assert_eq!(on_a, on_b, "both recycle bins hold the same row");
}

/// Restoring from the recycle bin is an explicit action producing a
/// version past the tombstone head — it legally revives the snippet on
/// every device.
#[test]
fn a_recycle_bin_restore_revives_the_snippet_across_devices() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_110_000;
    seed_base(
        &mut a,
        &mut b,
        &snippet("snip-r", "Base", "base body", now),
        now,
    );

    {
        let _guard = a.attach_observer();
        a.trash_snippet("snip-r", now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push tombstone");
    let report = b.engine.sync(&b.conn, &b.store, now + 5).expect("B pull");
    assert_eq!(report.applied, 1);
    assert_eq!(
        b.snippet("snip-r").expect("row").deleted_at,
        Some(now + 3),
        "the tombstone applied on B"
    );

    // The restore rides a version strictly past the tombstone head.
    {
        let _guard = a.attach_observer();
        a.restore_snippet("snip-r", now + 6);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 7)
        .expect("A push revival");
    let report = b.engine.sync(&b.conn, &b.store, now + 8).expect("B pull");
    assert_eq!(report.applied, 1);
    let revived = b.snippet("snip-r").expect("row");
    assert_eq!(revived.deleted_at, None, "the snippet revived on B");
    let shadow = SyncStateRepo::new(&b.conn)
        .shadow_get(SyncEntityType::Snippet, "snip-r")
        .expect("shadow")
        .expect("head");
    assert_eq!(shadow.version, 3, "revival version is past the tombstone");
    assert!(shadow.document.is_some());
}

/// Merge rule 3 for sensitive snippets: concurrent body edits are decided
/// on ciphertext inequality, the conflict copy carries the local K_vault
/// envelope byte-for-byte, and no vault plaintext exists anywhere at rest.
#[test]
fn concurrent_sensitive_body_edits_pass_envelopes_through_with_zero_plaintext() {
    const MARKER_BASE: &[u8] = b"AKIA_FAKE_BASE_BODY";
    const MARKER_ALPHA: &[u8] = b"AKIA_FAKE_ALPHA_BODY";
    const MARKER_BETA: &[u8] = b"AKIA_FAKE_BETA_BODY";

    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_120_000;

    // Real inner K_vault envelopes: the sync layer must treat every one of
    // them as opaque bytes (double envelope).
    let vault_key = SymmetricKey::generate();
    let seal = |marker: &[u8]| {
        typvia_crypto::seal(&vault_key, 1, &typvia_crypto::aad_record("snip-s"), marker)
            .expect("seal inner envelope")
    };
    let mut base = snippet("snip-s", "Prod secret", "", now);
    base.snippet_type = SnippetType::Sensitive;
    base.security_level = SecurityLevel::Sensitive;
    base.content = SnippetContent::Ciphertext(seal(MARKER_BASE));
    seed_base(&mut a, &mut b, &base, now);

    let envelope_alpha = seal(MARKER_ALPHA);
    {
        let _guard = a.attach_observer();
        let mut mine = a.snippet("snip-s").expect("row");
        mine.content = SnippetContent::Ciphertext(envelope_alpha.clone());
        mine.updated_at = now + 3;
        mine.version = 2;
        a.write_snippet(&mine, now + 3);
    }
    a.engine
        .sync(&a.conn, &a.store, now + 4)
        .expect("A push edit");
    let envelope_beta = seal(MARKER_BETA);
    {
        let _guard = b.attach_observer();
        let mut mine = b.snippet("snip-s").expect("row");
        mine.content = SnippetContent::Ciphertext(envelope_beta.clone());
        mine.updated_at = now + 5;
        mine.version = 2;
        b.write_snippet(&mine, now + 5);
    }

    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B merge");
    assert_eq!(report.merged, 1);
    assert_eq!(report.conflict_copy_ids.len(), 1);
    let copy_id = report.conflict_copy_ids[0].clone();
    let report = a.engine.sync(&a.conn, &a.store, now + 7).expect("A pull");
    assert_eq!(report.applied, 2);

    for instance in [&a, &b] {
        let entity = instance.snippet("snip-s").expect("entity");
        assert_eq!(
            entity.content,
            SnippetContent::Ciphertext(envelope_alpha.clone()),
            "the server-head envelope won the entity byte-for-byte"
        );
        let copy = instance.snippet(&copy_id).expect("copy");
        assert_eq!(
            copy.content,
            SnippetContent::Ciphertext(envelope_beta.clone()),
            "the local envelope passed through byte-for-byte"
        );
        assert_eq!(copy.conflict_of.as_deref(), Some("snip-s"));
        assert_eq!(copy.security_level, SecurityLevel::Sensitive);
        assert_eq!(copy.snippet_type, SnippetType::Sensitive);
    }

    // Red line: none of the vault plaintext markers exist at rest on
    // either client or the server, at the byte level.
    for instance in [&a, &b] {
        instance
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint");
    }
    for db in [&a.db_path, &b.db_path, &server.db_path] {
        let bytes = db_bytes(db);
        for marker in [MARKER_BASE, MARKER_ALPHA, MARKER_BETA] {
            assert!(
                !contains(&bytes, marker),
                "vault plaintext leaked into {db:?}"
            );
        }
    }
    // Scan self-check: the beta envelope itself legitimately rests in B's
    // database (as the conflict copy's ciphertext).
    assert!(contains(&db_bytes(&b.db_path), &envelope_beta));
}

// ----- pairing / recovery / revocation scenarios --------------------------

/// The full pairing chain with vault access: A owns a vault and data, B
/// pairs through begin/code/SAS/offer/claim, adopts the vault under its
/// own master password, pulls everything, and can decrypt the sensitive
/// body end to end.
#[test]
fn real_pairing_shares_account_and_vault_end_to_end() {
    const VAULT_MARKER: &[u8] = b"AKIA_FAKE_VAULT_BODY";
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let now = 1_700_000_200_000;

    let mut a = Instance::new(&dir, "alpha", &server.url);
    let mk = SymmetricKey::generate();
    init_vault(&a.conn, &mk, b"FAKE_alpha_master", now);
    let account_id = a
        .engine
        .create_account(
            &a.conn,
            &a.store,
            &typvia_sync::NewAccount {
                device_name: "E2E first device",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: Some(&mk),
            },
            now + 1,
        )
        .expect("create account");

    // Sensitive content sealed through the product path (K_vault inner
    // envelope), plus a normal snippet.
    let mut a_vault = VaultSession::new();
    a_vault
        .unlock_with_password(&a.conn, b"FAKE_alpha_master", now + 2)
        .expect("unlock A vault");
    let inner_envelope = a_vault
        .encrypt_content(&a.conn, "snip-vault", VAULT_MARKER)
        .expect("seal sensitive body");
    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("snip-plain", "Plain", "plain body", now + 3),
            now + 3,
        );
        let mut sensitive = snippet("snip-vault", "Secret", "", now + 4);
        sensitive.snippet_type = SnippetType::Sensitive;
        sensitive.security_level = SecurityLevel::Sensitive;
        sensitive.content = SnippetContent::Ciphertext(inner_envelope.clone());
        a.write_snippet(&sensitive, now + 4);
    }
    a.engine.sync(&a.conn, &a.store, now + 5).expect("A push");

    // The real pairing flow, vault allowed; the helper asserts the SAS
    // matches on both ends.
    let mut b = Instance::new(&dir, "beta", &server.url);
    let adopted = pair_via_protocol(
        &server,
        &mut a,
        &mut b,
        &account_id,
        true,
        Some(&mk),
        Some(b"FAKE_beta_master"),
        now + 6,
    );
    assert_eq!(adopted.sync_key_id, 1);

    let report = b.engine.sync(&b.conn, &b.store, now + 7).expect("B pull");
    assert_eq!(report.applied, 2);
    assert_eq!(b.snippet("snip-plain"), a.snippet("snip-plain"));
    let restored = b.snippet("snip-vault").expect("sensitive row on B");
    assert_eq!(restored.content, SnippetContent::Ciphertext(inner_envelope));

    // B unlocks with its own master password (the MK was re-wrapped at
    // finalize) and decrypts the shared vault body.
    let mut b_vault = VaultSession::new();
    b_vault
        .unlock_with_password(&b.conn, b"FAKE_beta_master", now + 8)
        .expect("unlock B vault with B's own password");
    let SnippetContent::Ciphertext(envelope) = restored.content else {
        panic!("sensitive row must stay ciphertext");
    };
    let plain = b_vault
        .decrypt_content(&b.conn, "snip-vault", &envelope)
        .expect("decrypt shared vault body");
    assert_eq!(plain.as_slice(), VAULT_MARKER);
}

/// Restricted pairing: without vault permission the bundle carries no
/// MK/K_vault — the sensitive row syncs
/// as ciphertext that B provably cannot open — and a later sealed
/// key-update grant restores full access without re-pairing.
#[test]
fn restricted_pairing_withholds_vault_material_until_granted() {
    const VAULT_MARKER: &[u8] = b"AKIA_FAKE_RESTRICTED_BODY";
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let now = 1_700_000_210_000;

    let mut a = Instance::new(&dir, "alpha", &server.url);
    let mk = SymmetricKey::generate();
    init_vault(&a.conn, &mk, b"FAKE_alpha_master", now);
    let account_id = a
        .engine
        .create_account(
            &a.conn,
            &a.store,
            &typvia_sync::NewAccount {
                device_name: "E2E first device",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: Some(&mk),
            },
            now + 1,
        )
        .expect("create account");
    let mut a_vault = VaultSession::new();
    a_vault
        .unlock_with_password(&a.conn, b"FAKE_alpha_master", now + 2)
        .expect("unlock A vault");
    let inner_envelope = a_vault
        .encrypt_content(&a.conn, "snip-s", VAULT_MARKER)
        .expect("seal sensitive body");
    {
        let _guard = a.attach_observer();
        let mut sensitive = snippet("snip-s", "Secret", "", now + 3);
        sensitive.snippet_type = SnippetType::Sensitive;
        sensitive.security_level = SecurityLevel::Sensitive;
        sensitive.content = SnippetContent::Ciphertext(inner_envelope.clone());
        a.write_snippet(&sensitive, now + 3);
    }
    a.engine.sync(&a.conn, &a.store, now + 4).expect("A push");

    // Vault access denied at admission time.
    let mut b = Instance::new(&dir, "beta", &server.url);
    pair_via_protocol(
        &server,
        &mut a,
        &mut b,
        &account_id,
        false,
        None,
        None,
        now + 5,
    );
    let report = b.engine.sync(&b.conn, &b.store, now + 6).expect("B pull");
    assert_eq!(report.applied, 1);

    // The locked row is present, ciphertext byte-for-byte...
    let row = b.snippet("snip-s").expect("locked row on B");
    assert_eq!(
        row.content,
        SnippetContent::Ciphertext(inner_envelope.clone())
    );
    // ...and B holds no vault at all: no header, no domain keys, and
    // decryption fails under every secret B possesses (infeasibility, not
    // merely "not attempted").
    assert!(!VaultSession::is_initialized(&b.conn).expect("is_initialized"));
    assert!(
        VaultKeyRepo::new(&b.conn)
            .list_domain_keys()
            .expect("domain keys")
            .is_empty()
    );
    {
        let entries = b.store.entries.borrow();
        assert!(!entries.is_empty(), "B does hold sync secrets");
        for secret in entries.values() {
            let Ok(bytes) = <[u8; 32]>::try_from(secret.as_slice()) else {
                continue;
            };
            let key = SymmetricKey::from_bytes(bytes);
            assert!(
                typvia_crypto::open(&key, &typvia_crypto::aad_record("snip-s"), &inner_envelope)
                    .is_err(),
                "no secret held by B may open the vault envelope"
            );
        }
    }

    // Late authorization: a sealed key update carries MK + K_vault;
    // B adopts it under its own master password.
    a.engine
        .grant_vault_access(&a.conn, &a.store, &b.device_id, &mk, now + 7)
        .expect("grant vault access");
    // A round before adoption must not acknowledge the grant: B still needs
    // to find it once the user supplies a master password.
    b.engine
        .sync(&b.conn, &b.store, now + 8)
        .expect("B round with a pending grant");
    assert_eq!(b.key_update_cursor(), 0);

    let granted = b
        .engine
        .adopt_vault_grant(&b.conn, &b.store, b"FAKE_beta_master", now + 8)
        .expect("adopt vault grant");
    assert!(granted, "the pending grant must be found and adopted");
    // Once adopted the message is spent, so the next round acknowledges it.
    b.engine.sync(&b.conn, &b.store, now + 9).expect("B round");
    assert!(b.key_update_cursor() > 0);
    let mut b_vault = VaultSession::new();
    b_vault
        .unlock_with_password(&b.conn, b"FAKE_beta_master", now + 9)
        .expect("unlock B vault");
    let plain = b_vault
        .decrypt_content(&b.conn, "snip-s", &inner_envelope)
        .expect("decrypt after grant");
    assert_eq!(plain.as_slice(), VAULT_MARKER);
}

/// The recovery round trip: a fresh device with only the
/// recovery code re-roots the account, old devices are locked out, the
/// vault survives under a new master password, and the forced K_sync
/// rotation blinds pre-recovery devices to new records.
#[test]
fn recovery_re_roots_the_account_and_locks_out_old_devices() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let now = 1_700_000_220_000;

    let mut a = Instance::new(&dir, "alpha", &server.url);
    let mk = SymmetricKey::generate();
    init_vault(&a.conn, &mk, b"FAKE_alpha_master", now);
    let account_id = a
        .engine
        .create_account(
            &a.conn,
            &a.store,
            &typvia_sync::NewAccount {
                device_name: "E2E first device",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: Some(&mk),
            },
            now + 1,
        )
        .expect("create account");
    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("snip-r", "Survivor", "survives recovery", now + 2),
            now + 2,
        );
    }
    a.engine.sync(&a.conn, &a.store, now + 3).expect("A push");

    // The code round-trips through its display form, the way a user would
    // type it back in.
    let code = a
        .engine
        .publish_recovery(&a.conn, &a.store, &mk)
        .expect("publish recovery");
    let typed = RecoveryCode::parse(&code.display_groups()).expect("parse displayed code");

    // All devices lost: C recovers with server URL + account id + code.
    let mut c = Instance::new(&dir, "gamma", &server.url);
    let adopted = c
        .engine
        .recover_account(
            &c.conn,
            &c.store,
            &typvia_sync::RecoveryRequest {
                server_url: &server.url,
                account_id: &account_id,
                code: &typed,
                master_password: b"FAKE_gamma_master",
            },
            now + 10,
        )
        .expect("recover account");
    assert_eq!(
        adopted.sync_key_id, 2,
        "recovery must force a K_sync rotation past the recovered generation"
    );

    // The catch-up is deferred to the sync rounds: recovery only
    // persists the predecessor root as a resumable marker, so a network
    // drop after the re-root leaves a visible pending state, never a hole.
    assert!(
        SyncStateRepo::new(&c.conn)
            .config_get()
            .expect("config")
            .recovery_root
            .is_some(),
        "recovery must persist the catch-up marker"
    );
    assert!(
        c.snippet("snip-r").is_none(),
        "history arrives with the round, not inside the recovery call"
    );

    // A restart between the recovery and the first round loses nothing: a
    // fresh engine over the same database resumes from the marker.
    let relocal = ensure_local_device(
        &c.conn,
        &c.store,
        &LocalDeviceConfig {
            device_id: &c.device_id,
            name: "gamma",
            platform: Platform::Macos,
        },
        now + 10,
    )
    .expect("re-ensure local device");
    c.engine = SyncEngine::new(
        Box::new(HttpTransport::new(&server.url).expect("transport")),
        relocal.identity,
        c.device_id.clone(),
    );
    c.engine
        .sync(&c.conn, &c.store, now + 11)
        .expect("C catch-up round");
    assert_eq!(
        c.snippet("snip-r").expect("row on C").title,
        "Survivor",
        "pre-recovery data must arrive on the recovered device"
    );
    assert!(
        SyncStateRepo::new(&c.conn)
            .config_get()
            .expect("config")
            .recovery_root
            .is_none(),
        "a drained catch-up clears the marker"
    );
    // The vault survived and unlocks under the new master password.
    let mut c_vault = VaultSession::new();
    c_vault
        .unlock_with_password(&c.conn, b"FAKE_gamma_master", now + 12)
        .expect("unlock recovered vault");

    // The old device is revoked by the re-root: its next sync is refused
    // at the door.
    let err = a
        .engine
        .sync(&a.conn, &a.store, now + 13)
        .expect_err("old device must be refused");
    assert!(
        matches!(
            &err,
            SyncError::Transport(TransportError::Api { code, .. }) if code == "DEVICE_REVOKED"
        ),
        "unexpected refusal: {err:?}"
    );
    // ...and it never receives the rotated generation: new records stay
    // dark to it.
    assert!(
        a.store
            .retrieve("sync.k_sync.2")
            .expect("retrieve")
            .is_none(),
        "the revoked device must not hold the rotated key"
    );

    // The recovered device continues normally.
    {
        let _guard = c.attach_observer();
        c.write_snippet(
            &snippet("snip-after", "After", "post-recovery", now + 14),
            now + 14,
        );
    }
    let report = c.engine.sync(&c.conn, &c.store, now + 15).expect("C push");
    assert_eq!(report.pushed, 1);
}

/// Revocation + rotation: the revoked device is
/// refused, the rotation reaches the surviving device through a sealed
/// key update inside its normal sync, new records ride the new
/// generation, and the revoked device provably lacks it.
#[test]
fn revocation_locks_out_the_device_and_rotation_reaches_survivors() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, account_id) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_230_000;

    let mut c = Instance::new(&dir, "gamma", &server.url);
    pair_via_protocol(&server, &mut a, &mut c, &account_id, false, None, None, now);

    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("snip-base", "Base", "before revocation", now + 1),
            now + 1,
        );
    }
    a.engine.sync(&a.conn, &a.store, now + 2).expect("A push");
    b.engine.sync(&b.conn, &b.store, now + 3).expect("B pull");
    c.engine.sync(&c.conn, &c.store, now + 4).expect("C pull");

    // A revokes B; the forced rotation mints generation 2 and seals it to
    // the survivor C.
    let new_generation = a
        .engine
        .revoke_device(&a.conn, &a.store, &b.device_id, None, now + 5)
        .expect("revoke B");
    assert_eq!(new_generation, 2);

    // B is refused at the transport door (challenge gate).
    let err = b
        .engine
        .sync(&b.conn, &b.store, now + 6)
        .expect_err("revoked device must be refused");
    assert!(
        matches!(
            &err,
            SyncError::Transport(TransportError::Api { code, .. }) if code == "DEVICE_REVOKED"
        ),
        "unexpected refusal: {err:?}"
    );

    // A's next records seal under the new generation.
    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("snip-rotated", "Rotated", "post-rotation", now + 7),
            now + 7,
        );
    }
    a.engine.sync(&a.conn, &a.store, now + 8).expect("A push");
    {
        let conn = Connection::open(&server.db_path).expect("open server db");
        conn.busy_timeout(Duration::from_secs(5)).expect("timeout");
        let key_id: i64 = conn
            .query_row(
                "SELECT key_id FROM record WHERE entity_id = 'snip-rotated'",
                [],
                |row| row.get(0),
            )
            .expect("record key_id");
        assert_eq!(key_id, 2, "new records must ride the rotated generation");
    }

    // C's normal sync applies the sealed key update first, then the new
    // record — no manual key handling.
    let report = c.engine.sync(&c.conn, &c.store, now + 9).expect("C sync");
    assert!(report.applied >= 1);
    assert_eq!(
        c.snippet("snip-rotated").expect("row on C").title,
        "Rotated"
    );
    assert!(
        c.store
            .retrieve("sync.k_sync.2")
            .expect("retrieve")
            .is_some(),
        "the survivor received generation 2 through the key update"
    );
    // The revoked device is blind to the rotation: it holds no such key.
    assert!(
        b.store
            .retrieve("sync.k_sync.2")
            .expect("retrieve")
            .is_none(),
        "the revoked device must never receive the new generation"
    );
}

/// Pairing red lines and malicious paths: no raw key bytes on the server
/// at the byte level, no offer without the trusted side's confirmation, a
/// dead session answers NOT_FOUND, and a forged bundle signature is
/// refused with nothing installed.
#[test]
fn pairing_red_lines_hold_and_malicious_paths_are_refused() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let now = 1_700_000_240_000;

    let mut a = Instance::new(&dir, "alpha", &server.url);
    let mk = SymmetricKey::generate();
    let k_vault = init_vault(&a.conn, &mk, b"FAKE_alpha_master", now);
    let account_id = a
        .engine
        .create_account(
            &a.conn,
            &a.store,
            &typvia_sync::NewAccount {
                device_name: "E2E first device",
                platform: Platform::Macos,
                server_url: &server.url,
                master_key: Some(&mk),
            },
            now + 1,
        )
        .expect("create account");
    let mut b = Instance::new(&dir, "beta", &server.url);
    pair_via_protocol(
        &server,
        &mut a,
        &mut b,
        &account_id,
        true,
        Some(&mk),
        Some(b"FAKE_beta_master"),
        now + 2,
    );

    // Red line: the sealed bundle transited the server, but none of the
    // raw 32-byte keys exist in the server database at the byte level.
    let k_sync_raw = a
        .store
        .retrieve("sync.k_sync.1")
        .expect("retrieve")
        .expect("k_sync present");
    let server_bytes = db_bytes(&server.db_path);
    assert!(
        !contains(&server_bytes, mk.expose()),
        "raw MK bytes leaked into the server database"
    );
    assert!(
        !contains(&server_bytes, k_vault.expose()),
        "raw K_vault bytes leaked into the server database"
    );
    assert!(
        !contains(&server_bytes, &k_sync_raw),
        "raw K_sync bytes leaked into the server database"
    );
    // Scan self-check: public key material of the paired device does rest
    // in the server database (device row), so the scan sees real bytes.
    assert!(contains(&server_bytes, &b.identity().ed25519_public()));

    // Bad-SAS semantics: the trusted side never confirms, so no offer is
    // posted and the new device stays pending (the flow cannot be forced
    // from the new-device side).
    let mut c = Instance::new(&dir, "gamma", &server.url);
    let handle = c
        .engine
        .begin_pairing(&c.conn, &server.url, &account_id)
        .expect("begin pairing");
    assert!(
        c.engine.poll_pairing(&handle).expect("poll").is_none(),
        "no offer may exist before the trusted side confirms"
    );

    // A dead/unknown session is a uniform NOT_FOUND.
    let transport = HttpTransport::new(&server.url).expect("transport");
    let err = transport
        .pair_claim("00000000-0000-4000-8000-000000000000")
        .expect_err("unknown session must be refused");
    assert!(matches!(
        err,
        TransportError::Api { ref code, status: 404 } if code == "NOT_FOUND"
    ));

    // A forged offer (valid certificate, garbage sealed bundle and
    // signature) is refused by the new device with nothing installed.
    let code = PairingCode::decode(handle.code()).expect("decode code");
    let certificate = DeviceCertificate::issue(
        &a.identity(),
        &a.device_id,
        CertificateSubject {
            device_id: code.device_id.clone(),
            ed25519_pub: code.ed25519_pub,
            x25519_pub: code.x25519_pub,
            name: code.device_name.clone(),
            platform: code.platform,
            created_at: now + 3,
        },
        now + 3,
    )
    .expect("issue certificate");
    let a_identity = a.identity();
    let challenge = transport
        .auth_challenge(&a.device_id)
        .expect("auth challenge");
    let mut message = b"typvia.auth.v1".to_vec();
    message.extend_from_slice(&challenge);
    message.extend_from_slice(a.device_id.as_bytes());
    let signature = a_identity.sign(&message).to_bytes();
    let token = transport
        .auth_session(&a.device_id, &signature)
        .expect("auth session");
    transport
        .pair_offer(
            &token,
            handle.session_id(),
            &certificate,
            b"garbage sealed bytes that are long enough to parse",
            &[0u8; 64],
        )
        .expect("the server relays; the client is the verification authority");
    let err = c
        .engine
        .poll_pairing(&handle)
        .expect_err("forged bundle signature must be refused");
    assert!(
        matches!(
            err,
            SyncError::Pairing(typvia_sync::PairingError::BadSignature)
        ),
        "unexpected error: {err:?}"
    );
    // Nothing was installed on the refusing device.
    assert!(
        c.store
            .retrieve("sync.k_sync.1")
            .expect("retrieve")
            .is_none()
    );
    assert!(
        !SyncStateRepo::new(&c.conn)
            .config_get()
            .expect("config")
            .enabled
    );
}

/// Sync off detaches the observer, so writes made in that window queue
/// nothing. Re-enabling must catch them up — creations,
/// edits and deletions alike — or the two devices diverge silently.
#[test]
fn writes_made_while_sync_was_off_catch_up_on_resume() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_030_000;

    // Both devices agree on two snippets first.
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-keep", "Keep", "body keep", now), now);
        a.write_snippet(&snippet("snip-gone", "Gone", "body gone", now), now);
    }
    a.engine.sync(&a.conn, &a.store, now + 1).expect("A push");
    b.engine.sync(&b.conn, &b.store, now + 2).expect("B pull");
    assert_eq!(b.snippet("snip-keep").expect("row").title, "Keep");

    // Sync off: the switch flips and the observer goes away.
    a.engine.disable(&a.conn, now + 3).expect("disable");
    let mut edited = snippet("snip-keep", "Keep (edited offline)", "body keep", now + 4);
    edited.version = 2;
    a.write_snippet(&edited, now + 4);
    a.write_snippet(
        &snippet("snip-new", "New", "written while off", now + 5),
        now + 5,
    );
    a.trash_snippet("snip-gone", now + 6);
    // The reported backlog is honest and empty — that is exactly why the
    // catch-up has to exist.
    assert_eq!(a.pending_count(), 0);

    a.enable_sync(now + 7);
    let guard = a.attach_observer();
    assert_eq!(a.reconcile(now + 7), 3, "edit + creation + deletion");
    // Running it again before anything is pushed queues nothing: the
    // comparison basis is what the queue would already deliver.
    assert_eq!(a.reconcile(now + 8), 0);
    assert_eq!(a.pending_count(), 3);

    let report = a.engine.sync(&a.conn, &a.store, now + 9).expect("A push");
    assert_eq!(report.pushed, 3);
    // After the push the shadow bases moved; still nothing to catch up.
    assert_eq!(a.reconcile(now + 10), 0);
    drop(guard);

    let report = b.engine.sync(&b.conn, &b.store, now + 11).expect("B pull");
    assert_eq!(report.applied, 3);
    assert_eq!(
        b.snippet("snip-keep").expect("row").title,
        "Keep (edited offline)"
    );
    assert_eq!(
        b.snippet("snip-new").expect("row").content,
        SnippetContent::Plaintext("written while off".to_string())
    );
    assert_eq!(
        b.snippet("snip-gone").expect("row").deleted_at,
        Some(now + 6),
        "the deletion made while sync was off must travel as a tombstone"
    );
}

/// A consumed key update advances the persisted cursor, and the next pull
/// — which sends that cursor as its acknowledgment — makes the relay drop
/// the message instead of replaying it forever.
#[test]
fn a_consumed_key_update_advances_the_cursor_and_empties_the_relay() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let mut b = b;
    let now = 1_700_000_060_000;

    assert_eq!(b.key_update_cursor(), 0);
    let new_key_id = a
        .engine
        .rotate_sync_key(&a.conn, &a.store, None, now)
        .expect("rotate");
    assert_eq!(relayed_key_updates(&server, &b.device_id), 1);

    // The round installs the new generation and acknowledges the message.
    b.engine.sync(&b.conn, &b.store, now + 1).expect("B round");
    assert_eq!(
        SyncStateRepo::new(&b.conn)
            .config_get()
            .expect("config")
            .sync_key_id,
        new_key_id
    );
    assert_eq!(b.key_update_cursor(), 1);

    // The next pull carries the acknowledgment, so the relay is emptied.
    b.engine.sync(&b.conn, &b.store, now + 2).expect("B round");
    assert_eq!(relayed_key_updates(&server, &b.device_id), 0);
    assert_eq!(b.key_update_cursor(), 1);
}

/// A catch-up that fails part-way leaves the queue exactly as it was: no
/// half-reconciled state, and a retry does the whole job.
#[test]
fn a_failed_catch_up_leaves_the_queue_untouched() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, _b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let now = 1_700_000_050_000;

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-one", "One", "body one", now), now);
    }
    a.engine.sync(&a.conn, &a.store, now + 1).expect("A push");

    a.engine.disable(&a.conn, now + 2).expect("disable");
    a.write_snippet(&snippet("snip-two", "Two", "body two", now + 3), now + 3);
    a.write_snippet(
        &snippet("snip-three", "Three", "body three", now + 4),
        now + 4,
    );
    assert_eq!(a.pending_count(), 0);

    // Fault injection: the queue rejects the second insert of the walk, so
    // the first one must not survive either.
    a.conn
        .execute_batch(
            "CREATE TRIGGER e2e_fail_second BEFORE INSERT ON sync_record \
             WHEN (SELECT COUNT(*) FROM sync_record WHERE state = 'pending') >= 1 \
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        )
        .expect("install trigger");
    a.enable_sync(now + 5);
    let _guard = a.attach_observer();
    let config = SyncStateRepo::new(&a.conn).config_get().expect("config");
    let keys = SyncKeys::load(&a.store, config.sync_key_id).expect("keys");
    let err = a
        .engine
        .reconcile(&a.conn, &keys, now + 5)
        .expect_err("injected failure");
    assert!(
        matches!(err, SyncError::Repo(_)),
        "unexpected error: {err:?}"
    );
    assert_eq!(a.pending_count(), 0, "nothing may be left half queued");

    a.conn
        .execute_batch("DROP TRIGGER e2e_fail_second")
        .expect("drop trigger");
    assert_eq!(a.reconcile(now + 6), 2);
}

/// The catch-up costs nothing when nothing changed: a library that sat
/// untouched while sync was off queues zero records, and a device with an
/// offline backlog does not re-queue what it is already sending.
#[test]
fn a_quiet_library_queues_nothing_when_sync_resumes() {
    let dir = tempdir();
    let server = TestServer::start(&dir);
    let (a, _b, _) = paired_instances(&dir, &server);
    let mut a = a;
    let now = 1_700_000_040_000;

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("snip-quiet", "Quiet", "body quiet", now), now);
        a.write_snippet(&snippet("snip-quiet-2", "Quiet two", "body two", now), now);
    }
    a.engine.sync(&a.conn, &a.store, now + 1).expect("A push");

    // Off and straight back on, nothing touched in between.
    a.engine.disable(&a.conn, now + 2).expect("disable");
    a.enable_sync(now + 3);
    let guard = a.attach_observer();
    assert_eq!(a.reconcile(now + 3), 0);
    assert_eq!(a.pending_count(), 0);

    // A queued-but-unpushed record is what the server will see next, so
    // the catch-up leaves it alone instead of queueing a duplicate.
    let mut edited = snippet("snip-quiet", "Quiet (edited)", "body quiet", now + 4);
    edited.version = 2;
    a.write_snippet(&edited, now + 4);
    drop(guard);
    assert_eq!(a.pending_count(), 1);
    a.engine.disable(&a.conn, now + 5).expect("disable");
    a.enable_sync(now + 6);
    let _guard = a.attach_observer();
    assert_eq!(a.reconcile(now + 6), 0);
    assert_eq!(a.pending_count(), 1);
}

// ----- helpers ------------------------------------------------------------

/// Key-update messages the relay still holds for one device.
fn relayed_key_updates(server: &TestServer, device_id: &str) -> i64 {
    let conn = Connection::open(&server.db_path).expect("open server db");
    conn.busy_timeout(Duration::from_secs(5)).expect("timeout");
    conn.query_row(
        "SELECT COUNT(*) FROM key_update WHERE target_device_id = ?1",
        [device_id],
        |row| row.get(0),
    )
    .expect("count key updates")
}

fn db_bytes(path: &Path) -> Vec<u8> {
    let mut bytes = std::fs::read(path).expect("read db file");
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{}", path.display(), suffix));
        if let Ok(more) = std::fs::read(side) {
            bytes.extend(more);
        }
    }
    bytes
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

fn tempdir() -> PathBuf {
    // A process-wide counter keeps concurrent tests in distinct
    // directories even when they start within the same clock tick.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "e2e-{}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}
