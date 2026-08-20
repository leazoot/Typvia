//! Sync orchestration: outbox sealing at write time, ordered batch push,
//! ordered pull application in a single writer transaction per page,
//! cursor persistence, conflict parking, and network backoff.
//!
//! Application of pulled records writes through the core repositories
//! directly — never through host write use cases — so an applied change is
//! structurally unable to re-enter the outbox (echo suppression).

use std::collections::{HashMap, HashSet};
use std::fmt;

use rusqlite::Connection;
use typvia_core::model::{
    OutboxRecord, OutboxState, PendingReason, PendingRemoteRecord, Platform, SnippetVersion,
    SyncConfig, SyncEntityType, SyncRecord, SyncShadow, TimestampMs, TransportKind,
};
use typvia_core::repo::{
    AppRuleRepo, DeviceRepo, FolderRepo, RepoError, SnippetRepo, SyncOutboxRepo, SyncStateRepo,
    TagRepo, TemplateFieldRepo, VersionRepo, new_id,
};
use typvia_core::sync_hooks::{ChangeObserver, EntityChange, HookError};
use typvia_crypto::{SecureStore, SymmetricKey};
use zeroize::Zeroizing;

use crate::cert::{CertificateSubject, RootStatement};
use crate::directory::TrustState;
use crate::error::{CertificateError, RecordError};
use crate::identity::{DeviceIdentity, Fingerprint};
use crate::keyring::{KeyringError, SyncKeys, provision_initial_key};
use crate::merge::{self, MergeSide};
use crate::pairing::PairingError;
use crate::payload::{self, PayloadError, snippet_tag_entity_id};
use crate::record::{
    OpenedRecord, RecordMeta, SyncKeyring, WireRecord, record_aad, seal_record, seal_tombstone,
    verify_and_open,
};
use crate::recovery::RecoveryError;
use crate::replay::{AppliedHead, ServerSeqCursor, check_not_replayed};
use crate::transport::{ConflictHead, PushOutcome, SessionToken, SyncTransport, TransportError};

/// Push batch limits.
const MAX_PUSH_RECORDS: u32 = 500;
const MAX_PUSH_BYTES: usize = 4 * 1024 * 1024;
/// Pull page size: conservative so a page of maximum-size envelopes stays
/// well inside the transport's response cap.
const PULL_PAGE_LIMIT: u32 = 100;
/// Network backoff: exponential from base to cap (client policy; the
/// offline backlog itself is unbounded).
const BACKOFF_BASE_MS: i64 = 5_000;
const BACKOFF_CAP_MS: i64 = 300_000;
/// Upper bound on push→pull→merge cycles per sync call: each
/// fresh 409 re-runs the cycle; the bound keeps a racing or hostile server
/// from spinning the client, and leftovers stay queued for the next call.
const MERGE_ROUNDS_MAX: u32 = 4;
/// Snippet page size for the full-library walks (initial export and
/// reconcile): bounds peak memory on a 50k library.
const EXPORT_PAGE: u32 = 500;

/// Short-lived database access for a sync round: the engine
/// locks the connection for each local phase and releases it before any
/// network I/O, so host reads are never blocked on the network. The host
/// lock order is `sync` → `conn` (documented at each host's sync module):
/// the caller already holds the sync guard when the engine takes the
/// connection, never the reverse.
pub trait SyncDb {
    /// Locks the database for one local phase.
    fn db(&self) -> Result<Box<dyn std::ops::Deref<Target = Connection> + '_>, SyncError>;
}

/// Single-threaded access: an exclusive connection is its own guard. This
/// keeps tests and the user-blocking flows (pairing, recovery) on plain
/// `&Connection` arguments.
impl SyncDb for Connection {
    fn db(&self) -> Result<Box<dyn std::ops::Deref<Target = Connection> + '_>, SyncError> {
        Ok(Box::new(self))
    }
}

/// Host access: each phase takes the shared mutex and drops it with the
/// returned guard. A poisoned mutex means a writer panicked mid-operation;
/// the round reports [`SyncError::HostGone`] instead of unwrapping.
impl SyncDb for std::sync::Mutex<Connection> {
    fn db(&self) -> Result<Box<dyn std::ops::Deref<Target = Connection> + '_>, SyncError> {
        match self.lock() {
            Ok(guard) => Ok(Box::new(guard)),
            Err(_) => Err(SyncError::HostGone),
        }
    }
}

/// Engine failures. Variants carry structural facts only — never entity
/// content, tokens, or key material.
#[derive(Debug)]
pub enum SyncError {
    /// Sync is disabled or not bound to an account.
    NotEnabled,
    /// The host's database lock is poisoned; the process is unhealthy and
    /// the round cannot safely touch the database.
    HostGone,
    /// A recent network failure put the engine in backoff; retry later.
    BackedOff {
        until: TimestampMs,
    },
    /// The pull response violated server_seq monotonicity: the round was
    /// aborted and nothing was applied.
    CursorViolation,
    Transport(TransportError),
    Trust(CertificateError),
    Keyring(KeyringError),
    Record(RecordError),
    Repo(RepoError),
    Pairing(PairingError),
    Recovery(RecoveryError),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnabled => f.write_str("sync is not enabled"),
            Self::HostGone => f.write_str("the database lock is unavailable"),
            Self::BackedOff { until } => write!(f, "sync is backing off until {until}"),
            Self::CursorViolation => f.write_str("pull response violated cursor monotonicity"),
            Self::Transport(e) => write!(f, "{e}"),
            Self::Trust(e) => write!(f, "{e}"),
            Self::Keyring(e) => write!(f, "{e}"),
            Self::Record(e) => write!(f, "{e}"),
            Self::Repo(e) => write!(f, "{e}"),
            Self::Pairing(e) => write!(f, "{e}"),
            Self::Recovery(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<TransportError> for SyncError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}
impl From<CertificateError> for SyncError {
    fn from(e: CertificateError) -> Self {
        Self::Trust(e)
    }
}
impl From<KeyringError> for SyncError {
    fn from(e: KeyringError) -> Self {
        Self::Keyring(e)
    }
}
impl From<RecordError> for SyncError {
    fn from(e: RecordError) -> Self {
        Self::Record(e)
    }
}
impl From<RepoError> for SyncError {
    fn from(e: RepoError) -> Self {
        Self::Repo(e)
    }
}
impl From<PairingError> for SyncError {
    fn from(e: PairingError) -> Self {
        Self::Pairing(e)
    }
}
impl From<RecoveryError> for SyncError {
    fn from(e: RecoveryError) -> Self {
        Self::Recovery(e)
    }
}

/// What one sync round did (the backlog count feeds the UI indicator).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    /// Records accepted by the server this round.
    pub pushed: usize,
    /// Entities parked as conflicted this round (409 heads).
    pub conflicts: Vec<ConflictHead>,
    /// Remote records applied locally.
    pub applied: usize,
    /// Conflicted entities resolved by three-way merge this round: the
    /// merged state was applied and re-queued past the server head.
    pub merged: usize,
    /// Conflict-copy snippets created by rule 3 (concurrent body edits);
    /// each preserves the local version pending user resolution.
    pub conflict_copy_ids: Vec<String>,
    /// Remote records parked (unknown key generation / payload ahead /
    /// unsupported entity) — kept, not dropped.
    pub parked: usize,
    /// Remote records skipped after an isolated failure, or as replays.
    pub skipped: usize,
    /// Own records echoed back by the pull and ignored.
    pub skipped_own: usize,
    /// Snippets touched by application; the host re-indexes these (the
    /// search index lives above this crate in the dependency order).
    pub applied_snippet_ids: Vec<String>,
    /// Outbox records still pending after the round.
    pub pending_backlog: u64,
}

