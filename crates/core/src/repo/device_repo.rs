// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Device directory persistence.
//!
//! `public_key` holds the device's Ed25519 verifying key — the identity
//! anchor whose fingerprint users compare during pairing; the X25519
//! exchange key travels inside device certificates, not this table. Only
//! public material is ever stored here (private keys live in the platform
//! secure store).

use rusqlite::{Connection, Row, params};

use super::RepoError;
use crate::model::{Device, TimestampMs, TrustLevel};

/// Device repository over a single connection.
pub struct DeviceRepo<'c> {
    conn: &'c Connection,
}

impl<'c> DeviceRepo<'c> {
    pub fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts a validated device row.
    pub fn insert(&self, device: &Device) -> Result<(), RepoError> {
        device.validate()?;
        self.conn.execute(
            "INSERT INTO device \
             (id, name, platform, public_key, trust_level, last_seen_at, created_at, revoked_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                device.id,
                device.name,
                device.platform.as_str(),
                device.public_key,
                device.trust_level.as_str(),
                device.last_seen_at,
                device.created_at,
                device.revoked_at,
            ],
        )?;
        Ok(())
    }

    /// Loads one device by id.
    pub fn get(&self, id: &str) -> Result<Option<Device>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, platform, public_key, trust_level, \
             last_seen_at, created_at, revoked_at FROM device WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_device(row)?)),
            None => Ok(None),
        }
    }

    /// Lists every device, oldest first (the directory is small by nature —
    /// one row per paired device).
    pub fn list_all(&self) -> Result<Vec<Device>, RepoError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, platform, public_key, trust_level, \
             last_seen_at, created_at, revoked_at FROM device \
             ORDER BY created_at, id",
        )?;
        let mut rows = stmt.query([])?;
        let mut devices = Vec::new();
        while let Some(row) = rows.next()? {
            devices.push(row_to_device(row)?);
        }
        Ok(devices)
    }

    /// Marks a device revoked: trust_level
    /// and revoked_at flip together, honoring the schema CHECK. Revocation
    /// is one-way and the earliest time wins — a repeated revocation cannot
    /// move the trust boundary later, so it is an idempotent no-op.
    pub fn revoke(&self, id: &str, revoked_at: TimestampMs) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE device SET trust_level = ?2, revoked_at = ?3 \
             WHERE id = ?1 AND revoked_at IS NULL",
            params![id, TrustLevel::Revoked.as_str(), revoked_at],
        )?;
        if changed == 0 && self.get(id)?.is_none() {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    /// Updates the last-seen time as a lightweight independent statement,
    /// matching the usage-column convention.
    pub fn touch_last_seen(&self, id: &str, at: TimestampMs) -> Result<(), RepoError> {
        let changed = self.conn.execute(
            "UPDATE device SET last_seen_at = ?2 WHERE id = ?1",
            params![id, at],
        )?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }
}

/// Maps a row; unknown TEXT enum values surface as [`RepoError::UnknownEnum`]
/// so callers handle forward compatibility explicitly.
fn row_to_device(row: &Row<'_>) -> Result<Device, RepoError> {
    let platform_text: String = row.get(2)?;
    let trust_text: String = row.get(4)?;
    Ok(Device {
        id: row.get(0)?,
        name: row.get(1)?,
        platform: platform_text.parse()?,
        public_key: row.get(3)?,
        trust_level: trust_text.parse()?,
        last_seen_at: row.get(5)?,
        created_at: row.get(6)?,
        revoked_at: row.get(7)?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::{migrate_to_latest, open_in_memory};
    use crate::model::Platform;

    fn db() -> Connection {
        let mut conn = open_in_memory().unwrap();
        migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn device(id: &str, created_at: TimestampMs) -> Device {
        Device {
            id: id.to_string(),
            name: format!("Device {id}"),
            platform: Platform::Macos,
            public_key: vec![0x42; 32],
            trust_level: TrustLevel::Trusted,
            last_seen_at: None,
            created_at,
            revoked_at: None,
        }
    }

    #[test]
    fn insert_and_get_round_trip() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        let stored = device("d1", 1_700_000_000_000);
        repo.insert(&stored).unwrap();
        assert_eq!(repo.get("d1").unwrap().unwrap(), stored);
    }

    #[test]
    fn get_is_none_for_a_missing_device() {
        let conn = db();
        assert_eq!(DeviceRepo::new(&conn).get("missing").unwrap(), None);
    }

    #[test]
    fn rejects_an_invalid_device_before_it_reaches_storage() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        let mut d = device("d1", 1);
        d.public_key.clear();
        assert!(matches!(repo.insert(&d), Err(RepoError::Validation(_))));
        assert_eq!(repo.get("d1").unwrap(), None);
    }

    #[test]
    fn rejects_a_duplicate_device_id() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        repo.insert(&device("d1", 1)).unwrap();
        assert!(matches!(
            repo.insert(&device("d1", 2)),
            Err(RepoError::Sqlite(_))
        ));
    }

    #[test]
    fn list_all_returns_devices_oldest_first() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        repo.insert(&device("newer", 2_000)).unwrap();
        repo.insert(&device("older", 1_000)).unwrap();
        let ids: Vec<String> = repo.list_all().unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["older".to_string(), "newer".to_string()]);
    }

    #[test]
    fn revoke_sets_trust_level_and_revocation_time_together() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        repo.insert(&device("d1", 1)).unwrap();
        repo.revoke("d1", 5_000).unwrap();
        let revoked = repo.get("d1").unwrap().unwrap();
        assert_eq!(revoked.trust_level, TrustLevel::Revoked);
        assert_eq!(revoked.revoked_at, Some(5_000));
        assert_eq!(revoked.validate(), Ok(()));
    }

    #[test]
    fn revoking_twice_keeps_the_earliest_revocation_time() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        repo.insert(&device("d1", 1)).unwrap();
        repo.revoke("d1", 5_000).unwrap();
        repo.revoke("d1", 9_000).unwrap();
        assert_eq!(repo.get("d1").unwrap().unwrap().revoked_at, Some(5_000));
    }

    #[test]
    fn revoking_a_missing_device_is_not_found() {
        let conn = db();
        assert!(matches!(
            DeviceRepo::new(&conn).revoke("ghost", 1),
            Err(RepoError::NotFound)
        ));
    }

    #[test]
    fn touch_last_seen_updates_only_that_column() {
        let conn = db();
        let repo = DeviceRepo::new(&conn);
        repo.insert(&device("d1", 1)).unwrap();
        repo.touch_last_seen("d1", 7_000).unwrap();
        let seen = repo.get("d1").unwrap().unwrap();
        assert_eq!(seen.last_seen_at, Some(7_000));
        assert_eq!(seen.trust_level, TrustLevel::Trusted);
    }

    #[test]
    fn touching_a_missing_device_is_not_found() {
        let conn = db();
        assert!(matches!(
            DeviceRepo::new(&conn).touch_last_seen("ghost", 1),
            Err(RepoError::NotFound)
        ));
    }
}
