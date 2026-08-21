// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The WebDAV round orchestration: the second sync backend beside the
//! server form. Everything cryptographic — sealing, verification,
//! three-way merge, shadows, replay protection — is the engine's existing
//! machinery; this module replaces only transport and sequencing:
//! per-device record streams with per-device cursors instead of the global
//! `server_seq`, and pull-before-push with WebDAV-specific conflict rules
//! instead of push-time 409s.

use rusqlite::Connection;
use typvia_core::model::{OutboxRecord, SyncConfig, SyncEntityType, TimestampMs, TransportKind};
use typvia_core::repo::{RepoError, SyncOutboxRepo, SyncStateRepo};
use typvia_crypto::SecureStore;

use crate::SyncDb;
use crate::cert::{CertificateSubject, RootStatement};
use crate::directory::{TrustState, root_statement_from_json, root_statement_to_json};
use crate::engine::{
    NewAccount, SyncEngine, SyncError, SyncReport, open_own_document, outbox_to_wire,
    pinned_fingerprint,
};
use crate::identity::DeviceIdentity;
use crate::keyring::{SyncKeys, provision_initial_key};
use crate::record::verify_and_open;
use crate::transport::{
    DeviceDirectory, DirectoryDevice, HandshakeInfo, PairClaim, PairingSession, PullPage,
    PushOutcome, ReRootOutcome, SessionToken, SyncTransport, TransportError,
};
use crate::webdav::PutOutcome;
use crate::webdav_store::WebdavStore;

/// The engine still owns a transport slot; in the WebDAV form no code path
/// reaches it (all I/O goes through [`WebdavStore`]), so the slot is filled
/// with this always-refusing stub. Reaching it is a bug, and it reports as
/// one — never as silent success.
struct NullTransport;

const NULL_TRANSPORT_MSG: &str = "webdav backend must not use the server transport";

impl SyncTransport for NullTransport {
    fn handshake(&self) -> Result<HandshakeInfo, TransportError> {
        Err(null_transport())
    }
    fn create_account(&self, _root: &RootStatement) -> Result<String, TransportError> {
        Err(null_transport())
    }
    fn auth_challenge(&self, _device_id: &str) -> Result<Vec<u8>, TransportError> {
        Err(null_transport())
    }
    fn auth_session(
        &self,
        _device_id: &str,
        _signature: &[u8; 64],
    ) -> Result<SessionToken, TransportError> {
        Err(null_transport())
    }
    fn push_records(
        &self,
        _token: &SessionToken,
        _records: &[crate::record::WireRecord],
    ) -> Result<PushOutcome, TransportError> {
        Err(null_transport())
    }
    fn pull_records(
        &self,
        _token: &SessionToken,
        _since: u64,
        _limit: u32,
    ) -> Result<PullPage, TransportError> {
        Err(null_transport())
    }
    fn device_directory(&self, _token: &SessionToken) -> Result<DeviceDirectory, TransportError> {
        Err(null_transport())
    }
    fn revoke_device(
        &self,
        _token: &SessionToken,
        _device_id: &str,
        _revoked_at: i64,
        _signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        Err(null_transport())
    }
    fn pair_begin(&self, _account_id: &str) -> Result<PairingSession, TransportError> {
        Err(null_transport())
    }
    fn pair_offer(
        &self,
        _token: &SessionToken,
        _session_id: &str,
        _certificate: &crate::cert::DeviceCertificate,
        _sealed_bundle: &[u8],
        _bundle_signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        Err(null_transport())
    }
    fn pair_claim(&self, _session_id: &str) -> Result<PairClaim, TransportError> {
        Err(null_transport())
    }
    fn put_key_update(
        &self,
        _token: &SessionToken,
        _target_device_id: &str,
        _payload: &[u8],
    ) -> Result<i64, TransportError> {
        Err(null_transport())
    }
    fn list_key_updates(
        &self,
        _token: &SessionToken,
        _since: i64,
    ) -> Result<Vec<crate::transport::KeyUpdate>, TransportError> {
        Err(null_transport())
    }
    fn put_recovery_blob(
        &self,
        _token: &SessionToken,
        _blob: &[u8],
        _rootproof_pub: &[u8],
    ) -> Result<(), TransportError> {
        Err(null_transport())
    }
    fn get_recovery_blob(&self, _account_id: &str) -> Result<Vec<u8>, TransportError> {
        Err(null_transport())
    }
    fn re_root(
        &self,
        _account_id: &str,
        _proof_signature: Option<&[u8; 64]>,
        _new_root: Option<&RootStatement>,
    ) -> Result<ReRootOutcome, TransportError> {
        Err(null_transport())
    }
}

