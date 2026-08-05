//! Crypto red-line tests (docs/06_SECURITY_MODEL.md §11, regression-
//! mandatory): wrong keys must fail, nonces must not repeat, AAD binds
//! ciphertexts to their purpose, the KDF is deterministic per salt.
//! All passwords and payloads are obvious fakes.

#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

use typvia_crypto::{
    ENVELOPE_VERSION, KdfParams, SymmetricKey, aad_domain_key, aad_master_key, aad_record,
    derive_kek, envelope_key_id, open, seal, unwrap_key, wrap_key,
};

const FAKE_SECRET: &[u8] = b"FAKE_VAULT_BODY_XKCD9931";

/// Fast Argon2id parameters for tests that only need determinism, not the
/// production cost (the v1 baseline is exercised once separately).
fn fast_params(salt_byte: u8) -> KdfParams {
    let mut params = KdfParams::v1_with_salt([salt_byte; 16]);
    params.m_cost_kib = 8;
    params.t_cost = 1;
    params
}

#[test]
fn seal_open_roundtrip_with_record_aad() {
    let key = SymmetricKey::generate();
    let aad = aad_record("snippet-42");
    let envelope = seal(&key, 1, &aad, FAKE_SECRET).unwrap();
    let plain = open(&key, &aad, &envelope).unwrap();
    assert_eq!(plain.as_slice(), FAKE_SECRET);
    assert_eq!(envelope[0], ENVELOPE_VERSION);
    assert_eq!(envelope_key_id(&envelope).unwrap(), 1);
}

#[test]
fn wrong_key_fails_decryption() {
    let aad = aad_record("snippet-42");
    let envelope = seal(&SymmetricKey::generate(), 1, &aad, FAKE_SECRET).unwrap();
    let err = open(&SymmetricKey::generate(), &aad, &envelope).unwrap_err();
    assert_eq!(err, typvia_crypto::CryptoError::DecryptionFailed);
}

#[test]
fn mismatched_aad_fails_decryption() {
    let key = SymmetricKey::generate();
    let envelope = seal(&key, 1, &aad_record("snippet-a"), FAKE_SECRET).unwrap();
    // Swapping the ciphertext onto another record must fail.
    assert!(open(&key, &aad_record("snippet-b"), &envelope).is_err());
    // So must presenting it under a different purpose label.
    assert!(open(&key, &aad_master_key(), &envelope).is_err());
}

#[test]
fn tampered_ciphertext_fails_decryption() {
    let key = SymmetricKey::generate();
    let aad = aad_record("snippet-42");
    let mut envelope = seal(&key, 1, &aad, FAKE_SECRET).unwrap();
    let last = envelope.len() - 1;
    envelope[last] ^= 0x01;
    assert!(open(&key, &aad, &envelope).is_err());
}

#[test]
fn unknown_version_and_truncation_are_rejected() {
    let key = SymmetricKey::generate();
    let aad = aad_record("snippet-42");
    let envelope = seal(&key, 1, &aad, FAKE_SECRET).unwrap();

    let mut wrong_version = envelope.clone();
    wrong_version[0] = 0x7F;
    assert_eq!(
        open(&key, &aad, &wrong_version).unwrap_err(),
        typvia_crypto::CryptoError::UnsupportedVersion(0x7F)
    );

    assert_eq!(
        open(&key, &aad, &envelope[..20]).unwrap_err(),
        typvia_crypto::CryptoError::InvalidEnvelope
    );
}

#[test]
fn nonces_are_never_repeated() {
    let key = SymmetricKey::generate();
    let aad = aad_record("snippet-42");
    let mut nonces = HashSet::new();
    for _ in 0..256 {
        let envelope = seal(&key, 1, &aad, FAKE_SECRET).unwrap();
        // nonce sits after version(1) + key_id(4).
        let nonce: [u8; 24] = envelope[5..29].try_into().unwrap();
        assert!(nonces.insert(nonce), "nonce reuse detected");
    }
}

#[test]
fn key_wrap_roundtrip_and_wrong_wrapper_fails() {
    let kek = SymmetricKey::generate();
    let mk = SymmetricKey::generate();
    let wrapped = wrap_key(&kek, 3, &aad_master_key(), &mk).unwrap();
    assert_eq!(envelope_key_id(&wrapped).unwrap(), 3);
    let unwrapped = unwrap_key(&kek, &aad_master_key(), &wrapped).unwrap();
    assert_eq!(unwrapped, mk);

    assert!(unwrap_key(&SymmetricKey::generate(), &aad_master_key(), &wrapped).is_err());
    // A wrapped master key must not open under a domain-key label.
    assert!(unwrap_key(&kek, &aad_domain_key("vault"), &wrapped).is_err());
}

#[test]
fn kdf_is_deterministic_per_password_and_salt() {
    let password = b"correct-horse-battery-staple-FAKE";
    let a = derive_kek(password, &fast_params(0x11)).unwrap();
    let b = derive_kek(password, &fast_params(0x11)).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, derive_kek(password, &fast_params(0x22)).unwrap());
    assert_ne!(
        a,
        derive_kek(b"other-password-FAKE", &fast_params(0x11)).unwrap()
    );
}

#[test]
fn kdf_v1_baseline_parameters_derive_a_key() {
    let params = KdfParams::v1();
    assert_eq!(
        (
            params.version,
            params.m_cost_kib,
            params.t_cost,
            params.p_cost
        ),
        (1, 64 * 1024, 3, 1)
    );
    // Full-cost derivation runs once to prove the baseline is usable.
    let kek = derive_kek(b"master-password-FAKE", &params).unwrap();
    let mk = SymmetricKey::generate();
    let wrapped = wrap_key(&kek, 1, &aad_master_key(), &mk).unwrap();
    assert_eq!(unwrap_key(&kek, &aad_master_key(), &wrapped).unwrap(), mk);
}

#[test]
fn fresh_salts_differ_between_headers() {
    assert_ne!(KdfParams::v1().salt, KdfParams::v1().salt);
}

#[test]
fn domain_labels_are_distinct_and_stable() {
    assert_eq!(aad_master_key(), b"typvia.mk.v1".to_vec());
    assert_eq!(aad_domain_key("sync"), b"typvia.domain.sync.v1".to_vec());
    assert_ne!(aad_domain_key("sync"), aad_domain_key("vault"));
}
