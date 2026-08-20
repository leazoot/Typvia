//! The `KeyboardSnapshot` read model.
//!
//! The snapshot is a versioned JSON document exported by the main app and
//! read-only for the iOS keyboard (App Group) and Android IME (private files
//! dir). Normal snippets carry the plaintext fields the keyboard needs for
//! offline search and insertion; sensitive snippets contribute nothing beyond
//! their id and the stored content-ciphertext envelope, which extensions
//! cannot decrypt.

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

/// One snippet entry inside a snapshot.
///
/// Serialized untagged: the two variants have disjoint required key sets
/// (`body` vs `encrypted_metadata`), so the JSON shape itself discriminates
/// them and a sensitive entry carries no extra tag field.
///
/// Red line: a sensitive entry must never expose title, trigger, body, or any
/// other plaintext clue — only the id and the opaque envelope bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnapshotSnippet {
    /// Normal-security snippet with the plaintext fields the keyboard needs.
    Normal {
        id: SnippetId,
        title: String,
        /// Controlled TEXT value of `SnippetType`; kept as text so readers
        /// degrade explicitly on values they do not know.
        snippet_type: String,
        trigger: Option<String>,
        /// Controlled TEXT value of `TriggerMode`; set together with
        /// `trigger`.
        trigger_mode: Option<String>,
        folder_id: Option<FolderId>,
        is_favorite: bool,
        body: String,
    },
    /// Sensitive snippet: id plus the stored content-ciphertext envelope,
    /// opaque to extensions (no vault key ever reaches them).
    Sensitive {
        id: SnippetId,
        encrypted_metadata: Vec<u8>,
    },
}

/// Read-only snapshot consumed by keyboard/IME extensions.
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
    /// Live, enabled snippets only; trashed and disabled rows never enter
    /// a snapshot.
    pub snippets: Vec<SnapshotSnippet>,
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

    fn normal_entry() -> SnapshotSnippet {
        SnapshotSnippet::Normal {
            id: "s1".to_string(),
            title: "Docker logs".to_string(),
            snippet_type: "command".to_string(),
            trigger: Some(":dlog".to_string()),
            trigger_mode: Some("delimiter".to_string()),
            folder_id: Some("f1".to_string()),
            is_favorite: true,
            body: "docker logs -f app".to_string(),
        }
    }

    fn sensitive_entry() -> SnapshotSnippet {
        SnapshotSnippet::Sensitive {
            id: "s9".to_string(),
            encrypted_metadata: vec![0xC0, 0xDE],
        }
    }

    fn snapshot() -> KeyboardSnapshot {
        KeyboardSnapshot {
            snapshot_version: SNAPSHOT_VERSION,
            generated_at: 1_700_000_000_000,
            device_id: "d1".to_string(),
            snippets: vec![normal_entry(), sensitive_entry()],
            recent_ids: vec!["s2".to_string(), "s1".to_string()],
            favorite_ids: vec!["s3".to_string()],
            folder_metadata: vec![FolderMetadata {
                id: "f1".to_string(),
                name: "Shell".to_string(),
                sort_order: 0,
            }],
        }
    }

    fn object_keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::clone)
            .collect();
        keys.sort();
        keys
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
            "snippets",
            "recent_ids",
            "favorite_ids",
            "folder_metadata",
        ] {
            assert!(json.contains(field), "missing field {field} in {json}");
        }
    }

    #[test]
    fn a_normal_entry_serializes_exactly_the_plaintext_field_set() {
        let value = serde_json::to_value(normal_entry()).unwrap();
        let mut expected = vec![
            "id",
            "title",
            "snippet_type",
            "trigger",
            "trigger_mode",
            "folder_id",
            "is_favorite",
            "body",
        ];
        expected.sort_unstable();
        assert_eq!(object_keys(&value), expected);
    }

    #[test]
    fn a_sensitive_entry_serializes_only_id_and_encrypted_metadata() {
        let value = serde_json::to_value(sensitive_entry()).unwrap();
        assert_eq!(object_keys(&value), vec!["encrypted_metadata", "id"]);
    }

    #[test]
    fn entry_variants_are_told_apart_by_shape_when_parsing() {
        let normal: SnapshotSnippet =
            serde_json::from_value(serde_json::to_value(normal_entry()).unwrap()).unwrap();
        assert_eq!(normal, normal_entry());
        let sensitive: SnapshotSnippet =
            serde_json::from_value(serde_json::to_value(sensitive_entry()).unwrap()).unwrap();
        assert_eq!(sensitive, sensitive_entry());
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
