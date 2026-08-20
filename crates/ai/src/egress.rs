//! The mandatory egress gate and audit log.
//!
//! Every AI request leaves the device through
//! [`crate::OpenAiCompatProvider`]'s two methods, and neither touches the
//! transport until (a) all outbound content has passed the sensitive scan
//! and (b) the egress log has durably recorded the request (fail-closed:
//! a log write failure aborts the send). There is no switch, no
//! configuration field and no alternative constructor that skips either
//! step — un-disableability is structural, and a red-line test scans this
//! crate's source to keep it that way.

use typvia_core::model::AiRequestClass;
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent};
use typvia_core::sensitive::detect;

use crate::error::AiError;

/// Metadata of one request that is about to leave the device: when it
/// happened is stamped by the sink (the data layer owns the clock), and
/// nothing here can carry content or key material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressEntry {
    pub provider_id: String,
    pub request_class: AiRequestClass,
    /// Request body bytes (0 for body-less probes).
    pub request_bytes: u64,
}

/// Opaque sink failure. Deliberately carries nothing: whatever went wrong
/// at the sink stays at the sink, so no storage detail can ride an error
/// into UI text or logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgressLogError;

/// Where egress entries go. The host implements this over the
/// `ai_egress_log` table and stamps `occurred_at` at write time. Recording
/// happens immediately before the
/// transport call, so an entry means "this device committed to sending" —
/// a cancellation or transport failure racing the send can leave a logged
/// request that never arrived, never the reverse.
pub trait EgressLog: Send + Sync {
    fn record(&self, entry: &EgressEntry) -> Result<(), EgressLogError>;
}

impl From<EgressLogError> for AiError {
    fn from(_: EgressLogError) -> Self {
        AiError::EgressLogFailed
    }
}

/// Refuses outbound text that looks like it contains a secret
/// (`typvia_core::sensitive::detect` — reused, not reimplemented). Runs on
/// every message of every completion request.
pub(crate) fn scan_outbound(text: &str) -> Result<(), AiError> {
    let kinds = detect(text);
    if kinds.is_empty() {
        Ok(())
    } else {
        Err(AiError::SuspectedSecretBlocked { kinds })
    }
}

/// The only sanctioned way for AI features to take a snippet's body for a
/// prompt. Sensitive snippets are refused under every permission scope —
/// and structurally could not pass anyway: their in-memory form is
/// ciphertext-only.
pub fn vetted_snippet_text(snippet: &Snippet) -> Result<&str, AiError> {
    if snippet.security_level != SecurityLevel::Normal {
        return Err(AiError::SensitiveSnippetBlocked);
    }
    match &snippet.content {
        SnippetContent::Plaintext(text) => Ok(text),
        SnippetContent::Ciphertext(_) => Err(AiError::SensitiveSnippetBlocked),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use typvia_core::model::SnippetType;
    use typvia_core::sensitive::SensitiveKind;

    use super::*;

    fn snippet(security_level: SecurityLevel, content: SnippetContent) -> Snippet {
        Snippet {
            id: "s1".to_string(),
            workspace_id: "w1".to_string(),
            title: "t".to_string(),
            content,
            snippet_type: if security_level == SecurityLevel::Sensitive {
                SnippetType::Sensitive
            } else {
                SnippetType::Text
            },
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            security_level,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 0,
            updated_at: 0,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        }
    }

    #[test]
    fn a_sensitive_snippet_is_refused_regardless_of_content_form() {
        let encrypted = snippet(
            SecurityLevel::Sensitive,
            SnippetContent::Ciphertext(vec![1, 2, 3]),
        );
        assert_eq!(
            vetted_snippet_text(&encrypted).unwrap_err(),
            AiError::SensitiveSnippetBlocked
        );
        // Even an (invalid, never-persistable) plaintext-bearing sensitive
        // snippet is refused: the level check comes first.
        let contradictory = snippet(
            SecurityLevel::Sensitive,
            SnippetContent::Plaintext("leaked".to_string()),
        );
        assert_eq!(
            vetted_snippet_text(&contradictory).unwrap_err(),
            AiError::SensitiveSnippetBlocked
        );
        // Ciphertext under a normal label never yields text either.
        let opaque = snippet(SecurityLevel::Normal, SnippetContent::Ciphertext(vec![9]));
        assert_eq!(
            vetted_snippet_text(&opaque).unwrap_err(),
            AiError::SensitiveSnippetBlocked
        );
    }

    #[test]
    fn a_normal_snippet_body_passes_through() {
        let normal = snippet(
            SecurityLevel::Normal,
            SnippetContent::Plaintext("kubectl get pods -A".to_string()),
        );
        assert_eq!(vetted_snippet_text(&normal).unwrap(), "kubectl get pods -A");
    }

    #[test]
    fn the_scan_reports_which_pattern_kinds_blocked_the_text() {
        assert_eq!(scan_outbound("translate this sentence"), Ok(()));
        match scan_outbound("aws key AKIAFAKEFAKEFAKEFAKE inside").unwrap_err() {
            AiError::SuspectedSecretBlocked { kinds } => {
                assert_eq!(kinds, vec![SensitiveKind::AwsAccessKey]);
            }
            other => panic!("expected SuspectedSecretBlocked, got {other:?}"),
        }
    }
}
