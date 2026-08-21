// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Device revocation and K_sync rotation, plus the receiving side of
//! sealed key updates: distribution of new generations to every verified
//! active device, late vault authorization, and the in-sync application
//! that feeds the existing UnknownKeyId parking/retry loop.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use typvia_core::model::{DomainKey, KeyDomain, TimestampMs};
use typvia_core::repo::{DeviceRepo, RepoError, SyncStateRepo, VaultKeyRepo};
use typvia_crypto::{SecureStore, SymmetricKey, aad_domain_key, wrap_key};

use crate::directory::TrustState;
use crate::engine::{SyncEngine, SyncError, pinned_fingerprint};
use crate::identity::Fingerprint;
use crate::keyring::{KeyringError, has_key, install_key};
use crate::pairing::{KeyBundle, PairingError, SealedMessage, open_key_update, seal_key_update};
use crate::provision::{
    build_key_bundle, install_sync_generations, install_vault_from_bundle, symmetric_key_from,
};
use crate::transport::KeyUpdate;

pub(crate) const REVOKE_CONTEXT: &[u8] = b"typvia.revoke.v1";
const KEY_UPDATE_FORMAT: &str = "typvia.keyupd.v1";

/// Wire form of a key-update payload (opaque to the server).
#[derive(Serialize, Deserialize)]
struct KeyUpdateDoc {
    format: String,
    sender_device_id: String,
    /// base64 sealed bundle bytes.
    sealed: String,
    /// base64 64-byte Ed25519 signature.
    signature: String,
}

/// Parses one relayed key-update payload; `None` for anything that is not
/// a well-formed current-format message (skipped, per-record isolation).
fn parse_key_update(payload: &[u8]) -> Option<(String, SealedMessage)> {
    let doc: KeyUpdateDoc = serde_json::from_slice(payload).ok()?;
    if doc.format != KEY_UPDATE_FORMAT {
        return None;
    }
    let sealed = BASE64.decode(&doc.sealed).ok()?;
    let signature: [u8; 64] = BASE64.decode(&doc.signature).ok()?.try_into().ok()?;
    Some((doc.sender_device_id, SealedMessage { sealed, signature }))
}

pub(crate) fn encode_key_update(sender_device_id: &str, message: &SealedMessage) -> Vec<u8> {
    // Serialization of plain strings cannot fail.
    serde_json::to_vec(&KeyUpdateDoc {
        format: KEY_UPDATE_FORMAT.to_string(),
        sender_device_id: sender_device_id.to_string(),
        sealed: BASE64.encode(&message.sealed),
        signature: BASE64.encode(message.signature),
    })
    .unwrap_or_default()
}

/// One account device as the device list should present it: the
/// facts come from the directory, but `verified` states whether the row's
/// certificate chain actually verifies to the pinned trust root — a
/// malicious server can list a device it cannot make verifiable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub created_at: TimestampMs,
    pub revoked_at: Option<TimestampMs>,
    pub verified: bool,
    pub is_this_device: bool,
    /// Set for the account's trust root (the first device).
    pub is_root: bool,
}

impl SyncEngine {
    /// Lists the account's devices from the server directory, pinned and
    /// verified against the local trust root. Read-only: nothing is stored
    /// and no key material is touched.
    pub fn account_devices(&mut self, conn: &Connection) -> Result<Vec<AccountDevice>, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let pinned = pinned_fingerprint(&config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        let root_device_id = trust.root().subject.device_id.clone();
        let mut devices: Vec<AccountDevice> = directory
            .devices
            .iter()
            .map(|device| AccountDevice {
                verified: trust.verified_device_keys(&pinned, &device.id).is_some(),
                is_this_device: device.id == self.device_id,
                is_root: device.id == root_device_id,
                device_id: device.id.clone(),
                name: device.name.clone(),
                platform: device.platform.clone(),
                created_at: device.created_at,
                revoked_at: device.revoked_at.or_else(|| trust.revoked_time(&device.id)),
            })
            .collect();
        // This device first (the drawing puts it centre-left), then the rest
        // oldest-first so the route order is stable between reloads.
        devices.sort_by(|a, b| {
            b.is_this_device
                .cmp(&a.is_this_device)
                .then(a.created_at.cmp(&b.created_at))
                .then(a.device_id.cmp(&b.device_id))
        });
        Ok(devices)
    }

