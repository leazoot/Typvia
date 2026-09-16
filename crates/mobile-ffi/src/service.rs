// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The main app's channel into the shared core.
//!
//! [`TypviaCore`] owns the one SQLite connection the process gets and hands
//! every call straight to `host-service`. Nothing here decides anything: no
//! validation, no transaction boundaries, no index policy, no encryption
//! order. That is the whole point of the layer — the same use cases the
//! desktop host calls run unchanged, and the platform above composes screens
//! out of the results.
//!
//! Threading. The connection is a single writer behind a mutex, matching the
//! database rules, so calls serialise. They are also synchronous and can hit
//! disk, which means the platform layer must keep them off the main thread;
//! `TypviaKit` does that by confining the object to one actor.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use typvia_core::vault::{SecureStore, VaultSession};
use typvia_host_service::service;

use crate::error::CoreError;
use crate::model::{
    Folder, History, LibraryCounts, SearchHit, Snippet, SnippetDraft, SnippetEdit, Tag,
    TemplateField, VersionBody,
};

/// The app's handle on its data.
///
/// One instance per process. Opening it twice against the same directory
/// would put two writers on one database, which the storage rules forbid.
#[derive(uniffi::Object)]
pub struct TypviaCore {
    // Shared so the sync round and the AI egress sink can take the same
    // single-writer lock on their own schedule, without a call holding it
    // across network I/O.
    conn: Arc<Mutex<Connection>>,
    /// Holds the master key while the vault is unlocked.
    vault: Mutex<VaultSession>,
    /// The platform's gated storage for key material: the Keychain on device,
    /// an unavailable stand-in elsewhere so a host without one still starts
    /// and still unlocks by password.
    secure_store: Box<dyn SecureStore + Send + Sync>,
    /// Device identity, the engine when an account is bound, and the
    /// write-path observer that seals changes into the outbox.
    sync: crate::sync::SyncState,
}

/// Wall clock in milliseconds. A clock before the epoch means the device
/// clock is unusable; every write stamps a timestamp, so that is a system
/// fault rather than something to paper over with a zero.
pub(crate) fn now_ms() -> Result<i64, CoreError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoreError::System)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| CoreError::System)
}

impl TypviaCore {
    pub(crate) fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CoreError> {
        // A poisoned mutex means an earlier call panicked mid-write. Report
        // it as a system fault rather than panicking every later caller.
        self.conn.lock().map_err(|_| CoreError::System)
    }

    pub(crate) fn vault(&self) -> Result<std::sync::MutexGuard<'_, VaultSession>, CoreError> {
        self.vault.lock().map_err(|_| CoreError::System)
    }

    pub(crate) fn secure_store(&self) -> &(dyn SecureStore + Send + Sync) {
        self.secure_store.as_ref()
    }

    pub(crate) fn sync(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, crate::sync::SyncHost>, CoreError> {
        Ok(self.sync.lock()?)
    }

    /// Where the sync host keeps its own small files, beside the database.
    pub(crate) fn sync_dir(&self) -> &Path {
        self.sync.data_dir()
    }

    /// The directory this instance was opened on — the database's own, and
    /// where the markers that belong to the install rather than to a row live.
    pub(crate) fn data_dir(&self) -> &Path {
        self.sync.data_dir()
    }

    /// A handle for the components that must take the connection lock on
    /// their own schedule — a sync round, the egress sink. Never called while
    /// already holding [`Self::conn`].
    pub(crate) fn conn_handle(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    fn open_with(
        data_dir: String,
        secure_store: Box<dyn typvia_core::vault::SecureStore + Send + Sync + 'static>,
    ) -> Result<Self, CoreError> {
        let dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&dir).map_err(|_| CoreError::System)?;
        let mut conn =
            typvia_core::db::open(&dir.join("typvia.db")).map_err(|_| CoreError::System)?;
        typvia_core::db::migrate_to_latest(&mut conn).map_err(|_| CoreError::System)?;
        // The sync host registers this install's device identity and the
        // seal-at-write observer against the one connection the process owns,
        // before that connection moves into the shared state. A host without
        // usable key storage stays unavailable rather than failing startup.
        let device_id = service::load_or_create_device_id(&dir)?;
        let sync_host = crate::sync::SyncHost::start(
            &conn,
            secure_store.as_ref(),
            device_id,
            service::current_platform(),
            now_ms()?,
        );
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            vault: Mutex::new(VaultSession::new()),
            secure_store,
            sync: crate::sync::SyncState::new(sync_host, dir),
        })
    }
}

