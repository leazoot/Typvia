// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Symmetric key material: fixed-size, zeroized on drop, redacted Debug.

use std::fmt;

use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Length of every symmetric key in the hierarchy (KEK, MK, domain keys).
pub const KEY_LEN: usize = 32;

/// A 32-byte symmetric key. Heap-boxed so moves never leave stale copies
/// on the stack; wiped on drop.
#[derive(Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct SymmetricKey(Box<[u8; KEY_LEN]>);

impl SymmetricKey {
    /// Fresh random key from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = Box::new([0u8; KEY_LEN]);
        OsRng.fill_bytes(bytes.as_mut());
        Self(bytes)
    }

    /// Wraps existing key bytes (e.g. read back from a [`crate::SecureStore`]).
    /// The caller should zeroize its own copy afterwards.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Box::new(bytes))
    }

    /// Raw bytes for feeding a cipher or a secure store. Deliberately not
    /// part of Display/Debug; keep the borrow short-lived.
    pub fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for SymmetricKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymmetricKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_are_distinct() {
        assert_ne!(SymmetricKey::generate(), SymmetricKey::generate());
    }

    #[test]
    fn zeroize_clears_key_bytes() {
        let mut key = SymmetricKey::from_bytes([0xAB; KEY_LEN]);
        key.zeroize();
        assert_eq!(key.expose(), &[0u8; KEY_LEN]);
    }

    #[test]
    fn debug_output_is_redacted() {
        let key = SymmetricKey::from_bytes([0xCD; KEY_LEN]);
        let rendered = format!("{key:?}");
        assert_eq!(rendered, "SymmetricKey(<redacted>)");
        assert!(!rendered.contains("cd") && !rendered.contains("CD"));
    }
}
