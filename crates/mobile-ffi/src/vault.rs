// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The vault surface.
//!
//! What crosses this boundary is deliberately thin. Passwords go in, status
//! and counts come back, and exactly one call returns a plaintext — the
//! explicit reveal of one secret the user asked to see. Key derivation,
//! unwrapping, encryption and the idle timer all stay behind it.
//!
//! Swift's whole share of the unlock decision is whether to attempt the
//! biometric read. It cannot pass or fail the gate on the core's behalf,
//! because the gate is the Keychain access control on the master-key copy:
//! the read either returns the key or it does not.

use typvia_host_service::dto::VaultStatusDto;
use typvia_host_service::service;

use crate::error::CoreError;
use crate::model::{Snippet, SnippetDraft, SnippetEdit};
use crate::service::TypviaCore;

/// What the vault screen can know while locked.
///
/// Carries no key material and no counts of what is inside — a locked vault
/// shows the shape of the room, never its contents.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VaultStatus {
    /// A vault has been set up on this device.
    pub initialized: bool,
    /// The session currently holds the master key.
    pub unlocked: bool,
    /// When the current unlock happened, in milliseconds, if unlocked.
    pub unlocked_at: Option<i64>,
    /// Last activity that deferred the idle timer, if unlocked.
    pub last_activity_at: Option<i64>,
    /// Idle window before the vault re-locks itself, in milliseconds.
    pub idle_timeout_ms: i64,
}

impl From<VaultStatusDto> for VaultStatus {
    fn from(s: VaultStatusDto) -> Self {
        Self {
            initialized: s.initialized,
            unlocked: s.unlocked,
            unlocked_at: s.unlocked_at,
            last_activity_at: s.last_activity_at,
            idle_timeout_ms: s.idle_timeout_ms,
        }
    }
}

