// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! API-key custody — a red line: keys never touch the database, logs or
//! error messages; storage is the platform secure store only.
//!
//! Entries are non-gated: the biometric gate is reserved for the vault
//! master-key copy, and provider keys follow the sync-key tier so
//! background AI calls never raise a biometric prompt.

use std::fmt;

use typvia_crypto::{SecureStore, SecureStoreError};
use zeroize::Zeroizing;

/// Secure-store entry prefix; one entry per provider
/// (`ai.api_key.<provider_id>`), same shape as the sync-key entries.
pub const API_KEY_ENTRY_PREFIX: &str = "ai.api_key.";

/// Secure-store entry holding the key for `provider_id`.
pub fn entry_name(provider_id: &str) -> String {
    format!("{API_KEY_ENTRY_PREFIX}{provider_id}")
}

/// An API key held in memory. Wipes on drop; renders redacted; the value
/// is reachable only through [`ApiKey::expose`], whose sole legitimate
/// destination is the `Authorization` header of a provider request.
#[derive(Clone)]
pub struct ApiKey(Zeroizing<String>);

impl ApiKey {
    /// Accepts printable-ASCII keys only — anything else cannot be a
    /// well-formed header value and would open header injection.
    pub fn new(key: &str) -> Result<Self, CredentialError> {
        validate_key_material(key)?;
        Ok(Self(Zeroizing::new(key.to_string())))
    }

    /// The raw value, for building the `Authorization` header.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(redacted)")
    }
}

/// Failures around key custody. Never carries key bytes.
#[derive(Debug)]
pub enum CredentialError {
    /// User error: the key value violates the named rule.
    Rejected(&'static str),
    /// System error: the platform store refused the operation.
    Store(SecureStoreError),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rule) => write!(f, "invalid API key: {rule}"),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CredentialError {}

impl From<SecureStoreError> for CredentialError {
    fn from(error: SecureStoreError) -> Self {
        Self::Store(error)
    }
}

fn validate_key_material(key: &str) -> Result<(), CredentialError> {
    if key.is_empty() {
        return Err(CredentialError::Rejected("must not be empty"));
    }
    if key.len() > 512 {
        return Err(CredentialError::Rejected("must be at most 512 bytes"));
    }
    if !key.chars().all(|c| c.is_ascii_graphic()) {
        return Err(CredentialError::Rejected(
            "must be printable ASCII without spaces",
        ));
    }
    Ok(())
}

/// Persists the key for `provider_id`, replacing any previous value.
pub fn store_api_key(
    store: &dyn SecureStore,
    provider_id: &str,
    key: &str,
) -> Result<(), CredentialError> {
    validate_key_material(key)?;
    store.store(&entry_name(provider_id), key.as_bytes())?;
    Ok(())
}

/// Loads the key for `provider_id`; `Ok(None)` when none is stored.
/// Stored material that is no longer a valid key is reported, not used.
pub fn load_api_key(
    store: &dyn SecureStore,
    provider_id: &str,
) -> Result<Option<ApiKey>, CredentialError> {
    let Some(bytes) = store.retrieve(&entry_name(provider_id))? else {
        return Ok(None);
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| CredentialError::Rejected("stored key material is invalid"))?;
    ApiKey::new(text).map(Some)
}

/// Removes the key for `provider_id`; removing a missing key is not an
/// error (clearing twice must be idempotent, matching `SecureStore`).
pub fn remove_api_key(store: &dyn SecureStore, provider_id: &str) -> Result<(), CredentialError> {
    store.remove(&entry_name(provider_id))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// In-memory secure-store double for tests (no platform I/O).
    pub(crate) struct MemoryStore {
        entries: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl MemoryStore {
        pub(crate) fn new() -> Self {
            Self {
                entries: RefCell::new(HashMap::new()),
            }
        }
    }

    impl SecureStore for MemoryStore {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            self.entries
                .borrow_mut()
                .insert(entry.to_string(), secret.to_vec());
            Ok(())
        }

        fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
            Ok(self
                .entries
                .borrow()
                .get(entry)
                .cloned()
                .map(Zeroizing::new))
        }

        fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
            self.entries.borrow_mut().remove(entry);
            Ok(())
        }
    }

    #[test]
    fn stores_loads_and_removes_a_key_under_the_provider_entry() {
        let store = MemoryStore::new();
        store_api_key(&store, "p1", "sk-FAKE-abc123").unwrap();
        assert!(
            store
                .entries
                .borrow()
                .contains_key(&"ai.api_key.p1".to_string())
        );

        let key = load_api_key(&store, "p1").unwrap().unwrap();
        assert_eq!(key.expose(), "sk-FAKE-abc123");

        remove_api_key(&store, "p1").unwrap();
        assert!(load_api_key(&store, "p1").unwrap().is_none());
        // Idempotent: clearing again is fine.
        remove_api_key(&store, "p1").unwrap();
    }

    #[test]
    fn rejects_key_material_that_cannot_be_a_header_value() {
        let store = MemoryStore::new();
        for (key, rule_part) in [
            ("", "empty"),
            ("with space", "printable"),
            ("evil\r\nHeader: x", "printable"),
            ("控制", "printable"),
            (&"x".repeat(513), "512"),
        ] {
            let error = store_api_key(&store, "p1", key).unwrap_err();
            match error {
                CredentialError::Rejected(rule) => assert!(rule.contains(rule_part), "{rule}"),
                other => panic!("expected rejection, got {other:?}"),
            }
        }
        assert!(store.entries.borrow().is_empty());
    }

    #[test]
    fn corrupt_stored_material_is_reported_not_used() {
        let store = MemoryStore::new();
        store.store(&entry_name("p1"), &[0xFF, 0xFE]).unwrap();
        assert!(matches!(
            load_api_key(&store, "p1"),
            Err(CredentialError::Rejected(_))
        ));
    }

    #[test]
    fn debug_output_never_contains_the_key() {
        let key = ApiKey::new("sk-FAKE-secret-value").unwrap();
        let rendered = format!("{key:?}");
        assert_eq!(rendered, "ApiKey(redacted)");
        assert!(!rendered.contains("secret-value"));
    }
}
