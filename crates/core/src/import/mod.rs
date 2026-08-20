//! Parse foreign snippet collections (Markdown / JSON / CSV) into a unified
//! import model.
//!
//! This layer is pure parsing: the host service owns persistence. It mirrors
//! the espanso import contract — malformed or hostile input is a business
//! error, never a panic, and every entry that cannot be imported is reported
//! as skipped with a reason instead of being silently dropped.
//!
//! Format conventions (recorded here as the single source of truth):
//! - Markdown: sections split at the first-seen ATX heading level (heading =
//!   title, section body = content); fenced code blocks never split. A file
//!   with no headings imports as one untitled snippet.
//! - JSON: a top-level array of objects, or `{"snippets": [...]}`. Each
//!   object needs a non-blank string `content`; `title`, `trigger`,
//!   `description` (strings) and `word` (bool) are optional.
//! - CSV: RFC 4180 with a header row; a `content` column is required and
//!   `title`, `trigger`, `description`, `word` columns are optional. Header
//!   names are case-insensitive; unknown columns are ignored.
//! - massCode: the app's `db.json` (see [`mod@masscode`]).
//! - CopyQ: a JSON item array from its scripting export (see [`mod@copyq`]).

mod copyq;
mod csv;
mod json;
mod markdown;
mod masscode;

/// Upper bound on the imported file's text size. Import files are user-picked
/// foreign input; anything larger is rejected up front (input length limit).
pub const MAX_IMPORT_BYTES: usize = 10 * 1024 * 1024;

/// Upper bound on entries taken from one file, far above any real snippet
/// collection; a bound keeps hostile files from flooding the library.
pub const MAX_IMPORT_ENTRIES: usize = 10_000;

/// One importable snippet in the unified model. Only `content` is guaranteed;
/// the persistence layer derives a title when none was parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedEntry {
    pub title: Option<String>,
    pub content: String,
    pub trigger: Option<String>,
    /// Whether the trigger expands only at a word boundary.
    pub word: bool,
    pub description: Option<String>,
}

/// An entry that could not be imported, with a human-readable reason. The
/// label identifies the entry for the report (title, trigger, row number…).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedEntry {
    pub label: Option<String>,
    pub reason: String,
}

/// Outcome of parsing an import file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedImport {
    pub entries: Vec<ImportedEntry>,
    pub skipped: Vec<SkippedEntry>,
}

/// File formats the generic importer accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    Markdown,
    Json,
    Csv,
    Masscode,
    Copyq,
}

impl std::str::FromStr for ImportFormat {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "markdown" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "masscode" => Ok(Self::Masscode),
            "copyq" => Ok(Self::Copyq),
            _ => Err(()),
        }
    }
}

/// Why an import file could not be parsed at all (whole-file failures; a
/// single bad entry is a [`SkippedEntry`], not an error).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportParseError {
    /// The file exceeds [`MAX_IMPORT_BYTES`].
    TooLarge,
    /// The file has nothing to import (blank, or an empty collection).
    Empty,
    /// The file holds more than [`MAX_IMPORT_ENTRIES`] entries.
    TooManyEntries,
    /// The text is not valid JSON.
    InvalidJson,
    /// Valid JSON, but not an array of snippets (or `{"snippets": [...]}`).
    NotSnippetJson,
    /// The text is not parseable CSV.
    InvalidCsv,
    /// The CSV header row has no `content` column.
    MissingContentColumn,
    /// Valid JSON, but not a massCode `db.json` (no `snippets` array).
    NotMasscodeJson,
    /// Valid JSON, but not a CopyQ item array.
    NotCopyqJson,
}

impl ImportParseError {
    /// Stable, user-safe message (never echoes file content).
    pub fn message(self) -> &'static str {
        match self {
            Self::TooLarge => "the file is larger than the 10 MB import limit",
            Self::Empty => "the file has nothing to import",
            Self::TooManyEntries => "the file holds more than 10000 entries",
            Self::InvalidJson => "could not parse the JSON file",
            Self::NotSnippetJson => "not a snippet JSON file (expected an array of snippets)",
            Self::InvalidCsv => "could not parse the CSV file",
            Self::MissingContentColumn => "the CSV header has no content column",
            Self::NotMasscodeJson => "not a massCode db.json (no snippets array)",
            Self::NotCopyqJson => {
                "not a CopyQ items file (expected the JSON array from copyq eval)"
            }
        }
    }
}

/// Parse the text of an import file in the given format. Returns the unified
/// entries plus a report of everything skipped, or a whole-file error.
pub fn parse(format: ImportFormat, text: &str) -> Result<ParsedImport, ImportParseError> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err(ImportParseError::TooLarge);
    }
    if text.trim().is_empty() {
        return Err(ImportParseError::Empty);
    }
    let parsed = match format {
        ImportFormat::Markdown => markdown::parse(text)?,
        ImportFormat::Json => json::parse(text)?,
        ImportFormat::Csv => csv::parse(text)?,
        ImportFormat::Masscode => masscode::parse(text)?,
        ImportFormat::Copyq => copyq::parse(text)?,
    };
    if parsed.entries.len() > MAX_IMPORT_ENTRIES {
        return Err(ImportParseError::TooManyEntries);
    }
    if parsed.entries.is_empty() && parsed.skipped.is_empty() {
        return Err(ImportParseError::Empty);
    }
    Ok(parsed)
}

/// Normalizes an optional trigger: surrounding whitespace is stripped and a
/// blank trigger is treated as absent (a snippet without a trigger is valid).
fn normalize_trigger(trigger: Option<&str>) -> Option<String> {
    trigger
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_file_over_the_size_limit() {
        let big = "a".repeat(MAX_IMPORT_BYTES + 1);
        assert_eq!(
            parse(ImportFormat::Markdown, &big),
            Err(ImportParseError::TooLarge)
        );
    }

    #[test]
    fn rejects_a_blank_file() {
        assert_eq!(
            parse(ImportFormat::Json, "  \n\t"),
            Err(ImportParseError::Empty)
        );
    }

    #[test]
    fn rejects_an_empty_collection() {
        assert_eq!(
            parse(ImportFormat::Json, "[]"),
            Err(ImportParseError::Empty)
        );
    }

    #[test]
    fn rejects_a_file_with_too_many_entries() {
        let entries: Vec<String> = (0..MAX_IMPORT_ENTRIES + 1)
            .map(|i| format!("{{\"content\":\"c{i}\"}}"))
            .collect();
        let json = format!("[{}]", entries.join(","));
        assert_eq!(
            parse(ImportFormat::Json, &json),
            Err(ImportParseError::TooManyEntries)
        );
    }

    #[test]
    fn normalizes_blank_triggers_to_absent() {
        assert_eq!(normalize_trigger(Some("  ")), None);
        assert_eq!(normalize_trigger(Some(" :sig ")), Some(":sig".to_string()));
        assert_eq!(normalize_trigger(None), None);
    }
}
