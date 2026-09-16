// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The sync surface.
//!
//! Each call takes the host guards in the fixed `sync` → `conn` → `vault`
//! order, hands off to the engine, and maps the failure. No decision about
//! trust, ordering or merging is made here.
//!
//! Two things this boundary deliberately refuses to make easy. Pairing has no
//! call that installs anything without a confirmed short authentication
//! string — the comparison is what defeats a relaying server, so there is no
//! way past it to offer. And a recovery code crosses exactly once, on the
//! call that generates it; nothing here can ask for it again.

use typvia_core::model::TimestampMs;
use typvia_core::repo::SnippetRepo;
use typvia_core::repo::SyncStateRepo;
use typvia_core::vault::VaultSession;
use typvia_host_service::dto::ConflictKeep;
use typvia_host_service::error::IpcError;
use typvia_host_service::service;
use typvia_sync::{
    NewAccount, PairingCode, RecoveryCode, RecoveryRequest, SyncFailure, SyncReport,
};

use super::{
    SyncHost, WEBDAV_CREDENTIALS_ENTRY, map_sync, mark_recovery_exported, recovery_exported_at,
    setup_state,
};
use crate::error::CoreError;
use crate::model::Snippet;
use crate::service::{TypviaCore, now_ms};

/// Why the last round stopped, as the screens are allowed to know it.
///
/// A category, never the underlying message. The engine's errors carry the
/// HTTP stack's text and the server's own words, and those can name the
/// server, the account, or a path; what a reader can act on is which kind of
/// thing went wrong, and that is all that crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SyncFailureKind {
    Unreachable,
    ServerAddress,
    Auth,
    ProtocolVersion,
    Busy,
    ServerRefused,
    ServerUnexpected,
    Trust,
    NotSetUp,
    ThisDevice,
}

impl From<SyncFailure> for SyncFailureKind {
    fn from(failure: SyncFailure) -> Self {
        match failure {
            SyncFailure::Unreachable => Self::Unreachable,
            SyncFailure::ServerAddress => Self::ServerAddress,
            SyncFailure::Auth => Self::Auth,
            SyncFailure::ProtocolVersion => Self::ProtocolVersion,
            SyncFailure::Busy => Self::Busy,
            SyncFailure::ServerRefused => Self::ServerRefused,
            SyncFailure::ServerUnexpected => Self::ServerUnexpected,
            SyncFailure::Trust => Self::Trust,
            SyncFailure::NotSetUp => Self::NotSetUp,
            SyncFailure::ThisDevice => Self::ThisDevice,
        }
    }
}

/// Everything the sync screen needs to describe the current state without a
/// second round trip.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncStatus {
    /// False when the platform secure store is unavailable: sync cannot be
    /// set up here, and the screen says so instead of offering a dead button.
    pub available: bool,
    /// An account is bound, even while sync is switched off.
    pub configured: bool,
    pub enabled: bool,
    pub server_url: Option<String>,
    pub account_id: Option<String>,
    pub device_id: String,
    pub device_name: String,
    /// Current key generation; rises on every rotation.
    pub key_generation: u32,
    /// Records still waiting in the outbox.
    pub pending_backlog: u64,
    /// Conflict copies awaiting a decision.
    pub conflict_count: u32,
    pub last_sync_at: Option<i64>,
    /// Why the last round stopped, when one did. Absent once a round
    /// finishes — so a screen can state the cause without inventing one and
    /// without keeping a stale one on display.
    pub last_failure: Option<SyncFailureKind>,
    /// A vault exists on this device; recovery codes need its master key.
    pub vault_ready: bool,
    pub vault_unlocked: bool,
    /// When this device last exported a recovery code.
    pub recovery_exported_at: Option<i64>,
    /// A recovery finished but its historical catch-up has not drained; the
    /// rounds resume it on their own.
    pub recovery_catchup_pending: bool,
    /// Which backend the account is bound to: `server` | `webdav`.
    pub transport_kind: String,
}

/// One row of the device list.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    /// The certificate chain verifies to the pinned trust root.
    pub verified: bool,
    pub is_this_device: bool,
    pub is_root: bool,
}

/// What one round did. Counts only — never which entities.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncRound {
    pub pushed: u32,
    pub applied: u32,
    pub merged: u32,
    pub conflict_copies: u32,
    pub parked: u32,
    pub skipped: u32,
    pub pending_backlog: u64,
    pub at: i64,
}

