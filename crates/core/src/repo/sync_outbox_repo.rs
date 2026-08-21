// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Outbox persistence over the `sync_record` table.
//!
//! Rows are sealed wire records: ciphertext, signature and metadata only —
//! sealing happens in the sync layer before anything reaches this repo.

use rusqlite::{Connection, Row, params};

use super::RepoError;
use crate::model::{OutboxRecord, OutboxState, SyncEntityType, SyncRecord};

/// Outbox repository over a single connection.
pub struct SyncOutboxRepo<'c> {
    conn: &'c Connection,
}

impl<'c> SyncOutboxRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Queues a sealed record. Runs inside the entity write transaction
    /// (seal-at-write); the UNIQUE (entity_type, entity_id, version) slot
    /// rejects duplicate versions.
    pub fn enqueue(&self, record: &OutboxRecord) -> Result<(), RepoError> {
        record.validate()?;
        self.conn.execute(
            "INSERT INTO sync_record \
             (id, entity_type, entity_id, version, ciphertext, deleted_at, updated_at, \
              device_id, key_id, signature, state, server_seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.record.id,
                record.record.entity_type.as_str(),
                record.record.entity_id,
                to_db_version(record.record.version)?,
                ciphertext_column(&record.record),
                record.record.deleted_at,
                record.record.updated_at,
                record.record.device_id,
                record.key_id,
                record.signature,
                record.state.as_str(),
                record.server_seq,
            ],
        )?;
        Ok(())
    }

    /// The next outbound version for an entity: one beyond the highest of
    /// the shadow head and any queued version (monotonicity).
    pub fn next_version(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<u64, RepoError> {
        let head: i64 = self.conn.query_row(
            "SELECT MAX(v) FROM (\
               SELECT COALESCE(MAX(version), 0) AS v FROM sync_record \
                 WHERE entity_type = ?1 AND entity_id = ?2 \
               UNION ALL \
               SELECT COALESCE(MAX(version), 0) AS v FROM sync_shadow \
                 WHERE entity_type = ?1 AND entity_id = ?2)",
            params![entity_type.as_str(), entity_id],
            |row| row.get(0),
        )?;
        Ok(from_db_version(head)? + 1)
    }

    /// Pending records in queue order (rowid = local causal order), bounded
    /// by count and by the total ciphertext byte budget of one push batch.
    pub fn list_pending(
        &self,
        max_records: u32,
        max_bytes: usize,
    ) -> Result<Vec<OutboxRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, state, server_seq \
             FROM sync_record WHERE state = 'pending' ORDER BY rowid LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![max_records])?;
        let mut records = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = rows.next()? {
            let record = row_to_outbox(row)?;
            bytes += record.record.ciphertext.len();
            if !records.is_empty() && bytes > max_bytes {
                break;
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Number of records still waiting to be pushed (backlog indicator).
    pub fn pending_count(&self) -> Result<u64, RepoError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sync_record WHERE state = 'pending'",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u64)
    }

    /// Marks one pushed record applied with its server-assigned sequence.
    pub fn mark_applied(&self, id: &str, server_seq: i64) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE sync_record SET state = ?2, server_seq = ?3 \
             WHERE id = ?1 AND state = 'pending'",
            params![id, OutboxState::Applied.as_str(), server_seq],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Parks every queued record of an entity as conflicted (409 outcome):
    /// nothing is dropped, nothing is faked as pushed; the three-way merge
    /// owns the resolution.
    /// The applied outbox record of an entity at an exact version, if this
    /// device published one (the WebDAV rival-publish check).
    pub fn applied_at_version(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
        version: u64,
    ) -> Result<Option<OutboxRecord>, RepoError> {
        let version = i64::try_from(version)
            .map_err(|_| RepoError::Conflict("version exceeds storable range"))?;
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, state, server_seq \
             FROM sync_record \
             WHERE entity_type = ?1 AND entity_id = ?2 AND version = ?3 AND state = 'applied' \
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![entity_type.as_str(), entity_id, version])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_outbox(row)?)),
            None => Ok(None),
        }
    }

    pub fn mark_entity_conflicted(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<u64, RepoError> {
        let changed = self.conn.execute(
            "UPDATE sync_record SET state = ?3 \
             WHERE entity_type = ?1 AND entity_id = ?2 AND state = 'pending'",
            params![
                entity_type.as_str(),
                entity_id,
                OutboxState::Conflict.as_str()
            ],
        )?;
        Ok(changed as u64)
    }

    /// The highest-version conflicted record of one entity — the local head
    /// the three-way merge resolves against. `None` when the
    /// entity has no parked conflict.
    pub fn conflict_head(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<Option<OutboxRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, state, server_seq \
             FROM sync_record WHERE entity_type = ?1 AND entity_id = ?2 \
             AND state = 'conflict' ORDER BY version DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![entity_type.as_str(), entity_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_outbox(row)?)),
            None => Ok(None),
        }
    }

    /// Removes every conflicted record of one entity after the three-way
    /// merge resolved it (the merged state re-enters the queue as a fresh
    /// record; the local head's content survives in the merge result,
    /// conflict copy, or version history). Returns rows removed.
    pub fn clear_entity_conflicts(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<u64, RepoError> {
        let removed = self.conn.execute(
            "DELETE FROM sync_record WHERE entity_type = ?1 AND entity_id = ?2 \
             AND state = 'conflict'",
            params![entity_type.as_str(), entity_id],
        )?;
        Ok(removed as u64)
    }

    /// The newest not-yet-accepted record of every entity — the state this
    /// device has already committed to sending. The reconcile sweep compares
    /// against it instead of the shadow base, so a second run with nothing
    /// pushed in between queues nothing new.
    pub fn queued_heads(&self) -> Result<Vec<OutboxRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, state, server_seq \
             FROM sync_record AS r WHERE state <> 'applied' \
             AND version = (SELECT MAX(version) FROM sync_record \
               WHERE entity_type = r.entity_type AND entity_id = r.entity_id \
               AND state <> 'applied')",
        )?;
        let mut rows = stmt.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(row_to_outbox(row)?);
        }
        Ok(records)
    }

    /// Records currently parked as conflicted, in queue order.
    pub fn list_conflicted(&self) -> Result<Vec<OutboxRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, state, server_seq \
             FROM sync_record WHERE state = 'conflict' ORDER BY rowid",
        )?;
        let mut rows = stmt.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(row_to_outbox(row)?);
        }
        Ok(records)
    }
}

