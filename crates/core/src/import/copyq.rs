//! CopyQ import: a JSON array of clipboard items.
//!
//! CopyQ's native "File → Export" writes an undocumented Qt binary format
//! (`.cpq`) that is not parseable outside Qt, so Typvia imports the JSON its
//! scripting interface produces instead (the UI shows the exact one-liner):
//!
//! ```text
//! copyq eval "var r=[];for(var i=0;i<size();i++)r.push(str(read(i)));JSON.stringify(r)"
//! ```
//!
//! Accepted items are plain strings, or objects carrying the text under a
//! `text` or `text/plain` key. Blank and non-text items (images, custom
//! MIME data) are reported as skipped.

use serde_json::Value;

use super::{ImportParseError, ImportedEntry, ParsedImport, SkippedEntry};

pub(super) fn parse(text: &str) -> Result<ParsedImport, ImportParseError> {
    let value: Value = serde_json::from_str(text).map_err(|_| ImportParseError::InvalidJson)?;
    let items = value.as_array().ok_or(ImportParseError::NotCopyqJson)?;

    let mut result = ParsedImport::default();
    for (index, item) in items.iter().enumerate() {
        let label = || Some(format!("item {}", index + 1));
        match item_text(item) {
            Some(content) if !content.trim().is_empty() => {
                result.entries.push(ImportedEntry {
                    title: None,
                    content,
                    trigger: None,
                    word: false,
                    description: None,
                });
            }
            Some(_) => result.skipped.push(SkippedEntry {
                label: label(),
                reason: "item is empty".to_string(),
            }),
            None => result.skipped.push(SkippedEntry {
                label: label(),
                reason: "item has no text".to_string(),
            }),
        }
    }
    Ok(result)
}

/// Extracts the text of one exported item: a string, or an object's `text` /
/// `text/plain` field.
fn item_text(item: &Value) -> Option<String> {
    match item {
        Value::String(text) => Some(text.clone()),
        Value::Object(map) => map
            .get("text")
            .or_else(|| map.get("text/plain"))
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_string_and_object_items() {
        let text = r#"["ssh admin@host", {"text": "SELECT 1;"}, {"text/plain": "third"}]"#;
        let parsed = parse(text).expect("parses");
        let bodies: Vec<&str> = parsed.entries.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(bodies, vec!["ssh admin@host", "SELECT 1;", "third"]);
        assert!(parsed.entries.iter().all(|e| e.title.is_none()));
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn skips_blank_and_non_text_items_with_reasons() {
        let text = r#"["", {"image/png": "…"}, 42, "kept"]"#;
        let parsed = parse(text).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        let reasons: Vec<&str> = parsed.skipped.iter().map(|s| s.reason.as_str()).collect();
        assert_eq!(
            reasons,
            vec!["item is empty", "item has no text", "item has no text"]
        );
        assert_eq!(parsed.skipped[0].label.as_deref(), Some("item 1"));
    }

    #[test]
    fn rejects_json_that_is_not_an_item_array() {
        assert_eq!(
            parse(r#"{"items": []}"#),
            Err(ImportParseError::NotCopyqJson)
        );
        assert_eq!(parse("{broken"), Err(ImportParseError::InvalidJson));
    }
}
