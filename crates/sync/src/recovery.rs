// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Recovery-code primitives: the one-time recovery code, the
//! Argon2id-wrapped recovery blob stored server-side, and the MK-derived
//! rootproof key pair used for the re-root exchange.
//!
//! The recovery code plaintext never persists anywhere — it exists in
//! memory for generation/derivation only and wipes on drop. The blob is
//! useless without the code (Argon2id with an independent salt).

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use typvia_crypto::{KdfParams, SALT_LEN, SymmetricKey, derive_kek};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::base32::{decode_nopad, encode_nopad};

/// Version prefix of the recovery code (two leading characters of the
/// 8×4 display form).
const CODE_PREFIX: &str = "T1";
/// Recovery-code entropy: 128-bit from the CSPRNG.
const SECRET_LEN: usize = 16;
/// base32 of 16 bytes is 26 characters.
const SECRET_CHARS: usize = 26;
/// Checksum length: prefix(2) + secret(26) + check(4) = 32 = 8 groups × 4.
const CHECK_CHARS: usize = 4;
const CODE_CHARS: usize = 32;

const CHECKSUM_CONTEXT: &[u8] = b"typvia.recovery.code.v1";
const ROOTPROOF_INFO: &[u8] = b"typvia.rootproof.v1";
const RECOVERY_AAD_CONTEXT: &[u8] = b"typvia.recovery.v1";

/// Recovery blob format version (independent of protocol version).
const BLOB_VERSION: u32 = 1;
/// Upper bounds for KDF parameters read back from a blob header: a
/// malicious server must not be able to dictate an absurd derivation cost.
const MAX_M_COST_KIB: u32 = 256 * 1024;
const MAX_T_COST: u32 = 10;
const MAX_P_COST: u32 = 4;

/// Failures of the recovery primitives. Structural facts only.
#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryError {
    /// The typed code has the wrong shape, alphabet, prefix, or checksum.
    MalformedCode,
    /// The blob is not a valid recovery document, or its KDF parameters
    /// fall outside the accepted bounds.
    MalformedBlob,
    /// The blob did not open under the derived key — wrong code, wrong
    /// account, or tampering (indistinguishable at the AEAD layer).
    DecryptFailed,
    /// The opened bundle carries no master key; a recovery bundle always
    /// includes it by construction.
    MissingMasterKey,
    /// Key derivation or sealing failed (system error).
    Crypto,
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::MalformedCode => "recovery code is malformed",
            Self::MalformedBlob => "recovery blob is malformed",
            Self::DecryptFailed => "recovery blob could not be opened",
            Self::MissingMasterKey => "recovery bundle carries no master key",
            Self::Crypto => "recovery cryptography failed",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for RecoveryError {}

/// The one-time recovery code. Displayed exactly once at generation; the
/// canonical form (32 characters, no separators) is the Argon2id input.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RecoveryCode {
    canonical: String,
}

impl RecoveryCode {
    /// Generates a fresh code: `T1` prefix + base32(128-bit CSPRNG) +
    /// 4-character checksum, 32 characters shown as 8 groups of 4.
    pub fn generate() -> Self {
        let mut secret = Zeroizing::new([0u8; SECRET_LEN]);
        OsRng.fill_bytes(secret.as_mut());
        let body = Zeroizing::new(encode_nopad(secret.as_ref()));
        let check = checksum(secret.as_ref());
        Self {
            canonical: format!("{CODE_PREFIX}{}{check}", body.as_str()),
        }
    }

