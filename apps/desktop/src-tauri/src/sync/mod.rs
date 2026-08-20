//! Desktop sync host: assembles the sync engine, keeps the core write-path
//! observer registered, and maps engine failures onto stable IPC codes.
//!
//! The engine lives here rather than in `host-service` because the shared
//! layer must not depend on `crates/sync`. Everything below is
//! wiring: sessions, ordering and error translation. The protocol itself —
//! sealing, trust, pairing, merging — stays in `crates/sync`.
//!
//! Lock order across the whole host is `sync` → `conn` → `vault`;
//! every command takes its guards in that order so the three mutexes can
//! never deadlock against each other. `sync` comes first because a sync
//! round holds it for the whole push→pull cycle while taking `conn` only
//! per local phase (through [`typvia_sync::SyncDb`]) — reads elsewhere in
//! the host are never blocked on the network. A command that skips a lock
//! must still acquire the ones it uses in this relative order.
//!
//! The [`SyncScheduler`]'s cadence mutex sits strictly last and
//! leaf-only: it is taken briefly with any of the three locks held (observer
//! poke, round outcome) but nothing is ever acquired while holding it — the
//! scheduler thread returns from its wait before taking `sync`/`conn`.

mod cadence;
mod commands;
mod dto;

pub use commands::*;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use typvia_core::model::TransportKind;
use typvia_core::model::{Platform, TimestampMs};
use typvia_core::repo::{DeviceRepo, SyncOutboxRepo, SyncStateRepo};
use typvia_core::sync_hooks::{
    ChangeObserver, EntityChange, HookError, ObserverGuard, register_observer,
};
use typvia_core::vault::SecureStore;
use typvia_host_service::error::IpcError;
use typvia_sync::{
    ClaimedPairing, DeviceIdentity, HttpTransport, LocalDeviceConfig, OutboxSealer, PairingError,
    PairingHandle, RecoveryError, SyncEngine, SyncError, SyncKeys, TransportError, WebdavClient,
    WebdavCredentials, WebdavStore, ensure_local_device,
};

use cadence::Cadence;
use dto::SyncSetupState;

/// Marker file recording when this device last exported a recovery code.
/// Only a timestamp — the code itself is shown once and never persisted.
const RECOVERY_MARKER_FILE: &str = "recovery-exported";

/// Wakes the background round when it is due: the write path
/// reports queued changes, rounds report their outcome, and the scheduler
/// thread sleeps on the condvar until the [`Cadence`] says a round is due —
/// debounced pushes for local edits, exponential backoff after failures, the
/// idle poll otherwise. Times run on a private monotonic origin.
pub struct SyncScheduler {
    origin: Instant,
    cadence: Mutex<Cadence>,
    wake: Condvar,
}

impl SyncScheduler {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            cadence: Mutex::new(Cadence::new(0)),
            wake: Condvar::new(),
        }
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn lock_cadence(&self) -> std::sync::MutexGuard<'_, Cadence> {
        self.cadence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A local write was queued into the outbox; a debounced round follows.
    pub fn note_local_change(&self) {
        self.lock_cadence().note_local_change(self.now_ms());
        self.wake.notify_all();
    }

    /// A round is about to run; the returned mark goes back into
    /// [`Self::note_round`] so a change queued while the round was on the
    /// wire keeps its debounce instead of being consumed as pushed.
    pub fn begin_round(&self) -> u64 {
        self.now_ms()
    }

    /// A round that started at `started_ms` finished with the given outcome.
    pub fn note_round(&self, started_ms: u64, ok: bool) {
        self.lock_cadence()
            .note_round(started_ms, self.now_ms(), ok);
        self.wake.notify_all();
    }

    /// Blocks the scheduler thread until the next round is due.
    pub fn wait_until_due(&self) {
        let mut cadence = self.lock_cadence();
        loop {
            let now = self.now_ms();
            if cadence.is_due(now) {
                return;
            }
            let wait = Duration::from_millis(cadence.next_due_ms().saturating_sub(now));
            let (guard, _) = self
                .wake
                .wait_timeout(cadence, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cadence = guard;
        }
    }
}

impl Default for SyncScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// The registered write-path observer: seals into the outbox exactly as
/// before, then pokes the scheduler so the change is pushed within the
/// debounce window instead of waiting for the idle poll. Sealing failures
/// still abort the write; the poke happens only after a successful enqueue.
struct NotifyingSealer {
    inner: OutboxSealer,
    scheduler: Arc<SyncScheduler>,
}

