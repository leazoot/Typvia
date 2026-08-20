//! CSV import: RFC 4180 with a header row.
//!
//! The header must name a `content` column; `title`, `trigger`,
//! `description` and `word` columns are optional. Header names are matched
//! case-insensitively and unknown columns are ignored. A row without content
//! is skipped with a reason; only an unparseable file fails as a whole.

use super::{ImportParseError, ImportedEntry, ParsedImport, SkippedEntry, normalize_trigger};

pub(super) fn parse(text: &str) -> Result<ParsedImport, ImportParseError> {
    let mut reader = ::csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| ImportParseError::InvalidCsv)?
        .clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case(name))
    };
    let content_col = column("content").ok_or(ImportParseError::MissingContentColumn)?;
    let title_col = column("title");
    let trigger_col = column("trigger");
    let description_col = column("description");
    let word_col = column("word");

    let mut result = ParsedImport::default();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|_| ImportParseError::InvalidCsv)?;
        // Row numbers in skip labels are 1-based file lines (header = row 1).
        let label = || Some(format!("row {}", index + 2));
        let cell = |col: Option<usize>| col.and_then(|c| record.get(c)).map(str::to_string);
        match cell(Some(content_col)).filter(|content| !content.trim().is_empty()) {
            Some(content) => result.entries.push(ImportedEntry {
                title: cell(title_col).filter(|t| !t.trim().is_empty()),
                content,
                trigger: normalize_trigger(cell(trigger_col).as_deref()),
                word: cell(word_col).is_some_and(|w| is_true(&w)),
                description: cell(description_col).filter(|d| !d.trim().is_empty()),
            }),
            None => result.skipped.push(SkippedEntry {
                label: label(),
                reason: "row has no content".to_string(),
            }),
        }
    }
    Ok(result)
}

/// Accepts the common spreadsheet spellings of a true flag.
fn is_true(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_rows_with_optional_columns() {
        let text = "title,content,trigger,word\nSig,Best regards,:sig,true\n,plain body,,\n";
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
                    content: "plain body".to_string(),
                    trigger: None,
                    word: false,
                    description: None,
                },
            ]
        );
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn handles_quoted_fields_with_commas_and_newlines() {
        let text = "content,title\n\"line one\nline two, still\",\"Multi\"\n";
        let parsed = parse(text).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].content, "line one\nline two, still");
        assert_eq!(parsed.entries[0].title.as_deref(), Some("Multi"));
    }

    #[test]
    fn matches_headers_case_insensitively() {
        let parsed = parse("Content,Trigger\nhello,:hi\n").expect("parses");
        assert_eq!(parsed.entries[0].trigger.as_deref(), Some(":hi"));
    }

    #[test]
    fn skips_rows_without_content() {
        let parsed = parse("content,title\n,Empty\nkept,Fine\n").expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.skipped.len(), 1);
        assert_eq!(parsed.skipped[0].label.as_deref(), Some("row 2"));
        assert_eq!(parsed.skipped[0].reason, "row has no content");
    }

    #[test]
    fn rejects_a_header_without_a_content_column() {
        assert_eq!(
            parse("name,value\na,b\n"),
            Err(ImportParseError::MissingContentColumn)
        );
    }

    #[test]
    fn a_short_row_missing_the_content_cell_is_skipped_not_a_panic() {
        let parsed = parse("title,content\nOnlyTitle\n").expect("parses");
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.skipped[0].reason, "row has no content");
    }
}
