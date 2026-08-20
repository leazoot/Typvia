//! Sync wire-format red lines (regression-mandatory).
//!
//! Proves at the byte level that a sealed change-set record carries no
//! plaintext trace of the document it encrypts, that the round trip is
//! byte-exact, and that the K_vault / K_sync domain split holds on the
//! wire: a sensitive body travels as an opaque inner K_vault envelope the
//! sync layer forwards verbatim, so opening a record never needs K_vault
//! and the sensitive plaintext never appears in the wire bytes.

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use typvia_core::model::{DeviceId, Platform, SyncEntityType, TimestampMs};
use typvia_crypto::SymmetricKey;
use typvia_sync::{
    CertificateSubject, DeviceCertificate, DeviceIdentity, OpenedRecord, RecordMeta, RootStatement,
    TrustContext, WireRecord, seal_record, verify_and_open,
};

/// Deliberately fake marker strings: no real secrets in tests.
const SYNC_MARKER: &str = "AKIA_FAKE_SYNC_MARKER";
const VAULT_MARKER: &str = "AKIA_FAKE_VAULT_BODY";

struct Account {
    pinned: typvia_sync::Fingerprint,
    root: RootStatement,
    chains: HashMap<DeviceId, Vec<DeviceCertificate>>,
    revoked_at: HashMap<DeviceId, TimestampMs>,
    laptop: DeviceIdentity,
    k_sync: SymmetricKey,
    keyring: HashMap<u32, SymmetricKey>,
}

impl Account {
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

        let k_sync = SymmetricKey::generate();
        let mut keyring = HashMap::new();
        keyring.insert(1, SymmetricKey::from_bytes(*k_sync.expose()));
        Self {
            pinned: root.fingerprint(),
            root,
            chains,
            revoked_at: HashMap::new(),
            laptop,
            k_sync,
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

fn meta(entity_id: &str) -> RecordMeta {
    RecordMeta {
        id: format!("rec-{entity_id}"),
        entity_type: SyncEntityType::Snippet,
        entity_id: entity_id.to_string(),
        version: 1,
        device_id: "laptop-dev".to_string(),
        updated_at: 1_700_000_500_000,
    }
}

/// Every byte a record puts on the wire, concatenated for scanning.
fn wire_bytes(record: &WireRecord) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(record.id.as_bytes());
    bytes.extend_from_slice(record.entity_type.as_str().as_bytes());
    bytes.extend_from_slice(record.entity_id.as_bytes());
    bytes.extend_from_slice(&record.version.to_le_bytes());
    bytes.extend_from_slice(&record.ciphertext);
    if let Some(deleted_at) = record.deleted_at {
        bytes.extend_from_slice(&deleted_at.to_le_bytes());
    }
    bytes.extend_from_slice(&record.updated_at.to_le_bytes());
    bytes.extend_from_slice(record.device_id.as_bytes());
    bytes.extend_from_slice(&record.key_id.to_le_bytes());
    bytes.extend_from_slice(&record.signature);
    bytes
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn sealed_record_bytes_never_contain_the_document_plaintext() {
    let account = Account::new();
    let document =
        format!(r#"{{"payload_version":1,"entity":{{"id":"s1","body":"{SYNC_MARKER}"}}}}"#)
            .into_bytes();

    // Scan validity canary: the marker is findable in the plaintext.
    assert!(contains(&document, SYNC_MARKER.as_bytes()));

    let record = seal_record(meta("s1"), &document, &account.k_sync, 1, &account.laptop).unwrap();
    let wire = wire_bytes(&record);
    assert!(
        !contains(&wire, SYNC_MARKER.as_bytes()),
        "plaintext marker leaked into the wire record"
    );

    // And the round trip still restores the document byte-for-byte.
    match verify_and_open(&record, &account.trust(), &account.keyring).unwrap() {
        OpenedRecord::Content { document: opened } => {
            assert_eq!(opened.as_slice(), document.as_slice());
        }
        other => panic!("expected content, got {other:?}"),
    }
}

#[test]
fn a_vault_inner_envelope_passes_through_verbatim_without_k_vault() {
    let account = Account::new();

    // A sensitive body as it sits in the database: sealed under K_vault
    // (the double envelope). The sync layer must forward these bytes
    // untouched — it never holds K_vault.
    let k_vault = SymmetricKey::generate();
    let inner_aad = typvia_crypto::aad_record("s-secret");
    let inner_envelope =
        typvia_crypto::seal(&k_vault, 1, &inner_aad, VAULT_MARKER.as_bytes()).unwrap();

    let document = serde_json::to_vec(&serde_json::json!({
        "payload_version": 1,
        "entity": {
            "id": "s-secret",
            "security_level": "sensitive",
            "content_ciphertext": inner_envelope,
        }
    }))
    .unwrap();

    let record = seal_record(
        meta("s-secret"),
        &document,
        &account.k_sync,
        1,
        &account.laptop,
    )
    .unwrap();

    // Wire bytes contain neither the vault plaintext nor (post outer
    // encryption) the raw document.
    let wire = wire_bytes(&record);
    assert!(!contains(&wire, VAULT_MARKER.as_bytes()));
    assert!(!contains(&wire, b"content_ciphertext"));

    // Opening needs only K_sync (the keyring holds no vault key) …
    let opened = match verify_and_open(&record, &account.trust(), &account.keyring).unwrap() {
        OpenedRecord::Content { document } => document,
        other => panic!("expected content, got {other:?}"),
    };
    // … and restores the document byte-for-byte.
    assert_eq!(opened.as_slice(), document.as_slice());

    // The embedded inner envelope survived verbatim: extract it and prove
    // K_vault still opens it to the original body.
    let value: serde_json::Value = serde_json::from_slice(&opened).unwrap();
    let roundtripped: Vec<u8> = value["entity"]["content_ciphertext"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
        .collect();
    assert_eq!(roundtripped, inner_envelope);
    let body = typvia_crypto::open(&k_vault, &inner_aad, &roundtripped).unwrap();
    assert_eq!(body.as_slice(), VAULT_MARKER.as_bytes());
}
