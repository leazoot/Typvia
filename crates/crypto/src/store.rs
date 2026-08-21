// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Platform secure-storage abstraction.
//!
//! Backends: macOS/iOS Keychain, Android Keystore, Windows Credential
//! Manager/DPAPI — implemented in the host layers. This crate only defines
//! the contract; nothing here performs platform I/O.

use zeroize::Zeroizing;

use crate::error::SecureStoreError;

/// Key-value store backed by the platform's secure storage. Entries hold
/// key material (the biometric-gated MK copy, device keys) — never the
/// master password or KEK. Implementations must apply the platform's
/// access-control gate (biometric / user presence) on `retrieve`.
pub trait SecureStore {
    /// Persists `secret` under `entry`, replacing any previous value.
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError>;

    /// Reads an entry back; `Ok(None)` when it does not exist. The buffer
    /// zeroizes itself on drop.
    fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError>;

    /// Deletes an entry; deleting a missing entry is not an error
    /// (disabling biometrics twice must be idempotent).
    fn remove(&self, entry: &str) -> Result<(), SecureStoreError>;
}
