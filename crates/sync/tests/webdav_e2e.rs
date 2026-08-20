//! End-to-end WebDAV sync: two real client databases against the
//! in-process WebDAV test server — foundation, convergence, offline
//! backlog, the concurrent-edit rules (single conflict copy account-wide),
//! and restart resumability. No Go server anywhere.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use common::WebdavServer;
use rusqlite::Connection;
use typvia_core::model::{DomainKey, KeyDomain, VaultKeyHeader};
use typvia_core::model::{
    Platform, SecurityLevel, Snippet, SnippetContent, SnippetType, SyncEntityType, TimestampMs,
};
use typvia_core::repo::{SnippetRepo, SyncStateRepo};
use typvia_core::repo::{VaultKeyRepo, new_id};
use typvia_core::sync_hooks::{self, EntityChange};
use typvia_crypto::{
    KdfParams, SymmetricKey, aad_domain_key, aad_master_key, derive_kek, wrap_key,
};
use typvia_crypto::{SecureStore, SecureStoreError};
use typvia_sync::{
    AdoptedAccount, CertificateSubject, DeviceCertificate, LocalDeviceConfig, NewAccount,
    OutboxSealer, PairingCode, SyncEngine, SyncKeys, WebdavClient, WebdavStore,
    ensure_local_device,
};

const NOW: TimestampMs = 1_700_000_300_000;

// ----- minimal harness (mirrors e2e_sync.rs, WebDAV form) -------------------

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

    /// A fresh identity handle for certificate issuing (the engine owns the
    /// first one); loading twice is idempotent per the secure store.
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

    fn snippet(&self, id: &str) -> Option<Snippet> {
        SnippetRepo::new(&self.conn).get(id).expect("get")
    }

    fn conflict_copy_count(&self) -> usize {
        SnippetRepo::new(&self.conn)
            .list_conflict_copies()
            .expect("copies")
            .len()
    }

    fn sync(&mut self, dav: &WebdavStore, now: i64) -> typvia_sync::SyncReport {
        self.engine
            .sync_webdav(&self.conn, &self.store, dav, now)
            .expect("webdav round")
    }
}

fn store_for(server: &WebdavServer) -> WebdavStore {
    WebdavStore::new(WebdavClient::new(&server.url, None).expect("client"))
}

fn snippet(id: &str, title: &str, body: &str, now: i64) -> Snippet {
    Snippet {
        id: id.to_string(),
        workspace_id: "ws-default".to_string(),
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
        platform_scope: vec![],
        created_at: now,
        updated_at: now,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

/// Founds the account on A and joins B the way pairing does:
/// K_sync copied, root pinned, B's certificate appended to the directory by
/// A (the root device).
fn found_and_join(dir: &Path, server: &WebdavServer) -> (Instance, Instance, WebdavStore, String) {
    let dav = store_for(server);
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
        .expect("found account");

    let mut b = Instance::new(dir, "beta");
    // Pairing stand-in: copy the K_sync generation and pin A's root.
    let key = a
        .store
        .retrieve("sync.k_sync.1")
        .expect("k_sync")
        .expect("generation 1");
    b.store.store("sync.k_sync.1", &key).expect("install key");
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
    .expect("issue cert");
    a.engine
        .webdav_admit_device(&dav, std::slice::from_ref(&cert))
        .expect("admit beta");

    let root_fingerprint = SyncStateRepo::new(&a.conn)
        .config_get()
        .expect("config")
        .root_fingerprint
        .expect("fingerprint");
    b.engine
        .adopt_webdav_account(
            &b.conn,
            &b.store,
            &AdoptedAccount {
                server_url: server.url.clone(),
                account_id: account_id.clone(),
                root_fingerprint: root_fingerprint.try_into().expect("32 bytes"),
                sync_key_id: 1,
            },
            NOW,
        )
        .expect("adopt");
    (a, b, dav, account_id)
}

fn tempdir() -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "webdav-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

// ----- scenarios ------------------------------------------------------------

#[test]
fn two_devices_converge_including_offline_backlog() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-1", "From alpha", "hello", NOW + 1), NOW + 1);
        // Offline backlog: several writes before any round.
        a.write_snippet(&snippet("s-2", "Backlog two", "b2", NOW + 2), NOW + 2);
        a.write_snippet(&snippet("s-3", "Backlog three", "b3", NOW + 3), NOW + 3);
    }
    let pushed = a.sync(&dav, NOW + 4);
    assert_eq!(pushed.pushed, 3);

    let pulled = b.sync(&dav, NOW + 5);
    assert_eq!(pulled.applied, 3);
    assert_eq!(b.snippet("s-1").expect("s-1 on B").title, "From alpha");
    assert_eq!(b.snippet("s-3").expect("s-3 on B").title, "Backlog three");

    // The other direction.
    {
        let _guard = b.attach_observer();
        b.write_snippet(&snippet("s-4", "From beta", "back", NOW + 6), NOW + 6);
    }
    b.sync(&dav, NOW + 7);
    a.sync(&dav, NOW + 8);
    assert_eq!(a.snippet("s-4").expect("s-4 on A").title, "From beta");

    // Idle rounds move nothing.
    let idle = a.sync(&dav, NOW + 9);
    assert_eq!((idle.pushed, idle.applied), (0, 0));
}

