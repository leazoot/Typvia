// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Pairing primitives: the pairing-code codec, the SAS short code both ends compare, the sealed
//! key-bundle construction, and the key-bundle document.
//!
//! Everything here is transport-free and deterministic given its inputs;
//! the engine flows (provision.rs / rotation.rs / recovery.rs) compose
//! these primitives against the server. Shared secrets, derived keys, and
//! bundle plaintext are zeroized; nothing in this module logs.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use chacha20poly1305::aead::{Aead, OsRng, Payload};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, VerifyingKey};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use typvia_core::model::Platform;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::base32::encode_nopad;
use crate::identity::DeviceIdentity;

/// Text prefix of the encoded pairing code.
pub const PAIRING_CODE_PREFIX: &str = "TYPVIA-PAIR.V1.";

const SAS_INFO: &[u8] = b"typvia.sas.v1";
const PAIR_CONTEXT: &[u8] = b"typvia.pair.v1";
const PAIR_SIG_CONTEXT: &[u8] = b"typvia.pairsig.v1";
const KEYUPD_CONTEXT: &[u8] = b"typvia.keyupd.v1";
const KEYUPD_SIG_CONTEXT: &[u8] = b"typvia.keyupdsig.v1";

/// Sealed layout: eph_pub(32) || nonce(24) || AEAD(ct || tag(16)).
const EPH_PUB_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const MIN_SEALED_LEN: usize = EPH_PUB_LEN + NONCE_LEN + TAG_LEN;

/// Failures of the pairing primitives and flows. Structural facts only —
/// never key material or bundle content.
#[derive(Debug, PartialEq, Eq)]
pub enum PairingError {
    /// The pairing-code string does not parse to the documented shape.
    MalformedCode,
    /// The scanned code belongs to another account or server than the one
    /// this device is bound to.
    ForeignAccount,
    /// The relayed certificate chain does not end at this device's keys.
    ForeignCertificate,
    /// The Ed25519 signature over the sealed bytes failed verification.
    BadSignature,
    /// The sealed bytes are structurally invalid or failed to decrypt
    /// (wrong recipient, mismatched session, or tampering).
    DecryptFailed,
    /// The decrypted key bundle is not a valid bundle document.
    MalformedBundle,
    /// The bundle carries the master key but no master password was
    /// provided to re-wrap it locally.
    MasterPasswordRequired,
    /// A vault already exists locally; adopting a foreign MK would orphan
    /// it, so the flow refuses instead of overwriting.
    VaultAlreadyInitialized,
    /// The vault is initialized but its master key was not provided, so
    /// the promised MK/K_vault material cannot be assembled.
    MasterKeyRequired,
    /// The AEAD cipher failed while sealing (system error).
    EncryptFailed,
}

impl fmt::Display for PairingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::MalformedCode => "pairing code is malformed",
            Self::ForeignAccount => "pairing code belongs to a different account",
            Self::ForeignCertificate => "certificate chain does not end at this device",
            Self::BadSignature => "sealed bundle signature verification failed",
            Self::DecryptFailed => "sealed bundle could not be opened",
            Self::MalformedBundle => "key bundle document is malformed",
            Self::MasterPasswordRequired => "a master password is required to adopt the vault",
            Self::VaultAlreadyInitialized => "a vault already exists on this device",
            Self::MasterKeyRequired => "the vault master key is required for this operation",
            Self::EncryptFailed => "sealing the key bundle failed",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PairingError {}

// ----- pairing code -------------------------------------------------------

/// The pairing-code payload the new device displays: the session facts
/// plus the new device's `device_id`/`name`/`platform`, which the
/// admitting side needs to build the certificate subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    pub server_url: String,
    pub account_id: String,
    pub session_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: Platform,
    pub ed25519_pub: [u8; 32],
    pub x25519_pub: [u8; 32],
}