#[uniffi::export]
impl TypviaCore {
    /// Opens (creating on first run) the database under `data_dir` and brings
    /// it to the current schema. A migration failure leaves the file
    /// untouched and surfaces here rather than half-applied.
    #[uniffi::constructor]
    pub fn open(data_dir: String) -> Result<Self, CoreError> {
        Self::open_with(data_dir, crate::secure_store::platform_secure_store())
    }

    /// The same, on a host that keeps the key store on its own side.
    ///
    /// Android's Keystore is Java: this crate cannot reach it, and a device
    /// whose keys have nowhere to live has no sync at all — no account, no
    /// pairing, no round. So the host hands its own store in, and everything
    /// above this line stops caring which platform it is on.
    ///
    /// iOS does not use this: its Keychain is a C API reachable from here,
    /// and one way in per platform is one thing that can be wrong.
    #[uniffi::constructor]
    #[cfg(not(target_os = "ios"))]
    pub fn open_with_key_keeper(
        data_dir: String,
        keeper: std::sync::Arc<dyn crate::secure_store::KeyKeeper>,
    ) -> Result<Self, CoreError> {
        Self::open_with(
            data_dir,
            Box::new(crate::secure_store::HostKeyStore::new(keeper)),
        )
    }

    // ===== Snippets =====