/// Seals one local entity change into the outbox, inside the caller's
/// write transaction (seal-at-write). Returns `false` when there
/// was nothing to seal (the entity vanished before sealing).
pub fn enqueue_change(
    conn: &Connection,
    identity: &DeviceIdentity,
    device_id: &str,
    key: &SymmetricKey,
    key_id: u32,
    change: &EntityChange,
) -> Result<bool, SyncError> {
    let outbox = SyncOutboxRepo::new(conn);
    let version = outbox.next_version(change.entity_type, &change.entity_id)?;
    let meta = RecordMeta {
        id: new_id(),
        entity_type: change.entity_type,
        entity_id: change.entity_id.clone(),
        version,
        device_id: device_id.to_string(),
        updated_at: change.now,
    };
    let wire = match change.deleted_at {
        Some(deleted_at) => seal_tombstone(meta, deleted_at, identity)?,
        None => {
            let document =
                match payload::build_document(conn, change.entity_type, &change.entity_id) {
                    Ok(Some(document)) => document,
                    Ok(None) => return Ok(false),
                    // No local build path (ai_action): nothing to queue in v1 —
                    // no write use case produces these yet.
                    Err(PayloadError::UnsupportedEntity) => return Ok(false),
                    Err(PayloadError::Repo(e)) => return Err(e.into()),
                    Err(_) => return Err(SyncError::Record(RecordError::MalformedPayload)),
                };
            seal_record(meta, &document, key, key_id, identity)?
        }
    };
    outbox.enqueue(&OutboxRecord {
        key_id: wire.key_id,
        signature: wire.signature.to_vec(),
        state: OutboxState::Pending,
        server_seq: None,
        record: wire_to_sync_record(&wire),
    })?;
    Ok(true)
}

fn wire_to_sync_record(wire: &WireRecord) -> SyncRecord {
    SyncRecord {
        id: wire.id.clone(),
        entity_type: wire.entity_type,
        entity_id: wire.entity_id.clone(),
        version: wire.version,
        ciphertext: wire.ciphertext.clone(),
        deleted_at: wire.deleted_at,
        updated_at: wire.updated_at,
        device_id: wire.device_id.clone(),
    }
}

pub(crate) fn outbox_to_wire(entry: &OutboxRecord) -> Result<WireRecord, SyncError> {
    let signature: [u8; 64] = entry
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| SyncError::Record(RecordError::BadSignature))?;
    Ok(WireRecord {
        id: entry.record.id.clone(),
        entity_type: entry.record.entity_type,
        entity_id: entry.record.entity_id.clone(),
        version: entry.record.version,
        ciphertext: entry.record.ciphertext.clone(),
        deleted_at: entry.record.deleted_at,
        updated_at: entry.record.updated_at,
        device_id: entry.record.device_id.clone(),
        key_id: entry.key_id,
        signature,
    })
}

/// The write-path observer (core sync_hooks seam): checks the persisted
/// switch, then seals into the outbox — all inside the write transaction.
/// Registered only while sync is enabled; the config check makes disabling
/// effective even before the host re-registers.
pub struct OutboxSealer {
    identity: DeviceIdentity,
    device_id: String,
    key: SymmetricKey,
    key_id: u32,
}

impl OutboxSealer {
    /// Builds the observer from the loaded keyring's current generation.
    pub fn new(
        identity: DeviceIdentity,
        device_id: String,
        keys: &SyncKeys,
    ) -> Result<Self, SyncError> {
        let key = keys
            .current_key()
            .ok_or(SyncError::Keyring(KeyringError::MissingCurrentKey(
                keys.current_id(),
            )))?;
        Ok(Self {
            identity,
            device_id,
            key: SymmetricKey::from_bytes(*key.expose()),
            key_id: keys.current_id(),
        })
    }
}

impl ChangeObserver for OutboxSealer {
    fn entity_changed(&self, conn: &Connection, change: &EntityChange) -> Result<(), HookError> {
        let config = SyncStateRepo::new(conn)
            .config_get()
            .map_err(HookError::new)?;
        if !config.is_active() {
            return Ok(());
        }
        enqueue_change(
            conn,
            &self.identity,
            &self.device_id,
            &self.key,
            self.key_id,
            change,
        )
        .map(|_| ())
        .map_err(HookError::new)
    }
}

impl fmt::Debug for OutboxSealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Key bytes never reach Debug output.
        f.debug_struct("OutboxSealer")
            .field("device_id", &self.device_id)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

/// Facts an adopted (paired) account arrives with; the K_sync generations
/// themselves are installed into the secure store beforehand
/// (`keyring::install_key`).
#[derive(Debug, Clone)]
pub struct AdoptedAccount {
    pub server_url: String,
    pub account_id: String,
    /// Pinned trust-root digest (32 bytes) verified during pairing.
    pub root_fingerprint: [u8; 32],
    /// Highest K_sync generation received.
    pub sync_key_id: u32,
}

/// One-off adapter so the replay check can run against a prefetched
/// shadow head.
struct FixedHead(Option<u64>);

impl AppliedHead for FixedHead {
    fn applied_head(&self, _entity_type: SyncEntityType, _entity_id: &str) -> Option<u64> {
        self.0
    }
}

/// Facts a fresh account is created with. The master key is optional:
/// without a vault only the secure-store K_sync copy exists.
#[derive(Clone, Copy)]
pub struct NewAccount<'a> {
    pub device_name: &'a str,
    pub platform: Platform,
    pub server_url: &'a str,
    pub master_key: Option<&'a SymmetricKey>,
}

/// The sync engine: transport, device identity, session cache, backoff.
/// All database access goes through the connection handed to each call
/// (single-writer discipline stays with the host).
pub struct SyncEngine {
    pub(crate) transport: Box<dyn SyncTransport>,
    pub(crate) identity: DeviceIdentity,
    pub(crate) device_id: String,
    token: Option<SessionToken>,
    failures: u32,
    next_attempt_at: TimestampMs,
}