impl ChangeObserver for NotifyingSealer {
    fn entity_changed(&self, conn: &Connection, change: &EntityChange) -> Result<(), HookError> {
        self.inner.entity_changed(conn, change)?;
        self.scheduler.note_local_change();
        Ok(())
    }
}

/// The host's sync state: identity, the engine when an account is bound,
/// the write-path observer registration, and the in-flight pairing session.
pub struct SyncState {
    inner: Mutex<SyncHost>,
    data_dir: PathBuf,
}

impl SyncState {
    pub fn new(host: SyncHost, data_dir: PathBuf) -> Self {
        Self {
            inner: Mutex::new(host),
            data_dir,
        }
    }

    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, SyncHost>, IpcError> {
        self.inner.lock().map_err(|_| IpcError::system())
    }

    fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

pub struct SyncHost {
    /// False when the platform secure store is unavailable: sync cannot be
    /// set up, but every other feature keeps working.
    available: bool,
    device_id: String,
    device_name: String,
    engine: Option<SyncEngine>,
    /// Kept alive for as long as the write path should seal into the outbox.
    observer: Option<ObserverGuard>,
    pairing: Option<PairingHandle>,
    claimed: Option<ClaimedPairing>,
    last_sync_at: Option<TimestampMs>,
    /// The WebDAV store when the account is bound to that backend;
    /// `None` in the server form.
    webdav: Option<WebdavStore>,
    /// Poked by the write-path observer and the round runners so the
    /// background schedule reacts to changes and outcomes.
    scheduler: Arc<SyncScheduler>,
}

impl SyncHost {
    /// Registers this install's device identity and row, then attaches the
    /// engine and the outbox observer when an account is already configured.
    /// A missing secure store leaves the host in its unavailable state
    /// instead of failing startup.
    pub fn start(
        conn: &Connection,
        store: &dyn SecureStore,
        device_id: String,
        platform: Option<Platform>,
        now: TimestampMs,
        scheduler: Arc<SyncScheduler>,
    ) -> Self {
        let device_name = existing_device_name(conn, &device_id).unwrap_or_else(local_device_name);
        let mut host = Self {
            available: false,
            device_id,
            device_name,
            engine: None,
            observer: None,
            pairing: None,
            claimed: None,
            last_sync_at: None,
            webdav: None,
            scheduler,
        };
        // Without a known platform the device row cannot be written in the
        // controlled enum, so sync stays unavailable on that host.
        let Some(platform) = platform else {
            return host;
        };
        let local = ensure_local_device(
            conn,
            store,
            &LocalDeviceConfig {
                device_id: &host.device_id,
                name: &host.device_name,
                platform,
            },
            now,
        );
        let Ok(local) = local else {
            return host;
        };
        host.device_name = local.device.name;
        host.available = true;
        host.attach_configured(conn, store, now);
        host
    }

    /// Builds the engine and registers the outbox observer when the stored
    /// configuration already names an account. Silent on failure: a
    /// unreachable server or a missing key generation must not stop the app
    /// from starting, and the Sync page reports the state.
    fn attach_configured(&mut self, conn: &Connection, store: &dyn SecureStore, now: TimestampMs) {
        let Ok(config) = SyncStateRepo::new(conn).config_get() else {
            return;
        };
        let (Some(server_url), true) = (config.server_url.as_deref(), config.is_active()) else {
            return;
        };
        let attached = match config.transport_kind {
            TransportKind::Server => self.attach_engine(store, server_url),
            TransportKind::Webdav => self.attach_webdav(store, server_url),
        };
        if attached.is_ok()
            && self
                .register_sealer(conn, store, config.sync_key_id)
                .is_ok()
        {
            let _ = self.reconcile(conn, store, config.sync_key_id, now);
        }
    }

    /// Queues whatever changed while sync was off.
    /// Always runs after the sealer is registered, so a write racing the
    /// catch-up is queued by the observer instead of being missed. Silent
    /// on failure at startup: an unreadable key generation must not stop
    /// the app from starting, and the next re-enable tries again.
    fn reconcile(
        &self,
        conn: &Connection,
        store: &dyn SecureStore,
        key_id: u32,
        now: TimestampMs,
    ) -> Result<usize, IpcError> {
        let keys = SyncKeys::load(store, key_id).map_err(|e| map_sync(SyncError::Keyring(e)))?;
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        engine.reconcile(conn, &keys, now).map_err(map_sync)
    }

