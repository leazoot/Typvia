// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! E2EE change-set records.
//!
//! Sealing turns a validated entity document (JSON, opaque bytes at this
//! layer) into the wire record: an outer K_sync envelope whose AAD binds
//! entity identity and version, plus the device's Ed25519 signature over
//! the signed byte string. Opening runs the receiver order — certificate
//! chain, strict signature, AAD-bound decryption, payload validation,
//! tombstone invariant — and classifies every rejection with a distinct
//! [`RecordError`] variant. Sensitive snippet bodies stay double-enveloped:
//! they travel inside the document as K_vault ciphertext this layer never
//! touches, so opening needs only K_sync.

use std::collections::HashMap;
use std::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use typvia_core::model::{DeviceId, SyncEntityType, SyncRecordId, TimestampMs};
use typvia_crypto::{CryptoError, SymmetricKey};
use zeroize::Zeroizing;

use crate::cert::{DeviceCertificate, RootStatement, verify_certificate_chain};
use crate::error::{CertificateError, RecordError};
use crate::identity::{DeviceIdentity, Fingerprint};

/// Per-record envelope size limit; enforced on both seal and
/// open so an oversized record is rejected before any cryptography.
pub const MAX_RECORD_ENVELOPE_LEN: usize = 256 * 1024;

/// The payload document schema version this client produces and applies;
/// it evolves independently of the protocol version.
pub const SUPPORTED_PAYLOAD_VERSION: u64 = 1;

const SIGNATURE_CONTEXT: &[u8] = b"typvia.syncrec.v1";
const AAD_CONTEXT: &[u8] = b"typvia.sync.v1";
/// `deleted_at` slot value for non-tombstone records.
const NOT_DELETED_SENTINEL: [u8; 8] = [0xFF; 8];
/// Tombstones carry no envelope; K_sync generations start at 1, so
/// 0 unambiguously means "no key was used".
const TOMBSTONE_KEY_ID: u32 = 0;

/// Entity metadata a change-set record is built from: the wire record
/// minus the cryptographic fields this module produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordMeta {
    /// Client-generated record UUID (idempotency key).
    pub id: SyncRecordId,
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    /// Monotonic per-entity version.
    pub version: u64,
    /// Producing device.
    pub device_id: DeviceId,
    /// Producer's local clock, milliseconds UTC.
    pub updated_at: TimestampMs,
}

/// The wire form of one change-set record. `ciphertext` holds the raw
/// envelope bytes (transport encodes them as base64); it is empty exactly
/// when the record is a tombstone.
#[derive(Clone, PartialEq, Eq)]
pub struct WireRecord {
    pub id: SyncRecordId,
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub version: u64,
    /// Outer K_sync envelope; empty for tombstones.
    pub ciphertext: Vec<u8>,
    /// Tombstone marker.
    pub deleted_at: Option<TimestampMs>,
    pub updated_at: TimestampMs,
    pub device_id: DeviceId,
    /// K_sync generation, mirrored out of the envelope header so clients
    /// without the new key can detect a pending rotation.
    pub key_id: u32,
    /// Ed25519 signature over the signed byte string.
    pub signature: [u8; 64],
}

impl WireRecord {
    /// True when this record marks a deletion.
    pub fn is_tombstone(&self) -> bool {
        self.deleted_at.is_some()
    }
}

impl fmt::Debug for WireRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Ciphertext and signature bytes stay out of Debug output: nothing
        // resembling record content may reach logs.
        f.debug_struct("WireRecord")
            .field("id", &self.id)
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("version", &self.version)
            .field("ciphertext_len", &self.ciphertext.len())
            .field("deleted_at", &self.deleted_at)
            .field("updated_at", &self.updated_at)
            .field("device_id", &self.device_id)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

