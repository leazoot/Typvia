//! The `AiProvider` entity: one configured AI endpoint. Device-local —
//! never part of the sync entity set (base URLs and model names describe
//! this machine's environment).
//!
//! Red line: this entity carries no key material. The API key lives only
//! in the platform secure store (`ai.api_key.<provider_id>`, non-gated).

use super::validation::{ValidationError, require_non_blank};
use super::{AiProviderId, AiProviderKind, TimestampMs};

/// A configured AI provider row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProvider {
    pub id: AiProviderId,
    /// User-facing label, distinguishing multiple instances of one kind.
    pub name: String,
    pub kind: AiProviderKind,
    /// OpenAI-compatible API root. Storage validity only requires it to be
    /// present; the TLS/loopback policy is enforced by `crates/ai` at the
    /// egress boundary (configuration and transport layers).
    pub base_url: String,
    /// Model name passed through verbatim; custom model names are allowed.
    pub model: String,
    pub timeout_ms: i64,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl AiProvider {
    /// Validates the row before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        require_non_blank("base_url", &self.base_url)?;
        require_non_blank("model", &self.model)?;
        if self.timeout_ms <= 0 {
            return Err(ValidationError::new("timeout_ms", "must be positive"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn provider() -> AiProvider {
        AiProvider {
            id: "p1".to_string(),
            name: "Local Ollama".to_string(),
            kind: AiProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "llama3".to_string(),
            timeout_ms: 30_000,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn accepts_a_well_formed_provider() {
        assert_eq!(provider().validate(), Ok(()));
    }

    #[test]
    fn rejects_blank_fields_and_non_positive_timeouts() {
        let mut blank_name = provider();
        blank_name.name = "  ".to_string();
        assert_eq!(blank_name.validate().unwrap_err().field, "name");

        let mut blank_url = provider();
        blank_url.base_url = String::new();
        assert_eq!(blank_url.validate().unwrap_err().field, "base_url");

        let mut zero_timeout = provider();
        zero_timeout.timeout_ms = 0;
        assert_eq!(zero_timeout.validate().unwrap_err().field, "timeout_ms");
    }
}