#[uniffi::export]
impl TypviaCore {
    /// Current vault state. Reading it also applies the idle timer, so a
    /// vault left alone past its window is already locked by the time the
    /// screen renders.
    pub fn vault_status(&self) -> Result<VaultStatus, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_status(&conn, &mut session, now)?.into())
    }

    /// First-run setup: sets the master password and leaves the vault open.
    pub fn vault_initialize(&self, password: String) -> Result<VaultStatus, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_initialize(&conn, &mut session, &password, now)?.into())
    }

    /// Unlocks with the master password. A wrong password fails here and
    /// nowhere else — no partial state, no hint about which part was wrong.
    pub fn vault_unlock_password(&self, password: String) -> Result<VaultStatus, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_unlock_password(&conn, &mut session, &password, now)?.into())
    }

    /// Unlocks through the Face ID-gated copy of the master key. The system
    /// presents the sheet as part of the Keychain read; this call returns only
    /// after the user has passed or failed it.
    pub fn vault_unlock_biometric(&self) -> Result<VaultStatus, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_unlock_biometric(&conn, &mut session, self.secure_store(), now)?.into())
    }

    /// Locks immediately, dropping the master key. Always available, always
    /// instantaneous.
    pub fn vault_lock(&self) -> Result<VaultStatus, CoreError> {
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_lock(&conn, &mut session)?.into())
    }

    /// Enrols a Face ID-gated copy of the master key. Needs an open vault:
    /// the key being copied has to be in hand.
    pub fn vault_enable_biometric(&self) -> Result<(), CoreError> {
        let session = self.vault()?;
        Ok(service::vault_enable_biometric(
            &session,
            self.secure_store(),
        )?)
    }

    /// Removes the biometric copy. Idempotent, and the master-password path is
    /// untouched — biometrics are a gate, never the source of the key.
    pub fn vault_disable_biometric(&self) -> Result<(), CoreError> {
        let session = self.vault()?;
        Ok(service::vault_disable_biometric(
            &session,
            self.secure_store(),
        )?)
    }

    /// How many secrets a reset would destroy, recycle bin included. Works
    /// while locked: only a number crosses, and the confirm copy has to be
    /// able to state the real one.
    pub fn vault_reset_preview(&self) -> Result<u32, CoreError> {
        let conn = self.conn()?;
        Ok(service::vault_reset_preview(&conn)?)
    }

    /// Destroys the vault for someone who lost the master password. Works
    /// while locked, because that is the situation it exists for.
    pub fn vault_reset(&self) -> Result<VaultStatus, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_reset(&conn, &mut session, self.secure_store(), now)?.into())
    }

    /// The vault's snippets as metadata only — every `body` is absent. A list
    /// of secrets is not a list of their contents, whether the vault is open
    /// or not.
    pub fn vault_list(&self, limit: u32, offset: u32) -> Result<Vec<Snippet>, CoreError> {
        let conn = self.conn()?;
        let rows = service::vault_list(&conn, limit, offset)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Decrypts one secret for the user to look at. Needs an open vault, and
    /// counts as activity, so reading defers the re-lock.
    pub fn vault_reveal(&self, id: String) -> Result<String, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let mut session = self.vault()?;
        Ok(service::vault_reveal(&conn, &mut session, &id, now)?)
    }

    /// Creates a secret: the body is encrypted before it reaches storage, and
    /// the row holds ciphertext with no plaintext column beside it.
    pub fn vault_create_secret(&self, draft: SnippetDraft) -> Result<Snippet, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let session = self.vault()?;
        Ok(service::vault_create_secret(&conn, &session, draft.into(), now)?.into())
    }

    /// Re-encrypts an edited secret. Every save produces a fresh envelope, so
    /// every save is a new version.
    pub fn vault_update_secret(&self, edit: SnippetEdit) -> Result<Snippet, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let session = self.vault()?;
        Ok(service::vault_update_secret(&conn, &session, edit.into(), now)?.into())
    }

    /// Promotes an ordinary snippet to a secret: encrypts what is there,
    /// rebuilds the index so the former plaintext leaves index storage, and
    /// drops the plaintext version history. A secret must not leave its own
    /// cleartext past behind.
    pub fn snippet_convert_to_sensitive(&self, id: String) -> Result<Snippet, CoreError> {
        let now = crate::service::now_ms()?;
        let conn = self.conn()?;
        let session = self.vault()?;
        Ok(service::snippet_convert_to_sensitive(&conn, &session, &id, now)?.into())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Deliberately fake secret body. Every leak assertion below greps for
    /// this exact string, in database bytes and in error text alike.
    const SECRET: &str = "AKIAFAKEEXAMPLE00000";
    const PASSWORD: &str = "correct-horse-t087";

    fn core() -> (TypviaCore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let core = TypviaCore::open(dir.path().to_string_lossy().to_string()).unwrap();
        (core, dir)
    }

    fn draft(title: &str, body: &str) -> SnippetDraft {
        SnippetDraft {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        }
    }

    /// Every byte the data directory holds, whichever file it landed in —
    /// the main database, the write-ahead log or the shared-memory index.
    fn stored_bytes(dir: &std::path::Path) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                bytes.extend(std::fs::read(&path).unwrap());
            }
        }
        bytes
    }

    #[test]
    fn a_new_vault_starts_uninitialized_and_locked() {
        let (core, _dir) = core();
        let status = core.vault_status().unwrap();
        assert!(!status.initialized);
        assert!(!status.unlocked);
        assert_eq!(status.unlocked_at, None);
    }

    #[test]
    fn initializing_leaves_the_vault_open_and_locking_closes_it_again() {
        let (core, _dir) = core();

        let opened = core.vault_initialize(PASSWORD.to_string()).unwrap();
        assert!(opened.initialized);
        assert!(opened.unlocked);
        assert!(opened.unlocked_at.is_some());

        let closed = core.vault_lock().unwrap();
        assert!(closed.initialized);
        assert!(!closed.unlocked);
        assert_eq!(closed.unlocked_at, None);
    }

    /// Red line: a wrong master password does not open the vault, and the
    /// failure says only that the unlock failed — not which part was wrong.
    #[test]
    fn a_wrong_master_password_does_not_open_the_vault() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_lock().unwrap();

        let error = core
            .vault_unlock_password("correct-horse-t088".to_string())
            .unwrap_err();

        assert_eq!(
            error,
            CoreError::PermissionDenied {
                reason: "unlock failed".to_string(),
            }
        );
        assert!(!core.vault_status().unwrap().unlocked);
    }

    /// Red line: a secret cannot be read while the vault is locked, whatever
    /// the caller asks for — and the refusal carries no content.
    #[test]
    fn a_locked_vault_refuses_to_reveal_and_says_nothing_about_the_secret() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        let secret = core
            .vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();
        core.vault_lock().unwrap();

        let error = core.vault_reveal(secret.id.clone()).unwrap_err();

        assert!(matches!(error, CoreError::PermissionDenied { .. }));
        assert!(!error.to_string().contains("AKIAFAKE"));
    }

    /// Red line: listing the vault yields metadata only. A list of secrets is
    /// not a list of their contents, open vault or not.
    #[test]
    fn listing_the_vault_carries_no_bodies_even_while_unlocked() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();

        let rows = core.vault_list(20, 0).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].body, None);
        assert_eq!(rows[0].security_level, "sensitive");
        // The record's own debug rendering is a leak path too.
        assert!(!format!("{rows:?}").contains("AKIAFAKE"));
    }

    /// Red line: a secret's body is not searchable. Not by content, not by
    /// any surface — the index knows its title and nothing else.
    #[test]
    fn a_secret_body_is_not_searchable_from_any_surface() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();

        for query in [SECRET, "AKIAFAKE", "AKIA"] {
            assert!(
                core.search_library(query.to_string(), 20)
                    .unwrap()
                    .is_empty()
            );
            assert!(core.search_all(query.to_string(), 20).unwrap().is_empty());
            assert!(
                core.search_snippets(query.to_string(), 20, 0)
                    .unwrap()
                    .is_empty()
            );
        }
        // The title is indexed, so the row is findable — as a locked row.
        let by_title = core.search_all("Deploy key".to_string(), 20).unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].body, None);
    }

    /// Red line, at the level that actually matters: the plaintext is not in
    /// the stored bytes. Not in the table, not in the FTS index, not in the
    /// write-ahead log.
    #[test]
    fn a_secret_body_never_appears_in_the_stored_bytes() {
        let (core, dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();

        let bytes = stored_bytes(dir.path());

        assert!(!bytes.is_empty(), "the fixture must have written something");
        assert!(
            !bytes
                .windows(SECRET.len())
                .any(|window| window == SECRET.as_bytes()),
            "the secret body reached storage in the clear"
        );
        // The master password is key input, not stored data.
        assert!(
            !bytes
                .windows(PASSWORD.len())
                .any(|window| window == PASSWORD.as_bytes()),
            "the master password reached storage"
        );
    }

    /// Promoting a snippet has to take its cleartext past with it: the body
    /// was in the table, the index and the version history a moment ago.
    ///
    /// Storage bytes are checked across **every file in the data directory**,
    /// not just the database. Deleting a row does not erase the page it was
    /// on, and the write-ahead log keeps its own copy of that page as it was;
    /// both files sit side by side on disk. Asserting only against
    /// `typvia.db` used to pass while the plaintext was still readable in
    /// `typvia.db-wal` a few bytes away.
    #[test]
    fn promoting_a_snippet_takes_its_cleartext_history_with_it() {
        let (core, dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        let mut plain = draft("Deploy key", SECRET);
        plain.snippet_type = "text".to_string();
        let created = core.snippet_create(plain).unwrap();
        assert_eq!(core.search_all(SECRET.to_string(), 20).unwrap().len(), 1);

        let promoted = core
            .snippet_convert_to_sensitive(created.id.clone())
            .unwrap();

        assert_eq!(promoted.security_level, "sensitive");
        assert_eq!(promoted.body, None);
        assert!(core.search_all(SECRET.to_string(), 20).unwrap().is_empty());
        // Only the ciphertext version survives; the plaintext ones are gone.
        let history = core.history_list(created.id, 20, 0).unwrap();
        assert_eq!(history.entries.len(), 1);
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            assert!(
                !bytes
                    .windows(SECRET.len())
                    .any(|window| window == SECRET.as_bytes()),
                "the former plaintext outlived the promotion in {}",
                path.display()
            );
        }
    }

    /// Where no gated store exists, biometric unlock reports that it is not
    /// set up and the master-password path stays open. Degrading is the
    /// requirement; failing the app is not.
    #[test]
    fn without_a_gated_store_biometric_unlock_degrades_to_the_password_path() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_lock().unwrap();

        let error = core.vault_unlock_biometric().unwrap_err();
        assert!(matches!(
            error,
            CoreError::Conflict { .. } | CoreError::System
        ));

        assert!(
            core.vault_unlock_password(PASSWORD.to_string())
                .unwrap()
                .unlocked
        );
    }

    /// The honest count for the confirm copy, available while locked —
    /// a number is not a content leak, and the user has to be told the truth
    /// before destroying anything.
    #[test]
    fn reset_preview_states_the_real_count_while_locked() {
        let (core, _dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();
        core.vault_create_secret(draft("Signing key", "another-fake-secret"))
            .unwrap();
        core.vault_lock().unwrap();

        assert_eq!(core.vault_reset_preview().unwrap(), 2);
    }

    #[test]
    fn a_reset_destroys_every_secret_and_leaves_no_ciphertext_behind() {
        let (core, dir) = core();
        core.vault_initialize(PASSWORD.to_string()).unwrap();
        core.vault_create_secret(draft("Deploy key", SECRET))
            .unwrap();
        core.vault_lock().unwrap();

        let status = core.vault_reset().unwrap();

        assert!(!status.initialized);
        assert!(!status.unlocked);
        assert!(core.vault_list(20, 0).unwrap().is_empty());
        let bytes = stored_bytes(dir.path());
        assert!(
            !bytes
                .windows(SECRET.len())
                .any(|window| window == SECRET.as_bytes())
        );
    }
}
