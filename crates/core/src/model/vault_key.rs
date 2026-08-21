// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Vault key-header model.
//!
//! These types persist only the KDF parameters and *wrapped* (encrypted) key
//! material. The master password, KEK, and MK plaintext never appear here —
//! the crypto crate owns derivation and (un)wrapping; core just stores the
//! opaque envelope bytes it hands back (core -> crypto is the allowed
//! dependency direction).

use typvia_crypto::KdfParams;

use super::{KeyDomain, TimestampMs};
use crate::model::ValidationError;

/// Minimum envelope length worth persisting: version(1) + key_id(4) +
/// nonce(24) + tag(16). A shorter blob cannot be a valid ciphertext envelope,
/// so we reject it at the door rather than storing garbage.
const MIN_ENVELOPE_LEN: usize = 45;

/// The master-key header: KDF parameters plus the KEK-wrapped master key.
/// Singleton per device vault (the repository keeps a single row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultKeyHeader {
    /// UUID text id.
    pub id: String,
    /// Argon2id parameters and salt used to derive the KEK from the master
    /// password. Persisted so old headers keep verifying after future tuning.
    pub kdf: KdfParams,
    /// MK sealed under the KEK (envelope; AAD `typvia.mk.v1`).
    pub wrapped_mk: Vec<u8>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl VaultKeyHeader {
    /// Rejects a header whose wrapped MK is too short to be a real envelope.
    /// The KDF salt length is guaranteed by [`KdfParams`]'s fixed-size field.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.trim().is_empty() {
            return Err(ValidationError::new("id", "must not be blank"));
        }
        if self.wrapped_mk.len() < MIN_ENVELOPE_LEN {
            return Err(ValidationError::new(
                "wrapped_mk",
                "is not a valid ciphertext envelope",
            ));
        }
        Ok(())
    }
}

/// A per-domain key wrapped under the MK, versioned by `key_id` so a domain
/// can rotate independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainKey {
    /// Which key domain this belongs to (`sync` / `vault`).
    pub domain: KeyDomain,
    /// Key generation; the newest is the highest `key_id` for a domain.
    pub key_id: u32,
    /// Domain key sealed under the MK (envelope; AAD
    /// `typvia.domain.<domain>.v1`).
    pub wrapped_key: Vec<u8>,
    pub created_at: TimestampMs,
}

impl DomainKey {
    /// Rejects a domain key whose wrapped bytes are too short to be an envelope.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.wrapped_key.len() < MIN_ENVELOPE_LEN {
            return Err(ValidationError::new(
                "wrapped_key",
                "is not a valid ciphertext envelope",
            ));
        }
        Ok(())
    }
}
