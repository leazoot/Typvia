//! massCode import: the app's `db.json` database file.
//!
//! massCode (v2/v3) stores `{"folders": [...], "snippets": [...], "tags":
//! [...]}` where each snippet has a `name` and a `content` array of fragments
//! (`{"label", "value", "language"}`). Each non-blank fragment imports as one
//! snippet titled `name` (`name · label` when a snippet has several
//! fragments); folders and tags are not imported. Snippets in massCode's
//! trash (`isDeleted: true`) are reported as skipped, as are blank fragments.
//! A `content` that is a plain string (older exports) is accepted as a single
//! fragment.

use serde_json::Value;

use super::{ImportParseError, ImportedEntry, ParsedImport, SkippedEntry};

pub(super) fn parse(text: &str) -> Result<ParsedImport, ImportParseError> {
    let value: Value = serde_json::from_str(text).map_err(|_| ImportParseError::InvalidJson)?;
    let snippets = value
        .get("snippets")
        .and_then(Value::as_array)
        .ok_or(ImportParseError::NotMasscodeJson)?;

    let mut result = ParsedImport::default();
    for (index, snippet) in snippets.iter().enumerate() {
        classify(snippet, index, &mut result);
    }
    Ok(result)
}

/// Routes one massCode snippet into imports or a skip with a reason.
fn classify(snippet: &Value, index: usize, result: &mut ParsedImport) {
    let name = snippet.get("name").and_then(Value::as_str).unwrap_or("");
    let label = || {
        Some(name.to_string())
            .filter(|n| !n.trim().is_empty())
            .or(Some(format!("snippet {}", index + 1)))
    };
    if !snippet.is_object() {
        result.skipped.push(SkippedEntry {
            label: label(),
            reason: "entry is not an object".to_string(),
        });
        return;
    }
    if snippet.get("isDeleted").and_then(Value::as_bool) == Some(true) {
        result.skipped.push(SkippedEntry {
            label: label(),
            reason: "in massCode trash".to_string(),
        });
        return;
    }

    let fragments = fragments(snippet.get("content"));
    if fragments.is_empty() {
        result.skipped.push(SkippedEntry {
            label: label(),
            reason: "snippet has no content".to_string(),
        });
        return;
    }
    let many = fragments.len() > 1;
    for (fragment_label, value) in fragments {
        let title = match (&fragment_label, many) {
            (Some(fragment), true) if !name.trim().is_empty() => {
                Some(format!("{name} · {fragment}"))
            }
            _ => Some(name.to_string()).filter(|n| !n.trim().is_empty()),
        };
        result.entries.push(ImportedEntry {
            title,
            content: value,
            trigger: None,
            word: false,
            description: None,
        });
    }
}

/// Collects the non-blank text fragments of a snippet's `content` field,
/// which is an array of `{label, value}` objects or (older exports) a string.
fn fragments(content: Option<&Value>) -> Vec<(Option<String>, String)> {
    match content {
        Some(Value::String(text)) if !text.trim().is_empty() => vec![(None, text.clone())],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let value = item.get("value").and_then(Value::as_str)?;
                if value.trim().is_empty() {
                    return None;
                }
                let label = item
                    .get("label")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|l| !l.trim().is_empty());
                Some((label, value.to_string()))
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_masscode_snippets_and_labels_multiple_fragments() {
        let text = r#"{
            "folders": [{"name": "Dev"}],
            "snippets": [
                {"name": "Greeting", "content": [{"label": "Fragment 1", "value": "Hello", "language": "plain_text"}], "isDeleted": false},
                {"name": "Setup", "content": [
                    {"label": "Install", "value": "brew install x"},
                    {"label": "Run", "value": "x --serve"}
                ]}
            ],
            "tags": []
        }"#;
        let parsed = parse(text).expect("parses");
        let titles: Vec<Option<&str>> = parsed.entries.iter().map(|e| e.title.as_deref()).collect();
        assert_eq!(
            titles,
            vec![
                Some("Greeting"),
                Some("Setup · Install"),
                Some("Setup · Run")
            ]
        );
        assert_eq!(parsed.entries[0].content, "Hello");
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn reports_trashed_and_empty_snippets_as_skipped() {
        let text = r#"{"snippets": [
            {"name": "Old", "content": [{"value": "gone"}], "isDeleted": true},
            {"name": "Hollow", "content": []}
        ]}"#;
        let parsed = parse(text).expect("parses");
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.skipped[0].label.as_deref(), Some("Old"));
        assert_eq!(parsed.skipped[0].reason, "in massCode trash");
        assert_eq!(parsed.skipped[1].reason, "snippet has no content");
    }

    #[test]
    fn accepts_a_plain_string_content_from_older_exports() {
        let parsed = parse(r#"{"snippets": [{"name": "Legacy", "content": "plain body"}]}"#)
            .expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].content, "plain body");
        assert_eq!(parsed.entries[0].title.as_deref(), Some("Legacy"));
    }

    #[test]
    fn rejects_json_that_is_not_a_masscode_database() {
        assert_eq!(parse("[1, 2]"), Err(ImportParseError::NotMasscodeJson));
        assert_eq!(
            parse(r#"{"folders": []}"#),
            Err(ImportParseError::NotMasscodeJson)
        );
        assert_eq!(parse("{broken"), Err(ImportParseError::InvalidJson));
    }
}