/// Packs a WebDAV username and password into the one string the transport
/// stores and reads.
///
/// Exported rather than spelled out on each screen: the shape of that string
/// belongs to the sync crate, and a screen that writes its own copy will still
/// be writing the old one after it changes — which shows up as an account
/// refusing a password the reader typed correctly.
///
/// Returns nil for a folder that needs no credentials, which is not the same
/// as a folder handed empty ones.
#[uniffi::export]
pub fn webdav_credentials(username: String, password: String) -> Option<String> {
    typvia_sync::WebdavCredentials::basic(&username, &password)
}

/// A started pairing session on the joining device.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingStart {
    /// The payload behind the QR code, and the paste fallback.
    pub code: String,
    pub session_id: String,
    /// How long the server said the session is good for. Absent when nobody
    /// promised a window (the WebDAV form has no session server), and the
    /// screen then says nothing about validity instead of making a number up.
    pub expires_in_seconds: Option<i64>,
}

/// The trusted device's view of a scanned code: the short authentication
/// string plus who is asking to join.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingSas {
    pub sas: String,
    /// The name the joining device gave itself, so the user knows which two
    /// screens they are comparing.
    pub device_name: String,
    pub platform: String,
    /// Whether this account has a vault at all — access can only be granted
    /// where there is something to grant.
    pub vault_ready: bool,
}

/// The joining device's view once the offer arrived: the same short
/// authentication string, computed independently. Nothing is installed until
/// the user confirms it matches.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingClaim {
    pub sas: String,
    /// Display form of the account's trust-root fingerprint.
    pub root_fingerprint: String,
}

/// A generated recovery code, returned exactly once.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecoveryCodeOut {
    /// Hyphen-grouped display form. Never persisted, never logged.
    pub code: String,
    pub account_id: String,
    pub server_url: String,
}

/// One snippet awaiting a conflict decision: the body that stayed on the
/// entity and the parked copy carrying the other device's body. A sensitive
/// pair arrives with both bodies absent — a locked conflict reads as two
/// locked rows, never decrypted for the comparison.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConflictPair {
    pub source: Snippet,
    pub copy: Snippet,
    pub sensitive: bool,
}

/// Which body survives a conflict decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ConflictResolution {
    /// Keep the snippet's current body; the copy moves to the recycle bin.
    Source,
    /// Move the copy's body onto the snippet as a new version; the copy then
    /// moves to the recycle bin.
    Copy,
    /// Keep both: the copy becomes an ordinary independent snippet.
    Both,
}

impl From<ConflictResolution> for ConflictKeep {
    fn from(keep: ConflictResolution) -> Self {
        match keep {
            ConflictResolution::Source => Self::Source,
            ConflictResolution::Copy => Self::Copy,
            ConflictResolution::Both => Self::Both,
        }
    }
}

fn round_out(report: &SyncReport, now: TimestampMs) -> SyncRound {
    SyncRound {
        pushed: u32::try_from(report.pushed).unwrap_or(u32::MAX),
        applied: u32::try_from(report.applied).unwrap_or(u32::MAX),
        merged: u32::try_from(report.merged).unwrap_or(u32::MAX),
        conflict_copies: u32::try_from(report.conflict_copy_ids.len()).unwrap_or(u32::MAX),
        parked: u32::try_from(report.parked).unwrap_or(u32::MAX),
        skipped: u32::try_from(report.skipped).unwrap_or(u32::MAX),
        pending_backlog: report.pending_backlog,
        at: now,
    }
}

impl TypviaCore {
    /// The status shape, assembled while the caller already holds the guards.
    fn status_of(
        &self,
        conn: &rusqlite::Connection,
        vault: &mut VaultSession,
        host: &SyncHost,
    ) -> Result<SyncStatus, IpcError> {
        let setup = setup_state(conn)?;
        let conflicts = SnippetRepo::new(conn).list_conflict_copies()?.len();
        Ok(SyncStatus {
            available: host.available,
            configured: setup.account_id.is_some(),
            enabled: setup.enabled,
            server_url: setup.server_url,
            account_id: setup.account_id,
            device_id: host.device_id.clone(),
            device_name: host.device_name.clone(),
            key_generation: setup.key_generation,
            pending_backlog: setup.pending_backlog,
            conflict_count: u32::try_from(conflicts).unwrap_or(u32::MAX),
            last_sync_at: host.last_sync_at,
            last_failure: host.last_failure.map(SyncFailureKind::from),
            vault_ready: VaultSession::is_initialized(conn)?,
            vault_unlocked: vault.is_unlocked(),
            recovery_exported_at: recovery_exported_at(self.sync_dir()),
            recovery_catchup_pending: setup.recovery_catchup_pending,
            transport_kind: setup.transport_kind,
        })
    }

