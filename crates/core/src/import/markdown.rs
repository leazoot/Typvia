// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Markdown import: sections split at the first-seen ATX heading level.
//!
//! The first heading in the file fixes the split level (e.g. `##`); every
//! later heading at that exact level starts a new snippet whose title is the
//! heading text and whose content is the section body. Deeper or shallower
//! headings stay inside the body, and headings inside fenced code blocks
//! never split. A file without headings imports as one untitled snippet.

use super::{ImportParseError, ImportedEntry, ParsedImport, SkippedEntry};

pub(super) fn parse(text: &str) -> Result<ParsedImport, ImportParseError> {
    let mut fence: Option<char> = None;
    let mut split_level: Option<usize> = None;
    let mut preamble: Vec<&str> = Vec::new();
    let mut sections: Vec<(String, Vec<&str>)> = Vec::new();

    for line in text.lines() {
        let stripped = line.trim_start();
        if let Some(marker) = fence_marker(stripped) {
            match fence {
                None => fence = Some(marker),
                Some(open) if open == marker => fence = None,
                Some(_) => {}
            }
        } else if fence.is_none()
            && let Some((level, title)) = heading(line)
        {
            if split_level.is_none() {
                split_level = Some(level);
            }
            if split_level == Some(level) {
                sections.push((title, Vec::new()));
                continue;
            }
        }
        match sections.last_mut() {
            Some((_, body)) => body.push(line),
            None => preamble.push(line),
        }
    }

    // No headings: the whole document is one untitled snippet (the caller
    // already rejected blank files, so the content is non-empty).
    if sections.is_empty() {
        return Ok(ParsedImport {
            entries: vec![ImportedEntry {
                title: None,
                content: text.trim().to_string(),
                trigger: None,
                word: false,
                description: None,
            }],
            skipped: Vec::new(),
        });
    }

    let mut result = ParsedImport::default();
    if preamble.iter().any(|line| !line.trim().is_empty()) {
        result.skipped.push(SkippedEntry {
            label: None,
            reason: "content before the first heading".to_string(),
        });
    }
    for (title, body) in sections {
        let content = body.join("\n").trim().to_string();
        if content.is_empty() {
            result.skipped.push(SkippedEntry {
                label: Some(title),
                reason: "section has no content".to_string(),
            });
            continue;
        }
        result.entries.push(ImportedEntry {
            title: Some(title).filter(|t| !t.is_empty()),
            content,
            trigger: None,
            word: false,
            description: None,
        });
    }
    Ok(result)
}

/// Returns the fence character of a code-fence line (``` or ~~~), if any.
fn fence_marker(stripped_line: &str) -> Option<char> {
    ['`', '~']
        .into_iter()
        .find(|&marker| stripped_line.chars().take_while(|c| *c == marker).count() >= 3)
}

/// Parses an ATX heading at the start of a line: `(level, title)` with any
/// closing `###` sequence stripped per CommonMark.
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let text = rest.trim();
    let without_closing = text.trim_end_matches('#');
    let title = if without_closing.len() < text.len()
        && (without_closing.is_empty() || without_closing.ends_with(' '))
    {
        without_closing.trim_end()
    } else {
        text
    };
    Some((level, title.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sections_at_the_first_seen_heading_level() {
        let text = "## Greeting\nHello there\n\n## Address\n1 Main St\n### Detail\nkept inside\n";
        let parsed = parse(text).expect("parses");
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].title.as_deref(), Some("Greeting"));
        assert_eq!(parsed.entries[0].content, "Hello there");
        assert_eq!(parsed.entries[1].title.as_deref(), Some("Address"));
        assert_eq!(
            parsed.entries[1].content,
            "1 Main St\n### Detail\nkept inside"
        );
        assert!(parsed.skipped.is_empty());
    }

    #[test]
    fn a_file_without_headings_imports_as_one_untitled_snippet() {
        let parsed = parse("just a note\nsecond line\n").expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].title, None);
        assert_eq!(parsed.entries[0].content, "just a note\nsecond line");
    }

    #[test]
    fn headings_inside_code_fences_do_not_split() {
        let text = "# Shell notes\n```md\n# not a heading\n```\ndone\n";
        let parsed = parse(text).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(
            parsed.entries[0].content,
            "```md\n# not a heading\n```\ndone"
        );
    }

    #[test]
    fn reports_preamble_and_empty_sections_as_skipped() {
        let text = "stray intro\n\n# Kept\nbody\n\n# Hollow\n\n";
        let parsed = parse(text).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].title.as_deref(), Some("Kept"));
        let reasons: Vec<&str> = parsed.skipped.iter().map(|s| s.reason.as_str()).collect();
        assert_eq!(
            reasons,
            vec!["content before the first heading", "section has no content"]
        );
        assert_eq!(parsed.skipped[1].label.as_deref(), Some("Hollow"));
    }

    #[test]
    fn strips_the_closing_hash_sequence_from_titles() {
        let parsed = parse("## Title ##\nbody\n").expect("parses");
        assert_eq!(parsed.entries[0].title.as_deref(), Some("Title"));
    }
}
