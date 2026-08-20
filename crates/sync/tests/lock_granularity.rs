//! While a sync round is waiting on the network, the host's database mutex
//! is free — reads elsewhere are not blocked.
//!
//! The engine runs on a second thread against a gated transport that
//! signals when the round has entered network I/O and then parks until
//! released. In that window the main thread must be able to take the
//! connection mutex and run a query; before this refactor the round held
//! the guard for its whole duration and this test deadlocks.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::Platform;
use typvia_core::repo::SyncStateRepo;
use typvia_crypto::{SecureStore, SecureStoreError, SymmetricKey};
use typvia_sync::{
    DeviceIdentity, LocalDeviceConfig, SyncEngine, SyncError, SyncTransport, TransportError,
    ensure_local_device, install_key,
};

/// Thread-safe secure-store double (the e2e one is RefCell-based and
/// deliberately single-threaded).
struct SharedStore {
    entries: Mutex<HashMap<String, Vec<u8>>>,
}

impl SharedStore {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl SecureStore for SharedStore {
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
        self.entries
            .lock()
            .unwrap()
            .insert(entry.to_string(), secret.to_vec());
        Ok(())
    }

    fn retrieve(
        &self,
        entry: &str,
    ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, SecureStoreError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(entry)
            .cloned()
            .map(zeroize::Zeroizing::new))
    }

    fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
        self.entries.lock().unwrap().remove(entry);
        Ok(())
    }
}

fn network() -> TransportError {
    TransportError::Network("gated".to_string())
}

/// Signals `entered` on the first network call of the round, then parks
/// until `release` fires, then fails as a plain network error so the round
/// ends without needing a server.
struct GateTransport {
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
}

impl SyncTransport for GateTransport {
    fn handshake(&self) -> Result<typvia_sync::HandshakeInfo, TransportError> {
        Err(network())
    }
    fn create_account(&self, _root: &typvia_sync::RootStatement) -> Result<String, TransportError> {
        Err(network())
    }
    fn auth_challenge(&self, _device_id: &str) -> Result<Vec<u8>, TransportError> {
        // The engine reaches here with no database guard held; prove it by
        // letting the main thread run a query before this call returns.
        let _ = self.entered.send(());
        let _ = self
            .release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(20));
        Err(network())
    }
    fn auth_session(
        &self,
        _device_id: &str,
        _signature: &[u8; 64],
    ) -> Result<typvia_sync::SessionToken, TransportError> {
        Err(network())
    }
    fn push_records(
        &self,
        _token: &typvia_sync::SessionToken,
        _records: &[typvia_sync::WireRecord],
    ) -> Result<typvia_sync::PushOutcome, TransportError> {
        Err(network())
    }
    fn pull_records(
        &self,
        _token: &typvia_sync::SessionToken,
        _since: u64,
        _limit: u32,
    ) -> Result<typvia_sync::PullPage, TransportError> {
        Err(network())
    }
    fn device_directory(
        &self,
        _token: &typvia_sync::SessionToken,
    ) -> Result<typvia_sync::DeviceDirectory, TransportError> {
        Err(network())
    }
    fn revoke_device(
        &self,
        _token: &typvia_sync::SessionToken,
        _device_id: &str,
        _revoked_at: i64,
        _signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        Err(network())
    }
    fn pair_begin(&self, _account_id: &str) -> Result<typvia_sync::PairingSession, TransportError> {
        Err(network())
    }
    fn pair_offer(
        &self,
        _token: &typvia_sync::SessionToken,
        _session_id: &str,
        _certificate: &typvia_sync::DeviceCertificate,
        _sealed_bundle: &[u8],
        _bundle_signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        Err(network())
    }
    fn pair_claim(&self, _session_id: &str) -> Result<typvia_sync::PairClaim, TransportError> {
        Err(network())
    }
    fn put_key_update(
        &self,
        _token: &typvia_sync::SessionToken,
        _target_device_id: &str,
        _payload: &[u8],
    ) -> Result<i64, TransportError> {
        Err(network())
    }
    fn list_key_updates(
        &self,
        _token: &typvia_sync::SessionToken,
        _since: i64,
    ) -> Result<Vec<typvia_sync::KeyUpdate>, TransportError> {
        Err(network())
    }
    fn put_recovery_blob(
        &self,
        _token: &typvia_sync::SessionToken,
        _blob: &[u8],
        _rootproof_pub: &[u8],
    ) -> Result<(), TransportError> {
        Err(network())
    }
    fn get_recovery_blob(&self, _account_id: &str) -> Result<Vec<u8>, TransportError> {
        Err(network())
    }
    fn re_root(
        &self,
        _account_id: &str,
        _proof_signature: Option<&[u8; 64]>,
        _new_root: Option<&typvia_sync::RootStatement>,
    ) -> Result<typvia_sync::ReRootOutcome, TransportError> {
        Err(network())
    }
}

#[test]
fn reads_are_not_blocked_while_a_round_waits_on_the_network() {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let store = Arc::new(SharedStore::new());

    // A configured, active sync install: device identity, one K_sync
    // generation and an account-bound config with a pinned root.
    let device_id = "device-lock-test".to_string();
    ensure_local_device(
        &conn,
        store.as_ref(),
        &LocalDeviceConfig {
            device_id: &device_id,
            name: "Lock test",
            platform: Platform::Macos,
        },
        1_000,
    )
    .unwrap();
    let identity = DeviceIdentity::load(store.as_ref()).unwrap().unwrap();
    install_key(store.as_ref(), 1, &SymmetricKey::generate()).unwrap();
    let state = SyncStateRepo::new(&conn);
    let mut config = state.config_get().unwrap();
    config.server_url = Some("https://sync.example.test".to_string());
    config.account_id = Some("acct-1".to_string());
    config.enabled = true;
    config.sync_key_id = 1;
    config.root_fingerprint = Some(vec![7u8; 32]);
    config.updated_at = 1_000;
    state.config_put(&config).unwrap();

    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut engine = SyncEngine::new(
        Box::new(GateTransport {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        }),
        identity,
        device_id,
    );

    let db = Arc::new(Mutex::new(conn));
    let round_db = Arc::clone(&db);
    let round_store = Arc::clone(&store);
    let round = std::thread::spawn(move || engine.sync(&*round_db, round_store.as_ref(), 2_000));

    // Wait until the round is provably inside network I/O…
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the round never reached the network");

    // …and take the connection mutex from this thread. A round that held
    // it for its whole duration would make this loop time out.
    let deadline = Instant::now() + Duration::from_secs(5);
    let acquired = loop {
        if let Ok(guard) = db.try_lock() {
            let snippets: i64 = guard
                .query_row("SELECT COUNT(*) FROM snippet", [], |row| row.get(0))
                .unwrap();
            assert_eq!(snippets, 0);
            break true;
        }
        if Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        acquired,
        "a read was blocked while the sync round waited on the network"
    );

    // Release the gate; the round ends as an ordinary network failure.
    release_tx.send(()).unwrap();
    let outcome = round.join().expect("round thread panicked");
    assert!(matches!(
        outcome,
        Err(SyncError::Transport(TransportError::Network(_)))
    ));
}