    pub fn snippet_create(&self, draft: SnippetDraft) -> Result<Snippet, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_create(&conn, draft.into(), now)?.into())
    }

    pub fn snippet_update(&self, edit: SnippetEdit) -> Result<Snippet, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_update(&conn, edit.into(), now)?.into())
    }

    pub fn snippet_get(&self, id: String) -> Result<Snippet, CoreError> {
        let conn = self.conn()?;
        Ok(service::snippet_get(&conn, &id)?.into())
    }

    /// Records one use, after this host has actually delivered the words.
    ///
    /// Called by the app when a snippet is copied or taken out — never
    /// speculatively. The keyboard cannot call it: an input method here does
    /// not open the database, it reads a snapshot, and that red line is worth
    /// more than a complete usage count.
    pub fn snippet_record_use(&self, id: String) -> Result<(), CoreError> {
        let conn = self.conn()?;
        Ok(service::snippet_record_use(&conn, &id, now_ms()?)?)
    }

    /// One page of a library view. `view` is the chapter vocabulary
    /// (`all` | `recent` | `starred` | `unsorted` | `folder`); `folder_id`
    /// belongs to the folder view and to no other.
    pub fn snippet_list_page(
        &self,
        view: String,
        folder_id: Option<String>,
        snippet_type: Option<String>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Snippet>, CoreError> {
        let conn = self.conn()?;
        let rows = service::snippet_list_page(
            &conn,
            &view,
            folder_id.as_deref(),
            snippet_type.as_deref(),
            // The mobile chapters keep each view's own order.
            None,
            limit,
            offset,
        )?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Exact row count behind one page query, so a list can size its scroll
    /// range without walking the table.
    pub fn snippet_count(
        &self,
        view: String,
        folder_id: Option<String>,
        snippet_type: Option<String>,
    ) -> Result<u32, CoreError> {
        let conn = self.conn()?;
        Ok(service::snippet_count(
            &conn,
            &view,
            folder_id.as_deref(),
            snippet_type.as_deref(),
        )?)
    }

    pub fn library_counts(&self) -> Result<LibraryCounts, CoreError> {
        let conn = self.conn()?;
        Ok(service::library_counts(&conn)?.into())
    }

    pub fn snippet_trash(&self, id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_trash(&conn, &id, now)?)
    }

    pub fn snippet_restore(&self, id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_restore(&conn, &id, now)?)
    }

    pub fn snippet_delete_forever(&self, id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_delete_forever(&conn, &id, now)?)
    }

    pub fn snippet_batch_move(
        &self,
        ids: Vec<String>,
        folder_id: Option<String>,
    ) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_batch_move(
            &conn,
            &ids,
            folder_id.as_deref(),
            now,
        )?)
    }

    pub fn snippet_batch_add_tag(&self, ids: Vec<String>, tag_id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_batch_add_tag(&conn, &ids, &tag_id, now)?)
    }

    pub fn snippet_batch_trash(&self, ids: Vec<String>) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::snippet_batch_trash(&conn, &ids, now)?)
    }

    // ===== Recycle bin =====

    pub fn trash_list(&self, limit: u32, offset: u32) -> Result<Vec<Snippet>, CoreError> {
        let conn = self.conn()?;
        let rows = service::trash_list(&conn, limit, offset)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Drops trashed rows past the retention window. Returns how many went.
    pub fn trash_purge_expired(&self) -> Result<u32, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let purged = service::trash_purge_expired(&conn, now)?;
        u32::try_from(purged).map_err(|_| CoreError::System)
    }

    /// Sweeps expired temporary snippets into the recycle bin along the same
    /// path a manual trash takes. Returns how many expired.
    pub fn temporary_expire(&self) -> Result<u32, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let expired = service::temporary_expire(&conn, now)?;
        u32::try_from(expired).map_err(|_| CoreError::System)
    }

    // ===== Folders and tags =====

    pub fn folder_create(
        &self,
        name: String,
        parent_id: Option<String>,
        sort_order: i32,
    ) -> Result<Folder, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let input = typvia_host_service::dto::FolderCreateInput {
            name,
            parent_id,
            sort_order,
        };
        Ok(service::folder_create(&conn, input, now)?.into())
    }

    pub fn folder_update(
        &self,
        id: String,
        name: String,
        parent_id: Option<String>,
        sort_order: i32,
    ) -> Result<Folder, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let input = typvia_host_service::dto::FolderUpdateInput {
            id,
            name,
            parent_id,
            sort_order,
        };
        Ok(service::folder_update(&conn, input, now)?.into())
    }

    pub fn folder_delete(&self, id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::folder_delete(&conn, &id, now)?)
    }

    /// Direct children of `parent_id`, or the top level when it is absent.
    pub fn folder_list_children(
        &self,
        parent_id: Option<String>,
    ) -> Result<Vec<Folder>, CoreError> {
        let conn = self.conn()?;
        let rows = service::folder_list_children(&conn, parent_id.as_deref())?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub fn tag_create(&self, name: String) -> Result<Tag, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::tag_create(&conn, name, now)?.into())
    }

    pub fn tag_list(&self) -> Result<Vec<Tag>, CoreError> {
        let conn = self.conn()?;
        let rows = service::tag_list(&conn)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub fn tag_rename(&self, id: String, name: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::tag_rename(&conn, &id, &name, now)?)
    }

    pub fn tag_delete(&self, id: String) -> Result<(), CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        Ok(service::tag_delete(&conn, &id, now)?)
    }

    // ===== Search =====

    /// Ranked hits with their match tier — titles and ids only, which is what
    /// a compact result row needs.
    pub fn search_snippets(
        &self,
        query: String,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<SearchHit>, CoreError> {
        let conn = self.conn()?;
        let hits = service::search_snippets(&conn, &query, limit, offset)?;
        Ok(hits.into_iter().map(Into::into).collect())
    }

    /// Ranked full rows for the library's own search. Sensitive snippets stay
    /// out: the vault is their only list.
    pub fn search_library(&self, query: String, limit: u32) -> Result<Vec<Snippet>, CoreError> {
        let conn = self.conn()?;
        let rows = service::search_library(&conn, &query, limit)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Ranked full rows for the home search, where a sensitive title hit does
    /// appear — as a locked row whose body is already absent, never content.
    pub fn search_all(&self, query: String, limit: u32) -> Result<Vec<Snippet>, CoreError> {
        let conn = self.conn()?;
        let rows = service::search_all(&conn, &query, limit)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ===== Version history =====

    /// Newest first, metadata only. A sensitive snippet's history is visible
    /// only on an unlocked vault — locked shows no version data at all, not
    /// even titles or timestamps.
    pub fn history_list(&self, id: String, limit: u32, offset: u32) -> Result<History, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        Ok(service::history_list(&conn, &mut vault, &id, limit, offset, now)?.into())
    }

    pub fn history_get(&self, id: String, version: u32) -> Result<VersionBody, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        Ok(service::history_get(&conn, &mut vault, &id, version, now)?.into())
    }

    /// Restores an old version by writing it forward: restoring v9 creates a
    /// new v10 and v9 survives.
    pub fn history_restore(&self, id: String, version: u32) -> Result<Snippet, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let mut vault = self.vault()?;
        Ok(service::history_restore(&conn, &mut vault, &id, version, now)?.into())
    }

    // ===== Templates =====

    /// Distinct `{{variable}}` names in a body, in first-seen order. Parsing
    /// stays in the shared engine; the editor asks rather than scanning.
    pub fn template_variables(&self, body: String) -> Result<Vec<String>, CoreError> {
        Ok(service::template_variables(&body)?)
    }

    pub fn template_fields(&self, snippet_id: String) -> Result<Vec<TemplateField>, CoreError> {
        let conn = self.conn()?;
        let fields = service::template_fields(&conn, &snippet_id)?;
        Ok(fields.into_iter().map(Into::into).collect())
    }

    /// Replaces a snippet's fields as one set, transactionally.
    pub fn template_save_fields(
        &self,
        snippet_id: String,
        fields: Vec<TemplateField>,
    ) -> Result<Vec<TemplateField>, CoreError> {
        let now = now_ms()?;
        let conn = self.conn()?;
        let dtos = fields.into_iter().map(Into::into).collect();
        let stored = service::template_save_fields(&conn, &snippet_id, dtos, now)?;
        Ok(stored.into_iter().map(Into::into).collect())
    }

    /// Lenient authoring preview: unfilled fields show a placeholder and a
    /// secret reference shows a fixed mask, never its plaintext.
    pub fn template_preview(
        &self,
        body: String,
        fields: Vec<TemplateField>,
        values: std::collections::HashMap<String, String>,
    ) -> Result<String, CoreError> {
        let dtos = fields.into_iter().map(Into::into).collect();
        Ok(service::template_preview(&body, dtos, values)?)
    }

    /// Exact render for insertion. Unlike the preview, a missing required
    /// field or an unresolved reference is an error rather than a placeholder.
    pub fn template_render(
        &self,
        id: String,
        values: std::collections::HashMap<String, String>,
    ) -> Result<String, CoreError> {
        let conn = self.conn()?;
        Ok(service::template_render(&conn, &id, &values)?)
    }

    // ===== First run =====

    /// Whether the first-run flow has been finished or skipped on this device.
    ///
    /// The marker is the desktop's: a file beside the database, so a fresh
    /// install always sees the flow and a deleted data directory brings it
    /// back. Keeping it out of user defaults matters here — defaults survive
    /// a reinstall on iOS when the app group does, and a reinstalled app that
    /// silently skips its own introduction is one that cannot be re-tried.
    pub fn onboarding_completed(&self) -> bool {
        service::onboarding_completed(self.data_dir())
    }

    /// Records that the reader is through. Skipping counts: the flow is not a
    /// gate, and a reader who chose to go straight in has been introduced.
    pub fn mark_onboarding_complete(&self) -> Result<(), CoreError> {
        Ok(service::mark_onboarding_complete(self.data_dir())?)
    }

    // ===== Sensitive detection =====

    /// Offline advisory scan of text being written. Returns the kinds found
    /// and never the matched text, so the result is safe to render and safe
    /// to cross this boundary.
    pub fn detect_sensitive(&self, text: String) -> Vec<String> {
        service::detect_sensitive(&text)
    }

    // ===== The library's way in and out =====
    //
    // Both directions are the shared layer's acts, asked for rather than
    // imitated: what a format is, what may be imported, what a backup holds
    // and what restoring one requires are decided in one place, so the three
    // hosts cannot come to disagree about the same file.

    /// Imports the text of a Markdown, JSON, CSV, CopyQ or massCode file as
    /// ordinary snippets.
    ///
    /// A trigger the library already uses is reported as a conflict rather
    /// than overwriting what is there; entries that cannot be imported are
    /// reported with a reason. The whole batch is one transaction, so a
    /// failure leaves the library exactly as it was.
    pub fn snippets_import(
        &self,
        format: String,
        text: String,
    ) -> Result<crate::model::ImportReport, CoreError> {
        let conn = self.conn()?;
        Ok(service::snippets_import(&conn, &format, &text, now_ms()?)?.into())
    }

    /// Exports the whole library — snippets, the bin, folders, tags, template
    /// fields, version history and the vault's wrapped keys — as the text of
    /// one encrypted file.
    ///
    /// The passphrase crosses once, derives the backup key here and is
    /// dropped: it is never stored and never logged. What comes back is
    /// ciphertext and key-derivation parameters, and nothing else — sensitive
    /// bodies stay in their vault envelopes inside it, undecrypted.
    pub fn backup_export(&self, passphrase: String) -> Result<String, CoreError> {
        let conn = self.conn()?;
        Ok(service::backup_export(&conn, &passphrase, now_ms()?)?)
    }

    /// Restores an encrypted backup into an **empty** library.
    ///
    /// Restore never merges: a library that already holds something refuses,
    /// because merging two libraries silently is how a reader loses the one
    /// they still wanted. A wrong passphrase, tampering and damage all fail
    /// the same way — telling them apart would tell an attacker which of the
    /// three they achieved.
    pub fn backup_restore(
        &self,
        passphrase: String,
        text: String,
    ) -> Result<crate::model::BackupRestored, CoreError> {
        let conn = self.conn()?;
        Ok(service::backup_restore(&conn, &passphrase, &text, now_ms()?)?.into())
    }
}