impl SyncEngine {
    pub fn new(
        transport: Box<dyn SyncTransport>,
        identity: DeviceIdentity,
        device_id: String,
    ) -> Self {
        Self {
            transport,
            identity,
            device_id,
            token: None,
            failures: 0,
            next_attempt_at: 0,
        }
    }

    /// Creates the account on the server (first device), provisions
    /// K_sync, pins the own root fingerprint, enables sync, and enqueues
    /// the initial full export of existing local entities — configuration
    /// and export in one transaction (a failure leaves sync disabled).
    pub fn create_account(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        account: &NewAccount<'_>,
        now: TimestampMs,
    ) -> Result<String, SyncError> {
        self.transport.handshake()?;
        let root = RootStatement::create(
            &self.identity,
            CertificateSubject {
                device_id: self.device_id.clone(),
                ed25519_pub: self.identity.ed25519_public(),
                x25519_pub: self.identity.x25519_public(),
                name: account.device_name.to_string(),
                platform: account.platform,
                created_at: now,
            },
        )?;
        let account_id = self.transport.create_account(&root)?;
        let key_id = provision_initial_key(conn, store, account.master_key, now)?;
        let keys = SyncKeys::load(store, key_id)?;
        self.activate(
            conn,
            &keys,
            SyncConfig {
                server_url: Some(account.server_url.to_string()),
                account_id: Some(account_id.clone()),
                enabled: true,
                applied_server_seq: 0,
                sync_key_id: key_id,
                root_fingerprint: Some(root.fingerprint().as_bytes().to_vec()),
                key_update_seq: 0,
                recovery_root: None,
                transport_kind: TransportKind::Server,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )?;
        Ok(account_id)
    }

    /// Binds this device to an existing account after pairing delivered
    /// the trust root and K_sync generations. The pairing UX lives
    /// elsewhere; tests drive the same primitives. Enables sync and
    /// exports any pre-existing local entities.
    pub fn adopt_account(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        adopted: &AdoptedAccount,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let keys = SyncKeys::load(store, adopted.sync_key_id)?;
        self.activate(
            conn,
            &keys,
            SyncConfig {
                server_url: Some(adopted.server_url.clone()),
                account_id: Some(adopted.account_id.clone()),
                enabled: true,
                applied_server_seq: 0,
                sync_key_id: adopted.sync_key_id,
                root_fingerprint: Some(adopted.root_fingerprint.to_vec()),
                key_update_seq: 0,
                recovery_root: None,
                transport_kind: TransportKind::Server,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )
    }

    /// Turns sync off: the product stays fully usable, the queue and all
    /// state are kept for a later re-enable.
    pub fn disable(&mut self, conn: &Connection, now: TimestampMs) -> Result<(), SyncError> {
        let state = SyncStateRepo::new(conn);
        let mut config = state.config_get()?;
        config.enabled = false;
        config.updated_at = now;
        state.config_put(&config)?;
        self.token = None;
        Ok(())
    }

    /// Writes the enabled configuration and enqueues the initial export in
    /// one transaction.
    pub(crate) fn activate(
        &mut self,
        conn: &Connection,
        keys: &SyncKeys,
        config: SyncConfig,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        self.activate_within(&tx, keys, config, now)?;
        tx.commit().map_err(RepoError::from)?;
        Ok(())
    }

    /// [`Self::activate`] inside a caller-owned transaction, for flows
    /// (pairing/recovery adoption) that must bind key installation and
    /// activation atomically.
    pub(crate) fn activate_within(
        &self,
        conn: &Connection,
        keys: &SyncKeys,
        config: SyncConfig,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let key = keys
            .current_key()
            .ok_or(SyncError::Keyring(KeyringError::MissingCurrentKey(
                keys.current_id(),
            )))?;
        SyncStateRepo::new(conn).config_put(&config)?;
        self.export_existing(conn, key, keys.current_id(), now)?;
        Ok(())
    }

    /// Enqueues every live syncable entity for the initial export:
    /// folders parents-first, then tags, then snippets with their links,
    /// template fields and app rules — an order the receiving side can
    /// apply under its foreign keys. Recycle-bin rows stay local.
    fn export_existing(
        &self,
        conn: &Connection,
        key: &SymmetricKey,
        key_id: u32,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        for (entity_type, entity_id) in live_entities(conn)? {
            enqueue_change(
                conn,
                &self.identity,
                &self.device_id,
                key,
                key_id,
                &EntityChange {
                    entity_type,
                    entity_id,
                    deleted_at: None,
                    now,
                },
            )?;
        }
        Ok(())
    }

    /// Catches up entities changed while the outbox observer was
    /// detached. Every live entity is compared against the last
    /// state this device committed to sending — the newest queued record,
    /// or the shadow base when nothing is queued — and only differences are
    /// enqueued; an entity whose shadow survives but that is gone (or in
    /// the recycle bin) locally gets its tombstone. Entities that already
    /// match queue nothing, so a library untouched while sync was off costs
    /// zero pushes and a second run adds nothing.
    ///
    /// Runs in one transaction: a failure leaves the queue exactly as it
    /// was rather than half caught up. Returns how many records it queued.
    pub fn reconcile(
        &self,
        conn: &Connection,
        keys: &SyncKeys,
        now: TimestampMs,
    ) -> Result<usize, SyncError> {
        let key = keys
            .current_key()
            .ok_or(SyncError::Keyring(KeyringError::MissingCurrentKey(
                keys.current_id(),
            )))?;
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        let queued = queued_documents(&tx, keys)?;
        let state = SyncStateRepo::new(&tx);
        let mut seen: HashSet<EntityKey> = HashSet::new();
        let mut enqueued = 0usize;

        for (entity_type, entity_id) in live_entities(&tx)? {
            seen.insert((entity_type, entity_id.clone()));
            let document = match payload::build_document(&tx, entity_type, &entity_id) {
                Ok(Some(document)) => Zeroizing::new(document),
                // Vanished mid-walk, or an entity kind with no local build
                // path: nothing to queue.
                Ok(None) | Err(PayloadError::UnsupportedEntity) => continue,
                Err(PayloadError::Repo(e)) => return Err(e.into()),
                Err(_) => return Err(SyncError::Record(RecordError::MalformedPayload)),
            };
            let basis: Option<Zeroizing<Vec<u8>>> =
                match queued.get(&(entity_type, entity_id.clone())) {
                    // A queued record is what the server sees next, so it —
                    // not the older shadow — is the state to compare with.
                    // `None` inside means a queued tombstone.
                    Some(queued) => queued.clone(),
                    None => state
                        .shadow_get(entity_type, &entity_id)?
                        .and_then(|shadow| shadow.document)
                        .map(Zeroizing::new),
                };
            if basis.as_deref().map(Vec::as_slice) == Some(document.as_slice()) {
                continue;
            }
            if enqueue_change(
                &tx,
                &self.identity,
                &self.device_id,
                key,
                keys.current_id(),
                &EntityChange {
                    entity_type,
                    entity_id,
                    deleted_at: None,
                    now,
                },
            )? {
                enqueued += 1;
            }
        }

        for (entity_type, entity_id) in state.shadow_keys()? {
            if seen.contains(&(entity_type, entity_id.clone())) {
                continue;
            }
            // A tombstone already queued for it says the same thing.
            if matches!(queued.get(&(entity_type, entity_id.clone())), Some(None)) {
                continue;
            }
            let Some(deleted_at) = gone_locally(&tx, entity_type, &entity_id, now)? else {
                continue;
            };
            if enqueue_change(
                &tx,
                &self.identity,
                &self.device_id,
                key,
                keys.current_id(),
                &EntityChange {
                    entity_type,
                    entity_id,
                    deleted_at: Some(deleted_at),
                    now,
                },
            )? {
                enqueued += 1;
            }
        }

        tx.commit().map_err(RepoError::from)?;
        Ok(enqueued)
    }

    /// One full sync round: push the outbox, then pull and apply. Network
    /// failures arm an exponential backoff; calling again earlier returns
    /// [`SyncError::BackedOff`] without touching the network.
    ///
    /// The connection is taken per local phase through [`SyncDb`] and never
    /// held across network I/O, so a host passing its shared mutex keeps
    /// serving reads while this round waits on the server.
    pub fn sync<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        store: &dyn SecureStore,
        now: TimestampMs,
    ) -> Result<SyncReport, SyncError> {
        if now < self.next_attempt_at {
            return Err(SyncError::BackedOff {
                until: self.next_attempt_at,
            });
        }
        let config = {
            let guard = db.db()?;
            SyncStateRepo::new(&guard).config_get()?
        };
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let pinned = pinned_fingerprint(&config)?;

        let result = self.sync_inner(db, store, &config, &pinned, now);
        match &result {
            Err(SyncError::Transport(TransportError::Network(_)))
            | Err(SyncError::Transport(TransportError::RateLimited)) => {
                self.failures = self.failures.saturating_add(1);
                let shift = self.failures.saturating_sub(1).min(16);
                let delay = BACKOFF_BASE_MS
                    .saturating_mul(1 << shift)
                    .min(BACKOFF_CAP_MS);
                self.next_attempt_at = now + delay;
            }
            Ok(_) => {
                self.failures = 0;
                self.next_attempt_at = 0;
            }
            Err(_) => {}
        }
        result
    }

    fn sync_inner<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        store: &dyn SecureStore,
        config: &SyncConfig,
        pinned: &Fingerprint,
        now: TimestampMs,
    ) -> Result<SyncReport, SyncError> {
        let mut report = SyncReport::default();
        let mut config = config.clone();
        // Merge-resolution loop: a 409 parks the entity, the
        // pull applies the server head and three-way merges it, and the
        // next iteration pushes the merged record. A fresh 409 re-runs the
        // cycle until a round completes without a merge.
        for _ in 0..MERGE_ROUNDS_MAX {
            let mut keys = SyncKeys::load(store, config.sync_key_id)?;
            self.push(db, &keys, &mut report)?;

            let directory = self.with_auth(|t, token| t.device_directory(token))?;
            let trust = TrustState::from_directory(&directory, pinned)?;
            // During a pending recovery catch-up, pre-re-root records
            // verify only against the predecessor root persisted at
            // recovery time, so the pull runs with it as a fallback trust.
            // The marker clears only after a pull drained the server.
            let catchup = match config.recovery_root.as_deref() {
                Some(json) => {
                    let old_root = crate::directory::root_statement_from_json(json)?;
                    let old_pin = old_root.fingerprint();
                    Some((TrustState::with_root(old_root, &directory)?, old_pin))
                }
                None => None,
            };
            // Sealed key updates land before the pull: a newly
            // received K_sync generation lets this round open records that
            // would otherwise park, and unparks earlier arrivals below.
            if self.apply_key_updates(db, store, &trust, pinned)? {
                config = {
                    let guard = db.db()?;
                    SyncStateRepo::new(&guard).config_get()?
                };
                keys = SyncKeys::load(store, config.sync_key_id)?;
            }
            let merged_before = report.merged;
            let fallback = catchup.as_ref().map(|(t, f)| (t, f));
            self.pull(
                db,
                &config,
                &trust,
                pinned,
                &keys,
                now,
                &mut report,
                fallback,
            )?;
            {
                let guard = db.db()?;
                self.retry_parked(&guard, &trust, pinned, &keys, now, &mut report, fallback)?;
                if catchup.is_some() {
                    // The pull above ran to the server head with the old
                    // root available: the historical catch-up is complete.
                    SyncStateRepo::new(&guard).recovery_root_clear()?;
                    config.recovery_root = None;
                }
            }
            if report.merged == merged_before {
                break;
            }
            // The pull advanced the watermark; the next iteration must
            // start from the persisted cursor, not the stale snapshot.
            config = {
                let guard = db.db()?;
                SyncStateRepo::new(&guard).config_get()?
            };
        }

        report.pending_backlog = {
            let guard = db.db()?;
            SyncOutboxRepo::new(&guard).pending_count()?
        };
        Ok(report)
    }