/// What a successfully verified record contains.
pub enum OpenedRecord {
    /// A content record; `document` is the decrypted JSON document,
    /// byte-identical to what the producer sealed.
    Content { document: Zeroizing<Vec<u8>> },
    /// A tombstone; application means local soft deletion.
    Tombstone { deleted_at: TimestampMs },
}

impl fmt::Debug for OpenedRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The decrypted document is user content and never reaches Debug.
        match self {
            Self::Content { document } => f
                .debug_struct("OpenedRecord::Content")
                .field("document_len", &document.len())
                .finish(),
            Self::Tombstone { deleted_at } => f
                .debug_struct("OpenedRecord::Tombstone")
                .field("deleted_at", deleted_at)
                .finish(),
        }
    }
}

/// The receiver's trust state: the pinned root, the account's root
/// statement, the device directory's certificate chains, and the
/// revocation table.
pub struct TrustContext<'a> {
    /// The root fingerprint pinned on this device at pairing time.
    pub known_root: &'a Fingerprint,
    pub root: &'a RootStatement,
    /// Root-first certificate chain per non-root device.
    pub chains: &'a HashMap<DeviceId, Vec<DeviceCertificate>>,
    /// Revocation time per revoked device.
    pub revoked_at: &'a HashMap<DeviceId, TimestampMs>,
}

/// Lookup of K_sync generations held by this client. Returning `None`
/// signals a pending rotation, not an error in the keyring.
pub trait SyncKeyring {
    /// The K_sync key for `key_id`, if this client holds that generation.
    fn key(&self, key_id: u32) -> Option<&SymmetricKey>;
}

impl SyncKeyring for HashMap<u32, SymmetricKey> {
    fn key(&self, key_id: u32) -> Option<&SymmetricKey> {
        self.get(&key_id)
    }
}

/// Seals a content record: validates the document, encrypts it under
/// K_sync with the record AAD, and signs the record byte string.
pub fn seal_record(
    meta: RecordMeta,
    document: &[u8],
    key: &SymmetricKey,
    key_id: u32,
    identity: &DeviceIdentity,
) -> Result<WireRecord, RecordError> {
    // Producers may only emit the version they support; anything else in
    // an outgoing document is a construction bug, not a pending state.
    if probe_payload_version(document)? != SUPPORTED_PAYLOAD_VERSION {
        return Err(RecordError::MalformedPayload);
    }
    let aad = record_aad(meta.entity_type, &meta.entity_id, meta.version)?;
    let ciphertext =
        typvia_crypto::seal(key, key_id, &aad, document).map_err(|_| RecordError::EncryptFailed)?;
    if ciphertext.len() > MAX_RECORD_ENVELOPE_LEN {
        return Err(RecordError::TooLarge);
    }
    finish_record(meta, ciphertext, None, key_id, identity)
}

/// Seals a tombstone: no envelope, the signature covers the
/// deletion time and the hash of the empty ciphertext.
pub fn seal_tombstone(
    meta: RecordMeta,
    deleted_at: TimestampMs,
    identity: &DeviceIdentity,
) -> Result<WireRecord, RecordError> {
    if deleted_at < 0 {
        return Err(RecordError::InvalidTimestamp);
    }
    finish_record(
        meta,
        Vec::new(),
        Some(deleted_at),
        TOMBSTONE_KEY_ID,
        identity,
    )
}

fn finish_record(
    meta: RecordMeta,
    ciphertext: Vec<u8>,
    deleted_at: Option<TimestampMs>,
    key_id: u32,
    identity: &DeviceIdentity,
) -> Result<WireRecord, RecordError> {
    let mut record = WireRecord {
        id: meta.id,
        entity_type: meta.entity_type,
        entity_id: meta.entity_id,
        version: meta.version,
        ciphertext,
        deleted_at,
        updated_at: meta.updated_at,
        device_id: meta.device_id,
        key_id,
        signature: [0; 64],
    };
    let bytes = signed_bytes(&record)?;
    record.signature = identity.sign(&bytes).to_bytes();
    Ok(record)
}

