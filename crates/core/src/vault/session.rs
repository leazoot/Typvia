//! Vault unlock state machine.
//!
//! Two states only — `Locked` / `Unlocked` — living in the core use-case
//! layer. While unlocked the session holds the master key (MK) in memory; on
//! `lock()` it is dropped and zeroized. All crypto is delegated to the crypto
//! crate; this layer orchestrates derive/unwrap and persistence, and never
//! writes the master password, KEK, or MK plaintext to the database or a log.
//!
//! Unlocking holds only the MK. The vault-domain key (K_vault) is never kept
//! resident: [`VaultSession::encrypt_content`]/[`VaultSession::decrypt_content`]
//! unwrap it from the held MK per call and drop it immediately, keeping the
//! materialized domain key's lifetime as short as possible.

use std::fmt;

use rusqlite::Connection;
use typvia_crypto::{
    CryptoError, KEY_LEN, KdfParams, SecureStore, SecureStoreError, SymmetricKey, aad_domain_key,
    aad_master_key, aad_record, derive_kek, open, seal, unwrap_key, wrap_key,
};
use zeroize::Zeroizing;

use super::throttle::UnlockThrottle;
use crate::model::{DomainKey, KeyDomain, TimestampMs, VaultKeyHeader};
use crate::repo::{RepoError, VaultKeyRepo, new_id};

/// Key generation used for the singleton MK wrap and the initial domain key.
const INITIAL_KEY_ID: u32 = 1;

/// Entry name under which the biometric-gated MK copy lives in the platform
/// secure store. Opaque to the store backend; stable across sessions.
pub const BIOMETRIC_MK_ENTRY: &str = "vault.mk.biometric";

/// What callers/UI may observe about the vault — never the keys themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockStatus {
    /// No key material in memory; sensitive content is unavailable.
    Locked,
    /// The MK is held; `since`/`last_activity` drive the idle timeout.
    Unlocked {
        since: TimestampMs,
        last_activity: TimestampMs,
    },
}

/// Live key material held only while unlocked. `SymmetricKey` redacts its own
/// `Debug` and zeroizes on drop, so dropping this wipes the MK.
#[derive(Debug)]
struct SessionKeys {
    mk: SymmetricKey,
    since: TimestampMs,
    last_activity: TimestampMs,
}

/// The vault session: the state machine plus its unlock-failure throttle.
#[derive(Debug, Default)]
pub struct VaultSession {
    keys: Option<SessionKeys>,
    throttle: UnlockThrottle,
}

impl VaultSession {
    /// A fresh, locked session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether key material is currently held.
    pub fn is_unlocked(&self) -> bool {
        self.keys.is_some()
    }

    /// The observable status (no key material).
    pub fn status(&self) -> UnlockStatus {
        match &self.keys {
            None => UnlockStatus::Locked,
            Some(k) => UnlockStatus::Unlocked {
                since: k.since,
                last_activity: k.last_activity,
            },
        }
    }

    /// Whether the device vault has been initialized (a header exists).
    pub fn is_initialized(conn: &Connection) -> Result<bool, VaultError> {
        Ok(VaultKeyRepo::new(conn).load_header()?.is_some())
    }

    /// First-run setup: generate the MK and the vault-domain key, wrap them,
    /// and persist the header. Refuses if a vault already exists. Leaves the
    /// session unlocked (the user just proved the password by choosing it).
    pub fn initialize(
        &mut self,
        conn: &Connection,
        password: &[u8],
        now: TimestampMs,
    ) -> Result<(), VaultError> {
        let repo = VaultKeyRepo::new(conn);
        if repo.load_header()?.is_some() {
            return Err(VaultError::AlreadyInitialized);
        }

        let params = KdfParams::v1();
        let kek = derive_kek(password, &params).map_err(VaultError::from_crypto)?;
        let mk = SymmetricKey::generate();
        let wrapped_mk = wrap_key(&kek, INITIAL_KEY_ID, &aad_master_key(), &mk)
            .map_err(VaultError::from_crypto)?;

        let vault_key = SymmetricKey::generate();
        let wrapped_vault = wrap_key(
            &mk,
            INITIAL_KEY_ID,
            &aad_domain_key(KeyDomain::Vault.as_str()),
            &vault_key,
        )
        .map_err(VaultError::from_crypto)?;

        repo.put_header(&VaultKeyHeader {
            id: new_id(),
            kdf: params,
            wrapped_mk,
            created_at: now,
            updated_at: now,
        })?;
        repo.put_domain_key(&DomainKey {
            domain: KeyDomain::Vault,
            key_id: INITIAL_KEY_ID,
            wrapped_key: wrapped_vault,
            created_at: now,
        })?;

        self.set_unlocked(mk, now);
        Ok(())
    }

