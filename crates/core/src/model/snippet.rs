//! The `Snippet` entity (PRD §15.1).

use super::enums::{Platform, SecurityLevel, SnippetType, TriggerMode};
use super::validation::{ValidationError, require_non_blank};
use super::{FolderId, SnippetId, TimestampMs, WorkspaceId};

/// Snippet body storage form.
///
/// The PRD models this as the mutually exclusive column pair
/// `content_plaintext` / `content_ciphertext`; encoding the pair as an enum
/// makes the "sensitive content is ciphertext-only" red line unrepresentable
/// to violate at the type level.
#[derive(Clone, PartialEq, Eq)]
pub enum SnippetContent {
    /// Plaintext body; only valid for `SecurityLevel::Normal`.
    Plaintext(String),
    /// Application-layer encrypted body; required for
    /// `SecurityLevel::Sensitive`. Encryption details belong to the crypto
    /// crate (docs/06_SECURITY_MODEL.md, TASK-023).
    Ciphertext(Vec<u8>),
}

// Manual Debug keeps snippet bodies out of logs and panic messages.
impl std::fmt::Debug for SnippetContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plaintext(s) => write!(f, "Plaintext(<{} chars redacted>)", s.chars().count()),
            Self::Ciphertext(b) => write!(f, "Ciphertext(<{} bytes>)", b.len()),
        }
    }
}

/// A saved text snippet, the central entity of the product (PRD §15.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub id: SnippetId,
    /// Reserved by the PRD; v1.0 has a single implicit workspace.
    pub workspace_id: WorkspaceId,
    pub title: String,
    /// Maps to the `content_plaintext` / `content_ciphertext` column pair.
    pub content: SnippetContent,
    pub snippet_type: SnippetType,
    pub description: Option<String>,
    pub folder_id: Option<FolderId>,
    /// Abbreviation trigger; always paired with `trigger_mode`.
    pub trigger: Option<String>,
    pub trigger_mode: Option<TriggerMode>,
    /// Code language for `SnippetType::Code`; free-form identifier.
    pub language: Option<String>,
    pub security_level: SecurityLevel,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub is_enabled: bool,
    /// Platforms the snippet is available on; empty means all platforms.
    pub platform_scope: Vec<Platform>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
    pub last_used_at: Option<TimestampMs>,
    pub usage_count: u64,
    /// Monotonic content version, starting at 1.
    pub version: u32,
}

impl Snippet {
    /// Validates cross-field invariants before the snippet enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("title", &self.title)?;

        match (&self.content, self.security_level) {
            (SnippetContent::Plaintext(_), SecurityLevel::Sensitive) => {
                return Err(ValidationError::new(
                    "content/security_level",
                    "sensitive snippets must store ciphertext only",
                ));
            }
            (SnippetContent::Ciphertext(_), SecurityLevel::Normal) => {
                return Err(ValidationError::new(
                    "content/security_level",
                    "normal snippets must store plaintext",
                ));
            }
            _ => {}
        }

        if self.snippet_type == SnippetType::Sensitive
            && self.security_level != SecurityLevel::Sensitive
        {
            return Err(ValidationError::new(
                "snippet_type/security_level",
                "sensitive type requires sensitive security level",
            ));
        }

        match (&self.trigger, self.trigger_mode) {
            (Some(trigger), Some(_)) => {
                require_non_blank("trigger", trigger)?;
                if trigger.chars().any(|c| c.is_whitespace() || c.is_control()) {
                    return Err(ValidationError::new(
                        "trigger",
                        "must not contain whitespace or control characters",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(ValidationError::new(
                    "trigger/trigger_mode",
                    "trigger and trigger_mode must be set together",
                ));
            }
        }

        if self.version == 0 {
            return Err(ValidationError::new("version", "must start at 1"));
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn normal_snippet() -> Snippet {
        Snippet {
            id: "s1".to_string(),
            workspace_id: "w1".to_string(),
            title: "Docker logs".to_string(),
            content: SnippetContent::Plaintext("docker logs -f app".to_string()),
            snippet_type: SnippetType::Command,
            description: None,
            folder_id: None,
            trigger: Some(":dlog".to_string()),
            trigger_mode: Some(TriggerMode::Delimiter),
            language: None,
            security_level: SecurityLevel::Normal,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
            last_used_at: None,
            usage_count: 0,
            version: 1,
        }
    }

    #[test]
    fn accepts_a_well_formed_normal_snippet() {
        assert_eq!(normal_snippet().validate(), Ok(()));
    }

    #[test]
    fn accepts_a_well_formed_sensitive_snippet() {
        let mut s = normal_snippet();
        s.snippet_type = SnippetType::Sensitive;
        s.security_level = SecurityLevel::Sensitive;
        s.content = SnippetContent::Ciphertext(vec![0xAA, 0xBB]);
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn rejects_sensitive_snippet_with_plaintext_content() {
        let mut s = normal_snippet();
        s.security_level = SecurityLevel::Sensitive;
        let err = s.validate().unwrap_err();
        assert_eq!(err.field, "content/security_level");
    }

    #[test]
    fn rejects_normal_snippet_with_ciphertext_content() {
        let mut s = normal_snippet();
        s.content = SnippetContent::Ciphertext(vec![1, 2, 3]);
        let err = s.validate().unwrap_err();
        assert_eq!(err.field, "content/security_level");
    }

    #[test]
    fn rejects_sensitive_type_with_normal_security_level() {
        let mut s = normal_snippet();
        s.snippet_type = SnippetType::Sensitive;
        let err = s.validate().unwrap_err();
        assert_eq!(err.field, "snippet_type/security_level");
    }

    #[test]
    fn rejects_blank_title() {
        let mut s = normal_snippet();
        s.title = "   ".to_string();
        assert_eq!(s.validate().unwrap_err().field, "title");
    }

    #[test]
    fn rejects_trigger_containing_whitespace() {
        let mut s = normal_snippet();
        s.trigger = Some(":my trig".to_string());
        assert_eq!(s.validate().unwrap_err().field, "trigger");
    }

    #[test]
    fn rejects_trigger_without_trigger_mode() {
        let mut s = normal_snippet();
        s.trigger_mode = None;
        assert_eq!(s.validate().unwrap_err().field, "trigger/trigger_mode");
    }

    #[test]
    fn rejects_version_zero() {
        let mut s = normal_snippet();
        s.version = 0;
        assert_eq!(s.validate().unwrap_err().field, "version");
    }

    #[test]
    fn debug_output_never_contains_snippet_body() {
        let s = normal_snippet();
        let debug = format!("{s:?}");
        assert!(!debug.contains("docker logs"), "body leaked: {debug}");
        assert!(debug.contains("redacted"));
    }
}