/// JSON body inside the encoded pairing code.
#[derive(Serialize, Deserialize)]
struct PairingCodeDoc {
    server_url: String,
    account_id: String,
    session_id: String,
    device_id: String,
    device_name: String,
    platform: String,
    ed25519_pub: String,
    x25519_pub: String,
}

impl PairingCode {
    /// Encodes the code as its text form (`TYPVIA-PAIR.V1.<base64(json)>`).
    pub fn encode(&self) -> String {
        // Construction from validated parts cannot fail to serialize.
        let doc = PairingCodeDoc {
            server_url: self.server_url.clone(),
            account_id: self.account_id.clone(),
            session_id: self.session_id.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            platform: self.platform.as_str().to_string(),
            ed25519_pub: BASE64.encode(self.ed25519_pub),
            x25519_pub: BASE64.encode(self.x25519_pub),
        };
        let json = serde_json::to_vec(&doc).unwrap_or_default();
        format!("{PAIRING_CODE_PREFIX}{}", BASE64.encode(json))
    }

    /// Parses and validates a scanned/pasted pairing code.
    pub fn decode(text: &str) -> Result<Self, PairingError> {
        let body = text
            .trim()
            .strip_prefix(PAIRING_CODE_PREFIX)
            .ok_or(PairingError::MalformedCode)?;
        let json = BASE64
            .decode(body)
            .map_err(|_| PairingError::MalformedCode)?;
        let doc: PairingCodeDoc =
            serde_json::from_slice(&json).map_err(|_| PairingError::MalformedCode)?;
        let code = Self {
            ed25519_pub: decode_32(&doc.ed25519_pub)?,
            x25519_pub: decode_32(&doc.x25519_pub)?,
            platform: doc
                .platform
                .parse()
                .map_err(|_| PairingError::MalformedCode)?,
            server_url: doc.server_url,
            account_id: doc.account_id,
            session_id: doc.session_id,
            device_id: doc.device_id,
            device_name: doc.device_name,
        };
        for field in [
            &code.server_url,
            &code.account_id,
            &code.session_id,
            &code.device_id,
            &code.device_name,
        ] {
            if field.is_empty() || field.contains('\0') {
                return Err(PairingError::MalformedCode);
            }
        }
        Ok(code)
    }
}

fn decode_32(b64: &str) -> Result<[u8; 32], PairingError> {
    BASE64
        .decode(b64)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(PairingError::MalformedCode)
}

// ----- SAS ----------------------------------------------------------------

/// The SAS short code: `HKDF-SHA-256(salt = session_id, ikm =
/// N.ed25519_pub || N.x25519_pub || T.ed25519_pub || root_fingerprint,
/// info = "typvia.sas.v1")`, base32-encoded, first 20 characters in 4
/// hyphen-joined groups of 5 (the fingerprint display format).
pub fn compute_sas(
    session_id: &str,
    new_ed25519_pub: &[u8; 32],
    new_x25519_pub: &[u8; 32],
    trusted_ed25519_pub: &[u8; 32],
    root_fingerprint: &[u8; 32],
) -> String {
    let mut ikm = Zeroizing::new([0u8; 128]);
    ikm[0..32].copy_from_slice(new_ed25519_pub);
    ikm[32..64].copy_from_slice(new_x25519_pub);
    ikm[64..96].copy_from_slice(trusted_ed25519_pub);
    ikm[96..128].copy_from_slice(root_fingerprint);
    let hk = Hkdf::<Sha256>::new(Some(session_id.as_bytes()), ikm.as_ref());
    let mut okm = Zeroizing::new([0u8; 32]);
    // A 32-byte output cannot overflow HKDF's expand limit.
    let _ = hk.expand(SAS_INFO, okm.as_mut());
    let code = encode_nopad(okm.as_ref());
    format!(
        "{}-{}-{}-{}",
        &code[0..5],
        &code[5..10],
        &code[10..15],
        &code[15..20]
    )
}

// ----- key bundle document -----------------------------------------------

