//! The `AppRule` entity.

use super::enums::{AppRuleType, Platform};
use super::validation::{ValidationError, require_non_blank};
use super::{AppRuleId, SnippetId};

/// A per-snippet application rule for desktop platforms.
///
/// Mobile v1.0 does no app-level matching; rules therefore target
/// desktop platforms, but the model does not restrict `platform` so future
/// scope changes stay a data change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRule {
    pub id: AppRuleId,
    pub snippet_id: SnippetId,
    pub platform: Platform,
    /// Bundle id (macOS) or executable name (Windows) of the target app.
    pub app_identifier: String,
    pub rule_type: AppRuleType,
    /// Optional window-title filter narrowing the match.
    pub window_title_pattern: Option<String>,
}

impl AppRule {
    /// Validates the rule before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("app_identifier", &self.app_identifier)?;
        if let Some(pattern) = &self.window_title_pattern {
            require_non_blank("window_title_pattern", pattern)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn rule() -> AppRule {
        AppRule {
            id: "r1".to_string(),
            snippet_id: "s1".to_string(),
            platform: Platform::Macos,
            app_identifier: "com.apple.Terminal".to_string(),
            rule_type: AppRuleType::ShowOnly,
            window_title_pattern: None,
        }
    }

    #[test]
    fn accepts_a_well_formed_rule() {
        assert_eq!(rule().validate(), Ok(()));
    }

    #[test]
    fn rejects_blank_app_identifier() {
        let mut r = rule();
        r.app_identifier = "".to_string();
        assert_eq!(r.validate().unwrap_err().field, "app_identifier");
    }

    #[test]
    fn rejects_blank_window_title_pattern_when_present() {
        let mut r = rule();
        r.window_title_pattern = Some("  ".to_string());
        assert_eq!(r.validate().unwrap_err().field, "window_title_pattern");
    }
}
