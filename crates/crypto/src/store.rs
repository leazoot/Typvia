//! Platform secure-storage abstraction (docs/06_SECURITY_MODEL.md §6).
//!
//! Backends: macOS/iOS Keychain, Android Keystore, Windows Credential
//! Manager/DPAPI — implemented in the host layers (feasibility proven by
//! TASK-013). This crate only defines the contract; nothing here performs
//! platform I/O.

use zeroize::Zeroizing;

use crate::error::SecureStoreError;

/// Key-value store backed by the platform's secure storage. Entries hold
/// key material (the biometric-gated MK copy, device keys) — never the
/// master password or KEK (§6). Implementations must apply the platform's
/// access-control gate (biometric / user presence) on `retrieve`.
pub trait SecureStore {
    /// Persists `secret` under `entry`, replacing any previous value.
    fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError>;

    /// Reads an entry back; `Ok(None)` when it does not exist. The buffer
    /// zeroizes itself on drop.
    fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError>;

    /// Deletes an entry; deleting a missing entry is not an error
    /// (disabling biometrics twice must be idempotent, §6).
    fn remove(&self, entry: &str) -> Result<(), SecureStoreError>;
}