/// How long the recycle bin keeps a thrown-away snippet, in whole days.
///
/// Derived from the core's own retention rather than restated. A screen that
/// prints "22 days left" is quoting a rule it does not own, and a copy of that
/// rule is a copy that can drift — at which point the screen is confidently
/// misstating a deadline.
#[uniffi::export]
pub fn trash_retention_days() -> u32 {
    let days = typvia_core::repo::TRASH_RETENTION_MS / 86_400_000;
    u32::try_from(days).unwrap_or(u32::MAX)
}

/// The marker a Typvia backup file carries in the clear, in its outer JSON.
///
/// Exported because a host has to tell a backup apart from an ordinary import
/// file **before** it knows which question to ask the reader — and a host that
/// keeps its own copy of the word will one day be looking for a marker the
/// core stopped writing.
#[uniffi::export]
pub fn backup_format_marker() -> String {
    typvia_core::backup::BACKUP_FORMAT.to_string()
}

/// The largest import file the core will parse, in bytes.
///
/// A host reads the file before handing the text over, so it needs the same
/// ceiling — otherwise a phone pulls a gigabyte into memory only to be told
/// the core would never have taken it.
#[uniffi::export]
pub fn import_max_bytes() -> u64 {
    u64::try_from(typvia_core::import::MAX_IMPORT_BYTES).unwrap_or(u64::MAX)
}