    /// Re-attempts parked remote records: once the missing
    /// key generation or payload support arrives, the stored wire bytes go
    /// through the full verification again. Superseded records (a newer
    /// version already applied) are released; still-blocked records stay.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retry_parked(
        &self,
        conn: &Connection,
        trust: &TrustState,
        pinned: &Fingerprint,
        keys: &SyncKeys,
        now: TimestampMs,
        report: &mut SyncReport,
        catchup: Option<(&TrustState, &Fingerprint)>,
    ) -> Result<(), SyncError> {
        let parked = SyncStateRepo::new(conn).pending_list()?;
        if parked.is_empty() {
            return Ok(());
        }
        let mut tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        for pending in &parked {
            let Ok(wire) = pending_to_wire(pending) else {
                continue;
            };
            let head = SyncStateRepo::new(&tx)
                .shadow_get(wire.entity_type, &wire.entity_id)?
                .map(|s| s.version);
            if check_not_replayed(&FixedHead(head), &wire).is_err() {
                // A newer version was applied meanwhile; the parked record
                // is obsolete and can be released.
                SyncStateRepo::new(&tx).pending_remove(&wire.id)?;
                continue;
            }
            // A verification failure means the record is still blocked
            // (key or payload support has not arrived): it stays parked,
            // never dropped. During a recovery catch-up the predecessor
            // root serves as the fallback verifier.
            let reopened = verify_and_open(&wire, &trust.context(pinned), keys)
                .ok()
                .or_else(|| {
                    catchup.and_then(|(old_trust, old_pin)| {
                        verify_and_open(&wire, &old_trust.context(old_pin), keys).ok()
                    })
                });
            if let Some(opened) = reopened {
                let conflict =
                    SyncOutboxRepo::new(&tx).conflict_head(wire.entity_type, &wire.entity_id)?;
                let savepoint = tx.savepoint().map_err(RepoError::from)?;
                match conflict {
                    Some(local_head) => {
                        // The record that finally applies is the server
                        // head a parked conflict was waiting for: resolve
                        // it by the same three-way merge as the pull path.
                        if let Ok(copy_id) = self.merge_conflicted(
                            &savepoint,
                            &wire,
                            &opened,
                            &local_head,
                            keys,
                            now,
                            true,
                        ) {
                            SyncStateRepo::new(&savepoint).pending_remove(&wire.id)?;
                            savepoint.commit().map_err(RepoError::from)?;
                            record_merge(report, &wire, copy_id);
                        }
                    }
                    None => {
                        if apply_opened(&savepoint, &wire, &opened).is_ok() {
                            SyncStateRepo::new(&savepoint).pending_remove(&wire.id)?;
                            savepoint.commit().map_err(RepoError::from)?;
                            report.applied += 1;
                            if wire.entity_type == SyncEntityType::Snippet {
                                report.applied_snippet_ids.push(wire.entity_id.clone());
                            }
                        }
                    }
                }
            }
        }
        tx.commit().map_err(RepoError::from)?;
        Ok(())
    }

    // ----- authentication (token refresh re-runs the exchange once) -----

    fn ensure_token(&mut self) -> Result<SessionToken, SyncError> {
        if let Some(token) = &self.token {
            return Ok(token.clone());
        }
        let challenge = self.transport.auth_challenge(&self.device_id)?;
        let mut message = b"typvia.auth.v1".to_vec();
        message.extend_from_slice(&challenge);
        message.extend_from_slice(self.device_id.as_bytes());
        let signature = self.identity.sign(&message).to_bytes();
        let token = self.transport.auth_session(&self.device_id, &signature)?;
        self.token = Some(token.clone());
        Ok(token)
    }

    pub(crate) fn with_auth<R>(
        &mut self,
        call: impl Fn(&dyn SyncTransport, &SessionToken) -> Result<R, TransportError>,
    ) -> Result<R, SyncError> {
        let token = self.ensure_token()?;
        match call(self.transport.as_ref(), &token) {
            Err(TransportError::SessionExpired) => {
                self.token = None;
                let token = self.ensure_token()?;
                call(self.transport.as_ref(), &token).map_err(SyncError::from)
            }
            other => other.map_err(SyncError::from),
        }
    }

    // ----- push -----

    fn push<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        keys: &SyncKeys,
        report: &mut SyncReport,
    ) -> Result<(), SyncError> {
        loop {
            // Batch read, network send and acknowledgement are separate
            // lock phases: the connection is free while the batch is on
            // the wire.
            let batch = {
                let guard = db.db()?;
                SyncOutboxRepo::new(&guard).list_pending(MAX_PUSH_RECORDS, MAX_PUSH_BYTES)?
            };
            if batch.is_empty() {
                return Ok(());
            }
            let wires = batch
                .iter()
                .map(outbox_to_wire)
                .collect::<Result<Vec<_>, _>>()?;
            let by_id: HashMap<&str, &WireRecord> =
                wires.iter().map(|w| (w.id.as_str(), w)).collect();

            match self.with_auth(|t, token| t.push_records(token, &wires))? {
                PushOutcome::Accepted(results) => {
                    let guard = db.db()?;
                    let conn: &Connection = &guard;
                    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
                    {
                        let outbox = SyncOutboxRepo::new(&tx);
                        let state = SyncStateRepo::new(&tx);
                        for result in &results {
                            outbox.mark_applied(&result.id, result.server_seq)?;
                            let Some(wire) = by_id.get(result.id.as_str()) else {
                                continue;
                            };
                            // The accepted record becomes the shadow
                            // base: reopen our own envelope for the
                            // agreed plaintext document.
                            let document = match wire.deleted_at {
                                Some(_) => None,
                                None => Some(open_own_document(wire, keys)?),
                            };
                            state.shadow_put(&SyncShadow {
                                entity_type: wire.entity_type,
                                entity_id: wire.entity_id.clone(),
                                version: wire.version,
                                document,
                            })?;
                        }
                    }
                    tx.commit().map_err(RepoError::from)?;
                    report.pushed += results.len();
                }
                PushOutcome::VersionConflict(heads) => {
                    let guard = db.db()?;
                    let conn: &Connection = &guard;
                    let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
                    {
                        let outbox = SyncOutboxRepo::new(&tx);
                        for head in &heads {
                            if let Ok(entity_type) = head.entity_type.parse::<SyncEntityType>() {
                                outbox.mark_entity_conflicted(entity_type, &head.entity_id)?;
                            }
                        }
                    }
                    tx.commit().map_err(RepoError::from)?;
                    report.conflicts.extend(heads);
                    // Remaining pending records (other entities) go out on
                    // the next loop iteration.
                }
            }
        }
    }

    // ----- pull and apply -----

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pull<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        config: &SyncConfig,
        trust: &TrustState,
        pinned: &Fingerprint,
        keys: &SyncKeys,
        now: TimestampMs,
        report: &mut SyncReport,
        catchup: Option<(&TrustState, &Fingerprint)>,
    ) -> Result<(), SyncError> {
        let mut watermark = u64::try_from(config.applied_server_seq).unwrap_or(0);
        loop {
            // The page fetch runs without the connection; application of
            // the page takes it for exactly one transaction.
            let page =
                self.with_auth(|t, token| t.pull_records(token, watermark, PULL_PAGE_LIMIT))?;
            if page.records.is_empty() {
                return Ok(());
            }

            let guard = db.db()?;
            let conn: &Connection = &guard;
            let mut tx = conn.unchecked_transaction().map_err(RepoError::from)?;
            let mut cursor = ServerSeqCursor::new(watermark);
            for pulled in &page.records {
                cursor
                    .advance(pulled.server_seq)
                    .map_err(|_| SyncError::CursorViolation)?;
                self.apply_one(
                    &mut tx, trust, pinned, keys, pulled, now, report, catchup, true,
                )?;
            }
            // Watermark and application commit or roll back together: no
            // half state beyond re-applying, which the monotonicity check
            // makes idempotent.
            SyncStateRepo::new(&tx).advance_watermark(
                i64::try_from(cursor.position()).map_err(|_| SyncError::CursorViolation)?,
            )?;
            tx.commit().map_err(RepoError::from)?;
            watermark = cursor.position();
            if !page.has_more {
                return Ok(());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_one(
        &self,
        tx: &mut rusqlite::Transaction<'_>,
        trust: &TrustState,
        pinned: &Fingerprint,
        keys: &SyncKeys,
        pulled: &crate::transport::PulledRecord,
        now: TimestampMs,
        report: &mut SyncReport,
        catchup: Option<(&TrustState, &Fingerprint)>,
        create_conflict_copy: bool,
    ) -> Result<(), SyncError> {
        let record = &pulled.record;
        if record.device_id == self.device_id {
            report.skipped_own += 1;
            return Ok(());
        }
        let head = SyncStateRepo::new(tx)
            .shadow_get(record.entity_type, &record.entity_id)?
            .map(|s| s.version);
        if check_not_replayed(&FixedHead(head), record).is_err() {
            report.skipped += 1;
            return Ok(());
        }

        let opened = match verify_and_open(record, &trust.context(pinned), keys) {
            Ok(opened) => opened,
            Err(RecordError::UnknownKeyId(_)) => {
                self.park(tx, pulled, PendingReason::UnknownKeyId, now)?;
                report.parked += 1;
                return Ok(());
            }
            Err(RecordError::PayloadVersionAhead(_)) => {
                self.park(tx, pulled, PendingReason::PayloadAhead, now)?;
                report.parked += 1;
                return Ok(());
            }
            Err(_) => {
                // During a recovery catch-up, pre-re-root records
                // verify only against the predecessor root: try it before
                // counting a skip. The fallback only widens acceptance to
                // records the old root vouches for.
                let Some(opened) = catchup.and_then(|(old_trust, old_pin)| {
                    verify_and_open(record, &old_trust.context(old_pin), keys).ok()
                }) else {
                    report.skipped += 1;
                    return Ok(());
                };
                opened
            }
        };

        let conflict =
            SyncOutboxRepo::new(tx).conflict_head(record.entity_type, &record.entity_id)?;
        // Per-record savepoint: an application failure rolls back
        // only this record's writes and is counted as a skip.
        let savepoint = tx.savepoint().map_err(RepoError::from)?;
        if let Some(local_head) = conflict {
            // The incoming record is the server head this entity's
            // parked 409 lost against: three-way merge instead of
            // a plain overwrite.
            match self.merge_conflicted(
                &savepoint,
                record,
                &opened,
                &local_head,
                keys,
                now,
                create_conflict_copy,
            ) {
                Ok(copy_id) => {
                    SyncStateRepo::new(&savepoint).pending_remove(&record.id)?;
                    savepoint.commit().map_err(RepoError::from)?;
                    record_merge(report, record, copy_id);
                }
                Err(_) => {
                    // The merge rolls back whole (no half state);
                    // the conflict stays parked for a later round.
                    drop(savepoint);
                    report.skipped += 1;
                }
            }
            return Ok(());
        }
        match apply_opened(&savepoint, record, &opened) {
            Ok(()) => {
                SyncStateRepo::new(&savepoint).pending_remove(&record.id)?;
                savepoint.commit().map_err(RepoError::from)?;
                report.applied += 1;
                if record.entity_type == SyncEntityType::Snippet {
                    report.applied_snippet_ids.push(record.entity_id.clone());
                }
            }
            Err(PayloadError::UnsupportedEntity) => {
                drop(savepoint);
                self.park(tx, pulled, PendingReason::UnsupportedEntity, now)?;
                report.parked += 1;
            }
            Err(_) => {
                // Savepoint drop rolls this record back; the rest
                // of the page proceeds.
                drop(savepoint);
                report.skipped += 1;
            }
        }
        Ok(())
    }

    fn park(
        &self,
        conn: &Connection,
        pulled: &crate::transport::PulledRecord,
        reason: PendingReason,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        SyncStateRepo::new(conn).pending_insert(&PendingRemoteRecord {
            record: wire_to_sync_record(&pulled.record),
            key_id: pulled.record.key_id,
            signature: pulled.record.signature.to_vec(),
            server_seq: i64::try_from(pulled.server_seq).map_err(|_| SyncError::CursorViolation)?,
            reason,
            received_at: now,
        })?;
        Ok(())
    }

    // ----- conflict resolution -----

    /// Resolves one parked conflict against the verified server head, all
    /// inside the caller's savepoint (a failure rolls the whole resolution
    /// back — no half-merged state). Content vs content runs the
    /// field merge and re-queues the merged state at `server head + 1`;
    /// any tombstone on either side wins, with the edited side's
    /// content preserved in the snippet version history first. Returns the
    /// conflict-copy id when rule 3 created one.
    #[allow(clippy::too_many_arguments)]
    fn merge_conflicted(
        &self,
        conn: &Connection,
        record: &WireRecord,
        opened: &OpenedRecord,
        local_head: &OutboxRecord,
        keys: &SyncKeys,
        now: TimestampMs,
        create_conflict_copy: bool,
    ) -> Result<Option<String>, SyncError> {
        let entity_type = record.entity_type;
        let entity_id = record.entity_id.as_str();
        let key = keys
            .current_key()
            .ok_or(SyncError::Keyring(KeyringError::MissingCurrentKey(
                keys.current_id(),
            )))?;
        let state = SyncStateRepo::new(conn);
        let outbox = SyncOutboxRepo::new(conn);

        let copy_id = match (opened, local_head.record.deleted_at) {
            // Remote tombstone vs local edit: the tombstone wins.
            // The local edit enters the version history, then the entity
            // follows the tombstone into the recycle bin. Nothing is
            // re-queued: the server head already is the agreed outcome.
            (OpenedRecord::Tombstone { deleted_at }, None) => {
                if entity_type == SyncEntityType::Snippet {
                    preserve_snippet_history(conn, entity_id, now)?;
                }
                payload::apply_tombstone(conn, entity_type, entity_id, *deleted_at)
                    .map_err(payload_to_sync)?;
                state.shadow_put(&SyncShadow {
                    entity_type,
                    entity_id: entity_id.to_string(),
                    version: record.version,
                    document: None,
                })?;
                outbox.clear_entity_conflicts(entity_type, entity_id)?;
                None
            }
            // Both sides deleted: the outcomes agree; adopt the server
            // head and drop the redundant local tombstone.
            (OpenedRecord::Tombstone { deleted_at }, Some(_)) => {
                payload::apply_tombstone(conn, entity_type, entity_id, *deleted_at)
                    .map_err(payload_to_sync)?;
                state.shadow_put(&SyncShadow {
                    entity_type,
                    entity_id: entity_id.to_string(),
                    version: record.version,
                    document: None,
                })?;
                outbox.clear_entity_conflicts(entity_type, entity_id)?;
                None
            }
            // Local tombstone vs remote edit: the tombstone wins.
            // The remote edit's content enters the version history and the
            // recycle-bin row (both bins end up with the edited state),
            // and the tombstone is re-queued past the server head.
            (OpenedRecord::Content { document }, Some(local_deleted_at)) => {
                if entity_type == SyncEntityType::Snippet {
                    payload::apply_document(conn, entity_type, entity_id, document)
                        .map_err(payload_to_sync)?;
                    preserve_snippet_history(conn, entity_id, now)?;
                    SnippetRepo::new(conn).soft_delete(entity_id, local_deleted_at)?;
                }
                // Structural entities are already gone locally (hard
                // delete); the remote content is not resurrected.
                state.shadow_put(&SyncShadow {
                    entity_type,
                    entity_id: entity_id.to_string(),
                    version: record.version,
                    document: Some(document.to_vec()),
                })?;
                outbox.clear_entity_conflicts(entity_type, entity_id)?;
                enqueue_change(
                    conn,
                    &self.identity,
                    &self.device_id,
                    key,
                    keys.current_id(),
                    &EntityChange {
                        entity_type,
                        entity_id: entity_id.to_string(),
                        deleted_at: Some(local_deleted_at),
                        now,
                    },
                )?;
                None
            }
            // Content vs content: the field-level three-way merge.
            (OpenedRecord::Content { document }, None) => {
                let base = state
                    .shadow_get(entity_type, entity_id)?
                    .and_then(|s| s.document);
                let local_document = payload::build_document(conn, entity_type, entity_id)
                    .map_err(payload_to_sync)?
                    .map(zeroize::Zeroizing::new);
                let Some(local_document) = local_document else {
                    // Nothing local survives to merge: adopt the server
                    // head as-is.
                    payload::apply_document(conn, entity_type, entity_id, document)
                        .map_err(payload_to_sync)?;
                    state.shadow_put(&SyncShadow {
                        entity_type,
                        entity_id: entity_id.to_string(),
                        version: record.version,
                        document: Some(document.to_vec()),
                    })?;
                    outbox.clear_entity_conflicts(entity_type, entity_id)?;
                    return Ok(None);
                };
                let outcome = merge::merge_documents(
                    entity_type,
                    base.as_deref(),
                    &MergeSide {
                        document: &local_document,
                        updated_at: local_head.record.updated_at,
                        device_id: &self.device_id,
                    },
                    &MergeSide {
                        document,
                        updated_at: record.updated_at,
                        device_id: &record.device_id,
                    },
                )
                .map_err(payload_to_sync)?;
                payload::apply_document(conn, entity_type, entity_id, &outcome.document)
                    .map_err(payload_to_sync)?;

                // In the WebDAV form both sides run this merge; only the
                // elected side materializes the conflict copy so exactly one
                // copy exists account-wide. Server form: always.
                let copy_id = if outcome.body_conflict && create_conflict_copy {
                    Some(self.create_conflict_copy(conn, entity_id, &outcome, keys, now)?)
                } else {
                    None
                };

                // The shadow becomes the server head (the last state both
                // sides agree on); the merged state re-enters the queue
                // one version past it.
                state.shadow_put(&SyncShadow {
                    entity_type,
                    entity_id: entity_id.to_string(),
                    version: record.version,
                    document: Some(document.to_vec()),
                })?;
                outbox.clear_entity_conflicts(entity_type, entity_id)?;
                enqueue_change(
                    conn,
                    &self.identity,
                    &self.device_id,
                    key,
                    keys.current_id(),
                    &EntityChange {
                        entity_type,
                        entity_id: entity_id.to_string(),
                        deleted_at: None,
                        now,
                    },
                )?;
                copy_id
            }
        };
        Ok(copy_id)
    }

    /// Materializes the rule-3 conflict copy: a fresh snippet carrying the
    /// local body, marked `conflict_of`, queued for sync like any other
    /// new entity so both devices see the pending resolution.
    pub(crate) fn create_conflict_copy(
        &self,
        conn: &Connection,
        entity_id: &str,
        outcome: &merge::MergeOutcome,
        keys: &SyncKeys,
        now: TimestampMs,
    ) -> Result<String, SyncError> {
        let key = keys
            .current_key()
            .ok_or(SyncError::Keyring(KeyringError::MissingCurrentKey(
                keys.current_id(),
            )))?;
        let copy_id = new_id();
        let device_name = DeviceRepo::new(conn)
            .get(&self.device_id)?
            .map(|d| d.name)
            .unwrap_or_else(|| self.device_id.clone());
        let merged_folder = SnippetRepo::new(conn)
            .get(entity_id)?
            .and_then(|s| s.folder_id)
            .map_or(serde_json::Value::Null, serde_json::Value::from);
        let copy_document = merge::conflict_copy_document(
            &outcome.local_entity,
            merged_folder,
            &copy_id,
            &device_name,
            now,
        )
        .map_err(payload_to_sync)?;
        payload::apply_document(conn, SyncEntityType::Snippet, &copy_id, &copy_document)
            .map_err(payload_to_sync)?;
        enqueue_change(
            conn,
            &self.identity,
            &self.device_id,
            key,
            keys.current_id(),
            &EntityChange {
                entity_type: SyncEntityType::Snippet,
                entity_id: copy_id.clone(),
                deleted_at: None,
                now,
            },
        )?;
        Ok(copy_id)
    }
}

impl fmt::Debug for SyncEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncEngine")
            .field("device_id", &self.device_id)
            .field("failures", &self.failures)
            .finish_non_exhaustive()
    }
}