#[test]
fn concurrent_edits_converge_with_exactly_one_conflict_copy() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    // A creates and both sides sync to the same base.
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-c", "Base", "base body", NOW + 1), NOW + 1);
    }
    a.sync(&dav, NOW + 2);
    b.sync(&dav, NOW + 3);

    // Concurrent divergent edits of the same body on both sides.
    {
        let _guard = a.attach_observer();
        let mut edited = a.snippet("s-c").expect("on A");
        edited.content = SnippetContent::Plaintext("alpha version".to_string());
        edited.updated_at = NOW + 10;
        a.write_snippet(&edited, NOW + 10);
    }
    {
        let _guard = b.attach_observer();
        let mut edited = b.snippet("s-c").expect("on B");
        edited.content = SnippetContent::Plaintext("beta version".to_string());
        edited.updated_at = NOW + 11;
        b.write_snippet(&edited, NOW + 11);
    }

    // Both publish, both pull-and-merge, then settle: convergence happens
    // in bounded rounds, and the identical-content rule collapses the
    // double merge.
    a.sync(&dav, NOW + 20);
    b.sync(&dav, NOW + 21);
    a.sync(&dav, NOW + 22);
    b.sync(&dav, NOW + 23);
    a.sync(&dav, NOW + 24);
    b.sync(&dav, NOW + 25);

    let head_a = a.snippet("s-c").expect("head on A");
    let head_b = b.snippet("s-c").expect("head on B");
    assert_eq!(head_a.content, head_b.content, "heads must converge");

    // Exactly one conflict copy account-wide (elected side only), and both
    // sides see it after settling.
    let copies_a = a.conflict_copy_count();
    let copies_b = b.conflict_copy_count();
    assert_eq!(
        (copies_a, copies_b),
        (1, 1),
        "one elected conflict copy, visible on both"
    );
}

#[test]
fn a_restarted_engine_resumes_from_persisted_cursors() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-r", "Persist me", "body", NOW + 1), NOW + 1);
    }
    a.sync(&dav, NOW + 2);
    let first = b.sync(&dav, NOW + 3);
    assert_eq!(first.applied, 1);

    // "Restart": a fresh engine over the same database and secure store.
    let local = ensure_local_device(
        &b.conn,
        &b.store,
        &LocalDeviceConfig {
            device_id: &b.device_id,
            name: "beta",
            platform: Platform::Macos,
        },
        NOW + 4,
    )
    .expect("re-ensure");
    b.engine = SyncEngine::new_webdav(local.identity, b.device_id.clone());
    let after_restart = b.sync(&dav, NOW + 5);
    assert_eq!(
        (after_restart.applied, after_restart.skipped),
        (0, 0),
        "persisted cursors must prevent re-application"
    );

    // A rolled-back store yields a quiet round, never regression.
    let snapshot = server.state.snapshot();
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-r2", "After snapshot", "x", NOW + 6), NOW + 6);
    }
    a.sync(&dav, NOW + 7);
    b.sync(&dav, NOW + 8);
    assert!(b.snippet("s-r2").is_some());
    server.state.restore(snapshot);
    let rolled = b.sync(&dav, NOW + 9);
    assert_eq!((rolled.applied, rolled.pushed), (0, 0));
    assert!(
        b.snippet("s-r2").is_some(),
        "applied data survives a storage rollback"
    );
}