    /// Revokes another device: the signed revocation statement goes
    /// to the server, the local device row is marked, and — because the
    /// revoked device holds the old K_sync — a rotation is forced.
    /// Returns the new K_sync generation.
    pub fn revoke_device(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        target_device_id: &str,
        master_key: Option<&SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, SyncError> {
        let mut message = Vec::with_capacity(REVOKE_CONTEXT.len() + target_device_id.len() + 8);
        message.extend_from_slice(REVOKE_CONTEXT);
        message.extend_from_slice(target_device_id.as_bytes());
        message.extend_from_slice(&now.to_le_bytes());
        let signature = self.identity.sign(&message).to_bytes();
        self.with_auth(|t, token| t.revoke_device(token, target_device_id, now, &signature))?;

        // The local directory only holds rows this client has seen; a
        // missing row is fine (the server-side mark is authoritative for
        // the challenge gate, the pulled directory for verification).
        let devices = DeviceRepo::new(conn);
        if devices.get(target_device_id)?.is_some() {
            devices.revoke(target_device_id, now)?;
        }

        self.rotate_sync_key(conn, store, master_key, now)
    }

    /// Rotates K_sync: a fresh generation (key_id + 1) enters the
    /// secure store (and, when the master key is available, the MK-wrapped
    /// canonical copy in `domain_key`), the configuration advances, and
    /// the new generation set is distributed to every verified active
    /// device through sealed key updates. History is not re-encrypted:
    /// rotation gives forward isolation only.
    pub fn rotate_sync_key(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_key: Option<&SymmetricKey>,
        now: TimestampMs,
    ) -> Result<u32, SyncError> {
        let (new_key_id, config) = self.rotate_local(conn, store, master_key, now)?;
        self.distribute_generations(conn, store, &config)?;
        Ok(new_key_id)
    }

    /// The storage-independent half of a rotation: fresh generation into
    /// the secure store (plus the MK-wrapped canonical copy when the master
    /// key is at hand) and the configuration advance. Distribution is the
    /// caller's transport-specific half (server messages / WebDAV
    /// mailboxes).
    pub(crate) fn rotate_local(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_key: Option<&SymmetricKey>,
        now: TimestampMs,
    ) -> Result<(u32, typvia_core::model::SyncConfig), SyncError> {
        let state = SyncStateRepo::new(conn);
        let mut config = state.config_get()?;
        if !config.is_active() || config.sync_key_id == 0 {
            return Err(SyncError::NotEnabled);
        }
        let new_key_id = config.sync_key_id + 1;
        let key = SymmetricKey::generate();
        install_key(store, new_key_id, &key)?;

        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        if let Some(mk) = master_key {
            let wrapped = wrap_key(
                mk,
                new_key_id,
                &aad_domain_key(KeyDomain::Sync.as_str()),
                &key,
            )
            .map_err(KeyringError::Crypto)?;
            VaultKeyRepo::new(&tx).put_domain_key(&DomainKey {
                domain: KeyDomain::Sync,
                key_id: new_key_id,
                wrapped_key: wrapped,
                created_at: now,
            })?;
        }
        config.sync_key_id = new_key_id;
        config.updated_at = now;
        SyncStateRepo::new(&tx).config_put(&config)?;
        tx.commit().map_err(RepoError::from)?;
        Ok((new_key_id, config))
    }

    /// Seals the current K_sync generation set to every verified, active,
    /// other device. Unverifiable directory entries are skipped —
    /// a malicious server cannot route key material to a forged device.
    fn distribute_generations(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        config: &typvia_core::model::SyncConfig,
    ) -> Result<(), SyncError> {
        let pinned = pinned_fingerprint(config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        let bundle = build_key_bundle(conn, store, config.sync_key_id, false, None, None)?;
        let bundle_json = bundle.to_json()?;

        for device in &directory.devices {
            if device.id == self.device_id || device.revoked_at.is_some() {
                continue;
            }
            let Some((_, x25519_pub)) = trust.verified_device_keys(&pinned, &device.id) else {
                continue;
            };
            let message = seal_key_update(&self.identity, &device.id, &x25519_pub, &bundle_json)?;
            let payload = encode_key_update(&self.device_id, &message);
            self.with_auth(|t, token| t.put_key_update(token, &device.id, &payload))?;
        }
        Ok(())
    }

    /// Grants vault access to an already-paired device after the fact:
    /// MK + all K_vault generations (and the K_sync set, so a stale device
    /// catches up in one message) travel as a sealed key update — no
    /// re-pairing.
    pub fn grant_vault_access(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        target_device_id: &str,
        master_key: &SymmetricKey,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let pinned = pinned_fingerprint(&config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
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

        let bundle = build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            true,
            Some(master_key),
            None,
        )?;
        let bundle_json = bundle.to_json()?;
        let message = seal_key_update(&self.identity, target_device_id, &x25519_pub, &bundle_json)?;
        let payload = encode_key_update(&self.device_id, &message);
        self.with_auth(|t, token| t.put_key_update(token, target_device_id, &payload))?;
        Ok(())
    }

    /// Applies pending sealed key updates during a sync round:
    /// K_sync generations this device does not hold yet are installed and
    /// the configured current generation advances. MK/K_vault grants are
    /// left for [`Self::adopt_vault_grant`] (they need the user's master
    /// password to persist). Returns whether any generation was installed.
    ///
    /// The pull starts at the persisted cursor and the cursor advances over
    /// the leading run of fully consumed messages, so a message
    /// this device cannot use yet — an unopenable bundle, or a vault grant
    /// awaiting the user's master password — holds the cursor and is seen
    /// again next round instead of being lost.
    pub(crate) fn apply_key_updates<D: crate::engine::SyncDb + ?Sized>(
        &mut self,
        db: &D,
        store: &dyn SecureStore,
        trust: &TrustState,
        pinned: &Fingerprint,
    ) -> Result<bool, SyncError> {
        // Cursor read, network fetch and installation are separate lock
        // phases: the connection is free during the fetch.
        let cursor = {
            let guard = db.db()?;
            SyncStateRepo::new(&guard).config_get()?.key_update_seq
        };
        let updates = self.with_auth(|t, token| t.list_key_updates(token, cursor))?;
        if updates.is_empty() {
            return Ok(false);
        }
        let guard = db.db()?;
        let conn: &Connection = &guard;
        let state = SyncStateRepo::new(conn);
        let vault_exists = VaultKeyRepo::new(conn).load_header()?.is_some();
        let mut installed = false;
        let mut highest = 0u32;
        let mut consumed_through = cursor;
        let mut still_consuming = true;
        for update in &updates {
            let Some(bundle) = self.open_update_bundle(trust, pinned, update) else {
                still_consuming = false;
                continue;
            };
            for generation in &bundle.k_sync {
                if has_key(store, generation.key_id)? {
                    highest = highest.max(generation.key_id);
                    continue;
                }
                let key = symmetric_key_from(&generation.key)?;
                install_key(store, generation.key_id, &key)?;
                installed = true;
                highest = highest.max(generation.key_id);
            }
            // A grant is only done with once this device has a vault; until
            // then `adopt_vault_grant` still needs to find it on the server.
            if bundle.mk.is_some() && !vault_exists {
                still_consuming = false;
            }
            if still_consuming {
                consumed_through = update.seq;
            }
        }
        let mut config = state.config_get()?;
        let mut changed = false;
        if highest > config.sync_key_id {
            config.sync_key_id = highest;
            installed = true;
            changed = true;
        }
        if consumed_through > config.key_update_seq {
            config.key_update_seq = consumed_through;
            changed = true;
        }
        if changed {
            state.config_put(&config)?;
        }
        Ok(installed)
    }

    /// Adopts a pending vault grant: scans the sealed key
    /// updates for a bundle carrying the MK, verifies and opens it, and
    /// installs the vault material re-wrapped under this device's own
    /// master password. Returns `false` when no grant is pending; refuses
    /// when a vault already exists.
    pub fn adopt_vault_grant(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_password: &[u8],
        now: TimestampMs,
    ) -> Result<bool, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        if VaultKeyRepo::new(conn).load_header()?.is_some() {
            return Err(SyncError::Pairing(PairingError::VaultAlreadyInitialized));
        }
        let pinned = pinned_fingerprint(&config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        // The round cursor never advances past an unadopted grant, so
        // starting there still sees every grant this device may adopt.
        let updates =
            self.with_auth(|t, token| t.list_key_updates(token, config.key_update_seq))?;

        // The newest valid grant wins (a re-grant supersedes older ones).
        let grant = updates.iter().rev().find_map(|update| {
            self.open_update_bundle(&trust, &pinned, update)
                .filter(|bundle| bundle.mk.is_some())
        });
        let Some(bundle) = grant else {
            return Ok(false);
        };
        // mk presence was just checked.
        let Some(mk_bytes) = &bundle.mk else {
            return Ok(false);
        };
        let mk = symmetric_key_from(mk_bytes)?;
        let sync_key_id = install_sync_generations(store, &bundle)?;

        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        install_vault_from_bundle(&tx, &bundle, &mk, master_password, now)?;
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
        Ok(true)
    }

    /// Verifies and opens one relayed key update: the sender must verify
    /// against the trust state and must not have been revoked at the time
    /// the message was created (the revocation cut-off). Anything
    /// unverifiable is skipped, never applied.
    pub(crate) fn open_update_bundle(
        &self,
        trust: &TrustState,
        pinned: &Fingerprint,
        update: &KeyUpdate,
    ) -> Option<KeyBundle> {
        let (sender_device_id, message) = parse_key_update(&update.payload)?;
        let (sender_ed25519_pub, _) = trust.verified_device_keys(pinned, &sender_device_id)?;
        if trust
            .revoked_time(&sender_device_id)
            .is_some_and(|revoked| update.created_at >= revoked)
        {
            return None;
        }
        let json = open_key_update(
            &self.identity,
            &self.device_id,
            &sender_ed25519_pub,
            &message,
        )
        .ok()?;
        KeyBundle::from_json(&json).ok()
    }
}