/// One key generation inside a bundle. Key bytes wipe on drop.
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct KeyGeneration {
    pub key_id: u32,
    #[serde(with = "b64_bytes")]
    pub key: Vec<u8>,
}

/// The key bundle: K_sync always (all held generations), MK and
/// K_vault only under the vault-access policy, and the account root
/// statement for pairing (absent in key-update bundles).
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop, Default)]
pub struct KeyBundle {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "opt_b64_bytes"
    )]
    pub mk: Option<Vec<u8>>,
    pub k_sync: Vec<KeyGeneration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k_vault: Option<Vec<KeyGeneration>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    pub root_statement: Option<serde_json::Value>,
}

impl KeyBundle {
    /// Serializes to the JSON document sealed into the bundle ciphertext.
    /// The buffer wipes on drop.
    pub fn to_json(&self) -> Result<Zeroizing<Vec<u8>>, PairingError> {
        serde_json::to_vec(self)
            .map(Zeroizing::new)
            .map_err(|_| PairingError::MalformedBundle)
    }

    /// Parses and structurally validates a decrypted bundle document.
    pub fn from_json(json: &[u8]) -> Result<Self, PairingError> {
        let bundle: Self =
            serde_json::from_slice(json).map_err(|_| PairingError::MalformedBundle)?;
        let key_ok = |g: &KeyGeneration| g.key.len() == 32 && g.key_id >= 1;
        if !bundle.k_sync.iter().all(key_ok)
            || !bundle.k_vault.iter().flatten().all(key_ok)
            || bundle.mk.as_ref().is_some_and(|mk| mk.len() != 32)
        {
            return Err(PairingError::MalformedBundle);
        }
        Ok(bundle)
    }
}

impl fmt::Debug for KeyBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Key bytes never reach Debug output.
        f.debug_struct("KeyBundle")
            .field("has_mk", &self.mk.is_some())
            .field("k_sync_generations", &self.k_sync.len())
            .field("k_vault_generations", &self.k_vault.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

mod b64_bytes {
    use super::BASE64;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        BASE64
            .decode(&text)
            .map_err(|_| serde::de::Error::custom("invalid base64"))
    }
}

mod opt_b64_bytes {
    use super::BASE64;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => s.serialize_some(&BASE64.encode(b)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        text.map(|t| {
            BASE64
                .decode(&t)
                .map_err(|_| serde::de::Error::custom("invalid base64"))
        })
        .transpose()
    }
}

// ----- sealed messages (key updates use the same construction) -----------

/// A sealed device-to-device message plus its sender signature.
pub struct SealedMessage {
    /// `eph_pub(32) || nonce(24) || AEAD(ct || tag)`.
    pub sealed: Vec<u8>,
    pub signature: [u8; 64],
}

impl fmt::Debug for SealedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedMessage")
            .field("sealed_len", &self.sealed.len())
            .finish_non_exhaustive()
    }
}

/// Seals a pairing key bundle to the new device: X25519
/// ephemeral ECDH, HKDF with the session id as salt, XChaCha20-Poly1305
/// with the AAD binding session and recipient identity, signed by the
/// trusted device over `"typvia.pairsig.v1" || SHA-256(sealed) ||
/// session_id`.
pub fn seal_pair_bundle(
    trusted: &DeviceIdentity,
    session_id: &str,
    new_ed25519_pub: &[u8; 32],
    new_x25519_pub: &[u8; 32],
    bundle_json: &[u8],
) -> Result<SealedMessage, PairingError> {
    let aad = pair_aad(session_id, new_ed25519_pub);
    let sealed = seal_to(
        new_x25519_pub,
        session_id.as_bytes(),
        PAIR_CONTEXT,
        &aad,
        bundle_json,
    )?;
    let signature = trusted
        .sign(&seal_sig_bytes(
            PAIR_SIG_CONTEXT,
            &sealed,
            session_id.as_bytes(),
        ))
        .to_bytes();
    Ok(SealedMessage { sealed, signature })
}