#[test]
fn rival_same_version_publishes_resolve_deterministically_with_one_copy() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    // Shared base on both sides.
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-c", "Base", "base body", NOW + 1), NOW + 1);
    }
    a.sync(&dav, NOW + 2);
    b.sync(&dav, NOW + 3);

    // A edits and publishes v2.
    {
        let _guard = a.attach_observer();
        let mut edited = a.snippet("s-c").expect("on A");
        edited.content = SnippetContent::Plaintext("alpha version".to_string());
        edited.updated_at = NOW + 10;
        a.write_snippet(&edited, NOW + 10);
    }
    a.sync(&dav, NOW + 10);

    // B edits and publishes its own v2 while A's stream is invisible to it
    // (the race window: pull saw nothing, push went out).
    {
        let _guard = b.attach_observer();
        let mut edited = b.snippet("s-c").expect("on B");
        edited.content = SnippetContent::Plaintext("beta version".to_string());
        edited.updated_at = NOW + 11;
        b.write_snippet(&edited, NOW + 11);
    }
    *server.state.hidden_prefix.lock().unwrap() =
        Some("typvia-sync/devices/dav-alpha-device".to_string());
    b.sync(&dav, NOW + 11);
    *server.state.hidden_prefix.lock().unwrap() = None;

    // Both settle. Deterministic winner: B (higher updated_at); the loser A
    // preserves "alpha version" as the single conflict copy.
    a.sync(&dav, NOW + 20);
    b.sync(&dav, NOW + 21);
    a.sync(&dav, NOW + 22);
    b.sync(&dav, NOW + 23);

    let head_a = a.snippet("s-c").expect("head on A");
    let head_b = b.snippet("s-c").expect("head on B");
    assert_eq!(head_a.content, head_b.content, "heads must converge");
    assert_eq!(
        head_a.content,
        SnippetContent::Plaintext("beta version".to_string()),
        "the (updated_at, device_id) winner takes the head"
    );
    assert_eq!(
        (a.conflict_copy_count(), b.conflict_copy_count()),
        (1, 1),
        "the loser's body survives as exactly one copy account-wide"
    );
    let copy = SnippetRepo::new(&a.conn)
        .list_conflict_copies()
        .expect("copies")
        .pop()
        .expect("copy on A");
    assert_eq!(
        copy.content,
        SnippetContent::Plaintext("alpha version".to_string()),
        "the copy carries the losing body"
    );
}

