// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! API-key database red line: after configuring providers in a real
//! migrated database and storing a key through this crate's credential
//! path, the database file must not contain the key bytes anywhere. The
//! provider table has no column for a key, and the credential path only
//! ever writes to the secure store — this test pins both facts at the
//! byte level.

use std::cell::RefCell;
use std::collections::HashMap;

use typvia_ai::credentials::{entry_name, load_api_key, store_api_key};
use typvia_core::db::{migrate_to_latest, open};
use typvia_core::model::{AiProvider, AiProviderKind};
use typvia_core::repo::AiProviderRepo;
use typvia_crypto::{SecureStore, SecureStoreError};
use zeroize::Zeroizing;

const KEY_CANARY: &str = "sk-FAKE-dbredline-canary-77";

/// In-memory secure-store double (no platform I/O).
struct MemoryStore {
    entries: RefCell<HashMap<String, Vec<u8>>>,
}

impl MemoryStore {
    fn new() -> Self {
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

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
#[allow(clippy::unwrap_used)]
fn a_configured_provider_with_a_stored_key_leaves_no_key_bytes_in_the_database() {
    let dir = std::env::temp_dir().join(format!("typvia-ai-redline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("library.db");
    let _ = std::fs::remove_file(&db_path);

    {
        let mut conn = open(&db_path).unwrap();
        migrate_to_latest(&mut conn).unwrap();

        // Real flow: the provider row goes to the database, the key goes
        // to the secure store — the two never meet.
        let repo = AiProviderRepo::new(&conn);
        repo.insert(&AiProvider {
            id: "p1".to_string(),
            name: "Hosted".to_string(),
            kind: AiProviderKind::OpenAiCompatible,
            base_url: "https://api.example.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            timeout_ms: 30_000,
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();

        let store = MemoryStore::new();
        store_api_key(&store, "p1", KEY_CANARY).unwrap();
        assert_eq!(
            load_api_key(&store, "p1").unwrap().unwrap().expose(),
            KEY_CANARY
        );
        // The key landed only under the provider's secure-store entry.
        assert_eq!(store.entries.borrow().len(), 1);
        assert!(store.entries.borrow().contains_key(&entry_name("p1")));

        // Fold WAL content back into the main file before scanning.
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
    }

    let bytes = std::fs::read(&db_path).unwrap();
    // Scanner self-proof: values that DID go to the database are found.
    assert!(contains(&bytes, b"https://api.example.com/v1"));
    assert!(contains(&bytes, b"gpt-4o"));
    // Red line: the key is nowhere in the database file.
    assert!(!contains(&bytes, KEY_CANARY.as_bytes()));

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir(&dir);
}