/// Books one successful merge resolution into the round report.
fn record_merge(report: &mut SyncReport, record: &WireRecord, copy_id: Option<String>) {
    report.applied += 1;
    report.merged += 1;
    if record.entity_type == SyncEntityType::Snippet {
        report.applied_snippet_ids.push(record.entity_id.clone());
    }
    if let Some(copy_id) = copy_id {
        report.applied_snippet_ids.push(copy_id.clone());
        report.conflict_copy_ids.push(copy_id);
    }
}

/// Appends the snippet's current title and body to its version history,
/// so the edit side's content survives a lost delete/edit race.
/// Skipped when the newest history entry already holds this exact state.
fn preserve_snippet_history(
    conn: &Connection,
    snippet_id: &str,
    now: TimestampMs,
) -> Result<(), SyncError> {
    let Some(snippet) = SnippetRepo::new(conn).get(snippet_id)? else {
        return Ok(());
    };
    let versions = VersionRepo::new(conn);
    let latest = versions.latest_version(snippet_id)?;
    if let Some(entry) = versions.get(snippet_id, latest)?
        && entry.title == snippet.title
        && entry.content == snippet.content
    {
        return Ok(());
    }
    versions.append(&SnippetVersion {
        id: new_id(),
        snippet_id: snippet_id.to_string(),
        version: latest.saturating_add(1),
        title: snippet.title,
        content: snippet.content,
        created_at: now,
    })?;
    versions.apply_retention(snippet_id, now)?;
    Ok(())
}