    /// Parses a user-typed code: separators and case are normalized, then
    /// prefix, alphabet, and checksum are verified.
    pub fn parse(text: &str) -> Result<Self, RecoveryError> {
        let canonical: String = text
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if canonical.len() != CODE_CHARS || !canonical.starts_with(CODE_PREFIX) {
            return Err(RecoveryError::MalformedCode);
        }
        let body = &canonical[CODE_PREFIX.len()..CODE_PREFIX.len() + SECRET_CHARS];
        let secret =
            Zeroizing::new(decode_nopad::<SECRET_LEN>(body).ok_or(RecoveryError::MalformedCode)?);
        let expected = checksum(secret.as_ref());
        if canonical[CODE_PREFIX.len() + SECRET_CHARS..] != expected {
            return Err(RecoveryError::MalformedCode);
        }
        Ok(Self { canonical })
    }

    /// The display form: 8 hyphen-joined groups of 4 characters. Shown
    /// once; never logged or persisted.
    pub fn display_groups(&self) -> String {
        self.canonical
            .as_bytes()
            .chunks(4)
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// The Argon2id password input (the canonical 32-character form).
    fn kdf_input(&self) -> &[u8] {
        self.canonical.as_bytes()
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The code never reaches Debug output (log red line).
        f.write_str("RecoveryCode(<redacted>)")
    }
}

fn checksum(secret: &[u8]) -> String {
    let mut hasher = sha2::Sha256::default();
    use sha2::Digest as _;
    hasher.update(CHECKSUM_CONTEXT);
    hasher.update(secret);
    let digest = hasher.finalize();
    encode_nopad(&digest)[..CHECK_CHARS].to_string()
}

// ----- recovery blob ------------------------------------------------------

/// Plaintext header + sealed bundle, as stored on the server.
#[derive(Serialize, Deserialize)]
struct RecoveryBlobDoc {
    recovery_version: u32,
    kdf: KdfDoc,
    /// base64 of the envelope sealing the key-bundle JSON.
    sealed: String,
}

#[derive(Serialize, Deserialize)]
struct KdfDoc {
    version: u8,
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
    /// base64, 16 bytes.
    salt: String,
}

/// Wraps a key-bundle document into the recovery blob: KEK_rc =
/// Argon2id(code, fresh independent salt), AAD
/// `"typvia.recovery.v1" || account_id`.
pub fn build_recovery_blob(
    code: &RecoveryCode,
    account_id: &str,
    bundle_json: &[u8],
) -> Result<Vec<u8>, RecoveryError> {
    let params = KdfParams::v1();
    let kek = derive_kek(code.kdf_input(), &params).map_err(|_| RecoveryError::Crypto)?;
    let sealed = typvia_crypto::seal(&kek, BLOB_VERSION, &recovery_aad(account_id), bundle_json)
        .map_err(|_| RecoveryError::Crypto)?;
    serde_json::to_vec(&RecoveryBlobDoc {
        recovery_version: BLOB_VERSION,
        kdf: KdfDoc {
            version: params.version,
            m_cost_kib: params.m_cost_kib,
            t_cost: params.t_cost,
            p_cost: params.p_cost,
            salt: BASE64.encode(params.salt),
        },
        sealed: BASE64.encode(sealed),
    })
    .map_err(|_| RecoveryError::Crypto)
}

/// Opens a recovery blob with the code. The KDF parameters come from the
/// blob header but are bounds-checked first (a hostile server must not be
/// able to dictate an absurd derivation cost).
pub fn open_recovery_blob(
    code: &RecoveryCode,
    account_id: &str,
    blob: &[u8],
) -> Result<Zeroizing<Vec<u8>>, RecoveryError> {
    let doc: RecoveryBlobDoc =
        serde_json::from_slice(blob).map_err(|_| RecoveryError::MalformedBlob)?;
    if doc.recovery_version != BLOB_VERSION
        || doc.kdf.version != 1
        || doc.kdf.m_cost_kib == 0
        || doc.kdf.m_cost_kib > MAX_M_COST_KIB
        || doc.kdf.t_cost == 0
        || doc.kdf.t_cost > MAX_T_COST
        || doc.kdf.p_cost == 0
        || doc.kdf.p_cost > MAX_P_COST
    {
        return Err(RecoveryError::MalformedBlob);
    }
    let salt: [u8; SALT_LEN] = BASE64
        .decode(&doc.kdf.salt)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(RecoveryError::MalformedBlob)?;
    let params = KdfParams {
        version: doc.kdf.version,
        m_cost_kib: doc.kdf.m_cost_kib,
        t_cost: doc.kdf.t_cost,
        p_cost: doc.kdf.p_cost,
        salt,
    };
    let sealed = BASE64
        .decode(&doc.sealed)
        .map_err(|_| RecoveryError::MalformedBlob)?;
    let kek = derive_kek(code.kdf_input(), &params).map_err(|_| RecoveryError::Crypto)?;
    typvia_crypto::open(&kek, &recovery_aad(account_id), &sealed)
        .map_err(|_| RecoveryError::DecryptFailed)
}

fn recovery_aad(account_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(RECOVERY_AAD_CONTEXT.len() + account_id.len());
    aad.extend_from_slice(RECOVERY_AAD_CONTEXT);
    aad.extend_from_slice(account_id.as_bytes());
    aad
}

// ----- rootproof key (MK-possession proof) --------------------------------

/// Derives the Ed25519 rootproof key pair from the master key: seed =
/// HKDF-SHA-256(ikm = MK, salt = account_id (UTF-8), info =
/// "typvia.rootproof.v1"). Deterministic, so the recovering device can
/// re-derive the same key the blob owner registered.
pub fn rootproof_signing_key(master_key: &SymmetricKey, account_id: &str) -> SigningKey {
    let hk = Hkdf::<Sha256>::new(Some(account_id.as_bytes()), master_key.expose());
    let mut seed = Zeroizing::new([0u8; 32]);
    // A 32-byte output cannot overflow HKDF's expand limit.
    let _ = hk.expand(ROOTPROOF_INFO, seed.as_mut());
    SigningKey::from_bytes(&seed)
}

/// `"typvia.rootproof.v1" || challenge(32B) || account_id` — the message
/// the rootproof key signs in the re-root exchange.
pub(crate) fn rootproof_signed_bytes(challenge: &[u8], account_id: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ROOTPROOF_INFO.len() + challenge.len() + account_id.len());
    bytes.extend_from_slice(ROOTPROOF_INFO);
    bytes.extend_from_slice(challenge);
    bytes.extend_from_slice(account_id.as_bytes());
    bytes
}

