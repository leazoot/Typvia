// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Compile Typvia snippets into an espanso match configuration.
//!
//! Security red line: a snippet whose `security_level` is anything other than
//! [`SecurityLevel::Normal`] — and any ciphertext body — must never reach
//! the generated configuration. That filter
//! lives in [`eligible_match`] and is guarded by a byte-level test.
//!
//! Only enabled, normal-security snippets that carry a trigger become matches.
//! The generated YAML is emitted by a small dedicated writer (rather than a
//! general YAML library) so the exact bytes leaving this crate are fully under
//! our control.

use std::fmt;

use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, TriggerMode};

/// A generated espanso match configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledConfig {
    /// The YAML document ready to be written to `match/typvia.yml`.
    pub yaml: String,
    /// Number of matches emitted (excludes filtered-out snippets).
    pub match_count: usize,
}

/// Compilation failures. Messages carry only the offending snippet id (an
/// opaque UUID), never any snippet body or trigger content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// An enabled, triggered snippet has a blank trigger after trimming.
    EmptyTrigger { snippet_id: String },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTrigger { snippet_id } => {
                write!(f, "snippet {snippet_id} has an empty trigger")
            }
        }
    }
}

impl std::error::Error for CompileError {}

/// Compile a snippet set into an espanso match configuration.
///
/// Snippets are silently skipped when they are disabled, sensitive, have a
/// ciphertext body, or carry no trigger — those are expected exclusions, not
/// errors. An included snippet with a blank trigger is a [`CompileError`], so
/// the caller can preserve the previous config rather than write a broken one.
pub fn compile_snippets(snippets: &[Snippet]) -> Result<CompiledConfig, CompileError> {
    let mut matches = Vec::new();
    for snippet in snippets {
        if let Some(m) = eligible_match(snippet)? {
            matches.push(m);
        }
    }
    let yaml = emit_yaml(&matches);
    Ok(CompiledConfig {
        yaml,
        match_count: matches.len(),
    })
}

/// The espanso form of a single match.
struct Match {
    trigger: Trigger,
    boundary: Boundary,
    replace: String,
}

enum Trigger {
    /// A literal abbreviation (`trigger:`).
    Literal(String),
    /// A regular-expression trigger (`regex:`). Pattern validity is espanso's
    /// to check at load time; the live `espanso match list` check surfaces a
    /// bad pattern (no regex engine is linked here).
    Regex(String),
}

enum Boundary {
    /// Fires as soon as the trigger is typed (no `word` option).
    Immediate,
    /// Fires only when the trigger stands as a whole word (`word: true`).
    Word,
}

/// Apply the red-line filter and map a snippet to its espanso match.
///
/// Returns `Ok(None)` for snippets that are intentionally excluded and
/// `Err` only for an included snippet that is malformed.
fn eligible_match(snippet: &Snippet) -> Result<Option<Match>, CompileError> {
    if !snippet.is_enabled {
        return Ok(None);
    }
    // Red line: sensitive snippets never enter the espanso config.
    if snippet.security_level != SecurityLevel::Normal {
        return Ok(None);
    }
    let replace = match &snippet.content {
        SnippetContent::Plaintext(body) => body.clone(),
        // Belt-and-suspenders: a ciphertext body never reaches the config even
        // if some future bug mislabeled it as normal-security.
        SnippetContent::Ciphertext(_) => return Ok(None),
    };
    let (trigger, mode) = match (&snippet.trigger, snippet.trigger_mode) {
        (Some(trigger), Some(mode)) => (trigger, mode),
        // No trigger → nothing to expand.
        _ => return Ok(None),
    };
    if trigger.trim().is_empty() {
        return Err(CompileError::EmptyTrigger {
            snippet_id: snippet.id.clone(),
        });
    }

    // Espanso's trigger model is coarser than Typvia's: it distinguishes only
    // "fire immediately" from "fire on a word boundary" (`word: true`). Both
    // Delimiter (expand after a terminator) and WordBoundary (expand as a whole
    // word) therefore map to `word: true` — a documented mapping collapse.
    let (trigger, boundary) = match mode {
        TriggerMode::Immediate => (Trigger::Literal(trigger.clone()), Boundary::Immediate),
        TriggerMode::Delimiter | TriggerMode::WordBoundary => {
            (Trigger::Literal(trigger.clone()), Boundary::Word)
        }
        TriggerMode::Regex => (Trigger::Regex(trigger.clone()), Boundary::Immediate),
    };
    Ok(Some(Match {
        trigger,
        boundary,
        replace,
    }))
}

