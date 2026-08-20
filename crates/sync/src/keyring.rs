//! K_sync availability.
//!
//! Sync must work while the vault is locked (offline sync of normal
//! snippets is a product promise), so each K_sync generation keeps a
//! runtime copy in the platform secure store under a non-gated entry.
//! When a vault exists, the MK-wrapped copy in `domain_key(domain='sync')`
//! remains the canonical source for pairing and recovery. The key bytes
//! never touch the database or logs (byte-level red line).

use std::collections::HashMap;
use std::fmt;

use rusqlite::Connection;
use typvia_core::model::{DomainKey, KeyDomain, TimestampMs};
use typvia_core::repo::{RepoError, VaultKeyRepo};
use typvia_crypto::{CryptoError, SecureStore, SecureStoreError, SymmetricKey};

use crate::record::SyncKeyring;

/// Secure-store entry prefix; one entry per generation
/// (`sync.k_sync.<key_id>`), same shape as the device-key entries.
pub const K_SYNC_ENTRY_PREFIX: &str = "sync.k_sync.";

pub(crate) fn entry_name(key_id: u32) -> String {
    format!("{K_SYNC_ENTRY_PREFIX}{key_id}")
}

/// Whether this client already holds a K_sync generation (key-update
/// application skips generations it has, keeping re-processing idempotent).
pub(crate) fn has_key(store: &dyn SecureStore, key_id: u32) -> Result<bool, KeyringError> {
    Ok(store.retrieve(&entry_name(key_id))?.is_some())
}

/// Failures around K_sync provisioning and loading. Never carries key bytes.
#[derive(Debug)]
pub enum KeyringError {
    Store(SecureStoreError),
    /// A stored entry has the wrong length for a symmetric key.
    CorruptKeyMaterial,
    /// The configuration names a current generation whose key is in
    /// neither the secure store nor an unwrappable database copy.
    MissingCurrentKey(u32),
    /// Wrapping or unwrapping the MK copy failed.
    Crypto(CryptoError),
    /// The domain_key table rejected the read or write.
    Repo(RepoError),
}

impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::CorruptKeyMaterial => f.write_str("stored sync key material is invalid"),
            Self::MissingCurrentKey(id) => {
                write!(f, "sync key generation {id} is not available")
            }
            Self::Crypto(e) => write!(f, "{e}"),
            Self::Repo(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for KeyringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Crypto(e) => Some(e),
            Self::Repo(e) => Some(e),
            Self::CorruptKeyMaterial | Self::MissingCurrentKey(_) => None,
        }
    }
}

impl From<SecureStoreError> for KeyringError {
    fn from(e: SecureStoreError) -> Self {
        Self::Store(e)
    }
}

impl From<RepoError> for KeyringError {
    fn from(e: RepoError) -> Self {
        Self::Repo(e)
    }
}

/// The K_sync generations this client holds, loaded from the secure store.
/// Generations start at 1; gaps are legal (a generation this device never
/// received keeps its records pending).
pub struct SyncKeys {
    keys: HashMap<u32, SymmetricKey>,
    current: u32,
}

impl SyncKeys {
    /// Loads generations `1..=current` from the secure store; absent
    /// generations are skipped, but the current one must exist.
    pub fn load(store: &dyn SecureStore, current: u32) -> Result<Self, KeyringError> {
        let mut keys = HashMap::new();
        for key_id in 1..=current {
            if let Some(secret) = store.retrieve(&entry_name(key_id))? {
                let bytes: [u8; typvia_crypto::KEY_LEN] = secret
                    .as_slice()
                    .try_into()
                    .map_err(|_| KeyringError::CorruptKeyMaterial)?;
                keys.insert(key_id, SymmetricKey::from_bytes(bytes));
            }
        }
        if current > 0 && !keys.contains_key(&current) {
            return Err(KeyringError::MissingCurrentKey(current));
        }
        Ok(Self { keys, current })
    }

    /// The generation new records are sealed under.
    pub fn current_id(&self) -> u32 {
        self.current
    }

    /// The current sealing key; `None` only for an unprovisioned keyring.
    pub fn current_key(&self) -> Option<&SymmetricKey> {
        self.keys.get(&self.current)
    }
}

impl SyncKeyring for SyncKeys {
    fn key(&self, key_id: u32) -> Option<&SymmetricKey> {
        self.keys.get(&key_id)
    }
}

impl fmt::Debug for SyncKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Key bytes never reach Debug output.
        f.debug_struct("SyncKeys")
            .field("current", &self.current)
            .field("generations", &self.keys.len())
            .finish_non_exhaustive()
    }
}

