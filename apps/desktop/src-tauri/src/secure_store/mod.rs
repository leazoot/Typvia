// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Platform secure-storage backends.
//!
//! Holds the biometric-gated copy of the master key (MK) — never the master
//! password or KEK. The trait is defined in the crypto crate; the host owns
//! the platform I/O (Keychain / Keystore / DPAPI). macOS is implemented here;
//! other desktop platforms fall back to [`UnavailableStore`] until their
//! batch lands.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::LIVE_TEST_GATED_ENTRY;

use typvia_crypto::{SecureStore, SecureStoreError};
use zeroize::Zeroizing;

/// Stand-in when no platform backend exists (non-macOS desktop for now, or a
/// backend that failed to initialize). Storing/retrieving report `Unavailable`
/// so biometrics degrade to the master-password path; removal is a no-op so
/// disabling biometrics stays idempotent.
pub struct UnavailableStore;

impl SecureStore for UnavailableStore {
    fn store(&self, _entry: &str, _secret: &[u8]) -> Result<(), SecureStoreError> {
        Err(SecureStoreError::Unavailable)
    }

    fn retrieve(&self, _entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
        Err(SecureStoreError::Unavailable)
    }

    fn remove(&self, _entry: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
}

/// The secure store for the current platform, or an error when none exists.
pub fn platform_secure_store() -> Result<Box<dyn SecureStore + Send + Sync>, SecureStoreError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacosKeychain::new()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(SecureStoreError::Unavailable)
    }
}

/// The platform store, or an [`UnavailableStore`] so startup never fails on a
/// platform without a backend (mirrors the injector's null fallback).
pub fn platform_secure_store_or_unavailable() -> Box<dyn SecureStore + Send + Sync> {
    platform_secure_store().unwrap_or_else(|_| Box::new(UnavailableStore))
}
