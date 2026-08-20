//! Pairing and recovery flows on the engine: the new-device side (begin →
//! code → claim → SAS → finalize), the trusted side (SAS → certificate +
//! sealed bundle → offer), and the recovery-code round trip (publish blob /
//! recover + re-root).
//!
//! The SAS check is the anti-MITM step and belongs to the user:
//! `approve_pairing` must only be called after the user confirmed the SAS
//! on the trusted device, and `finalize_pairing` only after the user
//! confirmed the same SAS shown by [`ClaimedPairing::sas`] on the new
//! device.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use serde::Deserialize;
use typvia_core::model::{
    DomainKey, KeyDomain, SyncConfig, TimestampMs, TransportKind, VaultKeyHeader,
};
use typvia_core::repo::{DeviceRepo, RepoError, SyncStateRepo, VaultKeyRepo, new_id};
use typvia_crypto::{
    KdfParams, SecureStore, SymmetricKey, aad_domain_key, aad_master_key, derive_kek, wrap_key,
};
use zeroize::Zeroize;

use crate::cert::{CertificateSubject, DeviceCertificate, RootStatement};
use crate::directory::{TrustState, cert_chain_from_json, root_statement_from_json};
use crate::engine::{AdoptedAccount, SyncEngine, SyncError};
use crate::identity::Fingerprint;
use crate::keyring::{self, KeyringError, SyncKeys, install_key};
use crate::pairing::{
    KeyBundle, KeyGeneration, PairingCode, PairingError, SealedMessage, compute_sas,
    open_pair_bundle, seal_pair_bundle,
};

/// An in-flight pairing session on the new device.
#[derive(Debug)]
pub struct PairingHandle {
    session_id: String,
    server_url: String,
    account_id: String,
    code: String,
}

impl PairingHandle {
    /// The encoded pairing code to display (QR or text; the caller renders it).
    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// A claimed offer on the new device, verified up to the SAS check: the
/// chain ends at this device, the bundle signature is valid, but nothing
/// is installed until the user confirms the SAS and `finalize_pairing`
/// runs.
pub struct ClaimedPairing {
    sas: String,
    root: RootStatement,
    trusted_ed25519_pub: [u8; 32],
    message: SealedMessage,
}

impl ClaimedPairing {
    /// The SAS short code to display for the user's cross-device check.
    pub fn sas(&self) -> &str {
        &self.sas
    }

    /// The account trust anchor carried by the offer.
    pub fn root_fingerprint(&self) -> Fingerprint {
        self.root.fingerprint()
    }
}

impl std::fmt::Debug for ClaimedPairing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaimedPairing")
            .field("sas", &self.sas)
            .finish_non_exhaustive()
    }
}

/// The relay payload the server composes for `claim`.
#[derive(Deserialize)]
struct ClaimPayloadDoc {
    root_statement: serde_json::Value,
    cert_chain: serde_json::Value,
    /// base64 sealed bundle bytes.
    sealed_bundle: String,
    /// base64 64-byte Ed25519 signature.
    bundle_signature: String,
}

impl SyncEngine {
    // ----- new device side -----

    /// Starts pairing on the new device: creates the relay session and
    /// builds the pairing code to display. The local device row must
    /// already exist (`ensure_local_device`); its name and platform enter
    /// the code so the admitting side can build the certificate subject.
    pub fn begin_pairing(
        &mut self,
        conn: &Connection,
        server_url: &str,
        account_id: &str,
    ) -> Result<PairingHandle, SyncError> {
        let device = DeviceRepo::new(conn)
            .get(&self.device_id)?
            .ok_or(SyncError::Repo(RepoError::NotFound))?;
        let session = self.transport.pair_begin(account_id)?;
        let code = PairingCode {
            server_url: server_url.to_string(),
            account_id: account_id.to_string(),
            session_id: session.session_id.clone(),
            device_id: self.device_id.clone(),
            device_name: device.name,
            platform: device.platform,
            ed25519_pub: self.identity.ed25519_public(),
            x25519_pub: self.identity.x25519_public(),
        }
        .encode();
        Ok(PairingHandle {
            session_id: session.session_id,
            server_url: server_url.to_string(),
            account_id: account_id.to_string(),
            code,
        })
    }