/// Verifies and opens one record in the receiver order: certificate
/// chain → strict signature → tombstone invariant → AAD-bound decryption →
/// payload validation. Version monotonicity is a separate step —
/// see [`crate::check_not_replayed`] — because it needs the applied state.
pub fn verify_and_open(
    record: &WireRecord,
    trust: &TrustContext<'_>,
    keys: &dyn SyncKeyring,
) -> Result<OpenedRecord, RecordError> {
    if record.ciphertext.len() > MAX_RECORD_ENVELOPE_LEN {
        return Err(RecordError::TooLarge);
    }
    let signer = resolve_signer(trust, record)?;
    let bytes = signed_bytes(record)?;
    let verifying = VerifyingKey::from_bytes(&signer)
        .map_err(|_| RecordError::Certificate(CertificateError::MalformedKey))?;
    verifying
        .verify_strict(&bytes, &Signature::from_bytes(&record.signature))
        .map_err(|_| RecordError::BadSignature)?;

    // The tombstone invariant is checked in both directions before any
    // decryption attempt — a missing envelope cannot be decrypted, so
    // checking later would fold the second direction into a decryption
    // failure.
    match record.deleted_at {
        Some(deleted_at) => {
            if !record.ciphertext.is_empty() {
                return Err(RecordError::TombstoneInvariant);
            }
            Ok(OpenedRecord::Tombstone { deleted_at })
        }
        None => {
            if record.ciphertext.is_empty() {
                return Err(RecordError::TombstoneInvariant);
            }
            let envelope_key_id =
                typvia_crypto::envelope_key_id(&record.ciphertext).map_err(map_crypto_error)?;
            if envelope_key_id != record.key_id {
                return Err(RecordError::KeyIdMismatch);
            }
            let key = keys
                .key(envelope_key_id)
                .ok_or(RecordError::UnknownKeyId(envelope_key_id))?;
            let aad = record_aad(record.entity_type, &record.entity_id, record.version)?;
            let document =
                typvia_crypto::open(key, &aad, &record.ciphertext).map_err(map_crypto_error)?;
            check_incoming_payload(&document)?;
            Ok(OpenedRecord::Content { document })
        }
    }
}

/// Resolves the producer's verified signing key: root pin and root
/// statement first (a mismatch is a hard failure), then the certificate
/// chain for non-root devices, then the revocation cut-off for the
/// producer.
fn resolve_signer(trust: &TrustContext<'_>, record: &WireRecord) -> Result<[u8; 32], RecordError> {
    if trust.root.fingerprint() != *trust.known_root {
        return Err(RecordError::Certificate(CertificateError::UnknownRoot));
    }
    trust.root.verify().map_err(RecordError::Certificate)?;

    let signer = if record.device_id == trust.root.subject.device_id {
        // The root device's identity is its (already verified) statement.
        trust.root.subject.ed25519_pub
    } else {
        let chain = trust
            .chains
            .get(&record.device_id)
            .ok_or(RecordError::UnknownDevice)?;
        verify_certificate_chain(trust.known_root, trust.root, chain, trust.revoked_at)?;
        // The verified chain must actually end at the producing device; a
        // chain filed under the wrong id is a directory forgery.
        match chain.last() {
            Some(leaf) if leaf.subject.device_id == record.device_id => leaf.subject.ed25519_pub,
            _ => return Err(RecordError::UnknownDevice),
        }
    };

    // Records produced at or after the device's revocation time are
    // rejected; its earlier history remains valid.
    if let Some(revoked) = trust.revoked_at.get(&record.device_id)
        && record.updated_at >= *revoked
    {
        return Err(RecordError::DeviceRevoked);
    }
    Ok(signer)
}

