//! The `Device` entity (PRD §15.7).

use super::enums::{Platform, TrustLevel};
use super::validation::{ValidationError, require_non_blank};
use super::{DeviceId, TimestampMs};

/// A paired device participating in sync (PRD §15.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub platform: Platform,
    /// Device public key bytes; the key scheme is fixed by
    /// docs/06_SECURITY_MODEL.md (TASK-023). Only public material is ever
    /// stored here.
    pub public_key: Vec<u8>,
    pub trust_level: TrustLevel,
    pub last_seen_at: Option<TimestampMs>,
    pub created_at: TimestampMs,
    /// Set exactly when the device is revoked.
    pub revoked_at: Option<TimestampMs>,
}

impl Device {
    /// Validates the device record before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        if self.public_key.is_empty() {
            return Err(ValidationError::new("public_key", "must not be empty"));
        }
        let revoked = self.trust_level == TrustLevel::Revoked;
        if revoked != self.revoked_at.is_some() {
            return Err(ValidationError::new(
                "trust_level/revoked_at",
                "revoked devices must have revoked_at set, trusted devices must not",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn device() -> Device {
        Device {
            id: "d1".to_string(),
            name: "MacBook".to_string(),
            platform: Platform::Macos,
            public_key: vec![0x01; 32],
            trust_level: TrustLevel::Trusted,
            last_seen_at: None,
            created_at: 0,
            revoked_at: None,
        }
    }

    #[test]
    fn accepts_a_trusted_device_without_revocation_time() {
        assert_eq!(device().validate(), Ok(()));
    }

    #[test]
    fn accepts_a_revoked_device_with_revocation_time() {
        let mut d = device();
        d.trust_level = TrustLevel::Revoked;
        d.revoked_at = Some(1_700_000_000_000);
        assert_eq!(d.validate(), Ok(()));
    }

    #[test]
    fn rejects_revoked_device_without_revocation_time() {
        let mut d = device();
        d.trust_level = TrustLevel::Revoked;
        assert_eq!(d.validate().unwrap_err().field, "trust_level/revoked_at");
    }

    #[test]
    fn rejects_trusted_device_with_revocation_time() {
        let mut d = device();
        d.revoked_at = Some(1);
        assert_eq!(d.validate().unwrap_err().field, "trust_level/revoked_at");
    }

    #[test]
    fn rejects_empty_public_key() {
        let mut d = device();
        d.public_key.clear();
        assert_eq!(d.validate().unwrap_err().field, "public_key");
    }
}