    /// Creates the engine for `server_url`. Replaces any existing engine, so
    /// re-pointing at another server never keeps a stale session token.
    fn attach_engine(&mut self, store: &dyn SecureStore, server_url: &str) -> Result<(), IpcError> {
        let identity = self.load_identity(store)?;
        let transport = HttpTransport::new(server_url).map_err(map_transport)?;
        self.engine = Some(SyncEngine::new(
            Box::new(transport),
            identity,
            self.device_id.clone(),
        ));
        Ok(())
    }

    /// Builds the WebDAV engine and store for `base_url`.
    /// Credentials come from the platform secure store entry
    /// `sync.webdav.credentials`; an unparsable entry degrades to an
    /// unauthenticated client, which the endpoint then refuses visibly.
    pub(crate) fn attach_webdav(
        &mut self,
        store: &dyn SecureStore,
        base_url: &str,
    ) -> Result<(), IpcError> {
        let identity = self.load_identity(store)?;
        let credentials = webdav_credentials(store);
        let client = WebdavClient::new(base_url, credentials.as_ref()).map_err(map_transport)?;
        self.webdav = Some(WebdavStore::new(client));
        self.engine = Some(SyncEngine::new_webdav(identity, self.device_id.clone()));
        Ok(())
    }

    /// The attached WebDAV store; a conflict when the account is not bound
    /// to the WebDAV backend.
    pub(crate) fn webdav(&self) -> Result<&WebdavStore, IpcError> {
        self.webdav
            .as_ref()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))
    }

    /// Whether the WebDAV backend is attached (drives command dispatch).
    pub(crate) fn is_webdav(&self) -> bool {
        self.webdav.is_some()
    }

    /// Mutable engine plus the attached WebDAV store in one split borrow.
    pub(crate) fn engine_and_webdav(
        &mut self,
    ) -> Result<(&mut SyncEngine, &WebdavStore), IpcError> {
        match (&mut self.engine, &self.webdav) {
            (Some(engine), Some(dav)) => Ok((engine, dav)),
            _ => Err(IpcError::conflict("sync is not set up on this device")),
        }
    }

    /// Registers the seal-at-write observer for `conn` under the current
    /// key generation. Replacing an existing registration is how a key
    /// rotation takes effect on the write path.
    fn register_sealer(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        key_id: u32,
    ) -> Result<(), IpcError> {
        let identity = self.load_identity(store)?;
        let keys = SyncKeys::load(store, key_id).map_err(|e| map_sync(SyncError::Keyring(e)))?;
        let sealer =
            OutboxSealer::new(identity, self.device_id.clone(), &keys).map_err(map_sync)?;
        self.observer = Some(register_observer(
            conn,
            Arc::new(NotifyingSealer {
                inner: sealer,
                scheduler: Arc::clone(&self.scheduler),
            }),
        ));
        Ok(())
    }

    /// Drops the observer so later writes stop queueing. The
    /// queue itself is kept for a re-enable.
    fn detach_sealer(&mut self) {
        self.observer = None;
    }

    /// Loads this device's key material from the platform secure store. The
    /// engine and the outbox sealer each own their instance, so the store
    /// stays the single source rather than a cached secret being cloned.
    fn load_identity(&self, store: &dyn SecureStore) -> Result<DeviceIdentity, IpcError> {
        DeviceIdentity::load(store)
            .map_err(|_| IpcError::system())?
            .ok_or_else(|| IpcError::conflict("this device has no secure key storage"))
    }

    fn engine_mut(&mut self) -> Result<&mut SyncEngine, IpcError> {
        self.engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))
    }

    /// One engine round on whichever backend is attached (server or
    /// WebDAV). Split-borrow helper: the engine and the store live on the
    /// same host, so commands cannot borrow both through accessors.
    pub(crate) fn run_engine_round(
        &mut self,
        db: &std::sync::Mutex<Connection>,
        store: &dyn SecureStore,
        now: TimestampMs,
    ) -> Result<typvia_sync::SyncReport, IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine.sync_webdav(db, store, dav, now).map_err(map_sync),
            None => engine.sync(db, store, now).map_err(map_sync),
        }
    }

    /// Revokes a device on whichever backend is attached.
    pub(crate) fn revoke_on_backend(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        device_id: &str,
        master_key: Option<&typvia_crypto::SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine
                .revoke_device_webdav(conn, store, dav, device_id, master_key, now)
                .map_err(map_sync),
            None => engine
                .revoke_device(conn, store, device_id, master_key, now)
                .map_err(map_sync),
        }
    }

    /// Rotates K_sync on whichever backend is attached.
    pub(crate) fn rotate_on_backend(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_key: Option<&typvia_crypto::SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine
                .rotate_sync_key_webdav(conn, store, dav, master_key, now)
                .map_err(map_sync),
            None => engine
                .rotate_sync_key(conn, store, master_key, now)
                .map_err(map_sync),
        }
    }

    /// Polls pairing on whichever backend the joining flow attached.
    pub(crate) fn poll_pairing_on_backend(
        &mut self,
        handle: &PairingHandle,
    ) -> Result<Option<ClaimedPairing>, IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine.poll_pairing_webdav(dav, handle).map_err(map_sync),
            None => engine.poll_pairing(handle).map_err(map_sync),
        }
    }

    /// Finalizes pairing on whichever backend the joining flow attached.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finalize_pairing_on_backend(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        handle: &PairingHandle,
        claimed: &ClaimedPairing,
        master_password: Option<&[u8]>,
        now: TimestampMs,
    ) -> Result<typvia_sync::AdoptedAccount, IpcError> {
        let webdav = self.webdav.is_some();
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        if webdav {
            engine
                .finalize_pairing_webdav(conn, store, handle, claimed, master_password, now)
                .map_err(map_sync)
        } else {
            engine
                .finalize_pairing(conn, store, handle, claimed, master_password, now)
                .map_err(map_sync)
        }
    }

    /// Approves pairing on whichever backend this account uses.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn approve_pairing_on_backend(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        code: &typvia_sync::PairingCode,
        allow_vault: bool,
        master_key: Option<&typvia_crypto::SymmetricKey>,
        now: TimestampMs,
    ) -> Result<(), IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine
                .approve_pairing_webdav(conn, store, dav, code, allow_vault, master_key, now)
                .map_err(map_sync),
            None => engine
                .approve_pairing(conn, store, code, allow_vault, master_key, now)
                .map_err(map_sync),
        }
    }

    /// Publishes the recovery blob on whichever backend this account uses.
    pub(crate) fn publish_recovery_on_backend(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_key: &typvia_crypto::SymmetricKey,
    ) -> Result<typvia_sync::RecoveryCode, IpcError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        match &self.webdav {
            Some(dav) => engine
                .publish_recovery_webdav(conn, store, dav, master_key)
                .map_err(map_sync),
            None => engine
                .publish_recovery(conn, store, master_key)
                .map_err(map_sync),
        }
    }

    /// The schedule this host feeds: commands report round
    /// outcomes and queued backlogs through it.
    pub(crate) fn scheduler(&self) -> &SyncScheduler {
        &self.scheduler
    }
}

