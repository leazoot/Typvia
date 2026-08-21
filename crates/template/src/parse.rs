// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Parsing a template body into an ordered sequence of literal and variable
//! segments.
//!
//! Syntax:
//! - `{{name}}` is a variable reference; whitespace just inside the braces is
//!   trimmed, interior whitespace or a brace is rejected.
//! - `\` is the only escape: `\{{` yields a literal `{{`, `\\` yields a literal
//!   `\`, and `\` before anything else is a literal backslash plus that char.

use crate::error::ParseError;

/// One piece of a parsed template body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text emitted verbatim (with escapes already resolved).
    Literal(String),
    /// A `{{name}}` variable reference; holds the trimmed variable name.
    Variable(String),
}

/// Parses `body` into ordered segments, resolving escapes.
///
/// Returns a [`ParseError`] for an unterminated or empty placeholder, or a
/// placeholder name that is not a valid `{{name}}` reference.
pub fn parse(body: &str) -> Result<Vec<Segment>, ParseError> {
    let chars: Vec<char> = body.chars().collect();
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\\' && matches!(chars.get(i + 1), Some('{')) && at(&chars, i + 2, '{') {
            literal.push_str("{{");
            i += 3;
        } else if chars[i] == '\\' && matches!(chars.get(i + 1), Some('\\')) {
            literal.push('\\');
            i += 2;
        } else if chars[i] == '{' && at(&chars, i + 1, '{') {
            let close = find_close(&chars, i + 2).ok_or(ParseError::UnterminatedPlaceholder)?;
            let raw: String = chars[i + 2..close].iter().collect();
            let name = raw.trim();
            if name.is_empty() {
                return Err(ParseError::EmptyPlaceholder);
            }
            if name
                .chars()
                .any(|c| c.is_whitespace() || c == '{' || c == '}')
            {
                return Err(ParseError::InvalidVariableName {
                    name: name.to_string(),
                });
            }
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }
            segments.push(Segment::Variable(name.to_string()));
            i = close + 2;
        } else {
            literal.push(chars[i]);
            i += 1;
        }
    }

    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}

/// True when `chars[idx]` exists and equals `c`.
fn at(chars: &[char], idx: usize, c: char) -> bool {
    chars.get(idx) == Some(&c)
}

/// Finds the index of the `}` that opens the closing `}}` at or after `from`.
fn find_close(chars: &[char], from: usize) -> Option<usize> {
    let mut j = from;
    while j + 1 < chars.len() {
        if chars[j] == '}' && chars[j + 1] == '}' {
            return Some(j);
        }
        j += 1;
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal_only_text() {
        assert_eq!(
            parse("hello world").unwrap(),
            vec![Segment::Literal("hello world".to_string())]
        );
    }

    #[test]
    fn parses_a_single_variable_between_literals() {
        assert_eq!(
            parse("Hi {{name}}!").unwrap(),
            vec![
                Segment::Literal("Hi ".to_string()),
                Segment::Variable("name".to_string()),
                Segment::Literal("!".to_string()),
            ]
        );
    }

    #[test]
    fn trims_whitespace_inside_braces() {
        assert_eq!(
            parse("{{  name  }}").unwrap(),
            vec![Segment::Variable("name".to_string())]
        );
    }

    #[test]
    fn escapes_open_braces() {
        assert_eq!(
            parse(r"\{{name}}").unwrap(),
            vec![Segment::Literal("{{name}}".to_string())]
        );
    }

    #[test]
    fn escapes_a_backslash() {
        assert_eq!(
            parse(r"a\\b").unwrap(),
            vec![Segment::Literal(r"a\b".to_string())]
        );
    }

    #[test]
    fn a_lone_backslash_is_literal() {
        assert_eq!(
            parse(r"c:\dir").unwrap(),
            vec![Segment::Literal(r"c:\dir".to_string())]
        );
    }

    #[test]
    fn preserves_cjk_literals() {
        assert_eq!(
            parse("你好 {{name}} 世界").unwrap(),
            vec![
                Segment::Literal("你好 ".to_string()),
                Segment::Variable("name".to_string()),
                Segment::Literal(" 世界".to_string()),
            ]
        );
    }

    #[test]
    fn rejects_an_unterminated_placeholder() {
        assert_eq!(parse("{{name"), Err(ParseError::UnterminatedPlaceholder));
    }

    #[test]
    fn rejects_an_empty_placeholder() {
        assert_eq!(parse("{{}}"), Err(ParseError::EmptyPlaceholder));
        assert_eq!(parse("{{   }}"), Err(ParseError::EmptyPlaceholder));
    }

    #[test]
    fn rejects_interior_whitespace_in_a_name() {
        assert_eq!(
            parse("{{first last}}"),
            Err(ParseError::InvalidVariableName {
                name: "first last".to_string()
            })
        );
    }
}
