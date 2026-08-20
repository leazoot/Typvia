//! Client-side sync orchestration state:
//! outbox rows, shadow base documents, parked remote records, and the
//! single-row sync configuration. None of these carry key material.

use std::fmt;

use super::TimestampMs;
use super::enums::{OutboxState, PendingReason, SyncEntityType, TransportKind};
use super::sync_record::SyncRecord;
use super::validation::ValidationError;

/// One outbox entry: a sealed wire record plus its outbound lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRecord {
    pub record: SyncRecord,
    /// Outer K_sync generation; 0 for tombstones.
    pub key_id: u32,
    /// Ed25519 device signature over the canonical record byte string.
    pub signature: Vec<u8>,
    pub state: OutboxState,
    /// Server-assigned cursor value, set once the record is applied.
    pub server_seq: Option<i64>,
}

impl OutboxRecord {
    /// Validates the entry before it is queued or its state is updated.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.record.validate()?;
        if self.signature.len() != 64 {
            return Err(ValidationError::new(
                "signature",
                "must be a 64-byte Ed25519 signature",
            ));
        }
        if self.state == OutboxState::Applied && self.server_seq.is_none() {
            return Err(ValidationError::new(
                "server_seq",
                "applied records must carry their server_seq",
            ));
        }
        Ok(())
    }
}

/// Shadow base of one synced entity: the last server-agreed version and
/// plaintext document. After a tombstone is applied the document
/// goes `None` while the row — and with it the applied head version —
/// survives to block revival by stale records.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncShadow {
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    pub version: u64,
    /// Last agreed document bytes; `None` once tombstoned.
    pub document: Option<Vec<u8>>,
}

impl fmt::Debug for SyncShadow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Shadow documents contain user content and never reach Debug.
        f.debug_struct("SyncShadow")
            .field("entity_type", &self.entity_type)
            .field("entity_id", &self.entity_id)
            .field("version", &self.version)
            .field("document_len", &self.document.as_ref().map(Vec::len))
            .finish()
    }
}

/// A pulled remote record parked instead of applied. Kept
/// as full wire bytes so re-application runs complete verification again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRemoteRecord {
    pub record: SyncRecord,
    pub key_id: u32,
    pub signature: Vec<u8>,
    pub server_seq: i64,
    pub reason: PendingReason,
    pub received_at: TimestampMs,
}

impl PendingRemoteRecord {
    /// Validates the entry before it is parked.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.record.validate()?;
        if self.signature.len() != 64 {
            return Err(ValidationError::new(
                "signature",
                "must be a 64-byte Ed25519 signature",
            ));
        }
        Ok(())
    }
}

/// Single-row sync configuration and pull cursor. Holds no key
/// material: `sync_key_id` is only the highest active K_sync generation
/// number; the key itself lives in the platform secure store.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncConfig {
    pub server_url: Option<String>,
    pub account_id: Option<String>,
    pub enabled: bool,
    /// Applied pull watermark.
    pub applied_server_seq: i64,
    /// Highest active K_sync generation; 0 while sync was never set up.
    pub sync_key_id: u32,
    /// Pinned trust-root fingerprint (32-byte SHA-256 digest).
    pub root_fingerprint: Option<Vec<u8>>,
    /// Consumed key-update cursor; sent as `since` on
    /// the next pull, where the server reads it as an acknowledgment.
    pub key_update_seq: i64,
    /// While a post-recovery catch-up is incomplete: the
    /// predecessor trust-root statement JSON — public signed material, never
    /// keys — so the catch-up can resume across restarts. `None` otherwise.
    pub recovery_root: Option<Vec<u8>>,
    /// Which sync backend the account is bound to.
    pub transport_kind: TransportKind,
    /// WebDAV form only: the highest record sequence this device has
    /// published under `devices/<self>/records/`; 0 = none yet.
    pub webdav_push_seq: i64,
    pub updated_at: TimestampMs,
}

impl SyncConfig {
    /// True when sync is enabled and bound to an account.
    pub fn is_active(&self) -> bool {
        self.enabled && self.account_id.is_some() && self.server_url.is_some()
    }

    /// Validates cross-field invariants before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.enabled && (self.account_id.is_none() || self.server_url.is_none()) {
            return Err(ValidationError::new(
                "enabled",
                "enabled sync requires server_url and account_id",
            ));
        }
        if let Some(fingerprint) = &self.root_fingerprint
            && fingerprint.len() != 32
        {
            return Err(ValidationError::new(
                "root_fingerprint",
                "must be a 32-byte digest",
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
            id: "r1".to_string(),
            entity_type: SyncEntityType::Snippet,
            entity_id: "s1".to_string(),
            version: 1,
            ciphertext: vec![0xEE; 16],
            deleted_at: None,
            updated_at: 1_700_000_000_000,
            device_id: "d1".to_string(),
        }
    }

    fn outbox_record() -> OutboxRecord {
        OutboxRecord {
            record: record(),
            key_id: 1,
            signature: vec![0xAA; 64],
            state: OutboxState::Pending,
            server_seq: None,
        }
    }

    #[test]
    fn accepts_a_pending_outbox_record() {
        assert_eq!(outbox_record().validate(), Ok(()));
    }

    #[test]
    fn rejects_a_wrong_length_signature() {
        let mut r = outbox_record();
        r.signature = vec![0xAA; 63];
        assert_eq!(r.validate().unwrap_err().field, "signature");
    }

    #[test]
    fn rejects_applied_state_without_server_seq() {
        let mut r = outbox_record();
        r.state = OutboxState::Applied;
        assert_eq!(r.validate().unwrap_err().field, "server_seq");
    }

    #[test]
    fn enabled_config_requires_account_and_server() {
        let config = SyncConfig {
            enabled: true,
            ..SyncConfig::default()
        };
        assert_eq!(config.validate().unwrap_err().field, "enabled");
        assert!(!config.is_active());
    }

    #[test]
    fn a_complete_enabled_config_is_active() {
        let config = SyncConfig {
            server_url: Some("https://sync.example".to_string()),
            account_id: Some("acc-1".to_string()),
            enabled: true,
            root_fingerprint: Some(vec![0xCC; 32]),
            ..SyncConfig::default()
        };
        assert_eq!(config.validate(), Ok(()));
        assert!(config.is_active());
    }

    #[test]
    fn rejects_a_wrong_length_root_fingerprint() {
        let config = SyncConfig {
            root_fingerprint: Some(vec![0xCC; 31]),
            ..SyncConfig::default()
        };
        assert_eq!(config.validate().unwrap_err().field, "root_fingerprint");
    }

    #[test]
    fn shadow_debug_output_carries_no_document_bytes() {
        let shadow = SyncShadow {
            entity_type: SyncEntityType::Snippet,
            entity_id: "s1".to_string(),
            version: 3,
            document: Some(b"secret body".to_vec()),
        };
        let rendered = format!("{shadow:?}");
        assert!(rendered.contains("document_len"));
        assert!(!rendered.contains("secret body"));
    }
}
