// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Parse a user's existing espanso match file into importable matches.
//!
//! This reads *foreign* YAML (the user's own espanso config) with a maintained
//! parser and extracts the plain `trigger` + `replace` pairs Typvia can model.
//! Anything espanso-specific that Typvia does not represent (regex triggers,
//! dynamic variables, forms, images, multi-trigger matches) is reported as
//! skipped rather than silently dropped. Malformed input is a business error,
//! never a panic (security rule: external input is validated before use).

use yaml_rust2::{Yaml, YamlLoader};

/// One importable match: a literal trigger and a plain-text replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedMatch {
    pub trigger: String,
    pub replace: String,
    /// espanso `word: true` → expands only as a whole word.
    pub word: bool,
}

/// A match that could not be imported, with a human-readable reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedMatch {
    pub trigger: Option<String>,
    pub reason: String,
}

/// Outcome of parsing a match file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedMatches {
    pub matches: Vec<ImportedMatch>,
    pub skipped: Vec<SkippedMatch>,
}

/// Why a match file could not be parsed at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportParseError {
    /// The text is not valid YAML.
    InvalidYaml,
    /// Valid YAML, but not an espanso match file (no `matches:` list).
    NotAMatchFile,
}

/// Parse the text of an espanso match file. Returns the importable matches plus
/// a report of everything skipped, or an error if the file is not a match file.
pub fn parse_matches(text: &str) -> Result<ParsedMatches, ImportParseError> {
    let docs = YamlLoader::load_from_str(text).map_err(|_| ImportParseError::InvalidYaml)?;
    let doc = docs.first().ok_or(ImportParseError::NotAMatchFile)?;
    let list = doc["matches"]
        .as_vec()
        .ok_or(ImportParseError::NotAMatchFile)?;

    let mut result = ParsedMatches::default();
    for item in list {
        classify(item, &mut result);
    }
    Ok(result)
}

/// Route one `matches` entry into either an import or a skip with a reason.
fn classify(item: &Yaml, result: &mut ParsedMatches) {
    let trigger = item["trigger"].as_str();

    // Espanso features Typvia does not model: report, never drop silently.
    let unsupported = [
        (!item["regex"].is_badvalue(), "regex trigger not supported"),
        (
            !item["triggers"].is_badvalue(),
            "multiple triggers not supported",
        ),
        (
            !item["vars"].is_badvalue(),
            "dynamic variables not supported",
        ),
        (!item["form"].is_badvalue(), "form matches not supported"),
        (
            !item["image_path"].is_badvalue(),
            "image matches not supported",
        ),
    ];
    if let Some((_, reason)) = unsupported.into_iter().find(|(present, _)| *present) {
        result.skipped.push(SkippedMatch {
            trigger: trigger.map(str::to_string),
            reason: reason.to_string(),
        });
        return;
    }

    match (trigger, item["replace"].as_str()) {
        (Some(trigger), Some(replace)) if !trigger.trim().is_empty() => {
            result.matches.push(ImportedMatch {
                trigger: trigger.to_string(),
                replace: replace.to_string(),
                word: item["word"].as_bool().unwrap_or(false),
            });
        }
        (Some(trigger), _) => result.skipped.push(SkippedMatch {
            trigger: Some(trigger.to_string()),
            reason: "replacement is not plain text".to_string(),
        }),
        (None, _) => result.skipped.push(SkippedMatch {
            trigger: None,
            reason: "match has no trigger".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_plain_trigger_replace_pairs() {
        let yaml = r#"
matches:
  - trigger: ":sig"
    replace: "Best regards"
  - trigger: ":addr"
    replace: "1 Main St"
    word: true
"#;
        let parsed = parse_matches(yaml).expect("parses");
        assert_eq!(
            parsed.matches,
            vec![
                ImportedMatch {
                    trigger: ":sig".to_string(),
                    replace: "Best regards".to_string(),
                    word: false,
                },
                ImportedMatch {
                    trigger: ":addr".to_string(),
                    replace: "1 Main St".to_string(),
                    word: true,
                },
            ]
        );
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn skips_unsupported_features_with_reasons() {
        let yaml = r#"
matches:
  - regex: ":num(?P<n>\\d+)"
    replace: "n={{n}}"
  - trigger: ":date"
    replace: "{{mydate}}"
    vars:
      - name: mydate
        type: date
  - triggers: [":a", ":b"]
    replace: "either"
  - trigger: ":ok"
    replace: "fine"
"#;
        let parsed = parse_matches(yaml).expect("parses");
        assert_eq!(parsed.matches.len(), 1);
        assert_eq!(parsed.matches[0].trigger, ":ok");
        let reasons: Vec<&str> = parsed.skipped.iter().map(|s| s.reason.as_str()).collect();
        assert!(reasons.contains(&"regex trigger not supported"));
        assert!(reasons.contains(&"dynamic variables not supported"));
        assert!(reasons.contains(&"multiple triggers not supported"));
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert_eq!(
            parse_matches("matches: [unclosed"),
            Err(ImportParseError::InvalidYaml)
        );
    }

    #[test]
    fn rejects_files_without_a_matches_list() {
        assert_eq!(
            parse_matches("global_vars:\n  - name: x"),
            Err(ImportParseError::NotAMatchFile)
        );
    }

    #[test]
    fn skips_a_match_missing_its_trigger() {
        let parsed = parse_matches("matches:\n  - replace: \"orphan\"\n").expect("parses");
        assert!(parsed.matches.is_empty());
        assert_eq!(parsed.skipped.len(), 1);
        assert_eq!(parsed.skipped[0].reason, "match has no trigger");
    }
}
