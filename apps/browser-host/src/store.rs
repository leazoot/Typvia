//! Read-only snapshot access with mtime-based lazy reload: the desktop app
//! is the only writer, this host re-reads the file when its metadata changes
//! and keeps serving the previous parse otherwise. Sensitive entries are
//! dropped at load — their ciphertext never even reaches the query layer.

use std::path::PathBuf;
use std::time::SystemTime;

use typvia_core::model::{KeyboardSnapshot, SnapshotSnippet};

/// One servable snippet: the Normal snapshot variant, flattened.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub snippet_type: String,
    pub trigger: Option<String>,
    pub is_favorite: bool,
    pub body: String,
}

/// The parsed, filtered view this host serves queries from.
#[derive(Debug, Default)]
pub struct View {
    pub generated_at: i64,
    pub entries: Vec<Entry>,
    pub recent_ids: Vec<String>,
    pub favorite_ids: Vec<String>,
}

struct Cached {
    modified: Option<SystemTime>,
    len: u64,
    view: View,
}

/// Lazy snapshot loader keyed on the file's (mtime, len) pair.
pub struct SnapshotStore {
    path: PathBuf,
    cached: Option<Cached>,
}

/// Why a snapshot could not be served; mapped to stable error codes.
#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    /// No snapshot file: integration off or never enabled.
    Unavailable,
    /// The file exists but does not parse/validate as a snapshot we read.
    Invalid,
}

impl SnapshotStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, cached: None }
    }

    /// Returns the current view, reloading if the file changed. A reload
    /// failure with a previously good parse keeps serving the old view
    /// (the writer replaces atomically, so a torn read resolves next call).
    pub fn view(&mut self) -> Result<&View, StoreError> {
        let metadata = std::fs::metadata(&self.path).map_err(|_| StoreError::Unavailable)?;
        let modified = metadata.modified().ok();
        let len = metadata.len();
        let unchanged = self
            .cached
            .as_ref()
            .is_some_and(|cached| cached.modified == modified && cached.len == len);
        if !unchanged {
            match load(&self.path) {
                Ok(view) => {
                    self.cached = Some(Cached {
                        modified,
                        len,
                        view,
                    });
                }
                Err(error) => {
                    if self.cached.is_none() {
                        return Err(error);
                    }
                }
            }
        }
        match &self.cached {
            Some(cached) => Ok(&cached.view),
            None => Err(StoreError::Unavailable),
        }
    }
}

fn load(path: &std::path::Path) -> Result<View, StoreError> {
    let bytes = std::fs::read(path).map_err(|_| StoreError::Unavailable)?;
    let snapshot: KeyboardSnapshot =
        serde_json::from_slice(&bytes).map_err(|_| StoreError::Invalid)?;
    snapshot.validate().map_err(|_| StoreError::Invalid)?;
    let entries = snapshot
        .snippets
        .into_iter()
        .filter_map(|entry| match entry {
            SnapshotSnippet::Normal {
                id,
                title,
                snippet_type,
                trigger,
                trigger_mode: _,
                folder_id: _,
                is_favorite,
                body,
            } => Some(Entry {
                id,
                title,
                snippet_type,
                trigger,
                is_favorite,
                body,
            }),
            // Sensitive rows never reach the query layer.
            SnapshotSnippet::Sensitive { .. } => None,
        })
        .collect();
    Ok(View {
        generated_at: snapshot.generated_at,
        entries,
        recent_ids: snapshot.recent_ids,
        favorite_ids: snapshot.favorite_ids,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_fixture(dir: &std::path::Path, generated_at: i64, body: &str) -> PathBuf {
        let path = dir.join("snapshot.json");
        let doc = serde_json::json!({
            "snapshot_version": 1,
            "generated_at": generated_at,
            "device_id": "d1",
            "snippets": [
                {
                    "id": "s1",
                    "title": "Tail logs",
                    "snippet_type": "command",
                    "trigger": ";dlog",
                    "trigger_mode": "delimiter",
                    "folder_id": null,
                    "is_favorite": false,
                    "body": body,
                },
                { "id": "s9", "encrypted_metadata": [1, 2, 3] },
            ],
            "recent_ids": ["s1"],
            "favorite_ids": [],
            "folder_metadata": [],
        });
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        path
    }

    #[test]
    fn loads_normal_entries_and_drops_sensitive_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(dir.path(), 1, "docker logs -f app");
        let mut store = SnapshotStore::new(path);

        let view = store.view().unwrap();
        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].id, "s1");
    }

    #[test]
    fn a_missing_file_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SnapshotStore::new(dir.path().join("snapshot.json"));
        assert_eq!(store.view().unwrap_err(), StoreError::Unavailable);
    }

    #[test]
    fn an_unparseable_file_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot.json");
        std::fs::write(&path, b"not json").unwrap();
        let mut store = SnapshotStore::new(path);
        assert_eq!(store.view().unwrap_err(), StoreError::Invalid);
    }

    #[test]
    fn a_newer_format_version_is_invalid_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "snapshot_version": 99,
                "generated_at": 1,
                "device_id": "d1",
                "snippets": [],
                "recent_ids": [],
                "favorite_ids": [],
                "folder_metadata": [],
            }))
            .unwrap(),
        )
        .unwrap();
        let mut store = SnapshotStore::new(path);
        assert_eq!(store.view().unwrap_err(), StoreError::Invalid);
    }

    #[test]
    fn reloads_when_the_file_changes_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(dir.path(), 1, "old body");
        let mut store = SnapshotStore::new(path.clone());
        assert_eq!(store.view().unwrap().entries[0].body, "old body");

        // A rewritten file (new length ensures a metadata change even on
        // coarse mtime filesystems) is picked up on the next call.
        write_fixture(dir.path(), 2, "new body that is longer");
        assert_eq!(
            store.view().unwrap().entries[0].body,
            "new body that is longer"
        );
    }
}