/// Verifies only the trusted device's signature over a sealed pairing
/// bundle, without decrypting — the fail-fast check the new device runs
/// when the offer arrives, before the user's SAS confirmation.
pub(crate) fn verify_pair_signature(
    trusted_ed25519_pub: &[u8; 32],
    session_id: &str,
    message: &SealedMessage,
) -> Result<(), PairingError> {
    verify_seal_signature(
        trusted_ed25519_pub,
        PAIR_SIG_CONTEXT,
        &message.sealed,
        session_id.as_bytes(),
        &message.signature,
    )
}

/// Verifies and opens a pairing key bundle on the new device:
/// the trusted device's signature first, then the AAD-bound decryption.
pub fn open_pair_bundle(
    new_device: &DeviceIdentity,
    session_id: &str,
    trusted_ed25519_pub: &[u8; 32],
    message: &SealedMessage,
) -> Result<Zeroizing<Vec<u8>>, PairingError> {
    verify_seal_signature(
        trusted_ed25519_pub,
        PAIR_SIG_CONTEXT,
        &message.sealed,
        session_id.as_bytes(),
        &message.signature,
    )?;
    let aad = pair_aad(session_id, &new_device.ed25519_public());
    open_from(
        new_device,
        session_id.as_bytes(),
        PAIR_CONTEXT,
        &aad,
        &message.sealed,
    )
}

/// Seals a key-update bundle to a target device: the pairing-bundle
/// construction with the target device id as HKDF salt and AAD suffix,
/// signed over `"typvia.keyupdsig.v1" || SHA-256(sealed) ||
/// target_device_id`).
pub fn seal_key_update(
    sender: &DeviceIdentity,
    target_device_id: &str,
    target_x25519_pub: &[u8; 32],
    bundle_json: &[u8],
) -> Result<SealedMessage, PairingError> {
    let aad = keyupd_aad(target_device_id);
    let sealed = seal_to(
        target_x25519_pub,
        target_device_id.as_bytes(),
        KEYUPD_CONTEXT,
        &aad,
        bundle_json,
    )?;
    let signature = sender
        .sign(&seal_sig_bytes(
            KEYUPD_SIG_CONTEXT,
            &sealed,
            target_device_id.as_bytes(),
        ))
        .to_bytes();
    Ok(SealedMessage { sealed, signature })
}

/// Verifies and opens a key-update bundle on the target device.
pub fn open_key_update(
    target: &DeviceIdentity,
    target_device_id: &str,
    sender_ed25519_pub: &[u8; 32],
    message: &SealedMessage,
) -> Result<Zeroizing<Vec<u8>>, PairingError> {
    verify_seal_signature(
        sender_ed25519_pub,
        KEYUPD_SIG_CONTEXT,
        &message.sealed,
        target_device_id.as_bytes(),
        &message.signature,
    )?;
    let aad = keyupd_aad(target_device_id);
    open_from(
        target,
        target_device_id.as_bytes(),
        KEYUPD_CONTEXT,
        &aad,
        &message.sealed,
    )
}

fn pair_aad(session_id: &str, new_ed25519_pub: &[u8; 32]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(PAIR_CONTEXT.len() + session_id.len() + 32);
    aad.extend_from_slice(PAIR_CONTEXT);
    aad.extend_from_slice(session_id.as_bytes());
    aad.extend_from_slice(new_ed25519_pub);
    aad
}

fn keyupd_aad(target_device_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(KEYUPD_CONTEXT.len() + target_device_id.len());
    aad.extend_from_slice(KEYUPD_CONTEXT);
    aad.extend_from_slice(target_device_id.as_bytes());
    aad
}

/// `context || SHA-256(sealed) || trailer` — the signed byte string of a
/// sealed message.
fn seal_sig_bytes(context: &[u8], sealed: &[u8], trailer: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(context.len() + 32 + trailer.len());
    bytes.extend_from_slice(context);
    bytes.extend_from_slice(&Sha256::digest(sealed));
    bytes.extend_from_slice(trailer);
    bytes
}