    /// Validates and persists WebDAV credentials; `None` clears the entry.
    fn store_webdav_credentials(&self, credentials: Option<&str>) -> Result<(), IpcError> {
        let store = self.secure_store();
        match credentials {
            Some(raw) => {
                typvia_sync::WebdavCredentials::parse(raw).map_err(super::map_transport)?;
                store
                    .store(WEBDAV_CREDENTIALS_ENTRY, raw.as_bytes())
                    .map_err(|_| IpcError::system())?;
            }
            None => {
                store
                    .remove(WEBDAV_CREDENTIALS_ENTRY)
                    .map_err(|_| IpcError::system())?;
            }
        }
        Ok(())
    }
}

#[uniffi::export]
impl TypviaCore {
    pub fn sync_status(&self) -> Result<SyncStatus, CoreError> {
        let host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Creates the account on `server_url` with this device as its trust root
    /// and starts queueing local changes.
    pub fn sync_enable(&self, server_url: String) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let platform = service::current_platform()
            .ok_or_else(|| IpcError::conflict("this platform cannot join an account"))?;
        host.attach_engine(self.secure_store(), &server_url)?;
        let device_name = host.device_name.clone();
        let account = NewAccount {
            device_name: &device_name,
            platform,
            server_url: &server_url,
            master_key: None,
        };
        let engine = host.engine_mut()?;
        vault
            .with_master_key(|master_key| {
                engine.create_account(
                    &conn,
                    self.secure_store(),
                    &NewAccount {
                        master_key,
                        ..account
                    },
                    now,
                )
            })
            .map_err(map_sync)?;
        let key_id = SyncStateRepo::new(&conn).config_get()?.sync_key_id;
        host.register_sealer(&conn, self.secure_store(), key_id)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Founds an account on the user's own WebDAV endpoint.
    pub fn sync_enable_webdav(
        &self,
        base_url: String,
        credentials: Option<String>,
    ) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let platform = service::current_platform()
            .ok_or_else(|| IpcError::conflict("this platform cannot join an account"))?;
        let store = self.secure_store();
        self.store_webdav_credentials(credentials.as_deref())?;
        host.attach_webdav(store, &base_url)?;
        host.webdav()?
            .client()
            .probe_preconditions(&typvia_core::repo::new_id())
            .map_err(super::map_transport)?;
        let device_name = host.device_name.clone();
        {
            let (engine, dav) = host.engine_and_webdav()?;
            vault
                .with_master_key(|master_key| {
                    engine.create_webdav_account(
                        &conn,
                        store,
                        dav,
                        &NewAccount {
                            device_name: &device_name,
                            platform,
                            server_url: &base_url,
                            master_key,
                        },
                        now,
                    )
                })
                .map_err(map_sync)?;
        }
        let key_id = SyncStateRepo::new(&conn).config_get()?.sync_key_id;
        host.register_sealer(&conn, store, key_id)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Turns sync off. Everything stays — the queue, the keys, the account
    /// binding — so switching it back on resumes instead of re-pairing.
    pub fn sync_disable(&self) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        host.engine_mut()?.disable(&conn, now).map_err(map_sync)?;
        host.detach_sealer();
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Switches sync back on for the already-bound account, queueing whatever
    /// changed while it was off.
    pub fn sync_resume(&self) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let repo = SyncStateRepo::new(&conn);
        let mut config = repo.config_get()?;
        let server_url = config
            .server_url
            .clone()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        config.enabled = true;
        config.updated_at = now;
        repo.config_put(&config)?;
        host.attach_engine(self.secure_store(), &server_url)?;
        host.register_sealer(&conn, self.secure_store(), config.sync_key_id)?;
        host.reconcile(&conn, self.secure_store(), config.sync_key_id, now)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Runs one push, pull and merge round now.
    ///
    /// Takes the connection mutex rather than a guard: the engine locks it per
    /// local phase and releases it across network I/O, so reads elsewhere keep
    /// flowing while the round is on the wire.
    pub fn sync_now(&self) -> Result<SyncRound, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let report = host.run_engine_round(&self.conn_handle(), self.secure_store(), now)?;
        // The search index sits above the sync crate in the dependency order,
        // so applied records are re-indexed here rather than in the engine.
        let conn = self.conn()?;
        service::reindex_snippets(&conn, &report.applied_snippet_ids)?;
        host.last_sync_at = Some(now);
        Ok(round_out(&report, now))
    }