/// Maps document-layer failures onto the engine error space: repository
/// causes keep their class, shape problems surface as a malformed payload.
pub(crate) fn payload_to_sync(error: PayloadError) -> SyncError {
    match error {
        PayloadError::Repo(e) => SyncError::Repo(e),
        _ => SyncError::Record(RecordError::MalformedPayload),
    }
}

pub(crate) fn pinned_fingerprint(config: &SyncConfig) -> Result<Fingerprint, SyncError> {
    let bytes: [u8; 32] = config
        .root_fingerprint
        .as_deref()
        .and_then(|b| b.try_into().ok())
        .ok_or(SyncError::Trust(CertificateError::UnknownRoot))?;
    Ok(Fingerprint::from_bytes(bytes))
}

/// Applies one verified record and updates its shadow base atomically
/// (the caller supplies the savepoint scope).
fn apply_opened(
    conn: &Connection,
    record: &WireRecord,
    opened: &OpenedRecord,
) -> Result<(), PayloadError> {
    match opened {
        OpenedRecord::Content { document } => {
            payload::apply_document(conn, record.entity_type, &record.entity_id, document)?;
        }
        OpenedRecord::Tombstone { deleted_at } => {
            payload::apply_tombstone(conn, record.entity_type, &record.entity_id, *deleted_at)?;
        }
    }
    SyncStateRepo::new(conn).shadow_put(&SyncShadow {
        entity_type: record.entity_type,
        entity_id: record.entity_id.clone(),
        version: record.version,
        document: match opened {
            OpenedRecord::Content { document } => Some(document.to_vec()),
            OpenedRecord::Tombstone { .. } => None,
        },
    })?;
    Ok(())
}