    /// Unlock with the master password: re-derive the KEK from the *stored*
    /// KDF parameters and unwrap the MK. A wrong password counts against the
    /// throttle; the error never says which part was wrong.
    pub fn unlock_with_password(
        &mut self,
        conn: &Connection,
        password: &[u8],
        now: TimestampMs,
    ) -> Result<(), VaultError> {
        if let Some(retry_at) = self.throttle.cooldown_until(now) {
            return Err(VaultError::Throttled { retry_at });
        }

        let header = VaultKeyRepo::new(conn)
            .load_header()?
            .ok_or(VaultError::NotInitialized)?;

        let kek = derive_kek(password, &header.kdf).map_err(VaultError::from_crypto)?;
        match unwrap_key(&kek, &aad_master_key(), &header.wrapped_mk) {
            Ok(mk) => {
                self.throttle.record_success();
                self.set_unlocked(mk, now);
                Ok(())
            }
            Err(CryptoError::DecryptionFailed) => {
                self.throttle.record_failure(now);
                Err(VaultError::WrongPassword)
            }
            // A structurally broken envelope is corrupt storage, not a guess:
            // it must not count against the throttle.
            Err(other) => Err(VaultError::from_crypto(other)),
        }
    }

    /// Stores a biometric-gated copy of the MK in the platform secure store.
    /// Requires an unlocked session. The master password and KEK are
    /// never stored — only the MK copy the OS will gate behind biometrics.
    pub fn enable_biometric(&self, store: &dyn SecureStore) -> Result<(), VaultError> {
        let keys = self.keys.as_ref().ok_or(VaultError::Locked)?;
        store.store(BIOMETRIC_MK_ENTRY, keys.mk.expose())?;
        Ok(())
    }

    /// Removes the biometric MK copy. Idempotent: disabling twice is fine,
    /// and it does not require an unlocked session.
    pub fn disable_biometric(&self, store: &dyn SecureStore) -> Result<(), VaultError> {
        store.remove(BIOMETRIC_MK_ENTRY)?;
        Ok(())
    }

    /// Unlock via the biometric MK copy. The OS enforces the biometric gate on
    /// `retrieve`; a missing copy means biometrics were never enabled. This
    /// path is independent of the password throttle.
    pub fn unlock_with_biometric(
        &mut self,
        store: &dyn SecureStore,
        now: TimestampMs,
    ) -> Result<(), VaultError> {
        let raw = store
            .retrieve(BIOMETRIC_MK_ENTRY)?
            .ok_or(VaultError::BiometricUnavailable)?;
        let mut bytes: [u8; KEY_LEN] =
            raw.as_slice().try_into().map_err(|_| VaultError::Corrupt)?;
        let mk = SymmetricKey::from_bytes(bytes);
        // `raw` zeroizes on drop; wipe the stack copy the array constructor made.
        bytes.fill(0);
        self.set_unlocked(mk, now);
        Ok(())
    }

    /// Encrypts sensitive snippet content under the vault domain key, binding
    /// the record id into the AAD (`aad_record`). Requires an unlocked session.
    /// K_vault is unwrapped from the held MK and dropped before returning; the
    /// returned envelope is `version‖key_id‖nonce‖AEAD`.
    pub fn encrypt_content(
        &self,
        conn: &Connection,
        record_id: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        let (key, key_id) = self.vault_key(conn)?;
        seal(&key, key_id, &aad_record(record_id), plaintext).map_err(VaultError::from_crypto)
    }

    /// Decrypts sensitive snippet content produced by [`Self::encrypt_content`].
    /// Requires an unlocked session; the record id must match the one bound at
    /// seal time or authentication fails. The plaintext is returned in a
    /// `Zeroizing` buffer so it is wiped on drop.
    pub fn decrypt_content(
        &self,
        conn: &Connection,
        record_id: &str,
        envelope: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let (key, _) = self.vault_key(conn)?;
        open(&key, &aad_record(record_id), envelope).map_err(VaultError::from_crypto)
    }