    /// Polls the claim endpoint. `None` while the trusted device has not
    /// confirmed and offered; once ready, the payload is verified (chain
    /// to its root, leaf = this device, bundle signature) and returned
    /// with the SAS for the user's check. The claim is single-use on the
    /// server, so the returned value must be carried to
    /// [`Self::finalize_pairing`] — polling again yields a session error.
    pub fn poll_pairing(
        &mut self,
        handle: &PairingHandle,
    ) -> Result<Option<ClaimedPairing>, SyncError> {
        let payload = match self.transport.pair_claim(&handle.session_id)? {
            crate::transport::PairClaim::Pending => return Ok(None),
            crate::transport::PairClaim::Ready(payload) => payload,
        };
        self.claimed_from_offer(handle, &payload).map(Some)
    }

    /// Polls the WebDAV pairing mailbox for the admitting side's answer;
    /// verification is byte-for-byte the server path's.
    pub fn poll_pairing_webdav(
        &mut self,
        dav: &crate::webdav_store::WebdavStore,
        handle: &PairingHandle,
    ) -> Result<Option<ClaimedPairing>, SyncError> {
        let Some(payload) = dav.read_pairing_answer(&pairing_claim_dir(&handle.session_id))? else {
            return Ok(None);
        };
        self.claimed_from_offer(handle, &payload).map(Some)
    }

    /// Verifies an offer payload (chain to its root, leaf = this device,
    /// bundle signature) and derives the SAS — shared by the server poll
    /// and the WebDAV mailbox poll.
    fn claimed_from_offer(
        &self,
        handle: &PairingHandle,
        payload: &[u8],
    ) -> Result<ClaimedPairing, SyncError> {
        let doc: ClaimPayloadDoc = serde_json::from_slice(payload)
            .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let root_json = serde_json::to_vec(&doc.root_statement)
            .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let root = root_statement_from_json(&root_json)?;
        let chain_json = serde_json::to_vec(&doc.cert_chain)
            .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let chain = cert_chain_from_json(&chain_json)?;

        // The chain must verify to its own root and end at this device's
        // keys; the root itself is authenticated by the user's SAS check
        // (the fingerprint is part of the SAS input).
        crate::cert::verify_certificate_chain(
            &root.fingerprint(),
            &root,
            &chain,
            &std::collections::HashMap::new(),
        )?;
        let leaf = chain
            .last()
            .ok_or(SyncError::Trust(crate::error::CertificateError::EmptyChain))?;
        if leaf.subject.device_id != self.device_id
            || leaf.subject.ed25519_pub != self.identity.ed25519_public()
            || leaf.subject.x25519_pub != self.identity.x25519_public()
        {
            return Err(SyncError::Pairing(PairingError::ForeignCertificate));
        }
        let trusted_ed25519_pub = if chain.len() >= 2 {
            chain[chain.len() - 2].subject.ed25519_pub
        } else {
            root.subject.ed25519_pub
        };

        let sealed = BASE64
            .decode(&doc.sealed_bundle)
            .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let signature: [u8; 64] = BASE64
            .decode(&doc.bundle_signature)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(SyncError::Pairing(PairingError::BadSignature))?;
        let message = SealedMessage { sealed, signature };
        // Fail fast on a forged offer; decryption itself waits for the
        // user's SAS confirmation in finalize.
        crate::pairing::verify_pair_signature(&trusted_ed25519_pub, &handle.session_id, &message)?;

        let sas = compute_sas(
            &handle.session_id,
            &self.identity.ed25519_public(),
            &self.identity.x25519_public(),
            &trusted_ed25519_pub,
            root.fingerprint().as_bytes(),
        );
        Ok(ClaimedPairing {
            sas,
            root,
            trusted_ed25519_pub,
            message,
        })
    }

