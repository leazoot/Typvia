//! Vault key-header persistence.
//!
//! Stores only KDF parameters and wrapped (encrypted) key material. This layer
//! performs no crypto: callers derive/wrap in the crypto crate and hand the
//! opaque envelope bytes here. The header is a singleton (one MK per device
//! vault), so a write replaces the whole table in one transaction.

use rusqlite::{Connection, Row, params};
use typvia_crypto::{KdfParams, SALT_LEN};

use super::RepoError;
use crate::model::{DomainKey, KeyDomain, VaultKeyHeader};

/// Vault key repository over a single connection.
pub struct VaultKeyRepo<'c> {
    conn: &'c Connection,
}

impl<'c> VaultKeyRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Persists the master-key header, replacing any existing one (singleton).
    /// The header is validated first; on any failure the previous row stands.
    /// When the caller already holds a transaction (e.g. a backup restore),
    /// the replace runs inside it instead of opening a nested one.
    pub fn put_header(&self, header: &VaultKeyHeader) -> Result<(), RepoError> {
        header.validate()?;
        if self.conn.is_autocommit() {
            let tx = self.conn.unchecked_transaction()?;
            Self::replace_header(&tx, header)?;
            tx.commit()?;
        } else {
            Self::replace_header(self.conn, header)?;
        }
        Ok(())
    }

    fn replace_header(conn: &Connection, header: &VaultKeyHeader) -> Result<(), RepoError> {
        conn.execute("DELETE FROM key_header", [])?;
        conn.execute(
            "INSERT INTO key_header \
             (id, kdf_version, kdf_m_cost_kib, kdf_t_cost, kdf_p_cost, kdf_salt, \
              wrapped_mk, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                header.id,
                i64::from(header.kdf.version),
                i64::from(header.kdf.m_cost_kib),
                i64::from(header.kdf.t_cost),
                i64::from(header.kdf.p_cost),
                header.kdf.salt.as_slice(),
                header.wrapped_mk,
                header.created_at,
                header.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Loads the master-key header, or `None` when the vault is not initialized.
    pub fn load_header(&self) -> Result<Option<VaultKeyHeader>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kdf_version, kdf_m_cost_kib, kdf_t_cost, kdf_p_cost, \
             kdf_salt, wrapped_mk, created_at, updated_at FROM key_header LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_header(row)?)),
            None => Ok(None),
        }
    }

    /// Persists a wrapped domain key, replacing any row with the same
    /// `(domain, key_id)` generation.
    pub fn put_domain_key(&self, key: &DomainKey) -> Result<(), RepoError> {
        key.validate()?;
        self.conn.execute(
            "INSERT OR REPLACE INTO domain_key (domain, key_id, wrapped_key, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                key.domain.as_str(),
                key.key_id,
                key.wrapped_key,
                key.created_at,
            ],
        )?;
        Ok(())
    }

    /// Lists every stored domain key (all domains, all generations), oldest
    /// generation first. Serves the whole-library backup; rows carry only
    /// wrapped key material.
    pub fn list_domain_keys(&self) -> Result<Vec<DomainKey>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT domain, key_id, wrapped_key, created_at FROM domain_key \
             ORDER BY domain, key_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut keys = Vec::new();
        while let Some(row) = rows.next()? {
            keys.push(row_to_domain_key(row)?);
        }
        Ok(keys)
    }

    /// Deletes the master-key header and every stored domain key — the vault
    /// reset. All `domain_key` rows are wrapped under the MK being
    /// destroyed, so nothing in either table stays recoverable; runtime sync
    /// keys live in the platform secure store and are not touched here.
    pub fn delete_all_key_material(&self) -> Result<(), RepoError> {
        self.conn.execute("DELETE FROM key_header", [])?;
        self.conn.execute("DELETE FROM domain_key", [])?;
        Ok(())
    }

    /// Loads the newest generation of a domain's key, or `None` if absent.
    pub fn load_latest_domain_key(
        &self,
        domain: KeyDomain,
    ) -> Result<Option<DomainKey>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT domain, key_id, wrapped_key, created_at FROM domain_key \
             WHERE domain = ?1 ORDER BY key_id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![domain.as_str()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_domain_key(row)?)),
            None => Ok(None),
        }
    }
}