    /// Materializes the latest vault domain key by unwrapping it from the held
    /// MK. Returns the key and its generation `key_id`. Requires an unlocked
    /// session; the key is the caller's to drop promptly (minimal lifetime).
    fn vault_key(&self, conn: &Connection) -> Result<(SymmetricKey, u32), VaultError> {
        let keys = self.keys.as_ref().ok_or(VaultError::Locked)?;
        let domain_key = VaultKeyRepo::new(conn)
            .load_latest_domain_key(KeyDomain::Vault)?
            .ok_or(VaultError::NotInitialized)?;
        let key = unwrap_key(
            &keys.mk,
            &aad_domain_key(KeyDomain::Vault.as_str()),
            &domain_key.wrapped_key,
        )
        .map_err(VaultError::from_crypto)?;
        Ok((key, domain_key.key_id))
    }

    /// Runs `use_key` with the held master key — `None` while locked — for
    /// the sync flows that must seal it into a key bundle or derive the
    /// recovery rootproof key.
    /// Deliberately a scoped borrow and not a getter: the MK must not
    /// outlive the call that needs it, and no caller can stash a copy.
    /// Callers that cannot proceed without it turn the `None` into their
    /// own refusal.
    pub fn with_master_key<R>(&self, use_key: impl FnOnce(Option<&SymmetricKey>) -> R) -> R {
        use_key(self.keys.as_ref().map(|keys| &keys.mk))
    }

    /// Drop into `Locked`, wiping the MK. Idempotent; leaves throttle state
    /// intact (locking is not an unlock failure).
    pub fn lock(&mut self) {
        self.keys = None;
    }

    /// Refreshes the activity clock so an active session is not idle-locked.
    pub fn note_activity(&mut self, now: TimestampMs) {
        if let Some(keys) = self.keys.as_mut() {
            keys.last_activity = now;
        }
    }

    /// Locks the session if it has been idle for at least `timeout_ms`.
    /// Returns whether it locked.
    pub fn enforce_idle_timeout(&mut self, now: TimestampMs, timeout_ms: i64) -> bool {
        let idle_for = match &self.keys {
            Some(keys) => now.saturating_sub(keys.last_activity),
            None => return false,
        };
        if idle_for >= timeout_ms {
            self.lock();
            true
        } else {
            false
        }
    }

    fn set_unlocked(&mut self, mk: SymmetricKey, now: TimestampMs) {
        self.keys = Some(SessionKeys {
            mk,
            since: now,
            last_activity: now,
        });
    }
}

/// Vault use-case errors. Messages are static and structural — they never
/// carry key material, the password, or which part of an unlock failed
/// (log red line).
#[derive(Debug)]
pub enum VaultError {
    /// No vault header exists yet; the master password has not been set.
    NotInitialized,
    /// `initialize` was called but a vault already exists.
    AlreadyInitialized,
    /// The master password did not unwrap the MK.
    WrongPassword,
    /// Attempts are refused until `retry_at` after too many failures.
    Throttled { retry_at: TimestampMs },
    /// The operation requires an unlocked session.
    Locked,
    /// No biometric MK copy is enrolled in the secure store.
    BiometricUnavailable,
    /// Stored key material is structurally corrupt (wrong length/envelope).
    Corrupt,
    /// A platform secure-store backend failure.
    SecureStore(SecureStoreError),
    /// A storage-layer failure.
    Storage(RepoError),
}

impl VaultError {
    /// Maps a crypto failure to a structural vault error. `DecryptionFailed`
    /// is handled by the caller (it is the wrong-password signal); anything
    /// else means corrupt stored material.
    fn from_crypto(_error: CryptoError) -> Self {
        Self::Corrupt
    }
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => f.write_str("vault is not initialized"),
            Self::AlreadyInitialized => f.write_str("vault is already initialized"),
            Self::WrongPassword => f.write_str("unlock failed"),
            Self::Throttled { .. } => f.write_str("too many attempts, try again later"),
            Self::Locked => f.write_str("vault is locked"),
            Self::BiometricUnavailable => f.write_str("biometric unlock is not set up"),
            Self::Corrupt => f.write_str("stored vault key material is invalid"),
            Self::SecureStore(e) => write!(f, "{e}"),
            Self::Storage(_) => f.write_str("vault storage error"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<RepoError> for VaultError {
    fn from(error: RepoError) -> Self {
        Self::Storage(error)
    }
}

impl From<SecureStoreError> for VaultError {
    fn from(error: SecureStoreError) -> Self {
        Self::SecureStore(error)
    }
}
