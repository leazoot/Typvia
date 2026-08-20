//! JSON import: a top-level array of snippet objects, or `{"snippets": [...]}`.
//!
//! Each object needs a non-blank string `content`; `title`, `trigger`,
//! `description` (strings) and `word` (bool) are optional and unknown fields
//! are ignored. A malformed entry is skipped with a reason; only a file that
//! is not a snippet collection at all fails as a whole.

use serde_json::{Map, Value};

use super::{ImportParseError, ImportedEntry, ParsedImport, SkippedEntry, normalize_trigger};

pub(super) fn parse(text: &str) -> Result<ParsedImport, ImportParseError> {
    let value: Value = serde_json::from_str(text).map_err(|_| ImportParseError::InvalidJson)?;
    let list = match &value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("snippets")
            .and_then(Value::as_array)
            .ok_or(ImportParseError::NotSnippetJson)?,
        _ => return Err(ImportParseError::NotSnippetJson),
    };

    let mut result = ParsedImport::default();
    for (index, item) in list.iter().enumerate() {
        match entry(item) {
            Ok(entry) => result.entries.push(entry),
            Err(reason) => result.skipped.push(SkippedEntry {
                label: Some(label(item, index)),
                reason: reason.to_string(),
            }),
        }
    }
    Ok(result)
}

/// Validates one array element into an entry, or explains why it is skipped.
fn entry(item: &Value) -> Result<ImportedEntry, &'static str> {
    let map = item.as_object().ok_or("entry is not an object")?;
    let content = match map.get("content") {
        Some(Value::String(content)) if !content.trim().is_empty() => content.clone(),
        Some(Value::String(_)) | None | Some(Value::Null) => return Err("entry has no content"),
        Some(_) => return Err("content is not text"),
    };
    Ok(ImportedEntry {
        title: optional_text(map, "title")?,
        content,
        trigger: normalize_trigger(optional_text(map, "trigger")?.as_deref()),
        word: match map.get("word") {
            Some(Value::Bool(word)) => *word,
            None | Some(Value::Null) => false,
            Some(_) => return Err("word is not a boolean"),
        },
        description: optional_text(map, "description")?,
    })
}

/// Reads an optional string field; a present non-string value is a skip
/// reason (foreign input is validated, not coerced).
fn optional_text(
    map: &Map<String, Value>,
    key: &'static str,
) -> Result<Option<String>, &'static str> {
    match map.get(key) {
        Some(Value::String(text)) => Ok(Some(text.clone()).filter(|t| !t.trim().is_empty())),
        None | Some(Value::Null) => Ok(None),
        Some(_) => Err(match key {
            "title" => "title is not text",
            "trigger" => "trigger is not text",
            _ => "description is not text",
        }),
    }
}

/// Identifies a skipped element in the report: its title or trigger when
/// readable, otherwise its position.
fn label(item: &Value, index: usize) -> String {
    item.get("title")
        .or_else(|| item.get("trigger"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("entry {}", index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_an_array_of_snippet_objects() {
        let text = r#"[
            {"title": "Sig", "content": "Best regards", "trigger": ":sig", "word": true},
            {"content": "no frills"}
        ]"#;
        let parsed = parse(text).expect("parses");
        assert_eq!(
            parsed.entries,
            vec![
                ImportedEntry {
                    title: Some("Sig".to_string()),
                    content: "Best regards".to_string(),
                    trigger: Some(":sig".to_string()),
                    word: true,
                    description: None,
                },
                ImportedEntry {
                    title: None,
                    content: "no frills".to_string(),
                    trigger: None,
                    word: false,
                    description: None,
                },
            ]
        );
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn accepts_a_snippets_wrapper_object() {
        let parsed = parse(r#"{"snippets": [{"content": "wrapped"}]}"#).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].content, "wrapped");
    }

    #[test]
    fn skips_malformed_entries_with_reasons() {
        let text = r#"[
            {"title": "NoBody"},
            {"content": 42},
            {"content": "ok", "trigger": 7},
            "just a string"
        ]"#;
        let parsed = parse(text).expect("parses");
        assert!(parsed.entries.is_empty());
        let reasons: Vec<&str> = parsed.skipped.iter().map(|s| s.reason.as_str()).collect();
        assert_eq!(
            reasons,
            vec![
                "entry has no content",
                "content is not text",
                "trigger is not text",
                "entry is not an object",
            ]
        );
        assert_eq!(parsed.skipped[0].label.as_deref(), Some("NoBody"));
        assert_eq!(parsed.skipped[3].label.as_deref(), Some("entry 4"));
    }

    #[test]
    fn rejects_non_collection_json() {
        assert_eq!(parse("\"hello\""), Err(ImportParseError::NotSnippetJson));
        assert_eq!(parse("{\"a\": 1}"), Err(ImportParseError::NotSnippetJson));
        assert_eq!(parse("{not json"), Err(ImportParseError::InvalidJson));
    }
}