    pub fn sync_devices(&self) -> Result<Vec<SyncDevice>, CoreError> {
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let devices = host
            .engine_mut()?
            .account_devices(&conn)
            .map_err(map_sync)?;
        Ok(devices
            .into_iter()
            .map(|device| SyncDevice {
                device_id: device.device_id,
                name: device.name,
                platform: device.platform,
                created_at: device.created_at,
                revoked_at: device.revoked_at,
                verified: device.verified,
                is_this_device: device.is_this_device,
                is_root: device.is_root,
            })
            .collect())
    }

    /// Revokes another device and rotates the sync key in the same call. A
    /// revoked device holds the old generation, so leaving it in place would
    /// make the revocation cosmetic.
    pub fn sync_revoke_device(&self, device_id: String) -> Result<u32, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let vault = self.vault()?;
        if device_id == host.device_id {
            return Err(IpcError::validation("a device cannot revoke itself").into());
        }
        let store = self.secure_store();
        let key_id = vault.with_master_key(|master_key| {
            host.revoke_on_backend(&conn, store, &device_id, master_key, now)
        })?;
        host.register_sealer(&conn, store, key_id)?;
        Ok(key_id)
    }

    pub fn sync_rotate_key(&self) -> Result<u32, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let vault = self.vault()?;
        let store = self.secure_store();
        let key_id = vault
            .with_master_key(|master_key| host.rotate_on_backend(&conn, store, master_key, now))?;
        host.register_sealer(&conn, store, key_id)?;
        Ok(key_id)
    }

    // ----- pairing: the joining device -----

    /// Starts a pairing session and returns the code to display.
    pub fn pairing_begin(
        &self,
        server_url: String,
        account_id: String,
    ) -> Result<PairingStart, CoreError> {
        let mut host = self.sync()?;
        let conn = self.conn()?;
        host.attach_engine(self.secure_store(), &server_url)?;
        let handle = host
            .engine_mut()?
            .begin_pairing(&conn, &server_url, &account_id)
            .map_err(map_sync)?;
        let started = PairingStart {
            code: handle.code().to_string(),
            session_id: handle.session_id().to_string(),
            expires_in_seconds: handle
                .expires_in_seconds()
                .and_then(|seconds| i64::try_from(seconds).ok()),
        };
        host.claimed = None;
        host.pairing = Some(handle);
        Ok(started)
    }

    /// Starts joining an existing WebDAV account from this device.
    pub fn pairing_begin_webdav(
        &self,
        base_url: String,
        credentials: Option<String>,
    ) -> Result<PairingStart, CoreError> {
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let store = self.secure_store();
        self.store_webdav_credentials(credentials.as_deref())?;
        host.attach_webdav(store, &base_url)?;
        let account = host
            .webdav()?
            .read_account()
            .map_err(super::map_transport)?
            .ok_or_else(|| IpcError::conflict("this account has no vault to recover"))?;
        let account_id = account.account_id;
        let handle = {
            let (engine, _) = host.engine_and_webdav()?;
            engine
                .begin_pairing_webdav(&conn, &base_url, &account_id)
                .map_err(map_sync)?
        };
        let started = PairingStart {
            code: handle.code().to_string(),
            session_id: handle.session_id().to_string(),
            expires_in_seconds: handle
                .expires_in_seconds()
                .and_then(|seconds| i64::try_from(seconds).ok()),
        };
        host.claimed = None;
        host.pairing = Some(handle);
        Ok(started)
    }

    /// Polls for the trusted device's offer. `None` while it has not
    /// confirmed. A ready offer is verified and its short authentication
    /// string returned for the user's own comparison — nothing is installed
    /// until `pairing_finalize`.
    pub fn pairing_poll(&self) -> Result<Option<PairingClaim>, CoreError> {
        let mut host = self.sync()?;
        if let Some(claimed) = &host.claimed {
            return Ok(Some(PairingClaim {
                sas: claimed.sas().to_string(),
                root_fingerprint: claimed.root_fingerprint().display_code(),
            }));
        }
        let Some(handle) = host.pairing.take() else {
            return Err(IpcError::conflict("no pairing is in progress").into());
        };
        let claimed = host.poll_pairing_on_backend(&handle);
        host.pairing = Some(handle);
        let Some(claimed) = claimed? else {
            return Ok(None);
        };
        let out = PairingClaim {
            sas: claimed.sas().to_string(),
            root_fingerprint: claimed.root_fingerprint().display_code(),
        };
        host.claimed = Some(claimed);
        Ok(Some(out))
    }

    /// Completes pairing after the user confirmed the string on both screens.
    /// A master password is required when the offer carries vault material:
    /// the master key is re-wrapped under this device's own password.
    pub fn pairing_finalize(
        &self,
        master_password: Option<String>,
    ) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let handle = host
            .pairing
            .take()
            .ok_or_else(|| IpcError::conflict("no pairing is in progress"))?;
        let claimed = host
            .claimed
            .take()
            .ok_or_else(|| IpcError::conflict("confirm the pairing code first"))?;
        let store = self.secure_store();
        let adopted = host.finalize_pairing_on_backend(
            &conn,
            store,
            &handle,
            &claimed,
            master_password.as_deref().map(str::as_bytes),
            now,
        )?;
        host.register_sealer(&conn, store, adopted.sync_key_id)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Abandons an in-flight pairing session on this device.
    pub fn pairing_cancel(&self) -> Result<(), CoreError> {
        let mut host = self.sync()?;
        host.pairing = None;
        host.claimed = None;
        Ok(())
    }

    // ----- pairing: the admitting device -----

    /// Reads a pairing code and returns the string to compare, without
    /// admitting anything yet.
    pub fn pairing_sas(&self, code: String) -> Result<PairingSas, CoreError> {
        let host = self.sync()?;
        let conn = self.conn()?;
        let parsed = PairingCode::decode(&code).map_err(|e| map_sync(e.into()))?;
        let engine = host
            .engine
            .as_ref()
            .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
        let sas = engine.pairing_sas(&conn, &parsed).map_err(map_sync)?;
        Ok(PairingSas {
            sas,
            device_name: parsed.device_name,
            platform: parsed.platform.as_str().to_string(),
            vault_ready: VaultSession::is_initialized(&conn).map_err(IpcError::from)?,
        })
    }

    /// Admits the device after the user confirmed the string on both screens.
    /// `allow_vault` decides whether the master key travels with the bundle;
    /// granting it needs an unlocked vault.
    pub fn pairing_approve(&self, code: String, allow_vault: bool) -> Result<(), CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let vault = self.vault()?;
        let parsed = PairingCode::decode(&code).map_err(|e| map_sync(e.into()))?;
        let store = self.secure_store();
        vault.with_master_key(|master_key| {
            host.approve_pairing_on_backend(&conn, store, &parsed, allow_vault, master_key, now)
        })?;
        Ok(())
    }

    // ----- recovery -----

    /// Generates a recovery code, uploads the wrapped blob, and returns the
    /// code exactly once. Needs an unlocked vault: the blob carries the
    /// master key.
    pub fn recovery_publish(&self) -> Result<RecoveryCodeOut, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let vault = self.vault()?;
        let setup = setup_state(&conn)?;
        let (Some(account_id), Some(server_url)) = (setup.account_id, setup.server_url) else {
            return Err(IpcError::conflict("sync is not set up on this device").into());
        };
        let store = self.secure_store();
        let code = vault.with_master_key(|master_key| match master_key {
            Some(master_key) => host.publish_recovery_on_backend(&conn, store, master_key),
            None => Err(IpcError::permission_denied("unlock the vault first")),
        })?;
        mark_recovery_exported(self.sync_dir(), now);
        Ok(RecoveryCodeOut {
            code: code.display_groups(),
            account_id,
            server_url,
        })
    }

    /// Recovers an account on this device with a recovery code: the trust root
    /// is replaced, every previous device is revoked server-side, and the sync
    /// key is rotated. `master_password` is the one this device will use for
    /// its own vault afterwards.
    pub fn recovery_recover(
        &self,
        server_url: String,
        account_id: String,
        code: String,
        master_password: String,
    ) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let parsed = RecoveryCode::parse(&code).map_err(|e| map_sync(e.into()))?;
        host.attach_engine(self.secure_store(), &server_url)?;
        let store = self.secure_store();
        let adopted = host
            .engine_mut()?
            .recover_account(
                &conn,
                store,
                &RecoveryRequest {
                    server_url: &server_url,
                    account_id: &account_id,
                    code: &parsed,
                    master_password: master_password.as_bytes(),
                },
                now,
            )
            .map_err(map_sync)?;
        host.register_sealer(&conn, store, adopted.sync_key_id)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    /// Recovers a WebDAV account on this device with the recovery code.
    pub fn recovery_recover_webdav(
        &self,
        base_url: String,
        credentials: Option<String>,
        code: String,
        master_password: String,
    ) -> Result<SyncStatus, CoreError> {
        let now = now_ms()?;
        let mut host = self.sync()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        let parsed = RecoveryCode::parse(&code).map_err(|e| map_sync(e.into()))?;
        let store = self.secure_store();
        self.store_webdav_credentials(credentials.as_deref())?;
        host.attach_webdav(store, &base_url)?;
        let adopted = {
            let (engine, dav) = host.engine_and_webdav()?;
            engine
                .recover_account_webdav(
                    &conn,
                    store,
                    dav,
                    &parsed,
                    master_password.as_bytes(),
                    &base_url,
                    now,
                )
                .map_err(map_sync)?
        };
        host.register_sealer(&conn, store, adopted.sync_key_id)?;
        Ok(self.status_of(&conn, &mut vault, &host)?)
    }

    // ----- conflicts -----

    pub fn sync_conflicts(&self) -> Result<Vec<ConflictPair>, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let pairs = service::sync_conflict_list(&conn, now)?;
        Ok(pairs
            .into_iter()
            .map(|pair| ConflictPair {
                source: pair.source.into(),
                copy: pair.copy.into(),
                sensitive: pair.sensitive,
            })
            .collect())
    }

    /// Records the user's decision. The losing body is never destroyed — it
    /// moves to the recycle bin, or stays as its own snippet for `Both`.
    pub fn sync_conflict_resolve(
        &self,
        copy_id: String,
        keep: ConflictResolution,
    ) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let session = self.vault()?;
        Ok(service::sync_conflict_resolve(
            &conn,
            &session,
            &copy_id,
            keep.into(),
            now,
        )?)
    }

    // ----- the snapshot the extensions read -----

    /// Regenerates the snapshot document the keyboard and widget read, in the
    /// directory the caller names — the App Group container, which only the
    /// platform layer knows how to resolve.
    ///
    /// The write is a temp file plus a rename, so a failure at any point
    /// leaves the previous snapshot exactly as it was. A stale snapshot is a
    /// recoverable state; a truncated one is not.
    pub fn write_snapshot(&self, directory: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let host = self.sync()?;
        let conn = self.conn()?;
        Ok(typvia_host_service::snapshot::write_snapshot(
            &conn,
            std::path::Path::new(&directory),
            &host.device_id,
            now,
        )?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn core() -> (TypviaCore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let core = TypviaCore::open(dir.path().to_string_lossy().to_string()).unwrap();
        (core, dir)
    }

    /// A host with no gated key storage cannot hold a device identity, so
    /// sync reports itself unavailable rather than offering a button that
    /// cannot work. Everything else about the app keeps running.
    #[test]
    fn without_key_storage_sync_reports_itself_unavailable() {
        let (core, _dir) = core();

        let status = core.sync_status().unwrap();

        assert!(!status.available);
        assert!(!status.configured);
        assert!(!status.enabled);
        assert_eq!(status.account_id, None);
        assert_eq!(status.pending_backlog, 0);
        assert_eq!(status.last_sync_at, None);
        // The device still has an identity locally, so the screen can name it.
        assert!(!status.device_id.is_empty());
        assert!(!status.device_name.is_empty());
    }

    /// Every path that needs an engine says the same thing when there is
    /// none, instead of half-working.
    #[test]
    fn sync_paths_refuse_cleanly_when_no_account_is_bound() {
        let (core, _dir) = core();
        let not_set_up = CoreError::Conflict {
            reason: "sync is not set up on this device".to_string(),
        };

        assert_eq!(core.sync_devices().unwrap_err(), not_set_up);
        assert_eq!(core.sync_disable().unwrap_err(), not_set_up);
        assert_eq!(core.sync_resume().unwrap_err(), not_set_up);
        assert_eq!(core.sync_rotate_key().unwrap_err(), not_set_up);
        assert_eq!(core.recovery_publish().unwrap_err(), not_set_up);
    }

    /// A screen that cannot say why is a screen that guesses. The status
    /// carries no reason until a round actually fails, then carries the
    /// category of that failure — so the offline line states a cause instead
    /// of inventing one.
    #[test]
    fn the_status_carries_no_failure_until_a_round_has_failed() {
        let (core, _dir) = core();

        assert_eq!(core.sync_status().unwrap().last_failure, None);

        core.sync_now().unwrap_err();

        assert_eq!(
            core.sync_status().unwrap().last_failure,
            Some(SyncFailureKind::NotSetUp)
        );
    }

    /// Red line: there is no way to finish pairing without the comparison.
    /// Polling with no session refuses; finalizing before a confirmed offer
    /// refuses; and neither refusal hints at a way around the check.
    #[test]
    fn pairing_cannot_be_completed_without_a_confirmed_code() {
        let (core, _dir) = core();

        assert_eq!(
            core.pairing_poll().unwrap_err(),
            CoreError::Conflict {
                reason: "no pairing is in progress".to_string(),
            }
        );
        assert_eq!(
            core.pairing_finalize(None).unwrap_err(),
            CoreError::Conflict {
                reason: "no pairing is in progress".to_string(),
            }
        );
        // Cancelling is always available and always idempotent: a user who
        // is unsure about a code must be able to stop.
        core.pairing_cancel().unwrap();
        core.pairing_cancel().unwrap();
    }

    /// A pairing code is external input: it is parsed and rejected before it
    /// reaches anything, and the rejection quotes none of it back.
    #[test]
    fn a_malformed_pairing_code_is_rejected_without_echoing_it() {
        let (core, _dir) = core();

        let error = core
            .pairing_sas("TYPVIA-PAIR.V1.AKIAFAKEEXAMPLE00000".to_string())
            .unwrap_err();

        assert!(matches!(error, CoreError::Validation { .. }));
        assert!(!error.to_string().contains("AKIAFAKE"));
    }

    /// A conflict decision needs a real conflict; an unknown copy is not
    /// found rather than silently doing nothing.
    #[test]
    fn resolving_an_unknown_conflict_is_not_found() {
        let (core, _dir) = core();

        assert!(core.sync_conflicts().unwrap().is_empty());
        assert_eq!(
            core.sync_conflict_resolve("ghost".to_string(), ConflictResolution::Source)
                .unwrap_err(),
            CoreError::NotFound
        );
    }

    /// The snapshot the extensions read is written atomically into the
    /// directory the platform names, and a snapshot with no live snippets is
    /// still a valid document rather than a missing file.
    #[test]
    fn the_extension_snapshot_is_written_where_the_platform_asks() {
        let (core, _dir) = core();
        let out = tempfile::tempdir().unwrap();

        core.write_snapshot(out.path().to_string_lossy().to_string())
            .unwrap();

        let written = std::fs::read(out.path().join("snapshot.json")).unwrap();
        assert!(!written.is_empty());
        // A successful write leaves no temp file behind.
        assert!(!out.path().join("snapshot.json.tmp").exists());
    }

    /// Red line: a secret never reaches the document the restricted processes
    /// read. Its row is there as an id and an opaque envelope; its body is
    /// not, in any form.
    #[test]
    fn a_secret_body_never_reaches_the_extension_snapshot() {
        let (core, _dir) = core();
        let out = tempfile::tempdir().unwrap();
        core.vault_initialize("correct-horse-t087".to_string())
            .unwrap();
        core.vault_create_secret(crate::model::SnippetDraft {
            title: "Deploy key".to_string(),
            body: "AKIAFAKEEXAMPLE00000".to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        })
        .unwrap();

        core.write_snapshot(out.path().to_string_lossy().to_string())
            .unwrap();

        let written = std::fs::read(out.path().join("snapshot.json")).unwrap();
        assert!(
            !written
                .windows("AKIAFAKE".len())
                .any(|window| window == b"AKIAFAKE"),
            "the secret body reached the snapshot the extensions read"
        );
    }
}