/// Content records store their envelope; tombstones store NULL, matching
/// the schema CHECK ((deleted_at IS NULL) = (ciphertext IS NOT NULL)).
fn ciphertext_column(record: &SyncRecord) -> Option<&[u8]> {
    if record.is_tombstone() {
        None
    } else {
        Some(&record.ciphertext)
    }
}

/// Versions are u64 in the protocol but INTEGER (i64) in SQLite; values
/// beyond i64 cannot be stored and are rejected as a conflict.
fn to_db_version(version: u64) -> Result<i64, RepoError> {
    i64::try_from(version).map_err(|_| RepoError::Conflict("version exceeds storable range"))
}

fn from_db_version(version: i64) -> Result<u64, RepoError> {
    u64::try_from(version).map_err(|_| RepoError::Conflict("stored version is negative"))
}

fn row_to_outbox(row: &Row<'_>) -> Result<OutboxRecord, RepoError> {
    let entity_type: String = row.get(1)?;
    let version: i64 = row.get(3)?;
    let ciphertext: Option<Vec<u8>> = row.get(4)?;
    let key_id: i64 = row.get(8)?;
    let state: String = row.get(10)?;
    Ok(OutboxRecord {
        record: SyncRecord {
            id: row.get(0)?,
            entity_type: entity_type.parse()?,
            entity_id: row.get(2)?,
            version: from_db_version(version)?,
            ciphertext: ciphertext.unwrap_or_default(),
            deleted_at: row.get(5)?,
            updated_at: row.get(6)?,
            device_id: row.get(7)?,
        },
        key_id: u32::try_from(key_id)
            .map_err(|_| RepoError::Conflict("stored key_id is out of range"))?,
        signature: row.get(9)?,
        state: state.parse()?,
        server_seq: row.get(11)?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::{Device, Platform, TimestampMs, TrustLevel};
    use crate::repo::DeviceRepo;

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        DeviceRepo::new(&conn)
            .insert(&Device {
                id: "d1".to_string(),
                name: "Test device".to_string(),
                platform: Platform::Macos,
                public_key: vec![0x42; 32],
                trust_level: TrustLevel::Trusted,
                last_seen_at: None,
                created_at: 1,
                revoked_at: None,
            })
            .unwrap();
        conn
    }

    fn entry(id: &str, entity_id: &str, version: u64) -> OutboxRecord {
        OutboxRecord {
            record: SyncRecord {
                id: id.to_string(),
                entity_type: SyncEntityType::Snippet,
                entity_id: entity_id.to_string(),
                version,
                ciphertext: vec![0xEE; 32],
                deleted_at: None,
                updated_at: 1_700_000_000_000,
                device_id: "d1".to_string(),
            },
            key_id: 1,
            signature: vec![0xAA; 64],
            state: OutboxState::Pending,
            server_seq: None,
        }
    }

    fn tombstone(id: &str, entity_id: &str, version: u64, at: TimestampMs) -> OutboxRecord {
        let mut e = entry(id, entity_id, version);
        e.record.ciphertext = Vec::new();
        e.record.deleted_at = Some(at);
        e.key_id = 0;
        e
    }

    #[test]
    fn enqueue_then_list_round_trips_in_queue_order() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s2", 1)).unwrap();
        let pending = repo.list_pending(500, usize::MAX).unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0], entry("r1", "s1", 1));
        assert_eq!(pending[1].record.id, "r2");
    }

    #[test]
    fn a_tombstone_round_trips_without_ciphertext() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&tombstone("r1", "s1", 2, 5_000)).unwrap();
        let pending = repo.list_pending(500, usize::MAX).unwrap();
        assert!(pending[0].record.is_tombstone());
        assert!(pending[0].record.ciphertext.is_empty());
        assert_eq!(pending[0].key_id, 0);
    }

    #[test]
    fn queued_heads_report_the_newest_unsent_record_per_entity() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s1", 2)).unwrap();
        repo.enqueue(&tombstone("r3", "s2", 1, 5_000)).unwrap();
        repo.enqueue(&entry("r4", "s3", 1)).unwrap();
        repo.mark_applied("r4", 7).unwrap();

        let mut heads = repo.queued_heads().unwrap();
        heads.sort_by(|a, b| a.record.entity_id.cmp(&b.record.entity_id));
        assert_eq!(heads.len(), 2, "an accepted record is no longer queued");
        assert_eq!(heads[0].record.id, "r2");
        assert_eq!(heads[0].record.version, 2);
        assert_eq!(heads[1].record.id, "r3");
        assert!(heads[1].record.is_tombstone());
    }

    #[test]
    fn next_version_starts_at_one_and_follows_the_queue() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        assert_eq!(repo.next_version(SyncEntityType::Snippet, "s1").unwrap(), 1);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s1", 2)).unwrap();
        assert_eq!(repo.next_version(SyncEntityType::Snippet, "s1").unwrap(), 3);
    }

    #[test]
    fn next_version_respects_the_shadow_head() {
        let conn = db();
        conn.execute(
            "INSERT INTO sync_shadow (entity_type, entity_id, version, document) \
             VALUES ('snippet', 's1', 7, x'00')",
            [],
        )
        .unwrap();
        assert_eq!(
            SyncOutboxRepo::new(&conn)
                .next_version(SyncEntityType::Snippet, "s1")
                .unwrap(),
            8
        );
    }

    #[test]
    fn duplicate_version_slot_is_rejected() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        assert!(matches!(
            repo.enqueue(&entry("r2", "s1", 1)),
            Err(RepoError::Sqlite(_))
        ));
    }

    #[test]
    fn mark_applied_records_the_server_seq() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.mark_applied("r1", 42).unwrap();
        assert!(repo.list_pending(500, usize::MAX).unwrap().is_empty());
        assert_eq!(repo.pending_count().unwrap(), 0);
    }

    #[test]
    fn marking_a_missing_or_settled_record_is_not_found() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        assert!(matches!(
            repo.mark_applied("ghost", 1),
            Err(RepoError::NotFound)
        ));
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.mark_applied("r1", 1).unwrap();
        assert!(matches!(
            repo.mark_applied("r1", 2),
            Err(RepoError::NotFound)
        ));
    }

    #[test]
    fn conflict_parking_takes_every_pending_version_of_the_entity() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s1", 2)).unwrap();
        repo.enqueue(&entry("r3", "s2", 1)).unwrap();
        let parked = repo
            .mark_entity_conflicted(SyncEntityType::Snippet, "s1")
            .unwrap();
        assert_eq!(parked, 2);
        let pending = repo.list_pending(500, usize::MAX).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].record.id, "r3");
        let conflicted = repo.list_conflicted().unwrap();
        assert_eq!(conflicted.len(), 2);
        assert!(conflicted.iter().all(|r| r.state == OutboxState::Conflict));
    }

    #[test]
    fn conflict_head_is_the_highest_conflicted_version() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s1", 2)).unwrap();
        assert_eq!(
            repo.conflict_head(SyncEntityType::Snippet, "s1").unwrap(),
            None
        );
        repo.mark_entity_conflicted(SyncEntityType::Snippet, "s1")
            .unwrap();
        let head = repo
            .conflict_head(SyncEntityType::Snippet, "s1")
            .unwrap()
            .unwrap();
        assert_eq!(head.record.version, 2);
        assert_eq!(head.state, OutboxState::Conflict);
    }

    #[test]
    fn clearing_conflicts_removes_only_that_entity() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s2", 1)).unwrap();
        repo.mark_entity_conflicted(SyncEntityType::Snippet, "s1")
            .unwrap();
        repo.mark_entity_conflicted(SyncEntityType::Snippet, "s2")
            .unwrap();
        assert_eq!(
            repo.clear_entity_conflicts(SyncEntityType::Snippet, "s1")
                .unwrap(),
            1
        );
        let remaining = repo.list_conflicted().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].record.entity_id, "s2");
    }

    #[test]
    fn list_pending_respects_the_byte_budget_but_returns_at_least_one() {
        let conn = db();
        let repo = SyncOutboxRepo::new(&conn);
        repo.enqueue(&entry("r1", "s1", 1)).unwrap();
        repo.enqueue(&entry("r2", "s2", 1)).unwrap();
        // Budget below one record still yields the first (progress beats
        // starvation; the per-record size cap lives in the seal layer).
        let one = repo.list_pending(500, 1).unwrap();
        assert_eq!(one.len(), 1);
        let both = repo.list_pending(500, 64).unwrap();
        assert_eq!(both.len(), 2);
    }
}