/// The largest backup file the core will open, in bytes. Same reason as
/// [`import_max_bytes`], different ceiling — a backup carries a whole library
/// and is allowed to be much larger than an import file.
#[uniffi::export]
pub fn backup_max_bytes() -> u64 {
    u64::try_from(typvia_core::backup::BACKUP_MAX_BYTES).unwrap_or(u64::MAX)
}

/// The shortest master password a new vault will be built on.
///
/// Exported for the same reason as the retention above: the setup screen has
/// to say the number out loud, and a screen that keeps its own copy of a rule
/// will one day ask for eight characters while the vault accepts six.
#[uniffi::export]
pub fn master_password_min_length() -> u32 {
    u32::try_from(typvia_core::vault::MASTER_PASSWORD_MIN_LEN).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn core() -> (TypviaCore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let core = TypviaCore::open(dir.path().to_string_lossy().to_string()).unwrap();
        (core, dir)
    }

    fn draft(title: &str, body: &str, trigger: Option<&str>) -> SnippetDraft {
        SnippetDraft {
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: "command".to_string(),
            description: None,
            folder_id: None,
            trigger: trigger.map(str::to_string),
            trigger_mode: trigger.map(|_| "delimiter".to_string()),
            language: None,
        }
    }

    fn field(name: &str) -> TemplateField {
        TemplateField {
            id: String::new(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: "single_line_text".to_string(),
            default_value: None,
            options: Vec::new(),
            validation: None,
            is_required: false,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    fn edit(snippet: &Snippet, title: &str, body: &str) -> SnippetEdit {
        SnippetEdit {
            id: snippet.id.clone(),
            title: title.to_string(),
            body: body.to_string(),
            snippet_type: snippet.snippet_type.clone(),
            description: snippet.description.clone(),
            folder_id: snippet.folder_id.clone(),
            trigger: snippet.trigger.clone(),
            trigger_mode: snippet.trigger_mode.clone(),
            language: snippet.language.clone(),
            is_favorite: snippet.is_favorite,
            is_pinned: snippet.is_pinned,
            is_enabled: snippet.is_enabled,
        }
    }

    #[test]
    fn a_created_snippet_reads_back_with_its_assigned_identity() {
        let (core, _dir) = core();

        let created = core
            .snippet_create(draft("Tail logs", "docker logs -f app", Some(";dlog")))
            .unwrap();

        assert!(!created.id.is_empty());
        assert_eq!(created.version, 1);
        assert_eq!(created.security_level, "normal");
        assert_eq!(core.snippet_get(created.id.clone()).unwrap(), created);
    }

    #[test]
    fn editing_the_body_bumps_the_version_and_records_history() {
        let (core, _dir) = core();
        let created = core
            .snippet_create(draft("Tail logs", "docker logs -f app", None))
            .unwrap();

        let updated = core
            .snippet_update(edit(&created, "Tail logs", "docker logs -f web"))
            .unwrap();

        assert_eq!(updated.version, 2);
        let history = core.history_list(created.id.clone(), 20, 0).unwrap();
        assert_eq!(history.current, 2);
        assert_eq!(history.entries.len(), 2);
        assert_eq!(
            core.history_get(created.id, 1).unwrap().body,
            "docker logs -f app"
        );
    }

    #[test]
    fn a_duplicate_trigger_is_a_conflict_naming_the_rule_only() {
        let (core, _dir) = core();
        core.snippet_create(draft("Tail logs", "docker logs", Some(";dlog")))
            .unwrap();

        let error = core
            .snippet_create(draft("Pod logs", "kubectl logs", Some(";dlog")))
            .unwrap_err();

        assert_eq!(
            error,
            CoreError::Conflict {
                reason: "trigger already in use".to_string(),
            }
        );
    }

    #[test]
    fn an_unknown_id_is_not_found_rather_than_an_internal_error() {
        let (core, _dir) = core();
        assert_eq!(
            core.snippet_get("no-such-id".to_string()).unwrap_err(),
            CoreError::NotFound
        );
    }

    #[test]
    fn an_unknown_library_view_is_a_correctable_validation_error() {
        let (core, _dir) = core();
        let error = core
            .snippet_list_page("chapters".to_string(), None, None, 20, 0)
            .unwrap_err();
        assert_eq!(
            error,
            CoreError::Validation {
                reason: "unknown library view".to_string(),
            }
        );
    }

    /// The vault setup screen prints the minimum in a sentence, so the number
    /// it prints has to be the one the vault refuses below.
    #[test]
    fn the_minimum_the_setup_screen_prints_is_the_one_the_vault_enforces() {
        assert_eq!(
            master_password_min_length() as usize,
            typvia_core::vault::MASTER_PASSWORD_MIN_LEN
        );
    }

    /// The recycle-bin screen prints "N days left" and must be quoting the
    /// rule the purge actually applies, not a number of its own.
    #[test]
    fn the_retention_the_screen_prints_is_the_one_the_core_enforces() {
        assert_eq!(
            i64::from(trash_retention_days()) * 86_400_000,
            typvia_core::repo::TRASH_RETENTION_MS
        );
    }

    /// The home screen's trigger lines ask for this view by name. If the
    /// bridge did not know the name, the whole shelf would fail to load and
    /// the screen would report a database it could not read — a long way from
    /// the actual mistake.
    #[test]
    fn the_usage_ordered_view_is_part_of_the_bridge_vocabulary() {
        let (core, _dir) = core();

        let rows = core
            .snippet_list_page("used".to_string(), None, None, 20, 0)
            .unwrap();

        assert!(rows.is_empty(), "nothing has been used in a fresh library");
    }

    #[test]
    fn trashing_moves_a_snippet_out_of_the_library_and_restoring_brings_it_back() {
        let (core, _dir) = core();
        let created = core
            .snippet_create(draft("Tail logs", "docker logs", None))
            .unwrap();

        core.snippet_trash(created.id.clone()).unwrap();
        assert_eq!(core.library_counts().unwrap().total, 0);
        assert_eq!(core.library_counts().unwrap().trash, 1);
        let trashed = core.trash_list(20, 0).unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].id, created.id);

        core.snippet_restore(created.id.clone()).unwrap();
        assert_eq!(core.library_counts().unwrap().total, 1);
        assert_eq!(core.library_counts().unwrap().trash, 0);
    }

    #[test]
    fn search_finds_a_snippet_by_title_and_reports_its_match_tier() {
        let (core, _dir) = core();
        let created = core
            .snippet_create(draft("Tail logs", "docker logs -f app", None))
            .unwrap();

        let hits = core
            .search_snippets("Tail logs".to_string(), 20, 0)
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet_id, created.id);
        assert_eq!(hits[0].tier, "title_exact");
        assert!(!hits[0].is_sensitive);
    }

    #[test]
    fn folders_and_tags_round_trip_through_the_bridge() {
        let (core, _dir) = core();

        let folder = core.folder_create("Shell".to_string(), None, 0).unwrap();
        assert_eq!(
            core.folder_list_children(None).unwrap(),
            vec![folder.clone()]
        );
        let renamed = core
            .folder_update(folder.id.clone(), "Terminal".to_string(), None, 1)
            .unwrap();
        assert_eq!(renamed.name, "Terminal");
        core.folder_delete(folder.id).unwrap();
        assert!(core.folder_list_children(None).unwrap().is_empty());

        let tag = core.tag_create("work".to_string()).unwrap();
        assert_eq!(core.tag_list().unwrap(), vec![tag.clone()]);
        core.tag_rename(tag.id.clone(), "office".to_string())
            .unwrap();
        assert_eq!(core.tag_list().unwrap()[0].name, "office");
        core.tag_delete(tag.id).unwrap();
        assert!(core.tag_list().unwrap().is_empty());
    }

    #[test]
    fn template_variables_and_render_come_from_the_shared_engine() {
        let (core, _dir) = core();
        let created = core
            .snippet_create(draft("Greeting", "Hi {{name}}, see {{pr}}.", None))
            .unwrap();

        assert_eq!(
            core.template_variables("Hi {{name}}, see {{pr}}.".to_string())
                .unwrap(),
            vec!["name", "pr"]
        );
        // A strict render resolves against saved fields, so the template has
        // to be declared before it can be filled.
        core.template_save_fields(created.id.clone(), vec![field("name"), field("pr")])
            .unwrap();
        let values = std::collections::HashMap::from([
            ("name".to_string(), "Lin".to_string()),
            ("pr".to_string(), "#12".to_string()),
        ]);
        assert_eq!(
            core.template_render(created.id, values).unwrap(),
            "Hi Lin, see #12."
        );
    }

    /// Red line: the advisory scan reports kinds, never the matched text.
    /// The fixture key is a deliberate fake so the assertion can grep for it.
    #[test]
    fn sensitive_detection_names_kinds_and_never_echoes_the_match() {
        let (core, _dir) = core();

        let kinds = core.detect_sensitive("aws key AKIAFAKEEXAMPLE00000 here".to_string());

        assert_eq!(kinds, vec!["aws_access_key".to_string()]);
        assert!(
            !format!("{kinds:?}").contains("AKIAFAKE"),
            "a detector result must not carry the matched text"
        );
    }
}