fn verify_seal_signature(
    public_key: &[u8; 32],
    context: &[u8],
    sealed: &[u8],
    trailer: &[u8],
    signature: &[u8; 64],
) -> Result<(), PairingError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| PairingError::BadSignature)?;
    key.verify_strict(
        &seal_sig_bytes(context, sealed, trailer),
        &Signature::from_bytes(signature),
    )
    .map_err(|_| PairingError::BadSignature)
}

/// ECDH + HKDF + XChaCha20-Poly1305 seal; `ss` and `k` zeroize on drop.
fn seal_to(
    recipient_x25519_pub: &[u8; 32],
    salt: &[u8],
    info: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, PairingError> {
    let eph = StaticSecret::random_from_rng(OsRng);
    let eph_pub = X25519PublicKey::from(&eph);
    let ss = eph.diffie_hellman(&X25519PublicKey::from(*recipient_x25519_pub));
    let key = derive_seal_key(ss.as_bytes(), salt, info);

    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| PairingError::EncryptFailed)?;

    let mut sealed = Vec::with_capacity(EPH_PUB_LEN + NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(eph_pub.as_bytes());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

fn open_from(
    recipient: &DeviceIdentity,
    salt: &[u8],
    info: &[u8],
    aad: &[u8],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, PairingError> {
    if sealed.len() < MIN_SEALED_LEN {
        return Err(PairingError::DecryptFailed);
    }
    let eph_pub: [u8; 32] = sealed[..EPH_PUB_LEN]
        .try_into()
        .map_err(|_| PairingError::DecryptFailed)?;
    let nonce = XNonce::from_slice(&sealed[EPH_PUB_LEN..EPH_PUB_LEN + NONCE_LEN]);
    let ss = recipient.diffie_hellman(&eph_pub);
    let key = derive_seal_key(ss.as_bytes(), salt, info);

    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &sealed[EPH_PUB_LEN + NONCE_LEN..],
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| PairingError::DecryptFailed)
}

fn derive_seal_key(shared_secret: &[u8; 32], salt: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared_secret);
    let mut okm = Zeroizing::new([0u8; 32]);
    // A 32-byte output cannot overflow HKDF's expand limit.
    let _ = hk.expand(info, okm.as_mut());
    okm
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn code() -> PairingCode {
        let identity = DeviceIdentity::generate();
        PairingCode {
            server_url: "https://sync.example.test".to_string(),
            account_id: "acct-1".to_string(),
            session_id: "sess-1".to_string(),
            device_id: "new-dev".to_string(),
            device_name: "New phone".to_string(),
            platform: Platform::Ios,
            ed25519_pub: identity.ed25519_public(),
            x25519_pub: identity.x25519_public(),
        }
    }

    #[test]
    fn pairing_code_round_trips_through_its_text_form() {
        let original = code();
        let decoded = PairingCode::decode(&original.encode()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn a_pairing_code_without_the_prefix_or_with_bad_body_is_rejected() {
        assert_eq!(
            PairingCode::decode("nonsense").unwrap_err(),
            PairingError::MalformedCode
        );
        assert_eq!(
            PairingCode::decode("TYPVIA-PAIR.V1.!!!").unwrap_err(),
            PairingError::MalformedCode
        );
        let valid_base64_garbage = format!("{PAIRING_CODE_PREFIX}{}", BASE64.encode(b"{}"));
        assert_eq!(
            PairingCode::decode(&valid_base64_garbage).unwrap_err(),
            PairingError::MalformedCode
        );
    }

    #[test]
    fn both_ends_compute_the_same_sas_from_the_same_inputs() {
        let n = DeviceIdentity::generate();
        let t = DeviceIdentity::generate();
        let root_fp = [0xAB; 32];
        let on_t = compute_sas(
            "sess-1",
            &n.ed25519_public(),
            &n.x25519_public(),
            &t.ed25519_public(),
            &root_fp,
        );
        let on_n = compute_sas(
            "sess-1",
            &n.ed25519_public(),
            &n.x25519_public(),
            &t.ed25519_public(),
            &root_fp,
        );
        assert_eq!(on_t, on_n);
        // 4 hyphen-joined groups of 5 base32 characters.
        let groups: Vec<&str> = on_t.split('-').collect();
        assert_eq!(groups.len(), 4);
        assert!(groups.iter().all(|g| g.len() == 5));
    }

    #[test]
    fn a_substituted_public_key_changes_the_sas() {
        // The MITM detection property: a server swapping the new
        // device's key produces a different SAS on the trusted side.
        let n = DeviceIdentity::generate();
        let attacker = DeviceIdentity::generate();
        let t = DeviceIdentity::generate();
        let root_fp = [0xAB; 32];
        let honest = compute_sas(
            "sess-1",
            &n.ed25519_public(),
            &n.x25519_public(),
            &t.ed25519_public(),
            &root_fp,
        );
        let swapped = compute_sas(
            "sess-1",
            &attacker.ed25519_public(),
            &attacker.x25519_public(),
            &t.ed25519_public(),
            &root_fp,
        );
        assert_ne!(honest, swapped);
    }

    #[test]
    fn a_pair_bundle_round_trips_between_the_two_identities() {
        let trusted = DeviceIdentity::generate();
        let new_device = DeviceIdentity::generate();
        let bundle = br#"{"k_sync":[{"key_id":1,"key":"AAAA"}]}"#;
        let message = seal_pair_bundle(
            &trusted,
            "sess-1",
            &new_device.ed25519_public(),
            &new_device.x25519_public(),
            bundle,
        )
        .unwrap();
        let opened =
            open_pair_bundle(&new_device, "sess-1", &trusted.ed25519_public(), &message).unwrap();
        assert_eq!(opened.as_slice(), bundle);
    }

    #[test]
    fn a_tampered_signature_or_sealed_byte_is_rejected() {
        let trusted = DeviceIdentity::generate();
        let new_device = DeviceIdentity::generate();
        let message = seal_pair_bundle(
            &trusted,
            "sess-1",
            &new_device.ed25519_public(),
            &new_device.x25519_public(),
            b"secret bundle",
        )
        .unwrap();

        let mut bad_sig = SealedMessage {
            sealed: message.sealed.clone(),
            signature: message.signature,
        };
        bad_sig.signature[0] ^= 0x01;
        assert_eq!(
            open_pair_bundle(&new_device, "sess-1", &trusted.ed25519_public(), &bad_sig)
                .unwrap_err(),
            PairingError::BadSignature
        );

        // Flipping ciphertext also breaks the signature (it covers the
        // sealed bytes), which is the first check to fire.
        let mut bad_ct = SealedMessage {
            sealed: message.sealed.clone(),
            signature: message.signature,
        };
        let last = bad_ct.sealed.len() - 1;
        bad_ct.sealed[last] ^= 0x01;
        assert_eq!(
            open_pair_bundle(&new_device, "sess-1", &trusted.ed25519_public(), &bad_ct)
                .unwrap_err(),
            PairingError::BadSignature
        );
    }

    #[test]
    fn a_bundle_sealed_for_one_session_does_not_open_under_another() {
        // A malicious relay moving the offer between sessions fails both
        // the HKDF salt and the AAD binding.
        let trusted = DeviceIdentity::generate();
        let new_device = DeviceIdentity::generate();
        let message = seal_pair_bundle(
            &trusted,
            "sess-1",
            &new_device.ed25519_public(),
            &new_device.x25519_public(),
            b"secret bundle",
        )
        .unwrap();
        // Re-sign for the other session so decryption (not the signature)
        // is what must reject the transplant.
        let resigned = SealedMessage {
            signature: trusted
                .sign(&seal_sig_bytes(
                    PAIR_SIG_CONTEXT,
                    &message.sealed,
                    b"sess-2",
                ))
                .to_bytes(),
            sealed: message.sealed,
        };
        assert_eq!(
            open_pair_bundle(&new_device, "sess-2", &trusted.ed25519_public(), &resigned)
                .unwrap_err(),
            PairingError::DecryptFailed
        );
    }

    #[test]
    fn a_bundle_sealed_for_another_recipient_does_not_open() {
        let trusted = DeviceIdentity::generate();
        let intended = DeviceIdentity::generate();
        let thief = DeviceIdentity::generate();
        let message = seal_pair_bundle(
            &trusted,
            "sess-1",
            &intended.ed25519_public(),
            &intended.x25519_public(),
            b"secret bundle",
        )
        .unwrap();
        assert_eq!(
            open_pair_bundle(&thief, "sess-1", &trusted.ed25519_public(), &message).unwrap_err(),
            PairingError::DecryptFailed
        );
    }

    #[test]
    fn a_key_update_round_trips_and_binds_its_target() {
        let sender = DeviceIdentity::generate();
        let target = DeviceIdentity::generate();
        let message = seal_key_update(
            &sender,
            "dev-b",
            &target.x25519_public(),
            b"rotation bundle",
        )
        .unwrap();
        let opened = open_key_update(&target, "dev-b", &sender.ed25519_public(), &message).unwrap();
        assert_eq!(opened.as_slice(), b"rotation bundle");

        // The same message under another target id fails the signature.
        assert_eq!(
            open_key_update(&target, "dev-c", &sender.ed25519_public(), &message).unwrap_err(),
            PairingError::BadSignature
        );
    }

    #[test]
    fn pair_and_key_update_signature_domains_are_separated() {
        // A valid pairing message must not verify as a key update even
        // with matching trailer bytes (distinct signature contexts).
        let sender = DeviceIdentity::generate();
        let target = DeviceIdentity::generate();
        let message = seal_pair_bundle(
            &sender,
            "same-trailer",
            &target.ed25519_public(),
            &target.x25519_public(),
            b"bundle",
        )
        .unwrap();
        assert_eq!(
            open_key_update(&target, "same-trailer", &sender.ed25519_public(), &message)
                .unwrap_err(),
            PairingError::BadSignature
        );
    }

    #[test]
    fn key_bundle_json_round_trips_and_validates_key_lengths() {
        let bundle = KeyBundle {
            mk: Some(vec![0xAA; 32]),
            k_sync: vec![
                KeyGeneration {
                    key_id: 1,
                    key: vec![0x01; 32],
                },
                KeyGeneration {
                    key_id: 2,
                    key: vec![0x02; 32],
                },
            ],
            k_vault: Some(vec![KeyGeneration {
                key_id: 1,
                key: vec![0x03; 32],
            }]),
            root_statement: Some(serde_json::json!({"device_id": "root"})),
        };
        let json = bundle.to_json().unwrap();
        let parsed = KeyBundle::from_json(&json).unwrap();
        assert_eq!(parsed.mk, bundle.mk);
        assert_eq!(parsed.k_sync.len(), 2);
        assert_eq!(parsed.k_vault.as_ref().unwrap().len(), 1);
        assert!(parsed.root_statement.is_some());

        // A short key is rejected structurally.
        let bad = br#"{"k_sync":[{"key_id":1,"key":"AAAA"}]}"#;
        assert_eq!(
            KeyBundle::from_json(bad).unwrap_err(),
            PairingError::MalformedBundle
        );
    }

    #[test]
    fn key_bundle_debug_output_carries_no_key_bytes() {
        let bundle = KeyBundle {
            mk: Some(vec![0xCD; 32]),
            k_sync: vec![KeyGeneration {
                key_id: 1,
                key: vec![0xEF; 32],
            }],
            k_vault: None,
            root_statement: None,
        };
        let rendered = format!("{bundle:?}");
        assert!(!rendered.contains("cd"));
        assert!(!rendered.contains("ef"));
        assert!(rendered.contains("has_mk"));
    }
}