/// Reads and parses the WebDAV credentials from the platform secure store
/// (entry `sync.webdav.credentials`); `None` when absent or
/// unparsable.
fn webdav_credentials(store: &dyn SecureStore) -> Option<WebdavCredentials> {
    let raw = store.retrieve(WEBDAV_CREDENTIALS_ENTRY).ok().flatten()?;
    let text = std::str::from_utf8(&raw).ok()?;
    WebdavCredentials::parse(text).ok()
}

/// Secure-store entry holding the WebDAV credentials. Never in
/// the database, never logged.
pub(crate) const WEBDAV_CREDENTIALS_ENTRY: &str = "sync.webdav.credentials";

/// The device row's stored name, so a restart never renames a device the
/// user already sees in their list.
fn existing_device_name(conn: &Connection, device_id: &str) -> Option<String> {
    DeviceRepo::new(conn)
        .get(device_id)
        .ok()
        .flatten()
        .map(|device| device.name)
}

/// A human-recognizable default name for this install. macOS exposes the
/// name the user chose in System Settings, which is exactly what the design
/// shows in the device list; other platforms fall back to a neutral label
/// the user can still recognize as "the one I am on".
fn local_device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()
            && output.status.success()
            && let Ok(name) = String::from_utf8(output.stdout)
        {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
        "This Mac".to_string()
    }
    #[cfg(not(target_os = "macos"))]
    {
        "This device".to_string()
    }
}

/// Reads the persisted sync configuration into the wire shape.
fn setup_state(conn: &Connection) -> Result<SyncSetupState, IpcError> {
    let config = SyncStateRepo::new(conn)
        .config_get()
        .map_err(IpcError::from)?;
    let pending = SyncOutboxRepo::new(conn)
        .pending_count()
        .map_err(IpcError::from)?;
    Ok(SyncSetupState {
        server_url: config.server_url,
        account_id: config.account_id,
        enabled: config.enabled,
        key_generation: config.sync_key_id,
        pending_backlog: pending,
        recovery_catchup_pending: config.recovery_root.is_some(),
        transport_kind: config.transport_kind.as_str().to_string(),
    })
}

