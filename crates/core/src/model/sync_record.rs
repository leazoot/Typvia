// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The `SyncRecord` entity.

use super::enums::SyncEntityType;
use super::validation::ValidationError;
use super::{DeviceId, SyncRecordId, TimestampMs};

/// One encrypted change-set entry exchanged with the sync server.
///
/// The server stores these blind: `ciphertext` is opaque and deletions are
/// tombstones (`deleted_at` set) so removal itself can replicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRecord {
    pub id: SyncRecordId,
    pub entity_type: SyncEntityType,
    /// Id of the entity this record describes, within `entity_type`.
    pub entity_id: String,
    /// Monotonic per-entity version used for conflict detection.
    pub version: u64,
    /// Encrypted entity payload; empty exactly when this is a tombstone.
    pub ciphertext: Vec<u8>,
    /// Tombstone marker: set when the entity was deleted.
    pub deleted_at: Option<TimestampMs>,
    pub updated_at: TimestampMs,
    /// Device that produced this change.
    pub device_id: DeviceId,
}

impl SyncRecord {
    /// True when this record marks a deletion rather than carrying content.
    pub fn is_tombstone(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Validates the record before it is queued or applied.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.entity_id.is_empty() {
            return Err(ValidationError::new("entity_id", "must not be empty"));
        }
        if !self.is_tombstone() && self.ciphertext.is_empty() {
            return Err(ValidationError::new(
                "ciphertext",
                "non-tombstone records must carry ciphertext",
            ));
        }
        if self.is_tombstone() && !self.ciphertext.is_empty() {
            return Err(ValidationError::new(
                "ciphertext",
                "tombstones must not carry ciphertext",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn record() -> SyncRecord {
        SyncRecord {
            id: "sr1".to_string(),
            entity_type: SyncEntityType::Snippet,
            entity_id: "s1".to_string(),
            version: 1,
            ciphertext: vec![0xEE; 16],
            deleted_at: None,
            updated_at: 1_700_000_000_000,
            device_id: "d1".to_string(),
        }
    }

    #[test]
    fn accepts_a_content_record_with_ciphertext() {
        assert_eq!(record().validate(), Ok(()));
    }

    #[test]
    fn accepts_a_tombstone_without_ciphertext() {
        let mut r = record();
        r.deleted_at = Some(1_700_000_001_000);
        r.ciphertext.clear();
        assert!(r.is_tombstone());
        assert_eq!(r.validate(), Ok(()));
    }

    #[test]
    fn rejects_content_record_without_ciphertext() {
        let mut r = record();
        r.ciphertext.clear();
        assert_eq!(r.validate().unwrap_err().field, "ciphertext");
    }

    #[test]
    fn rejects_tombstone_carrying_ciphertext() {
        let mut r = record();
        r.deleted_at = Some(1);
        assert_eq!(r.validate().unwrap_err().field, "ciphertext");
    }
}