    /// Completes pairing on the new device after the user confirmed the
    /// SAS: opens the bundle, installs the K_sync generations, adopts the
    /// vault material when present (the MK is re-wrapped under this
    /// device's own master password), pins the trust root, and
    /// activates sync — key installation and activation in one
    /// transaction. The first full pull is the next `sync()` call.
    pub fn finalize_pairing(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        handle: &PairingHandle,
        claimed: &ClaimedPairing,
        master_password: Option<&[u8]>,
        now: TimestampMs,
    ) -> Result<AdoptedAccount, SyncError> {
        self.finalize_pairing_with(
            conn,
            store,
            handle,
            claimed,
            master_password,
            now,
            TransportKind::Server,
        )
    }

    /// [`Self::finalize_pairing`] in the WebDAV form: identical
    /// key installation and activation, bound to the WebDAV backend.
    pub fn finalize_pairing_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        handle: &PairingHandle,
        claimed: &ClaimedPairing,
        master_password: Option<&[u8]>,
        now: TimestampMs,
    ) -> Result<AdoptedAccount, SyncError> {
        self.finalize_pairing_with(
            conn,
            store,
            handle,
            claimed,
            master_password,
            now,
            TransportKind::Webdav,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finalize_pairing_with(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        handle: &PairingHandle,
        claimed: &ClaimedPairing,
        master_password: Option<&[u8]>,
        now: TimestampMs,
        transport_kind: TransportKind,
    ) -> Result<AdoptedAccount, SyncError> {
        let bundle_json = open_pair_bundle(
            &self.identity,
            &handle.session_id,
            &claimed.trusted_ed25519_pub,
            &claimed.message,
        )?;
        let bundle = KeyBundle::from_json(&bundle_json)?;
        let sync_key_id = install_sync_generations(store, &bundle)?;

        let tx = conn.unchecked_transaction().map_err(RepoError::from)?;
        if let Some(mk_bytes) = &bundle.mk {
            let password =
                master_password.ok_or(SyncError::Pairing(PairingError::MasterPasswordRequired))?;
            let mk = symmetric_key_from(mk_bytes)?;
            install_vault_from_bundle(&tx, &bundle, &mk, password, now)?;
        }
        let root_fingerprint = *claimed.root.fingerprint().as_bytes();
        let keys = SyncKeys::load(store, sync_key_id)?;
        self.activate_within(
            &tx,
            &keys,
            SyncConfig {
                server_url: Some(handle.server_url.clone()),
                account_id: Some(handle.account_id.clone()),
                enabled: true,
                applied_server_seq: 0,
                sync_key_id,
                root_fingerprint: Some(root_fingerprint.to_vec()),
                key_update_seq: 0,
                recovery_root: None,
                transport_kind,
                webdav_push_seq: 0,
                updated_at: now,
            },
            now,
        )?;
        tx.commit().map_err(RepoError::from)?;
        Ok(AdoptedAccount {
            server_url: handle.server_url.clone(),
            account_id: handle.account_id.clone(),
            root_fingerprint,
            sync_key_id,
        })
    }

    // ----- trusted side -----

    /// Computes the SAS for a scanned pairing code on the trusted device.
    /// The code must belong to this device's account.
    pub fn pairing_sas(&self, conn: &Connection, code: &PairingCode) -> Result<String, SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() || config.account_id.as_deref() != Some(&code.account_id) {
            return Err(SyncError::Pairing(PairingError::ForeignAccount));
        }
        let pinned = crate::engine::pinned_fingerprint(&config)?;
        Ok(compute_sas(
            &code.session_id,
            &code.ed25519_pub,
            &code.x25519_pub,
            &self.identity.ed25519_public(),
            pinned.as_bytes(),
        ))
    }

    /// Admits the new device after the user confirmed the SAS: issues its
    /// certificate, assembles the key bundle (`allow_vault` governs
    /// MK/K_vault; K_sync always travels), seals it to the new device's
    /// exchange key, and posts the offer. Never call this without the
    /// user's SAS confirmation — it is what defeats a relaying server.
    pub fn approve_pairing(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        code: &PairingCode,
        allow_vault: bool,
        master_key: Option<&SymmetricKey>,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() || config.account_id.as_deref() != Some(&code.account_id) {
            return Err(SyncError::Pairing(PairingError::ForeignAccount));
        }
        let pinned = crate::engine::pinned_fingerprint(&config)?;
        let directory = self.with_auth(|t, token| t.device_directory(token))?;
        let trust = TrustState::from_directory(&directory, &pinned)?;

        // This device must itself verify to the pinned root before it may
        // vouch for anyone (the server stores admitting chain + new leaf).
        if self.device_id != trust.root().subject.device_id {
            trust
                .verified_device_keys(&pinned, &self.device_id)
                .ok_or(SyncError::Record(crate::error::RecordError::UnknownDevice))?;
        }

        let certificate = DeviceCertificate::issue(
            &self.identity,
            &self.device_id,
            CertificateSubject {
                device_id: code.device_id.clone(),
                ed25519_pub: code.ed25519_pub,
                x25519_pub: code.x25519_pub,
                name: code.device_name.clone(),
                platform: code.platform,
                created_at: now,
            },
            now,
        )?;
        let root_value: serde_json::Value =
            serde_json::from_slice(&crate::directory::root_statement_to_json(trust.root())?)
                .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let bundle = build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            allow_vault,
            master_key,
            Some(root_value),
        )?;
        let bundle_json = bundle.to_json()?;
        let message = seal_pair_bundle(
            &self.identity,
            &code.session_id,
            &code.ed25519_pub,
            &code.x25519_pub,
            &bundle_json,
        )?;
        self.with_auth(|t, token| {
            t.pair_offer(
                token,
                &code.session_id,
                &certificate,
                &message.sealed,
                &message.signature,
            )
        })
    }

    // ----- WebDAV pairing -----

    /// Starts pairing on the new device in the WebDAV form: there is no
    /// relay session — the session id is a local CSPRNG value carried
    /// inside the code, and the mailbox directory derives from it.
    pub fn begin_pairing_webdav(
        &mut self,
        conn: &Connection,
        server_url: &str,
        account_id: &str,
    ) -> Result<PairingHandle, SyncError> {
        let device = DeviceRepo::new(conn)
            .get(&self.device_id)?
            .ok_or(SyncError::Repo(RepoError::NotFound))?;
        let mut session_bytes = [0u8; 16];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut session_bytes);
        let session_id = crate::base32::encode_nopad(&session_bytes);
        let code = PairingCode {
            server_url: server_url.to_string(),
            account_id: account_id.to_string(),
            session_id: session_id.clone(),
            device_id: self.device_id.clone(),
            device_name: device.name,
            platform: device.platform,
            ed25519_pub: self.identity.ed25519_public(),
            x25519_pub: self.identity.x25519_public(),
        }
        .encode();
        Ok(PairingHandle {
            session_id,
            server_url: server_url.to_string(),
            account_id: account_id.to_string(),
            code,
        })
    }

    /// The admitting side in the WebDAV form: identical verification,
    /// certificate issuance and bundle sealing as [`Self::approve_pairing`];
    /// the offer goes into the pairing mailbox (exclusive — a code is
    /// single-use) and the new device's chain into the directory.
    #[allow(clippy::too_many_arguments)]
    pub fn approve_pairing_webdav(
        &mut self,
        conn: &Connection,
        store: &dyn SecureStore,
        dav: &crate::webdav_store::WebdavStore,
        code: &PairingCode,
        allow_vault: bool,
        master_key: Option<&SymmetricKey>,
        now: TimestampMs,
    ) -> Result<(), SyncError> {
        let config = SyncStateRepo::new(conn).config_get()?;
        if !config.is_active() || config.account_id.as_deref() != Some(&code.account_id) {
            return Err(SyncError::Pairing(PairingError::ForeignAccount));
        }
        let pinned = crate::engine::pinned_fingerprint(&config)?;
        let directory = dav
            .read_directory()?
            .ok_or(SyncError::Transport(
                crate::transport::TransportError::MalformedResponse,
            ))?
            .directory;
        let trust = TrustState::from_directory(&directory, &pinned)?;

        if self.device_id != trust.root().subject.device_id {
            trust
                .verified_device_keys(&pinned, &self.device_id)
                .ok_or(SyncError::Record(crate::error::RecordError::UnknownDevice))?;
        }

        let certificate = DeviceCertificate::issue(
            &self.identity,
            &self.device_id,
            CertificateSubject {
                device_id: code.device_id.clone(),
                ed25519_pub: code.ed25519_pub,
                x25519_pub: code.x25519_pub,
                name: code.device_name.clone(),
                platform: code.platform,
                created_at: now,
            },
            now,
        )?;
        // The chain the new device will verify: this admitting device's own
        // chain (empty when it is the root) plus the new leaf.
        let mut chain: Vec<DeviceCertificate> = if self.device_id == trust.root().subject.device_id
        {
            Vec::new()
        } else {
            let entry = directory
                .devices
                .iter()
                .find(|d| d.id == self.device_id)
                .ok_or(SyncError::Record(crate::error::RecordError::UnknownDevice))?;
            cert_chain_from_json(&entry.cert_chain_json)?
        };
        chain.push(certificate);

        let root_value: serde_json::Value =
            serde_json::from_slice(&crate::directory::root_statement_to_json(trust.root())?)
                .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let chain_value: serde_json::Value =
            serde_json::from_slice(&crate::directory::cert_chain_to_json(&chain)?)
                .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        let bundle = build_key_bundle(
            conn,
            store,
            config.sync_key_id,
            allow_vault,
            master_key,
            Some(root_value.clone()),
        )?;
        let bundle_json = bundle.to_json()?;
        let message = seal_pair_bundle(
            &self.identity,
            &code.session_id,
            &code.ed25519_pub,
            &code.x25519_pub,
            &bundle_json,
        )?;
        let payload = serde_json::json!({
            "root_statement": root_value,
            "cert_chain": chain_value,
            "sealed_bundle": BASE64.encode(&message.sealed),
            "bundle_signature": BASE64.encode(message.signature),
        });
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
        if dav.write_pairing_answer(&pairing_claim_dir(&code.session_id), &payload_bytes)?
            == crate::webdav::PutOutcome::PreconditionFailed
        {
            // The code was already answered: single-use, like the relay
            // session on the server.
            return Err(SyncError::Pairing(PairingError::MalformedCode));
        }
        self.webdav_admit_device(dav, &chain)?;
        Ok(())
    }
}

