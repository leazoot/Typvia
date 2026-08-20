//! Provider-call error taxonomy: user / business / system. Messages are
//! static rule text only — never key material, prompt content or provider
//! response bodies.

use std::fmt;

use typvia_core::sensitive::SensitiveKind;

/// The three-way classification the host layer maps onto IPC error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Correctable by the user (configuration, cancelled request).
    User,
    /// The provider or a rule refused the request; local data is fine.
    Business,
    /// Unexpected transport or provider failure; not user-correctable.
    System,
}

/// Failures of a provider call or connectivity check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    /// A configuration field violates the named rule.
    InvalidConfig {
        field: &'static str,
        rule: &'static str,
    },
    /// The provider kind requires an API key and none is stored.
    MissingApiKey,
    /// The request itself violates the named rule before any I/O.
    InvalidRequest { rule: &'static str },
    /// The caller cancelled via [`crate::CancelToken`].
    Cancelled,
    /// The egress gate refused: outbound content looks like it contains a
    /// secret. Carries only the pattern kinds (static codes), never the
    /// matched text.
    SuspectedSecretBlocked { kinds: Vec<SensitiveKind> },
    /// The egress gate refused: sensitive snippets never leave the device
    /// — no permission scope relaxes this.
    SensitiveSnippetBlocked,
    /// 401/403: the provider refused the credentials.
    AuthRejected,
    /// The configured model is unknown to the provider.
    ModelNotFound,
    /// 429: the provider throttled this client.
    RateLimited,
    /// Any other 4xx refusal.
    ProviderRejected { status: u16 },
    /// The provider did not answer within the configured timeout.
    Timeout,
    /// The provider could not be reached at all (DNS, connect, TLS).
    Unreachable,
    /// 5xx: the provider failed internally.
    ProviderFailure { status: u16 },
    /// The response was not the expected wire shape (or exceeded the
    /// size cap). The offending bytes are never included.
    InvalidResponse,
    /// The connection broke mid-request.
    Transport,
    /// The egress log could not be written; the request was not sent
    /// (fail-closed — no log entry, no egress).
    EgressLogFailed,
}

impl AiError {
    /// Classification for the host error mapping. `Timeout`/`Unreachable`
    /// are business errors, matching the established `Unavailable` IPC
    /// code (host-service): the product keeps working locally and the UI
    /// shows an offline state, not a failure.
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::InvalidConfig { .. }
            | Self::MissingApiKey
            | Self::InvalidRequest { .. }
            | Self::Cancelled => ErrorClass::User,
            // Gate refusals are business errors: a rule declined the
            // request and local data is untouched.
            Self::SuspectedSecretBlocked { .. }
            | Self::SensitiveSnippetBlocked
            | Self::AuthRejected
            | Self::ModelNotFound
            | Self::RateLimited
            | Self::ProviderRejected { .. }
            | Self::Timeout
            | Self::Unreachable => ErrorClass::Business,
            Self::ProviderFailure { .. }
            | Self::InvalidResponse
            | Self::Transport
            | Self::EgressLogFailed => ErrorClass::System,
        }
    }
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field, rule } => write!(f, "invalid {field}: {rule}"),
            Self::MissingApiKey => f.write_str("this provider requires an API key"),
            Self::InvalidRequest { rule } => write!(f, "invalid request: {rule}"),
            Self::Cancelled => f.write_str("request cancelled"),
            // Actionable, and deliberately free of the matched text: the
            // kinds field carries machine-readable codes for the UI.
            Self::SuspectedSecretBlocked { .. } => f.write_str(
                "this looks like it contains a secret, so nothing was sent — \
                 remove it, or save it as a sensitive snippet instead",
            ),
            Self::SensitiveSnippetBlocked => {
                f.write_str("sensitive snippets never leave this device, so nothing was sent")
            }
            Self::AuthRejected => f.write_str("the provider rejected the API key"),
            Self::ModelNotFound => f.write_str("the provider does not know the configured model"),
            Self::RateLimited => f.write_str("the provider is rate limiting requests"),
            Self::ProviderRejected { status } => {
                write!(f, "the provider rejected the request (status {status})")
            }
            Self::Timeout => f.write_str("the provider did not answer in time"),
            Self::Unreachable => f.write_str("the provider could not be reached"),
            Self::ProviderFailure { status } => {
                write!(f, "the provider failed (status {status})")
            }
            Self::InvalidResponse => f.write_str("the provider answer was not understood"),
            Self::Transport => f.write_str("the connection to the provider broke"),
            Self::EgressLogFailed => {
                f.write_str("the egress log could not be written, so nothing was sent")
            }
        }
    }
}

impl std::error::Error for AiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_variant() -> Vec<AiError> {
        vec![
            AiError::InvalidConfig {
                field: "base_url",
                rule: "r",
            },
            AiError::MissingApiKey,
            AiError::InvalidRequest { rule: "r" },
            AiError::Cancelled,
            AiError::SuspectedSecretBlocked {
                kinds: vec![SensitiveKind::AwsAccessKey],
            },
            AiError::SensitiveSnippetBlocked,
            AiError::AuthRejected,
            AiError::ModelNotFound,
            AiError::RateLimited,
            AiError::ProviderRejected { status: 418 },
            AiError::Timeout,
            AiError::Unreachable,
            AiError::ProviderFailure { status: 500 },
            AiError::InvalidResponse,
            AiError::Transport,
            AiError::EgressLogFailed,
        ]
    }

    #[test]
    fn classifies_each_variant_into_the_three_way_taxonomy() {
        use AiError as E;
        use ErrorClass as C;
        let expected = [
            C::User,     // InvalidConfig
            C::User,     // MissingApiKey
            C::User,     // InvalidRequest
            C::User,     // Cancelled
            C::Business, // SuspectedSecretBlocked
            C::Business, // SensitiveSnippetBlocked
            C::Business, // AuthRejected
            C::Business, // ModelNotFound
            C::Business, // RateLimited
            C::Business, // ProviderRejected
            C::Business, // Timeout
            C::Business, // Unreachable
            C::System,   // ProviderFailure
            C::System,   // InvalidResponse
            C::System,   // Transport
            C::System,   // EgressLogFailed
        ];
        let variants = every_variant();
        assert_eq!(variants.len(), expected.len());
        for (variant, class) in variants.iter().zip(expected) {
            assert_eq!(variant.class(), class, "{variant:?}");
        }
        assert_eq!(E::Timeout.class(), C::Business);
    }

    #[test]
    fn renders_only_static_rule_text() {
        // Regression guard for the log/error red line: no variant's
        // rendering may ever interpolate dynamic request content.
        for variant in every_variant() {
            let text = format!("{variant} / {variant:?}");
            assert!(!text.is_empty());
            for marker in ["sk-", "Bearer", "AKIA"] {
                assert!(!text.contains(marker), "{text}");
            }
        }
    }
}