// ----- engine flows -------------------------------------------------------

use rusqlite::Connection;
use typvia_core::model::{SyncConfig, TimestampMs, TransportKind};
use typvia_core::repo::{DeviceRepo, RepoError, SyncStateRepo};
use typvia_crypto::SecureStore;

use crate::cert::{CertificateSubject, RootStatement};
use crate::directory::{TrustState, root_statement_from_json, root_statement_to_json};
use crate::engine::{AdoptedAccount, SyncEngine, SyncError, pinned_fingerprint};
use crate::keyring::SyncKeys;
use crate::pairing::{KeyBundle, PairingError};
use crate::provision::{
    build_key_bundle, install_sync_generations, install_vault_from_bundle, symmetric_key_from,
};
use crate::transport::{ReRootOutcome, TransportError};

/// Facts a recovery runs with: where the account lives, the typed
/// recovery code, and the master password the recovered MK is re-wrapped
/// under on this device.
#[derive(Clone, Copy)]
pub struct RecoveryRequest<'a> {
    pub server_url: &'a str,
    pub account_id: &'a str,
    pub code: &'a RecoveryCode,
    pub master_password: &'a [u8],
}

impl SyncEngine {
    /// Generates a recovery code and uploads the wrapped recovery blob and
    /// rootproof public key. A vault must exist and its master key
    /// must be provided — the rootproof key derives from the MK, and the
    /// bundle always carries the full vault material. Re-running replaces
    /// the blob (the previous code stops working). The returned code is
    /// shown once and never persisted.
    pub fn publish_recovery(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        master_key: &SymmetricKey,
    ) -> Result<RecoveryCode, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let account_id = config.account_id.clone().ok_or(SyncError::NotEnabled)?;
        let pinned = pinned_fingerprint(&config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        let root_value: serde_json::Value =
            serde_json::from_slice(&root_statement_to_json(trust.root())?)
                .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;

        let bundle = build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            true,
            Some(master_key),
            Some(root_value),
        )?;
        if bundle.mk.is_none() {
            // No vault means no MK and no rootproof — recovery is not
            // available for this account state.
            return Err(SyncError::Recovery(RecoveryError::MissingMasterKey));
        }
        let bundle_json = bundle.to_json()?;
        let code = RecoveryCode::generate();
        let blob = build_recovery_blob(&code, &account_id, &bundle_json)?;
        let rootproof_pub = rootproof_signing_key(master_key, &account_id)
            .verifying_key()
            .to_bytes();
        self.with_auth(|t, token| t.put_recovery_blob(token, &blob, &rootproof_pub))?;
        Ok(code)
    }

    /// [`Self::publish_recovery`] in the WebDAV form: the blob
    /// and rootproof public key become files; everything else is identical.
    pub fn publish_recovery_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &crate::webdav_store::WebdavStore,
        master_key: &SymmetricKey,
    ) -> Result<RecoveryCode, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() {
            return Err(SyncError::NotEnabled);
        }
        let account_id = config.account_id.clone().ok_or(SyncError::NotEnabled)?;
        let pinned = pinned_fingerprint(&config)?;
        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;
        let root_value: serde_json::Value =
            serde_json::from_slice(&root_statement_to_json(trust.root())?)
                .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;

        let bundle = build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            true,
            Some(master_key),
            Some(root_value),
        )?;
        if bundle.mk.is_none() {
            return Err(SyncError::Recovery(RecoveryError::MissingMasterKey));
        }
        let bundle_json = bundle.to_json()?;
        let code = RecoveryCode::generate();
        let blob = build_recovery_blob(&code, &account_id, &bundle_json)?;
        let rootproof_pub = rootproof_signing_key(master_key, &account_id)
            .verifying_key()
            .to_bytes();
        dav.write_recovery(&blob, &rootproof_pub)?;
        Ok(code)
    }

    /// [`Self::recover_account`] in the WebDAV form: the re-root is a
    /// directory replacement — old devices self-refuse on their pinned
    /// root; their entries stay (revoked) so the catch-up can still verify
    /// historical records against the predecessor root. There is no
    /// server-side rootproof check: possession of the recovery code (which
    /// opens the blob) is the primary defense in this form.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_account_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &crate::webdav_store::WebdavStore,
        code: &RecoveryCode,
        master_password: &[u8],
        server_url: &str,
        now: TimestampMs,
    ) -> Result<AdoptedAccount, SyncError> {
        let device = DeviceRepo::new(conn)
            .get(&self.device_id)?
            .ok_or(SyncError::Repo(RepoError::NotFound))?;
        let account = dav
            .read_account()?
            .ok_or(SyncError::Transport(TransportError::MalformedResponse))?;
        let blob = dav
            .read_recovery_blob()?
            .ok_or(SyncError::Recovery(RecoveryError::MalformedBlob))?;
        let bundle_json = open_recovery_blob(code, &account.account_id, &blob)?;
        let bundle = KeyBundle::from_json(&bundle_json)?;
        let mk_bytes = bundle
            .mk
            .as_ref()
            .ok_or(SyncError::Recovery(RecoveryError::MissingMasterKey))?;
        let mk = symmetric_key_from(mk_bytes)?;

        let old_root_json = bundle
            .root_statement
            .as_ref()
            .and_then(|value| serde_json::to_vec(value).ok())
            .ok_or(SyncError::Pairing(PairingError::MalformedBundle))?;
        root_statement_from_json(&old_root_json)?;

        // Re-root: this device's self-signed statement replaces the
        // directory root; every prior device is marked revoked but keeps
        // its chain for the historical catch-up.
        let new_root = RootStatement::create(
            &self.identity,
            CertificateSubject {
                device_id: self.device_id.clone(),
                ed25519_pub: self.identity.ed25519_public(),
                x25519_pub: self.identity.x25519_public(),
                name: device.name.clone(),
                platform: device.platform,
                created_at: now,
            },
        )?;
        let mut directory = dav
            .read_directory()?
            .map(|f| f.directory)
            .unwrap_or_else(|| crate::transport::DeviceDirectory {
                root_statement_json: Vec::new(),
                devices: Vec::new(),
                revocations: Vec::new(),
            });
        for entry in &mut directory.devices {
            if entry.revoked_at.is_none() {
                entry.revoked_at = Some(now);
                directory
                    .revocations
                    .push(crate::transport::DirectoryRevocation {
                        device_id: entry.id.clone(),
                        revoked_at: now,
                    });
            }
        }
        directory.root_statement_json = root_statement_to_json(&new_root)?;
        directory.devices.push(crate::transport::DirectoryDevice {
            id: self.device_id.clone(),
            name: device.name,
            platform: device.platform.as_str().to_string(),
            ed25519_pub: self.identity.ed25519_public().to_vec(),
            x25519_pub: self.identity.x25519_public().to_vec(),
            cert_chain_json: b"[]".to_vec(),
            created_at: now,
            revoked_at: None,
        });
        dav.write_directory_overwrite(&directory)?;

        let sync_key_id = install_sync_generations(store, &bundle)?;
        let root_fingerprint = *new_root.fingerprint().as_bytes();
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        install_vault_from_bundle(&tx, &bundle, &mk, master_password, now)?;
        let keys = SyncKeys::load(store, sync_key_id)?;
        self.activate_within(
            &tx,
            &keys,
            SyncConfig {
                server_url: Some(server_url.to_string()),
                account_id: Some(account.account_id.clone()),
                enabled: true,
                applied_server_seq: 0,
                sync_key_id,
                root_fingerprint: Some(root_fingerprint.to_vec()),
                key_update_seq: 0,
                recovery_root: Some(old_root_json),
                transport_kind: typvia_core::model::TransportKind::Webdav,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )?;
        tx.commit().map_err(RepoError::from)?;

        let rotated = self.rotate_sync_key_webdav(conn, store, dav, Some(&mk), now)?;
        Ok(AdoptedAccount {
            server_url: server_url.to_string(),
            account_id: account.account_id,
            root_fingerprint,
            sync_key_id: rotated,
        })
    }

    /// Recovers an account on a fresh device: fetch and open the
    /// blob with the recovery code, prove MK possession to replace the
    /// trust root with this device's self-signed statement (all previous
    /// devices are revoked server-side), install the recovered keys (the
    /// MK re-wrapped under this device's own master password), activate
    /// sync pinned to the new root, pull the historical records under the
    /// predecessor root, and force a K_sync rotation.
    pub fn recover_account(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        request: &RecoveryRequest<'_>,
        now: TimestampMs,
    ) -> Result<AdoptedAccount, SyncError> {
        let RecoveryRequest {
            server_url,
            account_id,
            code,
            master_password,
        } = *request;
        let device = DeviceRepo::new(conn)
            .get(&self.device_id)?
            .ok_or(SyncError::Repo(RepoError::NotFound))?;

        let blob = self.transport.get_recovery_blob(account_id)?;
        let bundle_json = open_recovery_blob(code, account_id, &blob)?;
        let bundle = KeyBundle::from_json(&bundle_json)?;
        let mk_bytes = bundle
            .mk
            .as_ref()
            .ok_or(SyncError::Recovery(RecoveryError::MissingMasterKey))?;
        let mk = symmetric_key_from(mk_bytes)?;

        // Two-phase re-root: challenge, then proof + new self-signed root.
        let new_root = RootStatement::create(
            &self.identity,
            CertificateSubject {
                device_id: self.device_id.clone(),
                ed25519_pub: self.identity.ed25519_public(),
                x25519_pub: self.identity.x25519_public(),
                name: device.name,
                platform: device.platform,
                created_at: now,
            },
        )?;
        let challenge = match self.transport.re_root(account_id, None, None)? {
            ReRootOutcome::Challenge(challenge) => challenge,
            ReRootOutcome::Done => {
                return Err(SyncError::Transport(TransportError::MalformedResponse));
            }
        };
        let proof_key = rootproof_signing_key(&mk, account_id);
        use ed25519_dalek::Signer as _;
        let proof_sig = proof_key
            .sign(&rootproof_signed_bytes(&challenge, account_id))
            .to_bytes();
        match self
            .transport
            .re_root(account_id, Some(&proof_sig), Some(&new_root))?
        {
            ReRootOutcome::Done => {}
            ReRootOutcome::Challenge(_) => {
                return Err(SyncError::Transport(TransportError::MalformedResponse));
            }
        }

        // The historical catch-up material: records produced before the
        // re-root verify only against the predecessor root, which the
        // directory no longer serves. The sealed bundle carries that root
        // (authenticated by recovery-code possession); it is persisted in
        // the activation transaction as the `recovery_root` marker,
        // so an interrupted catch-up survives restarts and every
        // later sync round resumes it until the server is drained. Sanity:
        // the statement must parse before it is committed as the marker.
        let old_root_json = bundle
            .root_statement
            .as_ref()
            .and_then(|value| serde_json::to_vec(value).ok())
            .ok_or(SyncError::Pairing(PairingError::MalformedBundle))?;
        root_statement_from_json(&old_root_json)?;

        // Install keys and activate, pinned to the new (own) root.
        let sync_key_id = install_sync_generations(store, &bundle)?;
        let root_fingerprint = *new_root.fingerprint().as_bytes();
        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        install_vault_from_bundle(&tx, &bundle, &mk, master_password, now)?;
        let keys = SyncKeys::load(store, sync_key_id)?;
        self.activate_within(
            &tx,
            &keys,
            SyncConfig {
                server_url: Some(server_url.to_string()),
                account_id: Some(account_id.to_string()),
                enabled: true,
                applied_server_seq: 0,
                sync_key_id,
                root_fingerprint: Some(root_fingerprint.to_vec()),
                key_update_seq: 0,
                recovery_root: Some(old_root_json),
                transport_kind: TransportKind::Server,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )?;
        tx.commit().map_err(RepoError::from)?;

        // Every pre-recovery device saw the old K_sync, so rotate.
        // The catch-up itself runs in the sync rounds (idempotent by the
        // watermark), so a network drop here leaves a resumable state, not
        // a silent hole.
        let rotated = self.rotate_sync_key(conn, store, Some(&mk), now)?;
        Ok(AdoptedAccount {
            server_url: server_url.to_string(),
            account_id: account_id.to_string(),
            root_fingerprint,
            sync_key_id: rotated,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_code_has_the_documented_display_shape() {
        let code = RecoveryCode::generate();
        let display = code.display_groups();
        let groups: Vec<&str> = display.split('-').collect();
        assert_eq!(groups.len(), 8);
        assert!(groups.iter().all(|g| g.len() == 4));
        assert!(display.starts_with("T1"));
    }

    #[test]
    fn a_code_parses_back_from_its_display_form_with_sloppy_input() {
        let code = RecoveryCode::generate();
        let display = code.display_groups();
        // Lower case, extra whitespace, and hyphens all normalize away.
        let sloppy = format!("  {}  ", display.to_lowercase().replace('-', " - "));
        let parsed = RecoveryCode::parse(&sloppy).unwrap();
        assert_eq!(parsed.canonical, code.canonical);
    }

    #[test]
    fn a_typo_fails_the_checksum() {
        let code = RecoveryCode::generate();
        let mut chars: Vec<char> = code.canonical.chars().collect();
        // Flip one secret character to a different alphabet member.
        chars[5] = if chars[5] == 'A' { 'B' } else { 'A' };
        let typo: String = chars.into_iter().collect();
        assert_eq!(
            RecoveryCode::parse(&typo).unwrap_err(),
            RecoveryError::MalformedCode
        );
    }

    #[test]
    fn wrong_length_or_prefix_is_rejected() {
        assert_eq!(
            RecoveryCode::parse("T1AB").unwrap_err(),
            RecoveryError::MalformedCode
        );
        let code = RecoveryCode::generate();
        let wrong_prefix = format!("X9{}", &code.canonical[2..]);
        assert_eq!(
            RecoveryCode::parse(&wrong_prefix).unwrap_err(),
            RecoveryError::MalformedCode
        );
    }

    #[test]
    fn two_generated_codes_differ() {
        let a = RecoveryCode::generate();
        let b = RecoveryCode::generate();
        assert_ne!(a.canonical, b.canonical);
    }

    #[test]
    fn debug_output_is_redacted() {
        let code = RecoveryCode::generate();
        let rendered = format!("{code:?}");
        assert!(!rendered.contains(&code.canonical[2..10]));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn a_blob_round_trips_under_the_right_code() {
        let code = RecoveryCode::generate();
        let bundle = br#"{"k_sync":[]}"#;
        let blob = build_recovery_blob(&code, "acct-1", bundle).unwrap();
        let opened = open_recovery_blob(&code, "acct-1", &blob).unwrap();
        assert_eq!(opened.as_slice(), bundle);
        // The blob plaintext must not contain the bundle bytes.
        assert!(!blob.windows(bundle.len()).any(|w| w == bundle.as_slice()));
    }

    #[test]
    fn a_wrong_code_or_wrong_account_fails_to_open() {
        let code = RecoveryCode::generate();
        let other = RecoveryCode::generate();
        let blob = build_recovery_blob(&code, "acct-1", b"bundle bytes").unwrap();
        assert_eq!(
            open_recovery_blob(&other, "acct-1", &blob).unwrap_err(),
            RecoveryError::DecryptFailed
        );
        assert_eq!(
            open_recovery_blob(&code, "acct-2", &blob).unwrap_err(),
            RecoveryError::DecryptFailed
        );
    }

    #[test]
    fn absurd_kdf_parameters_in_a_blob_are_rejected_before_derivation() {
        let code = RecoveryCode::generate();
        let blob = build_recovery_blob(&code, "acct-1", b"bundle").unwrap();
        let mut doc: serde_json::Value = serde_json::from_slice(&blob).unwrap();
        doc["kdf"]["m_cost_kib"] = serde_json::json!(4 * 1024 * 1024);
        let poisoned = serde_json::to_vec(&doc).unwrap();
        assert_eq!(
            open_recovery_blob(&code, "acct-1", &poisoned).unwrap_err(),
            RecoveryError::MalformedBlob
        );
    }

    #[test]
    fn rootproof_key_is_deterministic_per_mk_and_account() {
        let mk = SymmetricKey::from_bytes([0x42; 32]);
        let a = rootproof_signing_key(&mk, "acct-1");
        let b = rootproof_signing_key(&mk, "acct-1");
        assert_eq!(a.verifying_key(), b.verifying_key());
        // Another account or another MK yields a different key.
        let c = rootproof_signing_key(&mk, "acct-2");
        assert_ne!(a.verifying_key(), c.verifying_key());
        let other_mk = SymmetricKey::from_bytes([0x43; 32]);
        let d = rootproof_signing_key(&other_mk, "acct-1");
        assert_ne!(a.verifying_key(), d.verifying_key());
    }
}
