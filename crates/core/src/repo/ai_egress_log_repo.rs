//! AI egress audit log storage.
//! Append-only: rows are never updated, and nothing here deletes — no
//! retention policy is defined yet, so the log keeps everything. The table
//! structurally cannot hold prompt content, response content or key material.

use rusqlite::{Connection, Row, params};

use super::RepoError;
use crate::model::{AiEgressRecord, AiRequestClass, TimestampMs};

/// Egress-log repository over a single connection.
pub struct AiEgressLogRepo<'c> {
    conn: &'c Connection,
}

impl<'c> AiEgressLogRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Appends one egress event. `now` comes from the caller (the data
    /// layer has no clock); `request_bytes` is the body size that left the
    /// device, 0 for body-less probes.
    pub fn append(
        &self,
        now: TimestampMs,
        provider_id: &str,
        request_class: AiRequestClass,
        request_bytes: i64,
    ) -> Result<(), RepoError> {
        if request_bytes < 0 {
            return Err(RepoError::Conflict("request_bytes must not be negative"));
        }
        self.conn.execute(
            "INSERT INTO ai_egress_log (occurred_at, provider_id, request_class, request_bytes)
             VALUES (?1, ?2, ?3, ?4)",
            params![now, provider_id, request_class.as_str(), request_bytes],
        )?;
        Ok(())
    }

    /// Newest entries first (settings viewer), paged.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<AiEgressRecord>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, occurred_at, provider_id, request_class, request_bytes
             FROM ai_egress_log ORDER BY id DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], row_to_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row??);
        }
        Ok(records)
    }

    /// Total number of recorded egress events.
    pub fn count(&self) -> Result<i64, RepoError> {
        let n = self
            .conn
            .query_row("SELECT COUNT(*) FROM ai_egress_log", [], |r| r.get(0))?;
        Ok(n)
    }
}

type RecordRowResult = Result<AiEgressRecord, RepoError>;

fn row_to_record(row: &Row<'_>) -> rusqlite::Result<RecordRowResult> {
    let class: String = row.get("request_class")?;
    // Stored TEXT enums parse explicitly; unknown values surface as errors
    // instead of being silently dropped (forward-compat rule).
    let parsed = (|| -> RecordRowResult {
        Ok(AiEgressRecord {
            id: row.get("id")?,
            occurred_at: row.get("occurred_at")?,
            provider_id: row.get("provider_id")?,
            request_class: class.parse().map_err(RepoError::from)?,
            request_bytes: row.get("request_bytes")?,
        })
    })();
    match parsed {
        Ok(record) => Ok(Ok(record)),
        Err(RepoError::Sqlite(e)) => Err(e),
        Err(other) => Ok(Err(other)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};

    fn setup() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    #[test]
    fn appends_and_lists_newest_first_with_paging() {
        let conn = setup();
        let repo = AiEgressLogRepo::new(&conn);
        repo.append(10, "p1", AiRequestClass::Connectivity, 0)
            .unwrap();
        repo.append(20, "p1", AiRequestClass::Completion, 512)
            .unwrap();
        repo.append(30, "p2", AiRequestClass::Completion, 1024)
            .unwrap();
        assert_eq!(repo.count().unwrap(), 3);

        let first_two = repo.list(2, 0).unwrap();
        assert_eq!(
            first_two
                .iter()
                .map(|r| (r.occurred_at, r.provider_id.as_str(), r.request_bytes))
                .collect::<Vec<_>>(),
            [(30, "p2", 1024), (20, "p1", 512)]
        );
        assert_eq!(first_two[0].request_class, AiRequestClass::Completion);

        let tail = repo.list(2, 2).unwrap();
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].request_class, AiRequestClass::Connectivity);
        assert_eq!(tail[0].request_bytes, 0);
    }

    #[test]
    fn rejects_a_negative_byte_count() {
        let conn = setup();
        let repo = AiEgressLogRepo::new(&conn);
        assert!(matches!(
            repo.append(1, "p1", AiRequestClass::Completion, -1)
                .unwrap_err(),
            RepoError::Conflict(_)
        ));
        assert_eq!(repo.count().unwrap(), 0);
    }

    #[test]
    fn an_unknown_stored_class_is_reported_not_silently_dropped() {
        let conn = setup();
        conn.execute(
            "INSERT INTO ai_egress_log (occurred_at, provider_id, request_class, request_bytes) \
             VALUES (1, 'p1', 'telepathy', 0)",
            [],
        )
        .unwrap();
        let repo = AiEgressLogRepo::new(&conn);
        match repo.list(10, 0).unwrap_err() {
            RepoError::UnknownEnum(e) => {
                assert_eq!(e.enum_name, "AiRequestClass");
                assert_eq!(e.value, "telepathy");
            }
            other => panic!("expected UnknownEnum, got {other:?}"),
        }
    }
}