/// Rebuilds the wire form of a parked record for re-verification.
fn pending_to_wire(
    pending: &typvia_core::model::PendingRemoteRecord,
) -> Result<WireRecord, SyncError> {
    let signature: [u8; 64] = pending
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| SyncError::Record(RecordError::BadSignature))?;
    Ok(WireRecord {
        id: pending.record.id.clone(),
        entity_type: pending.record.entity_type,
        entity_id: pending.record.entity_id.clone(),
        version: pending.record.version,
        ciphertext: pending.record.ciphertext.clone(),
        deleted_at: pending.record.deleted_at,
        updated_at: pending.record.updated_at,
        device_id: pending.record.device_id.clone(),
        key_id: pending.key_id,
        signature,
    })
}

/// Every live syncable entity in receiver-applicable order: folders
/// parents-first, then tags, then snippets, then the links, template
/// fields and app rules that reference them. Recycle-bin rows stay local
/// (their tombstones travel instead).
fn live_entities(conn: &Connection) -> Result<Vec<EntityKey>, SyncError> {
    let mut entities: Vec<EntityKey> = Vec::new();
    for folder in FolderRepo::new(conn).list_all_parents_first()? {
        entities.push((SyncEntityType::Folder, folder.id));
    }
    for tag in TagRepo::new(conn).list_all()? {
        entities.push((SyncEntityType::Tag, tag.id));
    }
    let snippets = SnippetRepo::new(conn);
    let mut offset = 0u32;
    loop {
        let page = snippets.list(EXPORT_PAGE, offset)?;
        let page_len = page.len();
        for snippet in page {
            for tag_id in snippets.tag_ids_of(&snippet.id)? {
                entities.push((
                    SyncEntityType::SnippetTag,
                    snippet_tag_entity_id(&snippet.id, &tag_id),
                ));
            }
            for field in TemplateFieldRepo::new(conn).list_by_snippet(&snippet.id)? {
                entities.push((SyncEntityType::TemplateField, field.id));
            }
            for rule in AppRuleRepo::new(conn).list_for_snippet(&snippet.id)? {
                entities.push((SyncEntityType::AppRule, rule.id));
            }
            entities.push((SyncEntityType::Snippet, snippet.id));
        }
        if page_len < EXPORT_PAGE as usize {
            break;
        }
        offset += EXPORT_PAGE;
    }
    // Snippets must land before their links/fields/rules on the receiving
    // side; re-order accordingly (folders/tags stay first).
    entities.sort_by_key(|(entity_type, _)| match entity_type {
        SyncEntityType::Folder => 0,
        SyncEntityType::Tag => 1,
        SyncEntityType::Snippet => 2,
        _ => 3,
    });
    Ok(entities)
}

