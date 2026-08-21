// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Encrypted-vault use-case layer.
//!
//! The unlock state machine lives here, in core. Platform secure-store
//! backends (macOS Keychain, ...) are implemented in the host layers and
//! injected through the crypto crate's [`SecureStore`] trait, which this module
//! re-exports for convenience.

mod session;
mod throttle;

pub use session::{BIOMETRIC_MK_ENTRY, UnlockStatus, VaultError, VaultSession};
pub use typvia_crypto::{SecureStore, SecureStoreError};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use typvia_crypto::SecureStoreError;
    use zeroize::Zeroizing;

    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use rusqlite::Connection;

    const NOW: i64 = 1_700_000_000_000;
    const IDLE_TIMEOUT: i64 = 15 * 60 * 1000;
    const PASSWORD: &[u8] = b"correct horse battery staple";

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    /// In-memory secure store for tests: no biometric gate, just a map. The
    /// real biometric behaviour is verified by the opt-in host live test.
    #[derive(Default)]
    struct FakeSecureStore {
        entries: RefCell<HashMap<String, Vec<u8>>>,
        deny: bool,
    }

    impl FakeSecureStore {
        fn denying() -> Self {
            Self {
                deny: true,
                ..Self::default()
            }
        }
    }

    impl SecureStore for FakeSecureStore {
        fn store(&self, entry: &str, secret: &[u8]) -> Result<(), SecureStoreError> {
            if self.deny {
                return Err(SecureStoreError::AccessDenied);
            }
            self.entries
                .borrow_mut()
                .insert(entry.to_string(), secret.to_vec());
            Ok(())
        }

        fn retrieve(&self, entry: &str) -> Result<Option<Zeroizing<Vec<u8>>>, SecureStoreError> {
            if self.deny {
                return Err(SecureStoreError::AccessDenied);
            }
            Ok(self
                .entries
                .borrow()
                .get(entry)
                .map(|v| Zeroizing::new(v.clone())))
        }

        fn remove(&self, entry: &str) -> Result<(), SecureStoreError> {
            self.entries.borrow_mut().remove(entry);
            Ok(())
        }
    }

    fn initialized_session(conn: &Connection) -> VaultSession {
        let mut session = VaultSession::new();
        session.initialize(conn, PASSWORD, NOW).unwrap();
        session
    }

    #[test]
    fn new_session_is_locked() {
        let session = VaultSession::new();
        assert!(!session.is_unlocked());
        assert_eq!(session.status(), UnlockStatus::Locked);
    }

    #[test]
    fn initialize_leaves_the_session_unlocked_and_persists_a_header() {
        let conn = db();
        assert!(!VaultSession::is_initialized(&conn).unwrap());
        let session = initialized_session(&conn);
        assert!(session.is_unlocked());
        assert!(VaultSession::is_initialized(&conn).unwrap());
        assert_eq!(
            session.status(),
            UnlockStatus::Unlocked {
                since: NOW,
                last_activity: NOW
            }
        );
    }

    #[test]
    fn initialize_refuses_a_second_time() {
        let conn = db();
        let mut session = initialized_session(&conn);
        let err = session.initialize(&conn, PASSWORD, NOW).unwrap_err();
        assert!(matches!(err, VaultError::AlreadyInitialized));
    }

    #[test]
    fn correct_password_unlocks_after_relock() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.lock();
        assert!(!session.is_unlocked());
        session
            .unlock_with_password(&conn, PASSWORD, NOW + 1)
            .unwrap();
        assert!(session.is_unlocked());
    }

    #[test]
    fn wrong_password_keeps_the_session_locked() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.lock();
        let err = session
            .unlock_with_password(&conn, b"wrong", NOW + 1)
            .unwrap_err();
        assert!(matches!(err, VaultError::WrongPassword));
        assert!(!session.is_unlocked());
    }

    #[test]
    fn unlock_before_initialization_reports_not_initialized() {
        let conn = db();
        let mut session = VaultSession::new();
        let err = session
            .unlock_with_password(&conn, PASSWORD, NOW)
            .unwrap_err();
        assert!(matches!(err, VaultError::NotInitialized));
    }

    #[test]
    fn repeated_wrong_passwords_trip_the_throttle() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.lock();
        // Five failures trip a cooldown; the sixth attempt is refused outright.
        for _ in 0..5 {
            let _ = session.unlock_with_password(&conn, b"wrong", NOW);
        }
        let err = session
            .unlock_with_password(&conn, PASSWORD, NOW)
            .unwrap_err();
        match err {
            VaultError::Throttled { retry_at } => assert!(retry_at > NOW),
            other => panic!("expected Throttled, got {other:?}"),
        }
        // Even the correct password is refused while throttled.
        assert!(!session.is_unlocked());
    }

    #[test]
    fn a_successful_unlock_resets_the_failure_run() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.lock();
        for _ in 0..4 {
            let _ = session.unlock_with_password(&conn, b"wrong", NOW);
        }
        session.unlock_with_password(&conn, PASSWORD, NOW).unwrap();
        session.lock();
        // The run was reset, so four fresh failures still do not throttle.
        for _ in 0..4 {
            let _ = session.unlock_with_password(&conn, b"wrong", NOW);
        }
        // The next correct password succeeds (no cooldown in effect).
        session.unlock_with_password(&conn, PASSWORD, NOW).unwrap();
        assert!(session.is_unlocked());
    }

    #[test]
    fn lock_makes_the_session_locked_again() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.lock();
        assert!(!session.is_unlocked());
        assert_eq!(session.status(), UnlockStatus::Locked);
    }

    #[test]
    fn idle_timeout_locks_only_after_the_window() {
        let conn = db();
        let mut session = initialized_session(&conn);
        assert!(!session.enforce_idle_timeout(NOW + IDLE_TIMEOUT - 1, IDLE_TIMEOUT));
        assert!(session.is_unlocked());
        assert!(session.enforce_idle_timeout(NOW + IDLE_TIMEOUT, IDLE_TIMEOUT));
        assert!(!session.is_unlocked());
    }

    #[test]
    fn activity_defers_the_idle_timeout() {
        let conn = db();
        let mut session = initialized_session(&conn);
        session.note_activity(NOW + IDLE_TIMEOUT - 1);
        // The window restarts from the last activity, so the old deadline passes.
        assert!(!session.enforce_idle_timeout(NOW + IDLE_TIMEOUT, IDLE_TIMEOUT));
        assert!(session.is_unlocked());
    }

    #[test]
    fn idle_timeout_on_a_locked_session_is_a_noop() {
        let mut session = VaultSession::new();
        assert!(!session.enforce_idle_timeout(NOW + IDLE_TIMEOUT, IDLE_TIMEOUT));
    }

    #[test]
    fn biometric_enable_then_unlock_round_trips_the_mk() {
        let conn = db();
        let store = FakeSecureStore::default();
        let mut session = initialized_session(&conn);
        session.enable_biometric(&store).unwrap();

        session.lock();
        session.unlock_with_biometric(&store, NOW + 1).unwrap();
        assert!(session.is_unlocked());
    }

    #[test]
    fn biometric_unlock_without_enrollment_reports_unavailable() {
        let store = FakeSecureStore::default();
        let mut session = VaultSession::new();
        let err = session.unlock_with_biometric(&store, NOW).unwrap_err();
        assert!(matches!(err, VaultError::BiometricUnavailable));
    }

    #[test]
    fn enabling_biometric_requires_an_unlocked_session() {
        let store = FakeSecureStore::default();
        let session = VaultSession::new();
        let err = session.enable_biometric(&store).unwrap_err();
        assert!(matches!(err, VaultError::Locked));
    }

    #[test]
    fn a_denied_biometric_gate_surfaces_as_a_secure_store_error() {
        let store = FakeSecureStore::denying();
        let mut session = VaultSession::new();
        let err = session.unlock_with_biometric(&store, NOW).unwrap_err();
        assert!(matches!(
            err,
            VaultError::SecureStore(SecureStoreError::AccessDenied)
        ));
    }

    #[test]
    fn disable_biometric_is_idempotent() {
        let store = FakeSecureStore::default();
        let conn = db();
        let mut session = initialized_session(&conn);
        session.enable_biometric(&store).unwrap();
        session.disable_biometric(&store).unwrap();
        // Second disable is fine; a later biometric unlock now reports unavailable.
        session.disable_biometric(&store).unwrap();
        session.lock();
        let err = session.unlock_with_biometric(&store, NOW).unwrap_err();
        assert!(matches!(err, VaultError::BiometricUnavailable));
    }

    #[test]
    fn content_round_trips_through_encrypt_then_decrypt() {
        let conn = db();
        let session = initialized_session(&conn);
        let plaintext = b"postgres://readonly@10.4.2.19:5432";
        let envelope = session.encrypt_content(&conn, "rec-1", plaintext).unwrap();
        // Ciphertext must not carry the plaintext (index/log red line spirit).
        assert!(
            envelope.windows(plaintext.len()).all(|w| w != plaintext),
            "plaintext leaked into the envelope"
        );
        let decrypted = session.decrypt_content(&conn, "rec-1", &envelope).unwrap();
        assert_eq!(decrypted.as_slice(), plaintext);
    }

    #[test]
    fn decrypt_rejects_a_different_record_id() {
        let conn = db();
        let session = initialized_session(&conn);
        let envelope = session.encrypt_content(&conn, "rec-1", b"secret").unwrap();
        // The record id is bound into the AAD, so another id fails to authenticate.
        let err = session
            .decrypt_content(&conn, "rec-2", &envelope)
            .unwrap_err();
        assert!(matches!(err, VaultError::Corrupt));
    }

    #[test]
    fn encrypt_and_decrypt_require_an_unlocked_session() {
        let conn = db();
        let mut session = initialized_session(&conn);
        let envelope = session.encrypt_content(&conn, "rec-1", b"secret").unwrap();
        session.lock();
        assert!(matches!(
            session.encrypt_content(&conn, "rec-1", b"secret"),
            Err(VaultError::Locked)
        ));
        assert!(matches!(
            session.decrypt_content(&conn, "rec-1", &envelope),
            Err(VaultError::Locked)
        ));
    }

    #[test]
    fn a_wrong_master_key_cannot_decrypt_after_reinitialization() {
        // Content sealed under one vault's K_vault must not open under another
        // vault's key domain (different MK -> different unwrapped K_vault).
        let conn = db();
        let session_a = initialized_session(&conn);
        let envelope = session_a
            .encrypt_content(&conn, "rec-1", b"secret")
            .unwrap();
        // Simulate a fresh device vault by wiping the key tables and re-init.
        conn.execute("DELETE FROM key_header", []).unwrap();
        conn.execute("DELETE FROM domain_key", []).unwrap();
        let mut session_b = VaultSession::new();
        session_b
            .initialize(&conn, b"a different password", NOW)
            .unwrap();
        let err = session_b
            .decrypt_content(&conn, "rec-1", &envelope)
            .unwrap_err();
        assert!(matches!(err, VaultError::Corrupt));
    }

    #[test]
    fn debug_output_never_exposes_key_material() {
        let conn = db();
        let session = initialized_session(&conn);
        let rendered = format!("{session:?}");
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn error_messages_carry_no_dynamic_secrets() {
        for error in [
            VaultError::NotInitialized,
            VaultError::AlreadyInitialized,
            VaultError::WrongPassword,
            VaultError::Throttled { retry_at: NOW },
            VaultError::Locked,
            VaultError::BiometricUnavailable,
            VaultError::Corrupt,
        ] {
            let message = error.to_string();
            assert!(!message.is_empty());
            assert!(message.is_ascii(), "static English messages only");
        }
    }

    /// The sync flows need the master key inside one call and
    /// nowhere else; the scoped accessor is what they get, and a locked
    /// session hands them nothing at all.
    #[test]
    fn the_master_key_is_only_borrowed_while_unlocked() {
        let conn = db();
        let mut session = VaultSession::new();
        assert!(session.with_master_key(|mk| mk.is_none()));

        session.initialize(&conn, PASSWORD, NOW).unwrap();
        let bytes = session.with_master_key(|mk| mk.map(|key| *key.expose()));
        assert!(bytes.is_some());

        session.lock();
        assert!(session.with_master_key(|mk| mk.is_none()));
    }
}