/// Path of the "recovery code exported" marker for this install.
fn recovery_marker_path(data_dir: &Path) -> PathBuf {
    data_dir.join(RECOVERY_MARKER_FILE)
}

fn recovery_exported_at(data_dir: &Path) -> Option<TimestampMs> {
    std::fs::read_to_string(recovery_marker_path(data_dir))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn mark_recovery_exported(data_dir: &Path, now: TimestampMs) {
    // Best effort: losing the marker only costs the "last exported" line.
    let _ = std::fs::write(recovery_marker_path(data_dir), now.to_string());
}

/// Maps engine failures onto the stable IPC codes. Messages stay structural
/// — no entity content, tokens, key material or server text ever crosses.
pub fn map_sync(error: SyncError) -> IpcError {
    match error {
        SyncError::NotEnabled => IpcError::conflict("sync is not set up on this device"),
        SyncError::HostGone => IpcError::system(),
        SyncError::BackedOff { .. } => {
            IpcError::unavailable("waiting before the next connection attempt")
        }
        SyncError::CursorViolation => {
            IpcError::conflict("the server's answer was out of order; nothing was applied")
        }
        SyncError::Transport(e) => map_transport(e),
        SyncError::Trust(_) => {
            IpcError::conflict("this account's trust root could not be verified")
        }
        SyncError::Record(_) => IpcError::conflict("a record could not be verified"),
        SyncError::Keyring(_) => IpcError::system(),
        SyncError::Repo(e) => IpcError::from(e),
        SyncError::Pairing(e) => map_pairing(e),
        SyncError::Recovery(e) => map_recovery(e),
    }
}

fn map_transport(error: TransportError) -> IpcError {
    match error {
        TransportError::InvalidServerUrl => {
            IpcError::validation("server address must start with https://")
        }
        TransportError::Network(_) | TransportError::RateLimited => {
            IpcError::unavailable("the server could not be reached")
        }
        TransportError::SessionExpired => IpcError::permission_denied("this device was signed out"),
        TransportError::ProtocolUnsupported => {
            IpcError::conflict("this server speaks a different protocol version")
        }
        TransportError::Api { ref code, .. } if code == "DEVICE_REVOKED" => {
            IpcError::permission_denied("this device was revoked from the account")
        }
        TransportError::Api { ref code, .. } if code == "NOT_FOUND" => IpcError::not_found(),
        TransportError::Api { ref code, .. } if code == "PRECONDITIONS_UNSUPPORTED" => {
            IpcError::conflict("this WebDAV endpoint cannot keep files consistent")
        }
        TransportError::Api { ref code, .. } if code == "WEBDAV_ACCOUNT_EXISTS" => {
            IpcError::conflict("this storage already holds an account — join it by pairing")
        }
        TransportError::Api { ref code, .. } if code == "WEBDAV_AUTH" => {
            IpcError::permission_denied("the WebDAV endpoint refused these credentials")
        }
        TransportError::Api { .. } | TransportError::MalformedResponse => {
            IpcError::conflict("the server refused this request")
        }
    }
}

fn map_pairing(error: PairingError) -> IpcError {
    match error {
        PairingError::MalformedCode => IpcError::validation("that is not a Typvia pairing code"),
        PairingError::ForeignAccount => {
            IpcError::conflict("that code belongs to a different account")
        }
        PairingError::ForeignCertificate | PairingError::BadSignature => {
            IpcError::permission_denied("the pairing answer could not be verified")
        }
        PairingError::DecryptFailed | PairingError::MalformedBundle => {
            IpcError::conflict("the pairing answer could not be opened")
        }
        PairingError::MasterPasswordRequired => {
            IpcError::validation("enter a master password for this device's vault")
        }
        PairingError::VaultAlreadyInitialized => {
            IpcError::conflict("this device already has its own vault")
        }
        PairingError::MasterKeyRequired => IpcError::permission_denied("unlock the vault first"),
        PairingError::EncryptFailed => IpcError::system(),
    }
}

fn map_recovery(error: RecoveryError) -> IpcError {
    match error {
        RecoveryError::MalformedCode => IpcError::validation("that recovery code is not complete"),
        RecoveryError::MalformedBlob | RecoveryError::DecryptFailed => {
            IpcError::permission_denied("that recovery code did not open this account")
        }
        RecoveryError::MissingMasterKey => {
            IpcError::conflict("this account has no vault to recover")
        }
        RecoveryError::Crypto => IpcError::system(),
    }
}