/// Entity key of a queued or shadowed record.
type EntityKey = (SyncEntityType, String);

/// What the outbox would deliver per entity: the plaintext document of the
/// newest queued record, or `None` for a queued tombstone.
type QueuedHeads = HashMap<EntityKey, Option<Zeroizing<Vec<u8>>>>;

/// The plaintext state every entity with a queued record would deliver:
/// the document of its newest queued record, or `None` for a queued
/// tombstone. This is the basis the reconcile pass compares against.
fn queued_documents(conn: &Connection, keys: &SyncKeys) -> Result<QueuedHeads, SyncError> {
    let mut heads = HashMap::new();
    for entry in SyncOutboxRepo::new(conn).queued_heads()? {
        let key = (entry.record.entity_type, entry.record.entity_id.clone());
        let document = match entry.record.deleted_at {
            Some(_) => None,
            None => Some(Zeroizing::new(open_own_document(
                &outbox_to_wire(&entry)?,
                keys,
            )?)),
        };
        heads.insert(key, document);
    }
    Ok(heads)
}

/// When an entity that still has a shadow base is gone as far as sync is
/// concerned, the time it went: a recycled snippet reports its own
/// `deleted_at`, a hard-deleted row reports `now`. `None` means the entity
/// is still there (a link of a recycled snippet, say) and needs no
/// tombstone.
fn gone_locally(
    conn: &Connection,
    entity_type: SyncEntityType,
    entity_id: &str,
    now: TimestampMs,
) -> Result<Option<TimestampMs>, SyncError> {
    if entity_type == SyncEntityType::Snippet {
        return Ok(match SnippetRepo::new(conn).get(entity_id)? {
            Some(snippet) => snippet.deleted_at,
            None => Some(now),
        });
    }
    match payload::build_document(conn, entity_type, entity_id) {
        Ok(Some(_)) => Ok(None),
        Ok(None) => Ok(Some(now)),
        // No local build path (ai_action): never reconciled either way.
        Err(PayloadError::UnsupportedEntity) => Ok(None),
        Err(PayloadError::Repo(e)) => Err(e.into()),
        Err(_) => Err(SyncError::Record(RecordError::MalformedPayload)),
    }
}

/// Reopens one of our own sealed records to obtain the agreed plaintext
/// document for the shadow base.
pub(crate) fn open_own_document(wire: &WireRecord, keys: &SyncKeys) -> Result<Vec<u8>, SyncError> {
    let key = keys
        .key(wire.key_id)
        .ok_or(SyncError::Record(RecordError::UnknownKeyId(wire.key_id)))?;
    let aad = record_aad(wire.entity_type, &wire.entity_id, wire.version)?;
    let document = typvia_crypto::open(key, &aad, &wire.ciphertext)
        .map_err(|_| SyncError::Record(RecordError::DecryptFailed))?;
    Ok(document.to_vec())
}
