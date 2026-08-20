//! The versioned ciphertext envelope:
//!
//! ```text
//! version(1B, =0x01) || key_id(4B LE) || nonce(24B) || AEAD(ct || tag)
//! ```
//!
//! Every seal draws a fresh 24-byte nonce from the OS CSPRNG — nonces are
//! never reused or derived. The AAD binds a ciphertext to
//! its purpose and record id, so envelopes cannot be swapped.

use chacha20poly1305::aead::{Aead, OsRng, Payload};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::keys::SymmetricKey;

/// Current envelope format version.
pub const ENVELOPE_VERSION: u8 = 1;

const KEY_ID_LEN: usize = 4;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const HEADER_LEN: usize = 1 + KEY_ID_LEN + NONCE_LEN;
const MIN_ENVELOPE_LEN: usize = HEADER_LEN + TAG_LEN;

/// Encrypts `plaintext` under `key`, stamping the envelope with `key_id`
/// (which key generation can open it, for rotation) and binding it to
/// `aad`.
pub fn seal(
    key: &SymmetricKey,
    key_id: u32,
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.expose().into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::DecryptionFailed)?;

    let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    envelope.push(ENVELOPE_VERSION);
    envelope.extend_from_slice(&key_id.to_le_bytes());
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Decrypts an envelope. Fails on structural damage, unknown version,
/// wrong key, mismatched AAD, or tampering — the error does not say
/// which. The plaintext comes back in a self-zeroizing buffer.
pub fn open(
    key: &SymmetricKey,
    aad: &[u8],
    envelope: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (_, rest) = parse_header(envelope)?;
    let nonce = XNonce::from_slice(&rest[..NONCE_LEN]);
    let cipher = XChaCha20Poly1305::new(key.expose().into());
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &rest[NONCE_LEN..],
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Reads which key generation sealed this envelope, without decrypting —
/// used during rotation to pick the right key.
pub fn envelope_key_id(envelope: &[u8]) -> Result<u32, CryptoError> {
    let (key_id_bytes, _) = parse_header(envelope)?;
    Ok(u32::from_le_bytes(key_id_bytes))
}

fn parse_header(envelope: &[u8]) -> Result<([u8; KEY_ID_LEN], &[u8]), CryptoError> {
    if envelope.len() < MIN_ENVELOPE_LEN {
        return Err(CryptoError::InvalidEnvelope);
    }
    if envelope[0] != ENVELOPE_VERSION {
        return Err(CryptoError::UnsupportedVersion(envelope[0]));
    }
    let mut key_id = [0u8; KEY_ID_LEN];
    key_id.copy_from_slice(&envelope[1..1 + KEY_ID_LEN]);
    Ok((key_id, &envelope[1 + KEY_ID_LEN..]))
}
