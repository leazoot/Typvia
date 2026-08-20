//! Shadow bases, parked remote records, and the single-row sync
//! configuration.

use rusqlite::{Connection, Row, params};

use super::RepoError;
use crate::model::{PendingRemoteRecord, SyncConfig, SyncEntityType, SyncRecord, SyncShadow};

/// Sync state repository over a single connection.
pub struct SyncStateRepo<'c> {
    conn: &'c Connection,
}

impl<'c> SyncStateRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    // ----- shadow bases -----

    /// Loads the shadow base of one entity, or `None` if never synced.
    pub fn shadow_get(
        &self,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<Option<SyncShadow>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_type, entity_id, version, document \
             FROM sync_shadow WHERE entity_type = ?1 AND entity_id = ?2",
        )?;
        let mut rows = stmt.query(params![entity_type.as_str(), entity_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_shadow(row)?)),
            None => Ok(None),
        }
    }

    /// Entity keys of every shadow base that still holds a document. The
    /// reconcile sweep walks them to find entities deleted while
    /// sync was off; tombstoned shadows are already agreed and excluded.
    /// Keys only — the documents stay in the database rather than being
    /// loaded together.
    pub fn shadow_keys(&self) -> Result<Vec<(SyncEntityType, String)>, RepoError> {
        let mut stmt = self
            .conn
            .prepare("SELECT entity_type, entity_id FROM sync_shadow WHERE document IS NOT NULL")?;
        let mut rows = stmt.query([])?;
        let mut keys = Vec::new();
        while let Some(row) = rows.next()? {
            let entity_type: String = row.get(0)?;
            keys.push((entity_type.parse()?, row.get(1)?));
        }
        Ok(keys)
    }

    /// Upserts a shadow base. Called in the same transaction as the pull
    /// application or the push acknowledgment it reflects.
    pub fn shadow_put(&self, shadow: &SyncShadow) -> Result<(), RepoError> {
        let version = i64::try_from(shadow.version)
            .map_err(|_| RepoError::Conflict("version exceeds storable range"))?;
        self.conn.execute(
            "INSERT INTO sync_shadow (entity_type, entity_id, version, document) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT (entity_type, entity_id) \
             DO UPDATE SET version = excluded.version, document = excluded.document",
            params![
                shadow.entity_type.as_str(),
                shadow.entity_id,
                version,
                shadow.document,
            ],
        )?;
        Ok(())
    }

    // ----- parked remote records -----

    /// Parks a pulled record that cannot be applied yet. Idempotent per
    /// record id (a re-pull of a still-parked record is not an error).
    pub fn pending_insert(&self, pending: &PendingRemoteRecord) -> Result<(), RepoError> {
        pending.validate()?;
        self.conn.execute(
            "INSERT OR IGNORE INTO sync_pending_record \
             (id, server_seq, entity_type, entity_id, version, ciphertext, deleted_at, \
              updated_at, device_id, key_id, signature, reason, received_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                pending.record.id,
                pending.server_seq,
                pending.record.entity_type.as_str(),
                pending.record.entity_id,
                i64::try_from(pending.record.version)
                    .map_err(|_| RepoError::Conflict("version exceeds storable range"))?,
                if pending.record.is_tombstone() {
                    None
                } else {
                    Some(&pending.record.ciphertext)
                },
                pending.record.deleted_at,
                pending.record.updated_at,
                pending.record.device_id,
                pending.key_id,
                pending.signature,
                pending.reason.as_str(),
                pending.received_at,
            ],
        )?;
        Ok(())
    }

    /// All parked records in server order.
    pub fn pending_list(&self) -> Result<Vec<PendingRemoteRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, server_seq, entity_type, entity_id, version, ciphertext, deleted_at, \
             updated_at, device_id, key_id, signature, reason, received_at \
             FROM sync_pending_record ORDER BY server_seq",
        )?;
        let mut rows = stmt.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(row_to_pending(row)?);
        }
        Ok(records)
    }

    /// Removes a parked record once it was applied (or superseded).
    pub fn pending_remove(&self, id: &str) -> Result<(), RepoError> {
        self.conn
            .execute("DELETE FROM sync_pending_record WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ----- configuration and cursor -----

    /// Loads the sync configuration (the migration guarantees the row).
    pub fn config_get(&self) -> Result<SyncConfig, RepoError> {
        self.conn
            .query_row(
                "SELECT server_url, account_id, enabled, applied_server_seq, sync_key_id, \
             root_fingerprint, updated_at, key_update_seq, recovery_root, \
             transport_kind, webdav_push_seq \
             FROM sync_config WHERE id = 1",
                [],
                |row| {
                    let key_id: i64 = row.get(4)?;
                    let transport: String = row.get(9)?;
                    Ok(SyncConfig {
                        server_url: row.get(0)?,
                        account_id: row.get(1)?,
                        enabled: row.get(2)?,
                        applied_server_seq: row.get(3)?,
                        sync_key_id: u32::try_from(key_id).unwrap_or(0),
                        root_fingerprint: row.get(5)?,
                        key_update_seq: row.get(7)?,
                        recovery_root: row.get(8)?,
                        transport_kind: transport.parse().map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                9,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                        webdav_push_seq: row.get(10)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .map_err(RepoError::from)
    }

    /// Replaces the configuration row.
    pub fn config_put(&self, config: &SyncConfig) -> Result<(), RepoError> {
        config.validate()?;
        let changed = self.conn.execute(
            "UPDATE sync_config SET server_url = ?1, account_id = ?2, enabled = ?3, \
             applied_server_seq = ?4, sync_key_id = ?5, root_fingerprint = ?6, \
             updated_at = ?7, key_update_seq = ?8, recovery_root = ?9, \
             transport_kind = ?10, webdav_push_seq = ?11 WHERE id = 1",
            params![
                config.server_url,
                config.account_id,
                config.enabled,
                config.applied_server_seq,
                config.sync_key_id,
                config.root_fingerprint,
                config.updated_at,
                config.key_update_seq,
                config.recovery_root,
                config.transport_kind.as_str(),
                config.webdav_push_seq,
            ],
        )?;
        if changed == 0 {
            return Err(RepoError::Conflict("sync_config row is missing"));
        }
        Ok(())
    }

    // ----- WebDAV per-device pull cursors -----

    /// The next record sequence to pull from `source_device_id`; 1 when the
    /// source was never pulled.
    pub fn webdav_cursor_get(&self, source_device_id: &str) -> Result<i64, RepoError> {
        let mut stmt = self
            .conn
            .prepare("SELECT next_seq FROM webdav_cursor WHERE source_device_id = ?1")?;
        let mut rows = stmt.query(params![source_device_id])?;
        match rows.next()? {
            Some(row) => Ok(row.get(0)?),
            None => Ok(1),
        }
    }

    /// Advances a source cursor; never moves backwards (mirror of the
    /// server-form watermark discipline). Runs in the same
    /// transaction as the applied records.
    pub fn webdav_cursor_put(
        &self,
        source_device_id: &str,
        next_seq: i64,
    ) -> Result<(), RepoError> {
        self.conn.execute(
            "INSERT INTO webdav_cursor (source_device_id, next_seq) VALUES (?1, ?2) \
             ON CONFLICT (source_device_id) \
             DO UPDATE SET next_seq = excluded.next_seq WHERE excluded.next_seq >= next_seq",
            params![source_device_id, next_seq],
        )?;
        Ok(())
    }

    /// Clears the recovery catch-up marker once the old-root catch-up has
    /// drained the server.
    pub fn recovery_root_clear(&self) -> Result<(), RepoError> {
        self.conn.execute(
            "UPDATE sync_config SET recovery_root = NULL WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    /// Advances the applied pull watermark; never moves backwards. Runs in
    /// the same transaction as the page application.
    pub fn advance_watermark(&self, server_seq: i64) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE sync_config SET applied_server_seq = ?1 \
             WHERE id = 1 AND applied_server_seq <= ?1",
            params![server_seq],
        )?;
        if changed == 0 {
            return Err(RepoError::Conflict(
                "watermark cannot move backwards or config row is missing",
            ));
        }
        Ok(())
    }
}

fn row_to_shadow(row: &Row<'_>) -> Result<SyncShadow, RepoError> {
    let entity_type: String = row.get(0)?;
    let version: i64 = row.get(2)?;
    Ok(SyncShadow {
        entity_type: entity_type.parse()?,
        entity_id: row.get(1)?,
        version: u64::try_from(version)
            .map_err(|_| RepoError::Conflict("stored version is negative"))?,
        document: row.get(3)?,
    })
}

fn row_to_pending(row: &Row<'_>) -> Result<PendingRemoteRecord, RepoError> {
    let entity_type: String = row.get(2)?;
    let version: i64 = row.get(4)?;
    let ciphertext: Option<Vec<u8>> = row.get(5)?;
    let key_id: i64 = row.get(9)?;
    let reason: String = row.get(11)?;
    Ok(PendingRemoteRecord {
        record: SyncRecord {
            id: row.get(0)?,
            entity_type: entity_type.parse()?,
            entity_id: row.get(3)?,
            version: u64::try_from(version)
                .map_err(|_| RepoError::Conflict("stored version is negative"))?,
            ciphertext: ciphertext.unwrap_or_default(),
            deleted_at: row.get(6)?,
            updated_at: row.get(7)?,
            device_id: row.get(8)?,
        },
        key_id: u32::try_from(key_id)
            .map_err(|_| RepoError::Conflict("stored key_id is out of range"))?,
        signature: row.get(10)?,
        server_seq: row.get(1)?,
        reason: reason.parse()?,
        received_at: row.get(12)?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::PendingReason;
    use crate::model::TransportKind;

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn shadow(entity_id: &str, version: u64) -> SyncShadow {
        SyncShadow {
            entity_type: SyncEntityType::Snippet,
            entity_id: entity_id.to_string(),
            version,
            document: Some(br#"{"payload_version":1,"entity":{}}"#.to_vec()),
        }
    }

    fn pending(id: &str, server_seq: i64) -> PendingRemoteRecord {
        PendingRemoteRecord {
            record: SyncRecord {
                id: id.to_string(),
                entity_type: SyncEntityType::Snippet,
                entity_id: "s1".to_string(),
                version: 3,
                ciphertext: vec![0xEE; 24],
                deleted_at: None,
                updated_at: 1_700_000_000_000,
                device_id: "remote-dev".to_string(),
            },
            key_id: 2,
            signature: vec![0xAB; 64],
            server_seq,
            reason: PendingReason::UnknownKeyId,
            received_at: 1_700_000_100_000,
        }
    }

    #[test]
    fn shadow_keys_skip_tombstoned_bases() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        repo.shadow_put(&shadow("s1", 1)).unwrap();
        let mut tombstoned = shadow("s2", 4);
        tombstoned.document = None;
        repo.shadow_put(&tombstoned).unwrap();

        assert_eq!(
            repo.shadow_keys().unwrap(),
            vec![(SyncEntityType::Snippet, "s1".to_string())]
        );
    }

    #[test]
    fn shadow_upsert_and_get_round_trip() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        let base = shadow("s1", 1);
        repo.shadow_put(&base).unwrap();
        assert_eq!(
            repo.shadow_get(SyncEntityType::Snippet, "s1").unwrap(),
            Some(base.clone())
        );

        let mut tombstoned = base;
        tombstoned.version = 2;
        tombstoned.document = None;
        repo.shadow_put(&tombstoned).unwrap();
        let stored = repo
            .shadow_get(SyncEntityType::Snippet, "s1")
            .unwrap()
            .unwrap();
        assert_eq!(stored.version, 2);
        assert_eq!(stored.document, None);
    }

    #[test]
    fn shadow_get_is_none_for_an_unsynced_entity() {
        let conn = db();
        assert_eq!(
            SyncStateRepo::new(&conn)
                .shadow_get(SyncEntityType::Folder, "missing")
                .unwrap(),
            None
        );
    }

    #[test]
    fn pending_insert_is_idempotent_per_record_id() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        repo.pending_insert(&pending("p1", 10)).unwrap();
        repo.pending_insert(&pending("p1", 10)).unwrap();
        let parked = repo.pending_list().unwrap();
        assert_eq!(parked.len(), 1);
        assert_eq!(parked[0], pending("p1", 10));
    }

    #[test]
    fn pending_list_orders_by_server_seq_and_remove_deletes() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        repo.pending_insert(&pending("p2", 20)).unwrap();
        repo.pending_insert(&pending("p1", 10)).unwrap();
        let ids: Vec<String> = repo
            .pending_list()
            .unwrap()
            .into_iter()
            .map(|p| p.record.id)
            .collect();
        assert_eq!(ids, vec!["p1".to_string(), "p2".to_string()]);
        repo.pending_remove("p1").unwrap();
        assert_eq!(repo.pending_list().unwrap().len(), 1);
    }

    #[test]
    fn config_defaults_then_round_trips() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        let initial = repo.config_get().unwrap();
        assert!(!initial.enabled);
        assert_eq!(initial.applied_server_seq, 0);
        assert_eq!(initial.sync_key_id, 0);
        assert_eq!(initial.key_update_seq, 0);
        assert_eq!(initial.recovery_root, None);

        let config = SyncConfig {
            server_url: Some("https://sync.example".to_string()),
            account_id: Some("acc-1".to_string()),
            enabled: true,
            applied_server_seq: 5,
            sync_key_id: 1,
            root_fingerprint: Some(vec![0xCD; 32]),
            key_update_seq: 4,
            recovery_root: Some(b"{\"fake\":\"root\"}".to_vec()),
            transport_kind: TransportKind::Webdav,
            webdav_push_seq: 7,
            updated_at: 1_700_000_000_000,
        };
        repo.config_put(&config).unwrap();
        assert_eq!(repo.config_get().unwrap(), config);

        // Draining the catch-up clears only the marker.
        repo.recovery_root_clear().unwrap();
        let cleared = repo.config_get().unwrap();
        assert_eq!(cleared.recovery_root, None);
        assert_eq!(cleared.applied_server_seq, 5);
    }

    #[test]
    fn webdav_cursors_default_to_one_and_never_move_backwards() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        assert_eq!(repo.webdav_cursor_get("dev-a").unwrap(), 1);
        repo.webdav_cursor_put("dev-a", 5).unwrap();
        assert_eq!(repo.webdav_cursor_get("dev-a").unwrap(), 5);
        // A rolled-back store must not drag the cursor backwards.
        repo.webdav_cursor_put("dev-a", 3).unwrap();
        assert_eq!(repo.webdav_cursor_get("dev-a").unwrap(), 5);
        repo.webdav_cursor_put("dev-b", 2).unwrap();
        assert_eq!(repo.webdav_cursor_get("dev-b").unwrap(), 2);
    }

    #[test]
    fn watermark_advances_forward_only() {
        let conn = db();
        let repo = SyncStateRepo::new(&conn);
        repo.advance_watermark(10).unwrap();
        assert_eq!(repo.config_get().unwrap().applied_server_seq, 10);
        // Re-asserting the same watermark is idempotent.
        repo.advance_watermark(10).unwrap();
        assert!(matches!(
            repo.advance_watermark(9),
            Err(RepoError::Conflict(_))
        ));
        assert_eq!(repo.config_get().unwrap().applied_server_seq, 10);
    }
}