/// Header written at the top of every generated file so a user who opens it
/// understands it is machine-managed.
const GENERATED_HEADER: &str =
    "# Generated by Typvia. Do not edit — this file is regenerated from your snippets.\n";

fn emit_yaml(matches: &[Match]) -> String {
    if matches.is_empty() {
        return format!("{GENERATED_HEADER}matches: []\n");
    }
    let mut out = String::from(GENERATED_HEADER);
    out.push_str("matches:\n");
    for m in matches {
        match &m.trigger {
            Trigger::Literal(trigger) => {
                out.push_str("  - trigger: ");
                out.push_str(&dquote(trigger));
            }
            Trigger::Regex(pattern) => {
                out.push_str("  - regex: ");
                out.push_str(&dquote(pattern));
            }
        }
        out.push('\n');
        out.push_str("    replace: ");
        out.push_str(&dquote(&m.replace));
        out.push('\n');
        if matches!(m.boundary, Boundary::Word) {
            out.push_str("    word: true\n");
        }
    }
    out
}

/// Emit a value as a YAML double-quoted scalar, escaping the characters that
/// would otherwise break the document. Non-ASCII (e.g. CJK) is kept literal —
/// valid UTF-8 inside a double-quoted scalar and readable to the user.
fn dquote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\x{:02X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal normal snippet builder for the compiler tests.
    fn snippet(id: &str, trigger: Option<&str>, mode: Option<TriggerMode>, body: &str) -> Snippet {
        Snippet {
            id: id.to_string(),
            workspace_id: "w1".to_string(),
            title: "t".to_string(),
            content: SnippetContent::Plaintext(body.to_string()),
            snippet_type: typvia_core::model::SnippetType::Text,
            description: None,
            folder_id: None,
            trigger: trigger.map(str::to_string),
            trigger_mode: mode,
            language: None,
            security_level: SecurityLevel::Normal,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 1,
            updated_at: 1,
            last_used_at: None,
            usage_count: 0,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        }
    }

    #[test]
    fn immediate_trigger_emits_no_word_option() {
        let set = [snippet(
            "s1",
            Some(":now"),
            Some(TriggerMode::Immediate),
            "value",
        )];
        let config = compile_snippets(&set).expect("compiles");
        assert_eq!(config.match_count, 1);
        assert!(config.yaml.contains("  - trigger: \":now\"\n"));
        assert!(config.yaml.contains("    replace: \"value\"\n"));
        assert!(!config.yaml.contains("word: true"));
    }

    #[test]
    fn delimiter_and_word_boundary_both_emit_word_true() {
        let delimiter = [snippet("s1", Some(":a"), Some(TriggerMode::Delimiter), "A")];
        let word = [snippet(
            "s2",
            Some(":b"),
            Some(TriggerMode::WordBoundary),
            "B",
        )];
        assert!(
            compile_snippets(&delimiter)
                .expect("compiles")
                .yaml
                .contains("    word: true\n")
        );
        assert!(
            compile_snippets(&word)
                .expect("compiles")
                .yaml
                .contains("    word: true\n")
        );
    }

    #[test]
    fn regex_mode_emits_regex_field_not_trigger() {
        let set = [snippet(
            "s1",
            Some(r":date\d+"),
            Some(TriggerMode::Regex),
            "value",
        )];
        let yaml = compile_snippets(&set).expect("compiles").yaml;
        assert!(
            yaml.contains("  - regex: \":date\\\\d+\"\n"),
            "yaml: {yaml}"
        );
        assert!(!yaml.contains("- trigger:"));
    }

    #[test]
    fn escapes_quotes_backslashes_and_newlines_in_replace() {
        let body = "line1\nquote \" and back\\slash\ttab";
        let set = [snippet(
            "s1",
            Some(":x"),
            Some(TriggerMode::Immediate),
            body,
        )];
        let yaml = compile_snippets(&set).expect("compiles").yaml;
        assert!(
            yaml.contains("    replace: \"line1\\nquote \\\" and back\\\\slash\\ttab\"\n"),
            "yaml: {yaml}"
        );
    }

    #[test]
    fn keeps_cjk_content_literal() {
        let set = [snippet(
            "s1",
            Some(":greet"),
            Some(TriggerMode::Immediate),
            "你好世界",
        )];
        let yaml = compile_snippets(&set).expect("compiles").yaml;
        assert!(yaml.contains("    replace: \"你好世界\"\n"), "yaml: {yaml}");
    }

    #[test]
    fn skips_disabled_and_untriggered_snippets() {
        let mut disabled = snippet("s1", Some(":d"), Some(TriggerMode::Immediate), "D");
        disabled.is_enabled = false;
        let no_trigger = snippet("s2", None, None, "N");
        let config = compile_snippets(&[disabled, no_trigger]).expect("compiles");
        assert_eq!(config.match_count, 0);
        assert_eq!(config.yaml, format!("{GENERATED_HEADER}matches: []\n"));
    }

    #[test]
    fn blank_trigger_on_included_snippet_is_an_error() {
        let set = [snippet(
            "s1",
            Some("   "),
            Some(TriggerMode::Immediate),
            "V",
        )];
        assert_eq!(
            compile_snippets(&set),
            Err(CompileError::EmptyTrigger {
                snippet_id: "s1".to_string()
            })
        );
    }

    #[test]
    fn sensitive_snippet_never_appears_in_output_at_the_byte_level() {
        // Red-line regression (must always run). A sensitive snippet — trigger,
        // ciphertext body and all — must not leak into the generated bytes.
        const CANARY_TRIGGER: &str = ":secretsig";
        const CANARY_BYTES: &[u8] = b"XKCD_SENSITIVE_LEAK_CANARY";
        const SENTINEL_BODY: &str = "NORMAL_BODY_SENTINEL";

        let mut sensitive = snippet(
            "sensitive",
            Some(CANARY_TRIGGER),
            Some(TriggerMode::Immediate),
            "unused",
        );
        sensitive.security_level = SecurityLevel::Sensitive;
        sensitive.content = SnippetContent::Ciphertext(CANARY_BYTES.to_vec());

        let normal = snippet(
            "normal",
            Some(":ok"),
            Some(TriggerMode::Immediate),
            SENTINEL_BODY,
        );

        let yaml = compile_snippets(&[sensitive, normal])
            .expect("compiles")
            .yaml;
        let bytes = yaml.as_bytes();

        // The normal snippet is present…
        assert!(yaml.contains(SENTINEL_BODY));
        assert!(yaml.contains(":ok"));
        // …and nothing of the sensitive one is, at the byte level.
        assert!(
            !contains_bytes(bytes, CANARY_TRIGGER.as_bytes()),
            "sensitive trigger leaked into config"
        );
        assert!(
            !contains_bytes(bytes, CANARY_BYTES),
            "sensitive ciphertext leaked into config"
        );
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn compile_error_lets_caller_preserve_the_previous_config() {
        // Failure-preserves-old: a compile error must let the caller keep the
        // existing config rather than write a broken one.
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("match").join("typvia.yml");
        crate::write_config(&target, "# OLD\nmatches: []\n").expect("seed old config");
        let before = std::fs::read_to_string(&target).expect("read old");

        let bad = [snippet("s1", Some("  "), Some(TriggerMode::Immediate), "V")];
        let result = compile_snippets(&bad);
        assert!(result.is_err());
        // The host writes only on success; on error the old config stays.
        if let Ok(config) = result {
            crate::write_config(&target, &config.yaml).expect("write");
        }

        assert_eq!(
            std::fs::read_to_string(&target).expect("read after"),
            before
        );
    }
}