/// The pairing mailbox directory for a session: a hash so the
/// path leaks nothing about the code's key material.
pub(crate) fn pairing_claim_dir(session_id: &str) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::default();
    hasher.update(b"typvia.pair.claim.v1");
    hasher.update(session_id.as_bytes());
    let digest = hasher.finalize();
    crate::base32::encode_nopad(&digest)[..16].to_string()
}

// ----- shared bundle helpers (also used by recovery and rotation) ---------

/// Assembles the key bundle from local state: every held K_sync
/// generation (the current one must exist), plus MK and all K_vault
/// generations when `include_vault` and a vault exists (the caller must
/// then supply the unlocked master key).
pub(crate) fn build_key_bundle(
    conn: &Connection,
    store: &dyn SecureStore,
    sync_key_id: u32,
    include_vault: bool,
    master_key: Option<&SymmetricKey>,
    root_statement: Option<serde_json::Value>,
) -> Result<KeyBundle, SyncError> {
    let mut k_sync = Vec::new();
    for key_id in 1..=sync_key_id {
        if let Some(secret) = store
            .retrieve(&keyring::entry_name(key_id))
            .map_err(KeyringError::from)?
        {
            k_sync.push(KeyGeneration {
                key_id,
                key: secret.to_vec(),
            });
        }
    }
    if !k_sync.iter().any(|g| g.key_id == sync_key_id) {
        return Err(SyncError::Keyring(KeyringError::MissingCurrentKey(
            sync_key_id,
        )));
    }

    let vault_exists = VaultKeyRepo::new(conn).load_header()?.is_some();
    let (mk, k_vault) = if include_vault && vault_exists {
        let mk = master_key.ok_or(SyncError::Pairing(PairingError::MasterKeyRequired))?;
        let mut generations = Vec::new();
        for domain_key in VaultKeyRepo::new(conn).list_domain_keys()? {
            if domain_key.domain != KeyDomain::Vault {
                continue;
            }
            let key = typvia_crypto::unwrap_key(
                mk,
                &aad_domain_key(KeyDomain::Vault.as_str()),
                &domain_key.wrapped_key,
            )
            .map_err(KeyringError::Crypto)?;
            generations.push(KeyGeneration {
                key_id: domain_key.key_id,
                key: key.expose().to_vec(),
            });
        }
        (Some(mk.expose().to_vec()), Some(generations))
    } else {
        (None, None)
    };

    Ok(KeyBundle {
        mk,
        k_sync,
        k_vault,
        root_statement,
    })
}