#[test]
fn rotation_travels_by_mailbox_and_revocation_blinds_the_old_device() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    // A rotates: generation 2 lands in B's mailbox on the store.
    let rotated = a
        .engine
        .rotate_sync_key_webdav(&a.conn, &a.store, &dav, None, NOW + 10)
        .expect("rotate");
    assert_eq!(rotated, 2);
    let mailbox_files = server
        .state
        .snapshot()
        .into_keys()
        .filter(|k| {
            k.starts_with("typvia-sync/keyupdates/dav-beta-device/") && !k.ends_with("head.json")
        })
        .count();
    assert_eq!(mailbox_files, 1, "one sealed update waits for B");

    // A writes under the new generation; B's round consumes the mailbox
    // first and can open the record immediately.
    {
        let _guard = a.attach_observer();
        a.write_snippet(
            &snippet("s-rot", "Post rotation", "gen2", NOW + 11),
            NOW + 11,
        );
    }
    a.sync(&dav, NOW + 12);
    let pulled = b.sync(&dav, NOW + 13);
    assert_eq!(pulled.applied, 1);
    assert!(b.snippet("s-rot").is_some());
    assert!(
        b.store
            .retrieve("sync.k_sync.2")
            .expect("retrieve")
            .is_some(),
        "generation 2 must be installed on B"
    );
    let leftover = server
        .state
        .snapshot()
        .into_keys()
        .filter(|k| {
            k.starts_with("typvia-sync/keyupdates/dav-beta-device/") && !k.ends_with("head.json")
        })
        .count();
    assert_eq!(leftover, 0, "consumed updates are deleted by the receiver");

    // A revokes B: directory marks it, rotation to generation 3 is
    // distributed to no one (everyone else is revoked).
    let after_revoke = a
        .engine
        .revoke_device_webdav(&a.conn, &a.store, &dav, &b.device_id, None, NOW + 20)
        .expect("revoke");
    assert_eq!(after_revoke, 3);

    // A's new records are dark to B (an unknown generation parks)…
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-dark", "Dark to B", "gen3", NOW + 21), NOW + 21);
    }
    a.sync(&dav, NOW + 22);
    let blind = b.sync(&dav, NOW + 23);
    assert!(
        blind.parked >= 1,
        "gen-3 record must park on the revoked device"
    );
    assert!(b.snippet("s-dark").is_none());

    // …and B's post-revocation writes are refused by A (revocation cut-off).
    {
        let _guard = b.attach_observer();
        b.write_snippet(&snippet("s-late", "Too late", "x", NOW + 30), NOW + 30);
    }
    b.sync(&dav, NOW + 31);
    let refused = a.sync(&dav, NOW + 32);
    assert_eq!(refused.applied, 0);
    assert!(a.snippet("s-late").is_none(), "revoked writes never apply");
}

/// Creates a vault with a known MK (mirror of the e2e_sync helper) so
/// recovery flows can publish and re-wrap it.
fn init_vault(conn: &Connection, mk: &SymmetricKey, password: &[u8], now: i64) {
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
}

#[test]
fn real_pairing_over_the_mailbox_admits_and_converges() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let dav = store_for(&server);
    let mut a = Instance::new(&dir, "alpha");
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
        .expect("found account");
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-p", "Paired data", "hello", NOW + 1), NOW + 1);
    }
    a.sync(&dav, NOW + 2);

    // New device C: begin → code → approve (mailbox) → poll → finalize.
    let mut c = Instance::new(&dir, "gamma");
    let handle = c
        .engine
        .begin_pairing_webdav(&c.conn, &server.url, &account_id)
        .expect("begin");
    let code = PairingCode::decode(handle.code()).expect("decode code");
    assert!(
        c.engine
            .poll_pairing_webdav(&dav, &handle)
            .expect("poll")
            .is_none(),
        "no answer before the approval"
    );
    a.engine
        .approve_pairing_webdav(&a.conn, &a.store, &dav, &code, false, None, NOW + 3)
        .expect("approve");
    // A second approval of the same code is refused (single-use).
    assert!(
        a.engine
            .approve_pairing_webdav(&a.conn, &a.store, &dav, &code, false, None, NOW + 3)
            .is_err()
    );
    let claimed = c
        .engine
        .poll_pairing_webdav(&dav, &handle)
        .expect("poll")
        .expect("answer ready");
    c.engine
        .finalize_pairing_webdav(&c.conn, &c.store, &handle, &claimed, None, NOW + 4)
        .expect("finalize");

    let pulled = c.sync(&dav, NOW + 5);
    assert_eq!(pulled.applied, 1);
    assert_eq!(c.snippet("s-p").expect("paired data").title, "Paired data");

    // And the paired device syncs back.
    {
        let _guard = c.attach_observer();
        c.write_snippet(&snippet("s-back", "From gamma", "x", NOW + 6), NOW + 6);
    }
    c.sync(&dav, NOW + 7);
    a.sync(&dav, NOW + 8);
    assert!(a.snippet("s-back").is_some());
}

