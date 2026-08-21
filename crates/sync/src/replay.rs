// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Replay and monotonicity checks.
//!
//! This layer only classifies: whether a rejected record is silently
//! dropped (replay) or aborts the batch (cursor violation) is the
//! caller's policy. Persistence of applied heads and the pull cursor
//! belongs to the sync orchestration.

use typvia_core::model::SyncEntityType;

use crate::error::RecordError;
use crate::record::WireRecord;

/// Query interface over the locally applied head version per entity.
/// The sync orchestration backs this with persistent state; tests use
/// in-memory maps.
pub trait AppliedHead {
    /// The highest version already applied for the entity, or `None` if
    /// the entity has never been applied locally.
    fn applied_head(&self, entity_type: SyncEntityType, entity_id: &str) -> Option<u64>;
}

/// Rejects records whose version is not beyond the applied head:
/// a replayed or stale record, including tombstone re-deliveries —
/// tombstones share the same monotonic sequence.
pub fn check_not_replayed(heads: &dyn AppliedHead, record: &WireRecord) -> Result<(), RecordError> {
    if let Some(applied_head) = heads.applied_head(record.entity_type, &record.entity_id)
        && record.version <= applied_head
    {
        return Err(RecordError::Replay { applied_head });
    }
    Ok(())
}

/// Strictly increasing `server_seq` guard for one pull response:
/// every accepted sequence must exceed the request cursor and every
/// earlier sequence in the batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSeqCursor {
    position: u64,
}

impl ServerSeqCursor {
    /// Starts at the request cursor (the persisted applied watermark).
    pub fn new(position: u64) -> Self {
        Self { position }
    }

    /// Accepts the next `server_seq` if it strictly increases; on
    /// violation the caller must abort the batch.
    pub fn advance(&mut self, server_seq: u64) -> Result<(), RecordError> {
        if server_seq <= self.position {
            return Err(RecordError::NonMonotonicServerSeq);
        }
        self.position = server_seq;
        Ok(())
    }

    /// The highest sequence accepted so far (the new watermark).
    pub fn position(&self) -> u64 {
        self.position
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// A structurally plausible record; replay checks only read identity
    /// and version, so the cryptographic fields can stay inert here.
    fn content_record_fixture(version: u64) -> WireRecord {
        WireRecord {
            id: "r1".to_string(),
            entity_type: SyncEntityType::Snippet,
            entity_id: "s1".to_string(),
            version,
            ciphertext: vec![0xEE; 32],
            deleted_at: None,
            updated_at: 1_700_000_000_000,
            device_id: "d1".to_string(),
            key_id: 1,
            signature: [0; 64],
        }
    }

    struct Heads(HashMap<(SyncEntityType, String), u64>);

    impl AppliedHead for Heads {
        fn applied_head(&self, entity_type: SyncEntityType, entity_id: &str) -> Option<u64> {
            self.0.get(&(entity_type, entity_id.to_string())).copied()
        }
    }

    fn heads_with(entity_type: SyncEntityType, entity_id: &str, head: u64) -> Heads {
        let mut map = HashMap::new();
        map.insert((entity_type, entity_id.to_string()), head);
        Heads(map)
    }

    #[test]
    fn rejects_a_record_at_the_applied_head_as_replay() {
        let record = content_record_fixture(5);
        let heads = heads_with(record.entity_type, &record.entity_id, 5);
        assert_eq!(
            check_not_replayed(&heads, &record),
            Err(RecordError::Replay { applied_head: 5 })
        );
    }

    #[test]
    fn rejects_a_record_below_the_applied_head_as_replay() {
        let record = content_record_fixture(4);
        let heads = heads_with(record.entity_type, &record.entity_id, 5);
        assert_eq!(
            check_not_replayed(&heads, &record),
            Err(RecordError::Replay { applied_head: 5 })
        );
    }

    #[test]
    fn accepts_a_record_beyond_the_applied_head() {
        let record = content_record_fixture(6);
        let heads = heads_with(record.entity_type, &record.entity_id, 5);
        assert_eq!(check_not_replayed(&heads, &record), Ok(()));
    }

    #[test]
    fn accepts_the_first_record_of_an_unknown_entity() {
        let record = content_record_fixture(1);
        let heads = Heads(HashMap::new());
        assert_eq!(check_not_replayed(&heads, &record), Ok(()));
    }

    #[test]
    fn cursor_accepts_strictly_increasing_sequences() {
        let mut cursor = ServerSeqCursor::new(10);
        assert_eq!(cursor.advance(11), Ok(()));
        assert_eq!(cursor.advance(15), Ok(()));
        assert_eq!(cursor.position(), 15);
    }

    #[test]
    fn cursor_rejects_a_repeated_sequence() {
        let mut cursor = ServerSeqCursor::new(10);
        assert_eq!(cursor.advance(11), Ok(()));
        assert_eq!(cursor.advance(11), Err(RecordError::NonMonotonicServerSeq));
    }

    #[test]
    fn cursor_rejects_a_sequence_at_or_below_the_request_cursor() {
        let mut cursor = ServerSeqCursor::new(10);
        assert_eq!(cursor.advance(10), Err(RecordError::NonMonotonicServerSeq));
        assert_eq!(cursor.advance(9), Err(RecordError::NonMonotonicServerSeq));
        // A violation does not move the watermark.
        assert_eq!(cursor.position(), 10);
    }
}