fn null_transport() -> TransportError {
    TransportError::Network(NULL_TRANSPORT_MSG.to_string())
}

impl SyncEngine {
    /// An engine for the WebDAV form: same identity, no server transport.
    pub fn new_webdav(identity: DeviceIdentity, device_id: String) -> Self {
        Self::new(Box::new(NullTransport), identity, device_id)
    }

    /// Founds a fresh account on the user's WebDAV endpoint: claims
    /// `account.json` exclusively, publishes the self-signed root as the
    /// first directory, provisions K_sync, and activates sync pinned to the
    /// own root. An endpoint that already carries an account refuses the
    /// foundation — joining is pairing's job, never an overwrite.
    pub fn create_webdav_account(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        account: &NewAccount<'_>,
        now: TimestampMs,
    ) -> Result<String, SyncError> {
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
        let account_id = typvia_core::repo::new_id();
        dav.ensure_device_layout(&self.device_id)?;
        if dav.create_account(&account_id, now)? == PutOutcome::PreconditionFailed {
            return Err(SyncError::Transport(TransportError::Api {
                code: "WEBDAV_ACCOUNT_EXISTS".to_string(),
                status: 0,
            }));
        }
        let directory = DeviceDirectory {
            root_statement_json: root_statement_to_json(&root)?,
            devices: vec![DirectoryDevice {
                id: self.device_id.clone(),
                name: account.device_name.to_string(),
                platform: account.platform.as_str().to_string(),
                ed25519_pub: self.identity.ed25519_public().to_vec(),
                x25519_pub: self.identity.x25519_public().to_vec(),
                cert_chain_json: b"[]".to_vec(),
                created_at: now,
                revoked_at: None,
            }],
            revocations: Vec::new(),
        };
        if dav.write_directory_exclusive(&directory)? == PutOutcome::PreconditionFailed {
            return Err(SyncError::Transport(TransportError::Api {
                code: "WEBDAV_ACCOUNT_EXISTS".to_string(),
                status: 0,
            }));
        }
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
                transport_kind: TransportKind::Webdav,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )?;
        Ok(account_id)
    }

    /// Binds this device to an existing WebDAV account after pairing (or a
    /// test harness) delivered the trust root and K_sync generations —
    /// [`SyncEngine::adopt_account`] in the WebDAV form.
    pub fn adopt_webdav_account(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        adopted: &crate::engine::AdoptedAccount,
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
                transport_kind: TransportKind::Webdav,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )
    }

    /// Appends a newly certified device to the WebDAV directory through
    /// the read-merge-write loop. `chain` runs root-first and ends at the new
    /// device; readers re-verify everything against the pinned root, so
    /// this write only shapes bytes — it grants nothing by itself.
    pub fn webdav_admit_device(
        &self,
        dav: &WebdavStore,
        chain: &[crate::cert::DeviceCertificate],
    ) -> Result<(), SyncError> {
        let subject = &chain
            .last()
            .ok_or(SyncError::Trust(crate::error::CertificateError::EmptyChain))?
            .subject;
        let entry = DirectoryDevice {
            id: subject.device_id.clone(),
            name: subject.name.clone(),
            platform: subject.platform.as_str().to_string(),
            ed25519_pub: subject.ed25519_pub.to_vec(),
            x25519_pub: subject.x25519_pub.to_vec(),
            cert_chain_json: crate::directory::cert_chain_to_json(chain)?,
            created_at: subject.created_at,
            revoked_at: None,
        };
        // Optimistic-concurrency loop: bounded retries, then the
        // caller sees the contention as a plain transport failure.
        for _ in 0..8 {
            let fetched = dav
                .read_directory()?
                .ok_or(SyncError::Transport(TransportError::MalformedResponse))?;
            let mut directory = fetched.directory;
            if directory.devices.iter().any(|d| d.id == entry.id) {
                return Ok(());
            }
            directory.devices.push(entry.clone());
            let outcome = match fetched.etag {
                Some(etag) => dav.write_directory_if_match(&etag, &directory)?,
                None => {
                    return Err(SyncError::Transport(TransportError::MalformedResponse));
                }
            };
            if outcome == PutOutcome::Done {
                return Ok(());
            }
        }
        Err(SyncError::Transport(TransportError::RateLimited))
    }

    /// One WebDAV round: pull every trusted device's stream first, applying
    /// the WebDAV conflict rules, then publish the outbox. Lock discipline
    /// matches the server round: the connection is taken per local phase and
    /// never held across WebDAV I/O.
    pub fn sync_webdav<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        now: TimestampMs,
    ) -> Result<SyncReport, SyncError> {
        let config = {
            let guard = db.db()?;
            SyncStateRepo::new(&guard).config_get()?
        };
        if !config.is_active() || config.transport_kind != TransportKind::Webdav {
            return Err(SyncError::NotEnabled);
        }
        let pinned = pinned_fingerprint(&config)?;

        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        // Sealed key updates land before the pull, so a fresh
        // K_sync generation opens this round's records.
        let key_id =
            if self.apply_key_updates_webdav(db, store, dav, &directory, &trust, &pinned)? {
                let guard = db.db()?;
                SyncStateRepo::new(&guard).config_get()?.sync_key_id
            } else {
                config.sync_key_id
            };
        let keys = SyncKeys::load(store, key_id)?;
        // A pending recovery catch-up carries its predecessor root exactly
        // as in the server form.
        let catchup = match config.recovery_root.as_deref() {
            Some(json) => {
                let old_root = root_statement_from_json(json)?;
                let old_pin = old_root.fingerprint();
                Some((TrustState::with_root(old_root, &directory)?, old_pin))
            }
            None => None,
        };
        let fallback = catchup.as_ref().map(|(t, f)| (t, f));

        let mut report = SyncReport::default();
        self.pull_webdav(
            db,
            dav,
            &directory,
            &trust,
            &pinned,
            &keys,
            now,
            &mut report,
            fallback,
        )?;
        {
            let guard = db.db()?;
            self.retry_parked(&guard, &trust, &pinned, &keys, now, &mut report, fallback)?;
            if catchup.is_some() {
                SyncStateRepo::new(&guard).recovery_root_clear()?;
            }
        }
        self.push_webdav(db, dav, &keys, &mut report)?;

        report.pending_backlog = {
            let guard = db.db()?;
            SyncOutboxRepo::new(&guard).pending_count()?
        };
        Ok(report)
    }

    /// Pulls every trusted source stream from its cursor to its head. The
    /// concurrency pre-check runs per record: a pending local head for the same
    /// entity means concurrency — identical documents collapse to
    /// agreement, anything else goes through the standard three-way merge
    /// (with the conflict copy created only on the elected side).
    #[allow(clippy::too_many_arguments)]
    fn pull_webdav<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        dav: &WebdavStore,
        directory: &DeviceDirectory,
        trust: &TrustState,
        pinned: &crate::identity::Fingerprint,
        keys: &SyncKeys,
        now: TimestampMs,
        report: &mut SyncReport,
        catchup: Option<(&TrustState, &crate::identity::Fingerprint)>,
    ) -> Result<(), SyncError> {
        // Pending local heads by entity, for the concurrency pre-check.
        let queued: Vec<OutboxRecord> = {
            let guard = db.db()?;
            SyncOutboxRepo::new(&guard).queued_heads()?
        };
        let pending_by_entity =
            |entity_type: SyncEntityType, entity_id: &str| -> Option<&OutboxRecord> {
                queued.iter().find(|o| {
                    o.record.entity_type == entity_type && o.record.entity_id == entity_id
                })
            };

        for source in &directory.devices {
            if source.id == self.device_id {
                continue;
            }
            let start = {
                let guard = db.db()?;
                let cursor = SyncStateRepo::new(&guard).webdav_cursor_get(&source.id)?;
                u64::try_from(cursor).unwrap_or(1)
            };
            let head = dav.read_head(&source.id)?;
            let mut seq = start;
            // `head` is the publisher's next_seq; probing one file past it
            // heals a "record written, head not yet bumped" crash.
            loop {
                if seq >= head.saturating_add(2) {
                    break;
                }
                let pulled = match dav.read_record(&source.id, seq) {
                    Ok(Some(pulled)) => pulled,
                    Ok(None) => break,
                    // A structurally broken file is tampered or corrupted
                    // storage — per-record isolation applies: it is counted
                    // and skipped, and the round proceeds.
                    // Network-class failures still abort for a later retry.
                    Err(TransportError::MalformedResponse) => {
                        let guard = db.db()?;
                        let next = i64::try_from(seq.saturating_add(1))
                            .map_err(|_| SyncError::CursorViolation)?;
                        SyncStateRepo::new(&guard).webdav_cursor_put(&source.id, next)?;
                        report.skipped += 1;
                        seq += 1;
                        continue;
                    }
                    Err(other) => return Err(other.into()),
                };
                let record = &pulled.record;

                // Concurrency pre-check against the queued local head. The puller still holding a pending record is the
                // round's sole merger (pull-before-push makes a second
                // merger structurally impossible), so its merge creates the
                // conflict copy exactly like the server form.
                if let Some(local) = pending_by_entity(record.entity_type, &record.entity_id) {
                    let same_content = !record.is_tombstone()
                        && !local.record.is_tombstone()
                        && matches!(
                            (
                                verify_and_open(record, &trust.context(pinned), keys),
                                outbox_to_wire(local).and_then(|w| open_own_document(&w, keys)),
                            ),
                            (
                                Ok(crate::record::OpenedRecord::Content { document }),
                                Ok(own),
                            ) if *document == own
                        );
                    let guard = db.db()?;
                    let outbox = SyncOutboxRepo::new(&guard);
                    if same_content {
                        // Agreement: both sides independently
                        // produced the same document — adopt it as the
                        // shared head instead of merging forever.
                        let seq_i64 = i64::try_from(pulled.server_seq)
                            .map_err(|_| SyncError::CursorViolation)?;
                        outbox.mark_applied(&local.record.id, seq_i64)?;
                        let document = open_own_document(&outbox_to_wire(local)?, keys)?;
                        SyncStateRepo::new(&guard).shadow_put(&typvia_core::model::SyncShadow {
                            entity_type: record.entity_type,
                            entity_id: record.entity_id.clone(),
                            version: record.version.max(local.record.version),
                            document: Some(document),
                        })?;
                        SyncStateRepo::new(&guard)
                            .webdav_cursor_put(&source.id, seq_i64.saturating_add(1))?;
                        report.skipped += 1;
                        seq += 1;
                        continue;
                    }
                    // Divergent concurrency: the standard three-way merge.
                    outbox.mark_entity_conflicted(record.entity_type, &record.entity_id)?;
                } else if self.resolve_rival_publish(
                    db, trust, pinned, keys, &pulled, &source.id, now, report,
                )? {
                    // The rival-publish race consumed the record (adopt or
                    // skip); the cursor already advanced.
                    seq += 1;
                    continue;
                }

                let guard = db.db()?;
                let conn: &Connection = &guard;
                let mut tx = conn.unchecked_transaction().map_err(RepoError::from)?;
                self.apply_one(
                    &mut tx, trust, pinned, keys, &pulled, now, report, catchup, true,
                )?;
                let next =
                    i64::try_from(seq.saturating_add(1)).map_err(|_| SyncError::CursorViolation)?;
                SyncStateRepo::new(&tx).webdav_cursor_put(&source.id, next)?;
                tx.commit().map_err(RepoError::from)?;
                seq += 1;
            }
        }
        Ok(())
    }

    /// Mailbox send side: seals the current K_sync generation set to every
    /// verified, active, other device via its WebDAV mailbox.
    fn distribute_generations_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        config: &SyncConfig,
        dav: &WebdavStore,
    ) -> Result<(), SyncError> {
        let pinned = pinned_fingerprint(config)?;
        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        let bundle =
            crate::provision::build_key_bundle(conn, store, config.sync_key_id, false, None, None)?;
        let bundle_json = bundle.to_json()?;
        for device in &directory.devices {
            if device.id == self.device_id || device.revoked_at.is_some() {
                continue;
            }
            let Some((_, x25519_pub)) = trust.verified_device_keys(&pinned, &device.id) else {
                continue;
            };
            let message = crate::pairing::seal_key_update(
                &self.identity,
                &device.id,
                &x25519_pub,
                &bundle_json,
            )?;
            let payload = crate::rotation::encode_key_update(&self.device_id, &message);
            dav.publish_key_update(&device.id, &self.device_id, &payload)?;
        }
        Ok(())
    }

    /// K_sync rotation in the WebDAV form (semantics unchanged): local
    /// rotation plus mailbox distribution.
    pub fn rotate_sync_key_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        master_key: Option<&typvia_crypto::SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, SyncError> {
        let (new_key_id, config) = self.rotate_local(conn, store, master_key, now)?;
        self.distribute_generations_webdav(conn, store, &config, dav)?;
        Ok(new_key_id)
    }

    /// Revokes a device in the WebDAV form: the directory file gains the
    /// revocation mark through the conditional-update loop —
    /// client verification is the authority exactly as in the server form —
    /// then the forced rotation blinds the revoked device to new records.
    pub fn revoke_device_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        target_device_id: &str,
        master_key: Option<&typvia_crypto::SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, SyncError> {
        for _ in 0..8 {
            let fetched = dav
                .read_directory()?
                .ok_or(SyncError::Transport(TransportError::MalformedResponse))?;
            let mut directory = fetched.directory;
            let mut changed = false;
            for device in &mut directory.devices {
                if device.id == target_device_id && device.revoked_at.is_none() {
                    device.revoked_at = Some(now);
                    changed = true;
                }
            }
            if !directory
                .revocations
                .iter()
                .any(|r| r.device_id == target_device_id)
            {
                directory
                    .revocations
                    .push(crate::transport::DirectoryRevocation {
                        device_id: target_device_id.to_string(),
                        revoked_at: now,
                    });
                changed = true;
            }
            if !changed {
                break;
            }
            let Some(etag) = fetched.etag else {
                return Err(SyncError::Transport(TransportError::MalformedResponse));
            };
            if dav.write_directory_if_match(&etag, &directory)? == PutOutcome::Done {
                break;
            }
        }
        let devices = typvia_core::repo::DeviceRepo::new(conn);
        if devices.get(target_device_id)?.is_some() {
            devices.revoke(target_device_id, now)?;
        }
        self.rotate_sync_key_webdav(conn, store, dav, master_key, now)
    }

    /// Mailbox receive side: walks this device's mailboxes (one per sender
    /// in the directory), installs what opens, and deletes consumed messages
    /// — the receiver is its own acknowledgment, so an unusable message
    /// stays in place and is seen again.
    /// Returns whether any generation was installed.
    fn apply_key_updates_webdav<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        directory: &DeviceDirectory,
        trust: &TrustState,
        pinned: &crate::identity::Fingerprint,
    ) -> Result<bool, SyncError> {
        let mut installed = false;
        let mut highest = 0u32;
        let vault_exists = {
            let guard = db.db()?;
            typvia_core::repo::VaultKeyRepo::new(&guard)
                .load_header()?
                .is_some()
        };
        for sender in &directory.devices {
            if sender.id == self.device_id {
                continue;
            }
            let head = dav.read_key_update_head(&self.device_id, &sender.id)?;
            for seq in 1..head.saturating_add(2) {
                let Some(payload) = dav.read_key_update(&self.device_id, &sender.id, seq)? else {
                    continue;
                };
                let update = crate::transport::KeyUpdate {
                    seq: i64::try_from(seq).unwrap_or(i64::MAX),
                    payload,
                    created_at: 0,
                };
                let Some(bundle) = self.open_update_bundle(trust, pinned, &update) else {
                    // Unopenable today (sender cert not yet visible, etc.):
                    // stays in place for a later round.
                    continue;
                };
                for generation in &bundle.k_sync {
                    if crate::keyring::has_key(store, generation.key_id)? {
                        highest = highest.max(generation.key_id);
                        continue;
                    }
                    let key = crate::provision::symmetric_key_from(&generation.key)?;
                    crate::keyring::install_key(store, generation.key_id, &key)?;
                    installed = true;
                    highest = highest.max(generation.key_id);
                }
                if bundle.mk.is_some() && !vault_exists {
                    // A pending vault grant: adoption needs the user's
                    // master password; keep it.
                    continue;
                }
                dav.delete_key_update(&self.device_id, &sender.id, seq)?;
            }
        }
        if highest > 0 {
            let guard = db.db()?;
            let state = SyncStateRepo::new(&guard);
            let mut config = state.config_get()?;
            if highest > config.sync_key_id {
                config.sync_key_id = highest;
                state.config_put(&config)?;
                installed = true;
            }
        }
        Ok(installed)
    }

    /// Grants vault access to an already-paired device in the WebDAV form:
    /// the sealed bundle — MK, every K_vault generation, and the K_sync set
    /// — goes into the target's mailbox.
    pub fn grant_vault_access_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        target_device_id: &str,
        master_key: &typvia_crypto::SymmetricKey,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let pinned = pinned_fingerprint(&config)?;
        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        if trust
            .revoked_time(target_device_id)
            .is_some_and(|r| r <= now)
        {
            return Err(SyncError::Record(crate::error::RecordError::DeviceRevoked));
        }
        let (_, x25519_pub) = trust
            .verified_device_keys(&pinned, target_device_id)
            .ok_or(SyncError::Record(crate::error::RecordError::UnknownDevice))?;

        let bundle = crate::provision::build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            true,
            Some(master_key),
            None,
        )?;
        let bundle_json = bundle.to_json()?;
        let message = crate::pairing::seal_key_update(
            &self.identity,
            target_device_id,
            &x25519_pub,
            &bundle_json,
        )?;
        let payload = crate::rotation::encode_key_update(&self.device_id, &message);
        dav.publish_key_update(target_device_id, &self.device_id, &payload)?;
        Ok(())
    }

    /// Adopts a pending vault grant in the WebDAV form: scans this
    /// device's mailboxes for the newest bundle carrying the MK, installs
    /// the vault material re-wrapped under this device's own master
    /// password, and deletes the adopted message (the receiver is its own
    /// acknowledgment). Returns `false` when no grant is pending; refuses
    /// when a vault already exists.
    pub fn adopt_vault_grant_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &WebdavStore,
        master_password: &[u8],
        now: TimestampMs,
    ) -> Result<bool, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        if typvia_core::repo::VaultKeyRepo::new(conn)
            .load_header()?
            .is_some()
        {
            return Err(SyncError::Pairing(
                crate::pairing::PairingError::VaultAlreadyInitialized,
            ));
        }
        let pinned = pinned_fingerprint(&config)?;
        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;

        // The newest valid grant across all sender mailboxes wins.
        let mut newest: Option<(String, u64, crate::pairing::KeyBundle)> = None;
        for sender in &directory.devices {
            if sender.id == self.device_id {
                continue;
            }
            let head = dav.read_key_update_head(&self.device_id, &sender.id)?;
            for seq in 1..head.saturating_add(2) {
                let Some(payload) = dav.read_key_update(&self.device_id, &sender.id, seq)? else {
                    continue;
                };
                let update = crate::transport::KeyUpdate {
                    seq: i64::try_from(seq).unwrap_or(i64::MAX),
                    payload,
                    created_at: 0,
                };
                if let Some(bundle) = self
                    .open_update_bundle(&trust, &pinned, &update)
                    .filter(|bundle| bundle.mk.is_some())
                {
                    newest = Some((sender.id.clone(), seq, bundle));
                }
            }
        }
        let Some((sender_id, seq, bundle)) = newest else {
            return Ok(false);
        };
        let Some(mk_bytes) = &bundle.mk else {
            return Ok(false);
        };
        let mk = crate::provision::symmetric_key_from(mk_bytes)?;
        let sync_key_id = crate::provision::install_sync_generations(store, &bundle)?;

        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        crate::provision::install_vault_from_bundle(&tx, &bundle, &mk, master_password, now)?;
        {
            let state = SyncStateRepo::new(&tx);
            let mut config = state.config_get()?;
            if sync_key_id > config.sync_key_id {
                config.sync_key_id = sync_key_id;
                config.updated_at = now;
                state.config_put(&config)?;
            }
        }
        tx.commit().map_err(RepoError::from)?;
        dav.delete_key_update(&self.device_id, &sender_id, seq)?;
        Ok(true)
    }

    /// The rival-publish race: both devices published the same entity
    /// version before pulling each other. Detected as "incoming version ==
    /// shadow version, bytes differ, and this device published its own
    /// record at that version". Resolution is deterministic on the
    /// (updated_at, device_id) absolute order: the winner's document is the
    /// head everywhere; the loser — exactly one side — preserves its own
    /// body as the single conflict copy. Returns true when the record was
    /// consumed here (cursor already advanced).
    #[allow(clippy::too_many_arguments)]
    fn resolve_rival_publish<D: SyncDb + ?Sized>(
        &self,
        db: &D,
        trust: &TrustState,
        pinned: &crate::identity::Fingerprint,
        keys: &SyncKeys,
        pulled: &crate::transport::PulledRecord,
        source_device_id: &str,
        now: TimestampMs,
        report: &mut SyncReport,
    ) -> Result<bool, SyncError> {
        let record = &pulled.record;
        if record.is_tombstone() {
            return Ok(false);
        }
        let guard = db.db()?;
        let conn: &Connection = &guard;
        let state = SyncStateRepo::new(conn);
        let Some(shadow) = state.shadow_get(record.entity_type, &record.entity_id)? else {
            return Ok(false);
        };
        let Some(own_document) = shadow.document.clone() else {
            return Ok(false);
        };
        if shadow.version != record.version {
            return Ok(false);
        }
        let Some(own) = SyncOutboxRepo::new(conn).applied_at_version(
            record.entity_type,
            &record.entity_id,
            record.version,
        )?
        else {
            return Ok(false);
        };
        if own.record.device_id != self.device_id {
            return Ok(false);
        }
        // Full verification before any decision: an unverifiable rival is
        // just a skipped record for the ordinary path.
        let opened = match verify_and_open(record, &trust.context(pinned), keys) {
            Ok(crate::record::OpenedRecord::Content { document }) => document,
            _ => return Ok(false),
        };
        if *opened == own_document {
            // Same bytes: agreement; the ordinary replay-skip is correct.
            return Ok(false);
        }
        let seq_next = i64::try_from(pulled.server_seq.saturating_add(1))
            .map_err(|_| SyncError::CursorViolation)?;
        let own_wins = (own.record.updated_at, own.record.device_id.as_str())
            > (record.updated_at, record.device_id.as_str());
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        if own_wins {
            // The rival side resolves symmetrically by adopting this head.
            SyncStateRepo::new(&tx).webdav_cursor_put(source_device_id, seq_next)?;
            tx.commit().map_err(RepoError::from)?;
            report.skipped += 1;
            return Ok(true);
        }
        // Incoming wins: adopt it as the head at the same version; this
        // side is the loser and keeps its own body as the conflict copy.
        crate::payload::apply_document(&tx, record.entity_type, &record.entity_id, &opened)
            .map_err(crate::engine::payload_to_sync)?;
        SyncStateRepo::new(&tx).shadow_put(&typvia_core::model::SyncShadow {
            entity_type: record.entity_type,
            entity_id: record.entity_id.clone(),
            version: record.version,
            document: Some(opened.to_vec()),
        })?;
        if record.entity_type == SyncEntityType::Snippet {
            let local_entity: serde_json::Value = serde_json::from_slice(&own_document)
                .ok()
                .and_then(|mut doc: serde_json::Value| {
                    doc.get_mut("entity").map(serde_json::Value::take)
                })
                .ok_or(SyncError::Transport(TransportError::MalformedResponse))?;
            let outcome = crate::merge::MergeOutcome {
                document: zeroize::Zeroizing::new(opened.to_vec()),
                body_conflict: true,
                local_entity,
            };
            let copy_id = self.create_conflict_copy(&tx, &record.entity_id, &outcome, keys, now)?;
            report.conflict_copy_ids.push(copy_id);
        }
        SyncStateRepo::new(&tx).webdav_cursor_put(source_device_id, seq_next)?;
        tx.commit().map_err(RepoError::from)?;
        report.applied += 1;
        if record.entity_type == SyncEntityType::Snippet {
            report.applied_snippet_ids.push(record.entity_id.clone());
        }
        Ok(true)
    }

    /// Publishes the outbox into this device's own stream: one exclusive
    /// PUT per record with sequence self-healing, then the head counter.
    /// Bookkeeping mirrors the server push acknowledgment (applied state +
    /// shadow base per record).
    fn push_webdav<D: SyncDb + ?Sized>(
        &mut self,
        db: &D,
        dav: &WebdavStore,
        keys: &SyncKeys,
        report: &mut SyncReport,
    ) -> Result<(), SyncError> {
        dav.ensure_device_layout(&self.device_id)?;
        let mut next_seq = {
            let guard = db.db()?;
            let seq = SyncStateRepo::new(&guard).config_get()?.webdav_push_seq;
            u64::try_from(seq).unwrap_or(0).saturating_add(1)
        };
        let mut published_any = false;
        loop {
            let batch: Vec<OutboxRecord> = {
                let guard = db.db()?;
                SyncOutboxRepo::new(&guard).list_pending(64, 4 * 1024 * 1024)?
            };
            if batch.is_empty() {
                break;
            }
            for entry in &batch {
                let wire = outbox_to_wire(entry)?;
                // Sequence self-healing: a collision means a prior
                // run published this slot before crashing; move on.
                loop {
                    match dav.publish_record(&self.device_id, next_seq, &wire)? {
                        PutOutcome::Done => break,
                        PutOutcome::PreconditionFailed => {
                            next_seq = next_seq.saturating_add(1);
                        }
                    }
                }
                published_any = true;
                let guard = db.db()?;
                let conn: &Connection = &guard;
                let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
                {
                    let outbox = SyncOutboxRepo::new(&tx);
                    let state = SyncStateRepo::new(&tx);
                    let seq_i64 =
                        i64::try_from(next_seq).map_err(|_| SyncError::CursorViolation)?;
                    outbox.mark_applied(&entry.record.id, seq_i64)?;
                    let document = match wire.deleted_at {
                        Some(_) => None,
                        None => Some(open_own_document(&wire, keys)?),
                    };
                    state.shadow_put(&typvia_core::model::SyncShadow {
                        entity_type: wire.entity_type,
                        entity_id: wire.entity_id.clone(),
                        version: wire.version,
                        document,
                    })?;
                    let mut config = state.config_get()?;
                    config.webdav_push_seq = seq_i64;
                    state.config_put(&config)?;
                }
                tx.commit().map_err(RepoError::from)?;
                report.pushed += 1;
                next_seq = next_seq.saturating_add(1);
            }
        }
        if published_any {
            dav.write_head(&self.device_id, next_seq)?;
        }
        Ok(())
    }
}