fn row_to_header(row: &Row<'_>) -> Result<VaultKeyHeader, RepoError> {
    let version: i64 = row.get(1)?;
    let m_cost: i64 = row.get(2)?;
    let t_cost: i64 = row.get(3)?;
    let p_cost: i64 = row.get(4)?;
    let salt_bytes: Vec<u8> = row.get(5)?;
    let salt: [u8; SALT_LEN] = salt_bytes
        .as_slice()
        .try_into()
        .map_err(|_| RepoError::Conflict("stored kdf salt has the wrong length"))?;

    Ok(VaultKeyHeader {
        id: row.get(0)?,
        kdf: KdfParams {
            version: clamp_u8(version),
            m_cost_kib: clamp_u32(m_cost),
            t_cost: clamp_u32(t_cost),
            p_cost: clamp_u32(p_cost),
            salt,
        },
        wrapped_mk: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn row_to_domain_key(row: &Row<'_>) -> Result<DomainKey, RepoError> {
    let domain_text: String = row.get(0)?;
    let domain = domain_text.parse::<KeyDomain>()?;
    Ok(DomainKey {
        domain,
        key_id: row.get(1)?,
        wrapped_key: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn clamp_u8(v: i64) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

fn clamp_u32(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    /// A cheap fake KDF params value; the round-trip semantics do not depend on
    /// Argon2 cost, and the real v1 cost is exercised by the crypto crate.
    fn cheap_kdf() -> KdfParams {
        KdfParams {
            version: 1,
            m_cost_kib: 8,
            t_cost: 1,
            p_cost: 1,
            salt: [7u8; SALT_LEN],
        }
    }

    fn header(id: &str) -> VaultKeyHeader {
        VaultKeyHeader {
            id: id.to_string(),
            kdf: cheap_kdf(),
            wrapped_mk: vec![0xAB; 60],
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        }
    }

    #[test]
    fn header_round_trips_through_storage() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        let stored = header("h1");
        repo.put_header(&stored).unwrap();
        let loaded = repo.load_header().unwrap().unwrap();
        assert_eq!(loaded, stored);
    }

    #[test]
    fn load_header_is_none_before_initialization() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        assert_eq!(repo.load_header().unwrap(), None);
    }

    #[test]
    fn put_header_keeps_a_single_row() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        repo.put_header(&header("first")).unwrap();
        repo.put_header(&header("second")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM key_header", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(repo.load_header().unwrap().unwrap().id, "second");
    }

    #[test]
    fn rejects_a_header_with_a_stub_wrapped_mk() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        let mut h = header("h1");
        h.wrapped_mk = vec![0x01, 0x02];
        assert!(matches!(repo.put_header(&h), Err(RepoError::Validation(_))));
    }

    #[test]
    fn latest_domain_key_wins_over_older_generations() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        for key_id in [0u32, 1, 2] {
            repo.put_domain_key(&DomainKey {
                domain: KeyDomain::Vault,
                key_id,
                wrapped_key: vec![key_id as u8; 60],
                created_at: 1_700_000_000_000 + i64::from(key_id),
            })
            .unwrap();
        }
        let latest = repo
            .load_latest_domain_key(KeyDomain::Vault)
            .unwrap()
            .unwrap();
        assert_eq!(latest.key_id, 2);
        // A different domain is independent and still absent.
        assert_eq!(repo.load_latest_domain_key(KeyDomain::Sync).unwrap(), None);
    }

    #[test]
    fn rejects_a_domain_key_with_a_stub_envelope() {
        let conn = db();
        let repo = VaultKeyRepo::new(&conn);
        let err = repo.put_domain_key(&DomainKey {
            domain: KeyDomain::Sync,
            key_id: 0,
            wrapped_key: vec![0x01],
            created_at: 1_700_000_000_000,
        });
        assert!(matches!(err, Err(RepoError::Validation(_))));
    }
}
