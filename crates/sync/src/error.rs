// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Error types for device identity and certificates. Messages carry only
//! structural facts — never key material or user content (log red line).

use std::fmt;

use typvia_core::repo::RepoError;
use typvia_crypto::SecureStoreError;

/// Failures around the locally stored device identity.
#[derive(Debug)]
pub enum IdentityError {
    /// The platform secure store failed (system error).
    Store(SecureStoreError),
    /// Exactly one of the two identity entries exists — regenerating would
    /// silently mint a new device identity, so this is surfaced instead.
    Incomplete,
    /// A stored entry has the wrong length or is not valid key material.
    CorruptKeyMaterial,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::Incomplete => f.write_str("device identity is incomplete in the secure store"),
            Self::CorruptKeyMaterial => f.write_str("stored device key material is invalid"),
        }
    }
}

impl std::error::Error for IdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Incomplete | Self::CorruptKeyMaterial => None,
        }
    }
}

impl From<SecureStoreError> for IdentityError {
    fn from(e: SecureStoreError) -> Self {
        Self::Store(e)
    }
}

/// Rejection reasons for root statements and certificate chains. Every
/// malicious-input class maps to a distinct variant so tests can assert
/// the classification.
#[derive(Debug, PartialEq, Eq)]
pub enum CertificateError {
    /// The root statement does not match the locally pinned trust root;
    /// an unknown root is a hard failure.
    UnknownRoot,
    /// An Ed25519 signature failed strict verification.
    BadSignature,
    /// A certificate's issuer is not the preceding link of the chain.
    BrokenChain,
    /// The chain is empty; only the root device itself needs no chain.
    EmptyChain,
    /// The certificate was issued at or after its issuer's revocation time.
    IssuerRevoked,
    /// A variable-length field contains a 0x00 byte, which the separator
    /// encoding cannot represent unambiguously.
    FieldContainsNul,
    /// A stored public key is not a valid Ed25519 point.
    MalformedKey,
    /// The subject's keys do not match the identity that is signing for
    /// itself (root statements are self-signed).
    SubjectKeyMismatch,
}

impl fmt::Display for CertificateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::UnknownRoot => "root statement does not match the known trust root",
            Self::BadSignature => "signature verification failed",
            Self::BrokenChain => "certificate chain is not contiguous",
            Self::EmptyChain => "certificate chain is empty",
            Self::IssuerRevoked => "certificate was issued after its issuer was revoked",
            Self::FieldContainsNul => "field contains a NUL byte",
            Self::MalformedKey => "public key is not a valid Ed25519 key",
            Self::SubjectKeyMismatch => "subject keys do not match the signing identity",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for CertificateError {}