/// `"typvia.sync.v1" || entity_type || 0x00 || entity_id || 0x00 ||
/// version(8B LE)` — binds the ciphertext to its entity identity and
/// version so transplants and version rewrites fail to decrypt.
/// Crate-visible so the engine can reopen its own sealed records when a
/// push acknowledgment turns them into shadow bases.
pub(crate) fn record_aad(
    entity_type: SyncEntityType,
    entity_id: &str,
    version: u64,
) -> Result<Vec<u8>, RecordError> {
    let mut aad = Vec::with_capacity(64);
    aad.extend_from_slice(AAD_CONTEXT);
    push_field(&mut aad, entity_type.as_str())?;
    push_field(&mut aad, entity_id)?;
    aad.extend_from_slice(&version.to_le_bytes());
    Ok(aad)
}

/// The signed byte string. The ciphertext hash always covers the actual
/// envelope bytes — for a well-formed tombstone that is SHA-256 of the
/// empty string, exactly as specified.
fn signed_bytes(record: &WireRecord) -> Result<Vec<u8>, RecordError> {
    let mut bytes = Vec::with_capacity(160);
    bytes.extend_from_slice(SIGNATURE_CONTEXT);
    push_field(&mut bytes, &record.device_id)?;
    push_field(&mut bytes, record.entity_type.as_str())?;
    push_field(&mut bytes, &record.entity_id)?;
    bytes.extend_from_slice(&record.version.to_le_bytes());
    match record.deleted_at {
        Some(deleted_at) => {
            if deleted_at < 0 {
                // A negative value cannot be told apart from the sentinel.
                return Err(RecordError::InvalidTimestamp);
            }
            bytes.extend_from_slice(&deleted_at.to_le_bytes());
        }
        None => bytes.extend_from_slice(&NOT_DELETED_SENTINEL),
    }
    bytes.extend_from_slice(&record.updated_at.to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(&record.ciphertext));
    Ok(bytes)
}

/// Appends a variable-length field with its 0x00 terminator; interior NUL
/// bytes would make the concatenation ambiguous and are rejected.
fn push_field(bytes: &mut Vec<u8>, value: &str) -> Result<(), RecordError> {
    if value.as_bytes().contains(&0) {
        return Err(RecordError::FieldContainsNul);
    }
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    Ok(())
}

/// Deserialization probe: validates full document syntax and extracts
/// `payload_version` while skipping (not retaining) entity content, so
/// decrypted plaintext is not copied into long-lived structures.
#[derive(Deserialize)]
struct PayloadProbe {
    payload_version: u64,
}

fn probe_payload_version(document: &[u8]) -> Result<u64, RecordError> {
    serde_json::from_slice::<PayloadProbe>(document)
        .map(|probe| probe.payload_version)
        .map_err(|_| RecordError::MalformedPayload)
}

/// Receiver rule: an unknown *newer* version is pending, not an
/// error; version 0 never existed, so anything below 1 is malformed.
fn check_incoming_payload(document: &[u8]) -> Result<(), RecordError> {
    let version = probe_payload_version(document)?;
    if version > SUPPORTED_PAYLOAD_VERSION {
        return Err(RecordError::PayloadVersionAhead(version));
    }
    if version < 1 {
        return Err(RecordError::MalformedPayload);
    }
    Ok(())
}

