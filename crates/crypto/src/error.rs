//! Error types. Messages never carry key material, plaintext, or
//! ciphertext — only structural facts (security rules: log red line).

use std::fmt;

/// Failures of the primitives in this crate.
#[derive(Debug, PartialEq, Eq)]
pub enum CryptoError {
    /// Authentication failed: wrong key, mismatched AAD, or tampered
    /// ciphertext. Deliberately not distinguished further (§12: unlock
    /// errors must not leak which part was wrong).
    DecryptionFailed,
    /// The envelope is structurally invalid (too short, malformed).
    InvalidEnvelope,
    /// The envelope declares a version this build does not understand.
    UnsupportedVersion(u8),
    /// Key derivation failed (invalid parameters).
    KdfFailed,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::DecryptionFailed => write!(f, "decryption failed"),
            CryptoError::InvalidEnvelope => write!(f, "invalid ciphertext envelope"),
            CryptoError::UnsupportedVersion(v) => {
                write!(f, "unsupported envelope version {v}")
            }
            CryptoError::KdfFailed => write!(f, "key derivation failed"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Failures of a platform [`crate::SecureStore`] backend. Variants carry
/// no secret data; `Backend` holds a platform diagnostic only.
#[derive(Debug)]
pub enum SecureStoreError {
    /// The platform store is not available (locked, missing entitlement).
    Unavailable,
    /// The user or platform denied access (e.g. biometric gate refused).
    AccessDenied,
    /// Any other backend failure, with a platform diagnostic message.
    Backend(String),
}

impl fmt::Display for SecureStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecureStoreError::Unavailable => write!(f, "secure store unavailable"),
            SecureStoreError::AccessDenied => write!(f, "secure store access denied"),
            SecureStoreError::Backend(msg) => write!(f, "secure store backend error: {msg}"),
        }
    }
}

impl std::error::Error for SecureStoreError {}
