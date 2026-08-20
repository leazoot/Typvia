//! Provider configuration. The four provider forms share the
//! OpenAI-compatible wire format; everything that distinguishes them is
//! expressed here as configuration, not as separate clients.

use crate::error::AiError;

/// Default per-request deadline. Completions on local models are slow;
/// connectivity checks pass a much shorter value.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Upper bound guarding against configurations that would hold a worker
/// forever.
pub const MAX_TIMEOUT_MS: u64 = 300_000;

/// The supported provider forms. The enum itself lives in core as the
/// single source of truth for stored TEXT values; this crate contributes
/// only the egress-side behavior via
/// [`ProviderKindExt`].
pub use typvia_core::model::AiProviderKind as ProviderKind;

/// Egress behavior per provider kind. Implemented for the core enum here
/// because core stays free of transport concerns.
pub trait ProviderKindExt {
    /// Conventional local endpoint for kinds that have one.
    fn default_base_url(&self) -> Option<&'static str>;

    /// Hosted services refuse anonymous calls; local daemons do not
    /// require a key (one may still be configured, e.g. behind a proxy).
    fn requires_api_key(&self) -> bool;
}

impl ProviderKindExt for ProviderKind {
    fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Ollama => Some("http://127.0.0.1:11434/v1"),
            Self::LmStudio => Some("http://127.0.0.1:1234/v1"),
            Self::OpenAiCompatible | Self::CustomBaseUrl => None,
        }
    }

    fn requires_api_key(&self) -> bool {
        matches!(self, Self::OpenAiCompatible)
    }
}

/// One configured provider. Holds no key material — the API key lives in
/// the platform secure store under [`crate::credentials::entry_name`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Stable identifier; also the suffix of the secure-store entry.
    pub id: String,
    pub kind: ProviderKind,
    /// OpenAI-compatible API root (typically ending in `/v1`), without a
    /// trailing slash.
    pub base_url: String,
    /// Model name passed through verbatim, so custom names are supported.
    pub model: String,
    pub timeout_ms: u64,
}

impl ProviderConfig {
    /// Builds a config, filling the kind's default base URL when the user
    /// left it empty.
    pub fn new(
        id: impl Into<String>,
        kind: ProviderKind,
        base_url: Option<&str>,
        model: impl Into<String>,
    ) -> Result<Self, AiError> {
        let base_url = match (base_url, kind.default_base_url()) {
            (Some(url), _) if !url.trim().is_empty() => url.trim().trim_end_matches('/').into(),
            (_, Some(default)) => default.into(),
            _ => {
                return Err(AiError::InvalidConfig {
                    field: "base_url",
                    rule: "this provider kind needs an explicit base URL",
                });
            }
        };
        let config = Self {
            id: id.into(),
            kind,
            base_url,
            model: model.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), AiError> {
        if self.id.trim().is_empty() {
            return Err(AiError::InvalidConfig {
                field: "id",
                rule: "must not be blank",
            });
        }
        if self.model.trim().is_empty() {
            return Err(AiError::InvalidConfig {
                field: "model",
                rule: "must not be blank",
            });
        }
        if !is_acceptable_url(&self.base_url) {
            return Err(AiError::InvalidConfig {
                field: "base_url",
                rule: "must be https://, or http:// toward loopback",
            });
        }
        if self.timeout_ms == 0 || self.timeout_ms > MAX_TIMEOUT_MS {
            return Err(AiError::InvalidConfig {
                field: "timeout_ms",
                rule: "must be between 1ms and 300000ms",
            });
        }
        Ok(())
    }
}

/// URL policy — API keys and snippet content never transit cleartext
/// networks: `https://` toward any host; `http://` only toward loopback,
/// which local daemons (Ollama, LM Studio) listen on. Plaintext LAN
/// endpoints are rejected. Same shape as the sync transport's rule.
pub(crate) fn is_acceptable_url(url: &str) -> bool {
    if let Some(rest) = url.strip_prefix("https://") {
        return !rest.is_empty();
    }
    if let Some(rest) = url.strip_prefix("http://") {
        // Bracketed IPv6 hosts contain ':' inside the brackets.
        let host = if rest.starts_with('[') {
            rest.split(']').next().map(|h| format!("{h}]"))
        } else {
            rest.split(['/', ':']).next().map(str::to_string)
        };
        return matches!(host.as_deref(), Some("localhost" | "127.0.0.1" | "[::1]"));
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn local_kinds_fall_back_to_their_conventional_endpoints() {
        let ollama = ProviderConfig::new("p1", ProviderKind::Ollama, None, "llama3").unwrap();
        assert_eq!(ollama.base_url, "http://127.0.0.1:11434/v1");
        assert!(!ollama.kind.requires_api_key());

        let lm_studio = ProviderConfig::new("p2", ProviderKind::LmStudio, None, "qwen").unwrap();
        assert_eq!(lm_studio.base_url, "http://127.0.0.1:1234/v1");
        assert!(!lm_studio.kind.requires_api_key());
    }

    #[test]
    fn hosted_kinds_demand_an_explicit_base_url() {
        let missing = ProviderConfig::new("p", ProviderKind::OpenAiCompatible, None, "gpt-4o");
        assert_eq!(
            missing.unwrap_err(),
            AiError::InvalidConfig {
                field: "base_url",
                rule: "this provider kind needs an explicit base URL",
            }
        );
        let ok = ProviderConfig::new(
            "p",
            ProviderKind::OpenAiCompatible,
            Some("https://api.example.com/v1/"),
            "gpt-4o",
        )
        .unwrap();
        // Trailing slash is normalized away so path joining stays stable.
        assert_eq!(ok.base_url, "https://api.example.com/v1");
        assert!(ok.kind.requires_api_key());
    }

    #[test]
    fn a_custom_base_url_overrides_any_default() {
        let config = ProviderConfig::new(
            "p",
            ProviderKind::CustomBaseUrl,
            Some("http://localhost:8080/v1"),
            "m",
        )
        .unwrap();
        assert_eq!(config.base_url, "http://localhost:8080/v1");
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn plaintext_urls_are_loopback_only() {
        for url in [
            "https://api.example.com/v1",
            "http://127.0.0.1:11434/v1",
            "http://localhost:1234/v1",
            "http://[::1]:8080/v1",
        ] {
            assert!(is_acceptable_url(url), "{url}");
        }
        for url in [
            "http://192.168.1.20:11434/v1",
            "http://ollama.lan/v1",
            "ftp://x",
            "file:///tmp/x",
            "api.example.com/v1",
            "https://",
        ] {
            assert!(!is_acceptable_url(url), "{url}");
        }
    }

    #[test]
    fn rejects_blank_fields_and_out_of_range_timeouts() {
        let base = ProviderConfig::new("p", ProviderKind::Ollama, None, "m").unwrap();

        let mut blank_id = base.clone();
        blank_id.id = "  ".into();
        assert_eq!(
            blank_id.validate().unwrap_err(),
            AiError::InvalidConfig {
                field: "id",
                rule: "must not be blank",
            }
        );

        let mut blank_model = base.clone();
        blank_model.model = String::new();
        assert!(matches!(
            blank_model.validate().unwrap_err(),
            AiError::InvalidConfig { field: "model", .. }
        ));

        for timeout_ms in [0, MAX_TIMEOUT_MS + 1] {
            let mut bad_timeout = base.clone();
            bad_timeout.timeout_ms = timeout_ms;
            assert!(matches!(
                bad_timeout.validate().unwrap_err(),
                AiError::InvalidConfig {
                    field: "timeout_ms",
                    ..
                }
            ));
        }
    }
}