/// Rejection reasons for sealing and opening sync change-set records.
/// Each malicious-payload class and each verification step maps to a
/// distinct variant so tests can assert
/// the classification; no variant ever carries plaintext content.
#[derive(Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The envelope exceeds the per-record limit of 256KB.
    TooLarge,
    /// A variable-length field contains a 0x00 byte, which the separator
    /// encoding cannot represent unambiguously.
    FieldContainsNul,
    /// A timestamp cannot be encoded (negative `deleted_at` would collide
    /// with the non-tombstone sentinel).
    InvalidTimestamp,
    /// The payload document is not valid JSON, or `payload_version` is
    /// missing, not an unsigned integer, or below the first version.
    MalformedPayload,
    /// The payload declares a version newer than this client supports.
    /// Pending semantics: the record must be kept, not applied and
    /// not dropped, until the application is upgraded.
    PayloadVersionAhead(u64),
    /// The producing device has no certificate chain in the directory —
    /// a forged device under the malicious-server model.
    UnknownDevice,
    /// The trust root or certificate chain failed verification.
    Certificate(CertificateError),
    /// The producing device was revoked before this record was produced;
    /// records from its revocation time onward are rejected.
    DeviceRevoked,
    /// The record signature failed strict Ed25519 verification.
    BadSignature,
    /// The redundant `key_id` field disagrees with the envelope header
    /// (the envelope is authoritative, so a mismatch is tampering).
    KeyIdMismatch,
    /// The envelope was sealed under a K_sync generation this client does
    /// not hold. Pending semantics: keep the record and request a
    /// key-update message; never drop it.
    UnknownKeyId(u32),
    /// The envelope carries a format version this client cannot parse
    /// (envelope evolution is independent of the protocol version).
    UnsupportedEnvelopeVersion(u8),
    /// The AEAD cipher failed while sealing (system error).
    EncryptFailed,
    /// Envelope decryption failed: wrong key, mismatched AAD (ciphertext
    /// transplant or version rewrite), or structural damage — the
    /// AEAD cannot distinguish these.
    DecryptFailed,
    /// Tombstone invariant violated in either direction: a tombstone
    /// carrying ciphertext, or a content record without ciphertext.
    TombstoneInvariant,
    /// The record version is not beyond the already-applied head for its
    /// entity — a replayed (or stale) record.
    Replay {
        /// The head version already applied for the entity.
        applied_head: u64,
    },
    /// A pull response's `server_seq` did not strictly increase;
    /// the caller must abort the batch.
    NonMonotonicServerSeq,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("record envelope exceeds the size limit"),
            Self::FieldContainsNul => f.write_str("field contains a NUL byte"),
            Self::InvalidTimestamp => f.write_str("timestamp cannot be encoded"),
            Self::MalformedPayload => f.write_str("payload document is malformed"),
            Self::PayloadVersionAhead(v) => {
                write!(f, "payload version {v} is newer than supported")
            }
            Self::UnknownDevice => f.write_str("producing device has no certificate chain"),
            Self::Certificate(e) => write!(f, "{e}"),
            Self::DeviceRevoked => f.write_str("producing device was revoked"),
            Self::BadSignature => f.write_str("record signature verification failed"),
            Self::KeyIdMismatch => f.write_str("record key id disagrees with the envelope"),
            Self::UnknownKeyId(id) => write!(f, "unknown sync key generation {id}"),
            Self::UnsupportedEnvelopeVersion(v) => {
                write!(f, "unsupported envelope version {v}")
            }
            Self::EncryptFailed => f.write_str("sealing the record envelope failed"),
            Self::DecryptFailed => f.write_str("record envelope decryption failed"),
            Self::TombstoneInvariant => f.write_str("tombstone ciphertext invariant violated"),
            Self::Replay { applied_head } => {
                write!(
                    f,
                    "record version is not beyond applied head {applied_head}"
                )
            }
            Self::NonMonotonicServerSeq => f.write_str("server sequence did not strictly increase"),
        }
    }
}

impl std::error::Error for RecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Certificate(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CertificateError> for RecordError {
    fn from(e: CertificateError) -> Self {
        Self::Certificate(e)
    }
}

/// Failures of the local-device registration use case.
#[derive(Debug)]
pub enum EnsureLocalDeviceError {
    /// Identity generation, load, or persistence failed.
    Identity(IdentityError),
    /// The device table rejected the read or write.
    Repo(RepoError),
    /// A device row exists for this id but its public key differs from the
    /// stored identity — two identities must never be silently merged.
    IdentityMismatch,
}

impl fmt::Display for EnsureLocalDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(e) => write!(f, "{e}"),
            Self::Repo(e) => write!(f, "{e}"),
            Self::IdentityMismatch => {
                f.write_str("device row public key does not match the stored identity")
            }
        }
    }
}

impl std::error::Error for EnsureLocalDeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(e) => Some(e),
            Self::Repo(e) => Some(e),
            Self::IdentityMismatch => None,
        }
    }
}

impl From<IdentityError> for EnsureLocalDeviceError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}

impl From<RepoError> for EnsureLocalDeviceError {
    fn from(e: RepoError) -> Self {
        Self::Repo(e)
    }
}
