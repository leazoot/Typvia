// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! App-rule evaluation.
//!
//! Rules are a convenience filter, never a security boundary — the vault
//! gate protects sensitive content regardless of the outcome here. That is
//! why every ambiguity resolves to *allow*: no rules, an unidentifiable
//! foreground app, or a rule this version cannot honor faithfully (a
//! window-title pattern, which v1 never collects) all leave the snippet
//! fully available.

use crate::model::{AppRule, AppRuleType, Platform};

/// What the current foreground app allows for one snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppRuleDecision {
    /// The snippet may appear in the panel / search results.
    pub visible: bool,
    /// Abbreviation expansion (Espanso) may fire. Never true while hidden:
    /// a snippet disabled in an app must not expand there either.
    pub expansion_enabled: bool,
    /// Sensitive injection into this app is permitted.
    pub sensitive_injection_allowed: bool,
}

impl AppRuleDecision {
    /// The default-allow decision.
    pub const ALLOW_ALL: Self = Self {
        visible: true,
        expansion_enabled: true,
        sensitive_injection_allowed: true,
    };
}

/// Evaluates one snippet's rules against the identified foreground app.
///
/// `foreground_app` is the bundle id collected at panel-summon time; `None`
/// means the app could not be identified and everything stays allowed.
/// Matching is ASCII case-insensitive; only rules for `platform` apply.
pub fn evaluate(
    rules: &[AppRule],
    platform: Platform,
    foreground_app: Option<&str>,
) -> AppRuleDecision {
    let Some(app) = foreground_app else {
        return AppRuleDecision::ALLOW_ALL;
    };
    // v1 never reads window titles (security red line), so a rule carrying a
    // pattern cannot be matched faithfully and is skipped (= allow).
    let applicable = || {
        rules
            .iter()
            .filter(|r| r.platform == platform && r.window_title_pattern.is_none())
    };
    let matches = |r: &AppRule| r.app_identifier.eq_ignore_ascii_case(app);

    let mut has_show_only = false;
    let mut show_only_hit = false;
    let mut disabled = false;
    let mut expansion_off = false;
    let mut sensitive_denied = false;
    for rule in applicable() {
        let hit = matches(rule);
        match rule.rule_type {
            AppRuleType::ShowOnly => {
                has_show_only = true;
                show_only_hit |= hit;
            }
            AppRuleType::Disable => disabled |= hit,
            AppRuleType::DisableExpansion => expansion_off |= hit,
            AppRuleType::DenySensitiveInjection => sensitive_denied |= hit,
        }
    }

    // `disable` beats `show_only`; an allowlist hides everywhere it misses.
    let visible = !disabled && (!has_show_only || show_only_hit);
    AppRuleDecision {
        visible,
        expansion_enabled: visible && !expansion_off,
        sensitive_injection_allowed: !sensitive_denied,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn rule(rule_type: AppRuleType, app: &str) -> AppRule {
        AppRule {
            id: format!("r-{}-{app}", rule_type.as_str()),
            snippet_id: "s1".to_string(),
            platform: Platform::Macos,
            app_identifier: app.to_string(),
            rule_type,
            window_title_pattern: None,
        }
    }

    #[test]
    fn no_rules_allow_everything() {
        let decision = evaluate(&[], Platform::Macos, Some("com.apple.Terminal"));
        assert_eq!(decision, AppRuleDecision::ALLOW_ALL);
    }

    #[test]
    fn unidentified_foreground_app_allows_everything() {
        let rules = vec![rule(AppRuleType::ShowOnly, "com.apple.Terminal")];
        assert_eq!(
            evaluate(&rules, Platform::Macos, None),
            AppRuleDecision::ALLOW_ALL
        );
    }

    #[test]
    fn show_only_hides_everywhere_but_the_matched_app() {
        let rules = vec![rule(AppRuleType::ShowOnly, "com.apple.Terminal")];
        assert!(evaluate(&rules, Platform::Macos, Some("com.apple.Terminal")).visible);
        let elsewhere = evaluate(&rules, Platform::Macos, Some("com.apple.Safari"));
        assert!(!elsewhere.visible);
        assert!(!elsewhere.expansion_enabled, "hidden implies no expansion");
    }

    #[test]
    fn any_matching_show_only_of_several_is_enough() {
        let rules = vec![
            rule(AppRuleType::ShowOnly, "com.apple.Terminal"),
            rule(AppRuleType::ShowOnly, "com.googlecode.iterm2"),
        ];
        assert!(evaluate(&rules, Platform::Macos, Some("com.googlecode.iterm2")).visible);
    }

    #[test]
    fn disable_beats_show_only() {
        let rules = vec![
            rule(AppRuleType::ShowOnly, "com.apple.Terminal"),
            rule(AppRuleType::Disable, "com.apple.Terminal"),
        ];
        assert!(!evaluate(&rules, Platform::Macos, Some("com.apple.Terminal")).visible);
    }

    #[test]
    fn disable_expansion_keeps_the_snippet_visible() {
        let rules = vec![rule(AppRuleType::DisableExpansion, "com.apple.Terminal")];
        let decision = evaluate(&rules, Platform::Macos, Some("com.apple.Terminal"));
        assert!(decision.visible);
        assert!(!decision.expansion_enabled);
        assert!(decision.sensitive_injection_allowed);
    }

    #[test]
    fn deny_sensitive_injection_only_blocks_sensitive_delivery() {
        let rules = vec![rule(
            AppRuleType::DenySensitiveInjection,
            "com.apple.Terminal",
        )];
        let decision = evaluate(&rules, Platform::Macos, Some("com.apple.Terminal"));
        assert!(decision.visible);
        assert!(decision.expansion_enabled);
        assert!(!decision.sensitive_injection_allowed);
    }

    #[test]
    fn matching_is_ascii_case_insensitive() {
        let rules = vec![rule(AppRuleType::Disable, "com.apple.Terminal")];
        assert!(!evaluate(&rules, Platform::Macos, Some("COM.APPLE.TERMINAL")).visible);
    }

    #[test]
    fn rules_for_another_platform_are_ignored() {
        let mut windows_rule = rule(AppRuleType::Disable, "com.apple.Terminal");
        windows_rule.platform = Platform::Windows;
        assert!(evaluate(&[windows_rule], Platform::Macos, Some("com.apple.Terminal")).visible);
    }

    #[test]
    fn a_window_title_pattern_rule_is_skipped_in_v1() {
        let mut patterned = rule(AppRuleType::Disable, "com.apple.Terminal");
        patterned.window_title_pattern = Some("prod console".to_string());
        // v1 never collects window titles, so the rule cannot be honored
        // faithfully; it must not fire on the app id alone.
        assert!(evaluate(&[patterned], Platform::Macos, Some("com.apple.Terminal")).visible);
    }
}
