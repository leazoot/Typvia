// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Cryptographic primitives for Typvia: key derivation, encryption, and
//! secure storage traits.
//!
//! Provides Argon2id derivation, the XChaCha20-Poly1305 envelope format,
//! key wrapping with domain AAD labels, and the platform [`SecureStore`]
//! abstraction. This crate knows nothing about business models — callers
//! pass opaque ids and domain names.
//!
//! Red lines enforced here and guarded by tests: decryption with a wrong
//! key or mismatched AAD must fail; nonces are never reused; key material
//! zeroizes on drop and never appears in `Debug` output or errors.

mod envelope;
mod error;
mod kdf;
mod keys;
mod store;

pub use envelope::{ENVELOPE_VERSION, envelope_key_id, open, seal};
pub use error::{CryptoError, SecureStoreError};
pub use kdf::{KdfParams, SALT_LEN, derive_kek};
pub use keys::{KEY_LEN, SymmetricKey};
pub use store::SecureStore;

/// AAD label binding a ciphertext to the master-key wrap purpose.
pub fn aad_master_key() -> Vec<u8> {
    b"typvia.mk.v1".to_vec()
}

/// AAD label for wrapping the key of one named domain (e.g. "sync",
/// "vault"). Domain names are defined by the caller, not this crate.
pub fn aad_domain_key(domain: &str) -> Vec<u8> {
    format!("typvia.domain.{domain}.v1").into_bytes()
}

/// AAD label binding a record ciphertext to its record id, so ciphertexts
/// cannot be swapped between records.
pub fn aad_record(record_id: &str) -> Vec<u8> {
    let mut aad = b"typvia.snippet.v1".to_vec();
    aad.extend_from_slice(record_id.as_bytes());
    aad
}

/// Encrypts `target` under `wrapping` (KEK→MK, MK→domain keys). The
/// AAD label states what is being wrapped, so a wrapped domain key cannot
/// be presented as a wrapped master key.
pub fn wrap_key(
    wrapping: &SymmetricKey,
    key_id: u32,
    aad: &[u8],
    target: &SymmetricKey,
) -> Result<Vec<u8>, CryptoError> {
    seal(wrapping, key_id, aad, target.expose())
}

/// Reverses [`wrap_key`]. Fails like [`open`] on any mismatch.
pub fn unwrap_key(
    wrapping: &SymmetricKey,
    aad: &[u8],
    wrapped: &[u8],
) -> Result<SymmetricKey, CryptoError> {
    let plain = open(wrapping, aad, wrapped)?;
    let mut bytes: [u8; KEY_LEN] = plain
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidEnvelope)?;
    let key = SymmetricKey::from_bytes(bytes);
    // `plain` zeroizes on drop; wipe the intermediate stack copy too.
    bytes.fill(0);
    Ok(key)
}