/// Installs every K_sync generation of a bundle into the secure store and
/// returns the highest generation. An empty K_sync set is malformed:
/// K_sync always travels.
pub(crate) fn install_sync_generations(
    store: &dyn SecureStore,
    bundle: &KeyBundle,
) -> Result<u32, SyncError> {
    let mut highest = 0u32;
    for generation in &bundle.k_sync {
        let key = symmetric_key_from(&generation.key)?;
        install_key(store, generation.key_id, &key)?;
        highest = highest.max(generation.key_id);
    }
    if highest == 0 {
        return Err(SyncError::Pairing(PairingError::MalformedBundle));
    }
    Ok(highest)
}

/// Adopts the vault material of a bundle on this device: the MK is
/// re-wrapped under a KEK derived from this device's own master password
/// (fresh salt, standard Argon2id parameters) into the key header, and the
/// K_vault / K_sync generations are MK-wrapped into `domain_key`. Refuses
/// when a vault already exists — a foreign MK must never silently replace
/// an existing one. Runs inside the caller's transaction.
pub(crate) fn install_vault_from_bundle(
    conn: &Connection,
    bundle: &KeyBundle,
    mk: &SymmetricKey,
    master_password: &[u8],
    now: TimestampMs,
) -> Result<(), SyncError> {
    let repo = VaultKeyRepo::new(conn);
    if repo.load_header()?.is_some() {
        return Err(SyncError::Pairing(PairingError::VaultAlreadyInitialized));
    }
    let params = KdfParams::v1();
    let kek = derive_kek(master_password, &params).map_err(KeyringError::Crypto)?;
    let wrapped_mk = wrap_key(&kek, 1, &aad_master_key(), mk).map_err(KeyringError::Crypto)?;
    repo.put_header(&VaultKeyHeader {
        id: new_id(),
        kdf: params,
        wrapped_mk,
        created_at: now,
        updated_at: now,
    })?;
    for generation in bundle.k_vault.iter().flatten() {
        let key = symmetric_key_from(&generation.key)?;
        let wrapped = wrap_key(
            mk,
            generation.key_id,
            &aad_domain_key(KeyDomain::Vault.as_str()),
            &key,
        )
        .map_err(KeyringError::Crypto)?;
        repo.put_domain_key(&DomainKey {
            domain: KeyDomain::Vault,
            key_id: generation.key_id,
            wrapped_key: wrapped,
            created_at: now,
        })?;
    }
    // The MK-wrapped K_sync copies are the canonical sealing source once a
    // vault exists.
    for generation in &bundle.k_sync {
        let key = symmetric_key_from(&generation.key)?;
        let wrapped = wrap_key(
            mk,
            generation.key_id,
            &aad_domain_key(KeyDomain::Sync.as_str()),
            &key,
        )
        .map_err(KeyringError::Crypto)?;
        repo.put_domain_key(&DomainKey {
            domain: KeyDomain::Sync,
            key_id: generation.key_id,
            wrapped_key: wrapped,
            created_at: now,
        })?;
    }
    Ok(())
}

/// Copies 32 key bytes into a self-zeroizing key; the stack copy is wiped.
pub(crate) fn symmetric_key_from(bytes: &[u8]) -> Result<SymmetricKey, SyncError> {
    let mut raw: [u8; 32] = bytes
        .try_into()
        .map_err(|_| SyncError::Pairing(PairingError::MalformedBundle))?;
    let key = SymmetricKey::from_bytes(raw);
    raw.zeroize();
    Ok(key)
}