#[test]
fn recovery_re_roots_the_store_and_locks_out_old_devices() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    let mk = SymmetricKey::generate();
    init_vault(&a.conn, &mk, b"FAKE_alpha_master", NOW);
    {
        let _guard = a.attach_observer();
        a.write_snippet(&snippet("s-h", "History", "survives", NOW + 1), NOW + 1);
    }
    a.sync(&dav, NOW + 2);
    b.sync(&dav, NOW + 3);

    let code = a
        .engine
        .publish_recovery_webdav(&a.conn, &a.store, &dav, &mk)
        .expect("publish recovery");

    // All devices lost: D recovers with the code alone.
    let mut d = Instance::new(&dir, "delta");
    let adopted = d
        .engine
        .recover_account_webdav(
            &d.conn,
            &d.store,
            &dav,
            &code,
            b"FAKE_delta_master",
            &server.url,
            NOW + 10,
        )
        .expect("recover");
    assert_eq!(
        adopted.sync_key_id, 2,
        "recovery must force a K_sync rotation"
    );

    // The catch-up marker is set; the first round pulls the history under
    // the predecessor root and clears it.
    assert!(
        SyncStateRepo::new(&d.conn)
            .config_get()
            .expect("config")
            .recovery_root
            .is_some()
    );
    d.sync(&dav, NOW + 11);
    assert_eq!(d.snippet("s-h").expect("history on D").title, "History");
    assert!(
        SyncStateRepo::new(&d.conn)
            .config_get()
            .expect("config")
            .recovery_root
            .is_none(),
        "a drained catch-up clears the marker"
    );

    // The old devices self-refuse on the pinned root.
    assert!(
        a.engine
            .sync_webdav(&a.conn, &a.store, &dav, NOW + 12)
            .is_err()
    );
    assert!(
        b.engine
            .sync_webdav(&b.conn, &b.store, &dav, NOW + 13)
            .is_err()
    );

    // The recovered vault opens under the new master password.
    let mut vault = typvia_core::vault::VaultSession::new();
    vault
        .unlock_with_password(&d.conn, b"FAKE_delta_master", NOW + 14)
        .expect("unlock recovered vault");
}

#[test]
fn a_vault_grant_travels_sealed_and_is_adopted_with_the_new_password() {
    let dir = tempdir();
    let server = WebdavServer::start();
    let (mut a, mut b, dav, _) = found_and_join(&dir, &server);

    // A has the vault; B was paired without it (found_and_join carries no
    // vault material).
    let mk = SymmetricKey::generate();
    init_vault(&a.conn, &mk, b"FAKE_alpha_master", NOW);

    // Nothing to adopt yet.
    assert!(
        !b.engine
            .adopt_vault_grant_webdav(&b.conn, &b.store, &dav, b"FAKE_beta_master", NOW + 1)
            .expect("scan")
    );

    a.engine
        .grant_vault_access_webdav(&a.conn, &a.store, &dav, &b.device_id, &mk, NOW + 2)
        .expect("grant");

    // The grant carries the MK — but only sealed: the raw MK bytes never
    // appear anywhere in the store.
    let mut all = Vec::new();
    for (_, bytes) in server.state.snapshot() {
        all.extend_from_slice(&bytes);
    }
    let mk_bytes = mk.expose().to_vec();
    assert!(
        !all.windows(mk_bytes.len()).any(|w| w == mk_bytes),
        "MK plaintext leaked into the store"
    );

    // B adopts under its own master password and can unlock.
    assert!(
        b.engine
            .adopt_vault_grant_webdav(&b.conn, &b.store, &dav, b"FAKE_beta_master", NOW + 3)
            .expect("adopt")
    );
    let mut vault = typvia_core::vault::VaultSession::new();
    vault
        .unlock_with_password(&b.conn, b"FAKE_beta_master", NOW + 4)
        .expect("unlock granted vault");

    // The adopted message was consumed (receiver-as-acknowledgment).
    let leftover = server
        .state
        .snapshot()
        .into_keys()
        .filter(|k| {
            k.starts_with("typvia-sync/keyupdates/dav-beta-device/") && !k.ends_with("head.json")
        })
        .count();
    assert_eq!(leftover, 0, "the adopted grant is deleted from the mailbox");
}