/// Generates the account's first K_sync (key_id = 1): the runtime
/// copy goes to the secure store; when the master key is available the
/// MK-wrapped canonical copy also lands in `domain_key` (same transaction
/// scope as the caller's connection).
pub fn provision_initial_key(
    conn: &Connection,
    store: &dyn SecureStore,
    master_key: Option<&SymmetricKey>,
    now: TimestampMs,
) -> Result<u32, KeyringError> {
    let key = SymmetricKey::generate();
    let key_id = 1u32;
    store.store(&entry_name(key_id), key.expose())?;
    if let Some(mk) = master_key {
        let wrapped = typvia_crypto::wrap_key(
            mk,
            key_id,
            &typvia_crypto::aad_domain_key(KeyDomain::Sync.as_str()),
            &key,
        )
        .map_err(KeyringError::Crypto)?;
        VaultKeyRepo::new(conn).put_domain_key(&DomainKey {
            domain: KeyDomain::Sync,
            key_id,
            wrapped_key: wrapped,
            created_at: now,
        })?;
    }
    Ok(key_id)
}

/// Installs a received K_sync generation (pairing bundle / key update):
/// runtime copy into the secure store, replacing any previous entry of the
/// same generation.
pub fn install_key(
    store: &dyn SecureStore,
    key_id: u32,
    key: &SymmetricKey,
) -> Result<(), KeyringError> {
    store.store(&entry_name(key_id), key.expose())?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::identity::tests::MemoryStore;
    use typvia_core::db::{migrate_to_latest, open_in_memory};

    #[test]
    fn provision_then_load_round_trips_through_the_secure_store() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let store = MemoryStore::new();
        let key_id = provision_initial_key(&conn, &store, None, 1_700_000_000_000).unwrap();
        assert_eq!(key_id, 1);

        let keys = SyncKeys::load(&store, key_id).unwrap();
        assert_eq!(keys.current_id(), 1);
        assert!(keys.current_key().is_some());
        // No vault: no MK-wrapped database copy is written.
        let domain_keys = VaultKeyRepo::new(&conn).list_domain_keys().unwrap();
        assert!(domain_keys.is_empty());
    }

    #[test]
    fn provision_with_a_master_key_also_writes_the_wrapped_copy() {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        let store = MemoryStore::new();
        let mk = SymmetricKey::generate();
        provision_initial_key(&conn, &store, Some(&mk), 1_700_000_000_000).unwrap();

        let domain_keys = VaultKeyRepo::new(&conn).list_domain_keys().unwrap();
        assert_eq!(domain_keys.len(), 1);
        assert_eq!(domain_keys[0].domain, KeyDomain::Sync);
        // The wrapped copy unwraps back to the same key held in the store.
        let unwrapped = typvia_crypto::unwrap_key(
            &mk,
            &typvia_crypto::aad_domain_key("sync"),
            &domain_keys[0].wrapped_key,
        )
        .unwrap();
        let stored = store.retrieve(&entry_name(1)).unwrap().unwrap();
        assert_eq!(unwrapped.expose().as_slice(), stored.as_slice());
    }

    #[test]
    fn a_missing_current_generation_is_an_error_not_a_silent_gap() {
        let store = MemoryStore::new();
        assert!(matches!(
            SyncKeys::load(&store, 1),
            Err(KeyringError::MissingCurrentKey(1))
        ));
    }

    #[test]
    fn missing_historic_generations_are_skipped() {
        let store = MemoryStore::new();
        let newer = SymmetricKey::generate();
        install_key(&store, 2, &newer).unwrap();
        let keys = SyncKeys::load(&store, 2).unwrap();
        assert!(keys.key(1).is_none());
        assert!(keys.key(2).is_some());
    }

    #[test]
    fn an_unprovisioned_keyring_loads_empty() {
        let store = MemoryStore::new();
        let keys = SyncKeys::load(&store, 0).unwrap();
        assert_eq!(keys.current_id(), 0);
        assert!(keys.current_key().is_none());
    }

    #[test]
    fn a_wrong_length_entry_is_corrupt_key_material() {
        let store = MemoryStore::new();
        store.store(&entry_name(1), &[0xAA; 16]).unwrap();
        assert!(matches!(
            SyncKeys::load(&store, 1),
            Err(KeyringError::CorruptKeyMaterial)
        ));
    }

    #[test]
    fn debug_output_carries_no_key_bytes() {
        let store = MemoryStore::new();
        let key = SymmetricKey::generate();
        install_key(&store, 1, &key).unwrap();
        let keys = SyncKeys::load(&store, 1).unwrap();
        let rendered = format!("{keys:?}");
        let hex: String = key.expose().iter().map(|b| format!("{b:02x}")).collect();
        assert!(!rendered.contains(&hex));
        assert!(rendered.contains("generations"));
    }
}
