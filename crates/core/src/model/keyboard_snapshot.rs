//! The `KeyboardSnapshot` read model (PRD §15.10, format per DEC-007 #4).
//!
//! The snapshot is a versioned JSON document exported by the main app and
//! read-only for the iOS keyboard (App Group) and Android IME (private files
//! dir). Plain-snippet search data lives inside `encrypted_index`; snapshot
//! encryption and key handling are specified by docs/06_SECURITY_MODEL.md
//! (TASK-023), so the payload stays opaque bytes here.

use serde::{Deserialize, Serialize};

use super::validation::ValidationError;
use super::{DeviceId, FolderId, SnippetId, TimestampMs};

/// Current snapshot document version written by this build.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Folder display data the keyboard needs without touching the main library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderMetadata {
    pub id: FolderId,
    pub name: String,
    pub sort_order: i32,
}

/// Read-only snapshot consumed by keyboard/IME extensions (PRD §15.10).
///
/// Sensitive snippets contribute encrypted metadata only; nothing in this
/// document may allow recovering sensitive content without vault unlock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardSnapshot {
    /// Document format version; readers must reject versions they do not
    /// understand and fall back to their previous snapshot.
    pub snapshot_version: u32,
    pub generated_at: TimestampMs,
    /// Device that exported the snapshot.
    pub device_id: DeviceId,
    /// Encrypted payload holding the plain-snippet search index and the
    /// encrypted metadata of sensitive snippets (opaque until TASK-023).
    pub encrypted_index: Vec<u8>,
    /// Recently used snippet ids, most recent first.
    pub recent_ids: Vec<SnippetId>,
    /// Favorite snippet ids in display order.
    pub favorite_ids: Vec<SnippetId>,
    pub folder_metadata: Vec<FolderMetadata>,
}

impl KeyboardSnapshot {
    /// Validates a snapshot after deserialization, before an extension
    /// trusts it (external input rule: parse then validate).
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.snapshot_version == 0 {
            return Err(ValidationError::new("snapshot_version", "must be >= 1"));
        }
        if self.snapshot_version > SNAPSHOT_VERSION {
            return Err(ValidationError::new(
                "snapshot_version",
                "newer than this reader supports",
            ));
        }
        if self.device_id.is_empty() {
            return Err(ValidationError::new("device_id", "must not be empty"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn snapshot() -> KeyboardSnapshot {
        KeyboardSnapshot {
            snapshot_version: SNAPSHOT_VERSION,
            generated_at: 1_700_000_000_000,
            device_id: "d1".to_string(),
            encrypted_index: vec![0xC0, 0xDE],
            recent_ids: vec!["s2".to_string(), "s1".to_string()],
            favorite_ids: vec!["s3".to_string()],
            folder_metadata: vec![FolderMetadata {
                id: "f1".to_string(),
                name: "Shell".to_string(),
                sort_order: 0,
            }],
        }
    }

    #[test]
    fn serializes_to_json_and_back_without_loss() {
        let original = snapshot();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: KeyboardSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn json_uses_the_prd_field_names() {
        let json = serde_json::to_string(&snapshot()).unwrap();
        for field in [
            "snapshot_version",
            "generated_at",
            "device_id",
            "encrypted_index",
            "recent_ids",
            "favorite_ids",
            "folder_metadata",
        ] {
            assert!(json.contains(field), "missing field {field} in {json}");
        }
    }

    #[test]
    fn rejects_snapshot_from_a_newer_format_version() {
        let mut s = snapshot();
        s.snapshot_version = SNAPSHOT_VERSION + 1;
        assert_eq!(s.validate().unwrap_err().field, "snapshot_version");
    }

    #[test]
    fn rejects_snapshot_without_device_id() {
        let mut s = snapshot();
        s.device_id.clear();
        assert_eq!(s.validate().unwrap_err().field, "device_id");
    }

    #[test]
    fn accepts_a_current_version_snapshot() {
        assert_eq!(snapshot().validate(), Ok(()));
    }
}