fn map_crypto_error(error: CryptoError) -> RecordError {
    match error {
        CryptoError::UnsupportedVersion(v) => RecordError::UnsupportedEnvelopeVersion(v),
        // Wrong key, mismatched AAD, tampering, and structural damage are
        // indistinguishable at the AEAD layer.
        _ => RecordError::DecryptFailed,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_core::model::Platform;

    use super::*;
    use crate::cert::CertificateSubject;

    const DOCUMENT: &[u8] = br#"{"payload_version":1,"entity":{"id":"s1","title":"Hello"}}"#;

    /// One account: root device plus a chained laptop; the laptop is the
    /// default record producer so the certificate path is exercised.
    struct Fixture {
        /// The root fingerprint as pinned at pairing time.
        pinned: Fingerprint,
        root: RootStatement,
        chains: HashMap<DeviceId, Vec<DeviceCertificate>>,
        revoked_at: HashMap<DeviceId, TimestampMs>,
        laptop: DeviceIdentity,
        key: SymmetricKey,
        keyring: HashMap<u32, SymmetricKey>,
    }

    impl Fixture {
        fn new() -> Self {
            let root_identity = DeviceIdentity::generate();
            let laptop = DeviceIdentity::generate();
            let root = RootStatement::create(
                &root_identity,
                subject("root-dev", "First Mac", &root_identity),
            )
            .unwrap();
            let cert = DeviceCertificate::issue(
                &root_identity,
                "root-dev",
                subject("laptop-dev", "Work laptop", &laptop),
                1_700_000_100_000,
            )
            .unwrap();
            let mut chains = HashMap::new();
            chains.insert("laptop-dev".to_string(), vec![cert]);

            let key = SymmetricKey::generate();
            let mut keyring = HashMap::new();
            keyring.insert(1, SymmetricKey::from_bytes(*key.expose()));
            Self {
                pinned: root.fingerprint(),
                root,
                chains,
                revoked_at: HashMap::new(),
                laptop,
                key,
                keyring,
            }
        }

        fn trust(&self) -> TrustContext<'_> {
            TrustContext {
                known_root: &self.pinned,
                root: &self.root,
                chains: &self.chains,
                revoked_at: &self.revoked_at,
            }
        }

        fn seal(&self, meta: RecordMeta) -> WireRecord {
            seal_record(meta, DOCUMENT, &self.key, 1, &self.laptop).unwrap()
        }

        /// Re-signs a hand-mutated record with the laptop's key — models a
        /// compromised-server forgery that controls everything except the
        /// device private keys it does not have; here the legitimate key
        /// makes the signature valid so deeper checks are reached.
        fn resign(&self, record: &mut WireRecord) {
            let bytes = signed_bytes(record).unwrap();
            record.signature = self.laptop.sign(&bytes).to_bytes();
        }

        fn open(&self, record: &WireRecord) -> Result<OpenedRecord, RecordError> {
            verify_and_open(record, &self.trust(), &self.keyring)
        }
    }

    fn subject(id: &str, name: &str, identity: &DeviceIdentity) -> CertificateSubject {
        CertificateSubject {
            device_id: id.to_string(),
            ed25519_pub: identity.ed25519_public(),
            x25519_pub: identity.x25519_public(),
            name: name.to_string(),
            platform: Platform::Macos,
            created_at: 1_700_000_000_000,
        }
    }

    fn meta(entity_id: &str, version: u64) -> RecordMeta {
        RecordMeta {
            id: format!("rec-{entity_id}-{version}"),
            entity_type: SyncEntityType::Snippet,
            entity_id: entity_id.to_string(),
            version,
            device_id: "laptop-dev".to_string(),
            updated_at: 1_700_000_500_000,
        }
    }

    #[test]
    fn seal_then_open_restores_the_document_byte_for_byte() {
        let fx = Fixture::new();
        let record = fx.seal(meta("s1", 1));
        match fx.open(&record).unwrap() {
            OpenedRecord::Content { document } => assert_eq!(document.as_slice(), DOCUMENT),
            other => panic!("expected content, got {other:?}"),
        }
    }

    #[test]
    fn a_tombstone_round_trips_with_its_deletion_time() {
        let fx = Fixture::new();
        let record = seal_tombstone(meta("s1", 3), 1_700_000_600_000, &fx.laptop).unwrap();
        assert!(record.is_tombstone());
        assert!(record.ciphertext.is_empty());
        match fx.open(&record).unwrap() {
            OpenedRecord::Tombstone { deleted_at } => assert_eq!(deleted_at, 1_700_000_600_000),
            other => panic!("expected tombstone, got {other:?}"),
        }
    }

    #[test]
    fn a_record_from_the_root_device_verifies_without_a_chain() {
        let root_identity = DeviceIdentity::generate();
        let root =
            RootStatement::create(&root_identity, subject("root-dev", "Root", &root_identity))
                .unwrap();
        let key = SymmetricKey::generate();
        let mut keyring = HashMap::new();
        keyring.insert(1, SymmetricKey::from_bytes(*key.expose()));
        let mut m = meta("s1", 1);
        m.device_id = "root-dev".to_string();
        let record = seal_record(m, DOCUMENT, &key, 1, &root_identity).unwrap();
        let pinned = root.fingerprint();
        let empty_chains = HashMap::new();
        let no_revocations = HashMap::new();
        let trust = TrustContext {
            known_root: &pinned,
            root: &root,
            chains: &empty_chains,
            revoked_at: &no_revocations,
        };
        assert!(verify_and_open(&record, &trust, &keyring).is_ok());
    }

    #[test]
    fn seal_rejects_an_unsupported_payload_version_at_construction() {
        let fx = Fixture::new();
        for document in [
            br#"{"payload_version":2,"entity":{}}"#.as_slice(),
            br#"{"payload_version":0,"entity":{}}"#.as_slice(),
            br#"{"entity":{}}"#.as_slice(),
            br#"{"payload_version":"1","entity":{}}"#.as_slice(),
            b"not json at all".as_slice(),
        ] {
            assert_eq!(
                seal_record(meta("s1", 1), document, &fx.key, 1, &fx.laptop).unwrap_err(),
                RecordError::MalformedPayload
            );
        }
    }

    #[test]
    fn seal_rejects_a_document_that_overflows_the_envelope_limit() {
        let fx = Fixture::new();
        let mut document = br#"{"payload_version":1,"entity":{"body":""#.to_vec();
        document.extend(std::iter::repeat_n(b'a', MAX_RECORD_ENVELOPE_LEN));
        document.extend_from_slice(br#""}}"#);
        assert_eq!(
            seal_record(meta("s1", 1), &document, &fx.key, 1, &fx.laptop).unwrap_err(),
            RecordError::TooLarge
        );
    }

    #[test]
    fn seal_rejects_fields_containing_nul_bytes() {
        let fx = Fixture::new();
        let mut with_nul_entity = meta("s1", 1);
        with_nul_entity.entity_id = "s\x001".to_string();
        assert_eq!(
            seal_record(with_nul_entity, DOCUMENT, &fx.key, 1, &fx.laptop).unwrap_err(),
            RecordError::FieldContainsNul
        );

        let mut with_nul_device = meta("s1", 1);
        with_nul_device.device_id = "d\0ev".to_string();
        assert_eq!(
            seal_record(with_nul_device, DOCUMENT, &fx.key, 1, &fx.laptop).unwrap_err(),
            RecordError::FieldContainsNul
        );
    }

    #[test]
    fn seal_rejects_a_negative_tombstone_time() {
        let fx = Fixture::new();
        assert_eq!(
            seal_tombstone(meta("s1", 1), -1, &fx.laptop).unwrap_err(),
            RecordError::InvalidTimestamp
        );
    }

    #[test]
    fn two_seals_of_the_same_input_use_distinct_nonces() {
        let fx = Fixture::new();
        let a = fx.seal(meta("s1", 1));
        let b = fx.seal(meta("s1", 1));
        // Envelope layout: version(1) || key_id(4) || nonce(24) || ct.
        assert_ne!(a.ciphertext[5..29], b.ciphertext[5..29]);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn a_flipped_signature_byte_is_rejected() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.signature[0] ^= 0x01;
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::BadSignature);
    }

    #[test]
    fn a_mutated_version_without_resigning_breaks_the_signature() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.version = 9;
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::BadSignature);
    }

    #[test]
    fn a_resigned_version_rewrite_fails_aad_decryption() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.version = 9;
        fx.resign(&mut record);
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::DecryptFailed);
    }

    #[test]
    fn a_resigned_entity_id_transplant_fails_aad_decryption() {
        let fx = Fixture::new();
        let donor = fx.seal(meta("s1", 1));
        let mut record = fx.seal(meta("s2", 1));
        record.ciphertext = donor.ciphertext;
        fx.resign(&mut record);
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::DecryptFailed);
    }

    #[test]
    fn swapped_ciphertexts_between_records_fail_even_when_resigned() {
        let fx = Fixture::new();
        let a = fx.seal(meta("s1", 1));
        let b = fx.seal(meta("s2", 1));
        let mut a_with_b = a.clone();
        a_with_b.ciphertext = b.ciphertext.clone();
        let mut b_with_a = b.clone();
        b_with_a.ciphertext = a.ciphertext.clone();
        fx.resign(&mut a_with_b);
        fx.resign(&mut b_with_a);
        assert_eq!(fx.open(&a_with_b).unwrap_err(), RecordError::DecryptFailed);
        assert_eq!(fx.open(&b_with_a).unwrap_err(), RecordError::DecryptFailed);
    }

    #[test]
    fn a_device_without_a_certificate_chain_is_unknown() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.device_id = "phantom-dev".to_string();
        fx.resign(&mut record);
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::UnknownDevice);
    }

    #[test]
    fn a_broken_certificate_chain_is_rejected() {
        let mut fx = Fixture::new();
        // Damage the only certificate: its signature no longer verifies.
        if let Some(chain) = fx.chains.get_mut("laptop-dev") {
            chain[0].signature[0] ^= 0x01;
        }
        let record = fx.seal(meta("s1", 1));
        assert_eq!(
            fx.open(&record).unwrap_err(),
            RecordError::Certificate(CertificateError::BadSignature)
        );
    }

    #[test]
    fn a_chain_filed_under_another_device_id_is_unknown() {
        let mut fx = Fixture::new();
        // The directory claims the laptop chain belongs to "other-dev";
        // its verified leaf does not end at that device.
        let chain = fx.chains.remove("laptop-dev").unwrap();
        fx.chains.insert("other-dev".to_string(), chain);
        let mut record = fx.seal(meta("s1", 1));
        record.device_id = "other-dev".to_string();
        fx.resign(&mut record);
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::UnknownDevice);
    }

    #[test]
    fn a_forged_root_is_a_hard_failure() {
        let fx = Fixture::new();
        let attacker = DeviceIdentity::generate();
        let forged =
            RootStatement::create(&attacker, subject("root-dev", "Evil", &attacker)).unwrap();
        let record = fx.seal(meta("s1", 1));
        let trust = TrustContext {
            known_root: &fx.pinned,
            root: &forged,
            chains: &fx.chains,
            revoked_at: &fx.revoked_at,
        };
        assert_eq!(
            verify_and_open(&record, &trust, &fx.keyring).unwrap_err(),
            RecordError::Certificate(CertificateError::UnknownRoot)
        );
    }

    #[test]
    fn a_record_produced_at_or_after_revocation_is_rejected() {
        let mut fx = Fixture::new();
        let record = fx.seal(meta("s1", 1));
        fx.revoked_at
            .insert("laptop-dev".to_string(), record.updated_at);
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::DeviceRevoked);
    }

    #[test]
    fn a_record_produced_before_revocation_still_verifies() {
        let mut fx = Fixture::new();
        let record = fx.seal(meta("s1", 1));
        fx.revoked_at
            .insert("laptop-dev".to_string(), record.updated_at + 1);
        assert!(fx.open(&record).is_ok());
    }

    #[test]
    fn an_oversized_envelope_is_rejected_before_any_verification() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.ciphertext = vec![0xAB; MAX_RECORD_ENVELOPE_LEN + 1];
        // Not resigned on purpose: the size gate must fire first.
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::TooLarge);
    }

    #[test]
    fn an_unheld_key_generation_is_pending_not_dropped() {
        let fx = Fixture::new();
        let rotated = SymmetricKey::generate();
        let record = seal_record(meta("s1", 1), DOCUMENT, &rotated, 2, &fx.laptop).unwrap();
        // The keyring only holds generation 1: report the id so the
        // caller can park the record and request a key update.
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::UnknownKeyId(2));
    }

    #[test]
    fn a_mismatched_redundant_key_id_field_is_rejected() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.key_id = 7;
        // The signature does not cover key_id; the envelope header is
        // authoritative and the mismatch must be caught explicitly.
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::KeyIdMismatch);
    }

    #[test]
    fn an_unknown_envelope_format_version_is_classified_distinctly() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.ciphertext[0] = 2;
        fx.resign(&mut record);
        assert_eq!(
            fx.open(&record).unwrap_err(),
            RecordError::UnsupportedEnvelopeVersion(2)
        );
    }

    #[test]
    fn a_payload_version_ahead_is_pending_not_malformed() {
        let fx = Fixture::new();
        // Craft what a newer client would produce: the seal path refuses
        // to emit version 2, so build the envelope and signature directly.
        let document = br#"{"payload_version":2,"entity":{"id":"s1"}}"#;
        let m = meta("s1", 1);
        let aad = record_aad(m.entity_type, &m.entity_id, m.version).unwrap();
        let ciphertext = typvia_crypto::seal(&fx.key, 1, &aad, document).unwrap();
        let record = finish_record(m, ciphertext, None, 1, &fx.laptop).unwrap();
        assert_eq!(
            fx.open(&record).unwrap_err(),
            RecordError::PayloadVersionAhead(2)
        );
    }

    #[test]
    fn a_decrypted_document_that_is_not_valid_json_is_malformed() {
        let fx = Fixture::new();
        let m = meta("s1", 1);
        let aad = record_aad(m.entity_type, &m.entity_id, m.version).unwrap();
        let ciphertext = typvia_crypto::seal(&fx.key, 1, &aad, b"garbage bytes").unwrap();
        let record = finish_record(m, ciphertext, None, 1, &fx.laptop).unwrap();
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::MalformedPayload);
    }

    #[test]
    fn a_tombstone_carrying_ciphertext_violates_the_invariant() {
        let fx = Fixture::new();
        let sealed = fx.seal(meta("s1", 1));
        let mut record = sealed.clone();
        record.deleted_at = Some(1_700_000_700_000);
        fx.resign(&mut record);
        assert_eq!(
            fx.open(&record).unwrap_err(),
            RecordError::TombstoneInvariant
        );
    }

    #[test]
    fn a_content_record_without_ciphertext_violates_the_invariant() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.ciphertext.clear();
        fx.resign(&mut record);
        assert_eq!(
            fx.open(&record).unwrap_err(),
            RecordError::TombstoneInvariant
        );
    }

    #[test]
    fn verify_rejects_fields_containing_nul_bytes() {
        let fx = Fixture::new();
        let mut record = fx.seal(meta("s1", 1));
        record.entity_id = "s\x001".to_string();
        assert_eq!(fx.open(&record).unwrap_err(), RecordError::FieldContainsNul);
    }

    #[test]
    fn debug_output_carries_no_ciphertext_or_document_bytes() {
        let fx = Fixture::new();
        let record = fx.seal(meta("s1", 1));
        let rendered = format!("{record:?}");
        assert!(rendered.contains("ciphertext_len"));
        // No raw envelope byte run may be rendered.
        assert!(!rendered.contains("[")); // byte arrays would render as [..]

        let opened = fx.open(&record).unwrap();
        let rendered = format!("{opened:?}");
        assert!(rendered.contains("document_len"));
        assert!(!rendered.contains("Hello"));
    }
}
