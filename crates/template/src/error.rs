// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Errors surfaced by template parsing and rendering.
//!
//! Every variant carries only structural metadata — variable names and field
//! names — never a fill-in value, default value, or template body. This keeps
//! the log/error red line intact: a rendered secret or snippet body can
//! never reach an error message.

use std::fmt;

/// A problem found while parsing a template body into segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// A `{{` was opened but never closed with `}}`.
    UnterminatedPlaceholder,
    /// A placeholder had no name (`{{}}` or `{{   }}`).
    EmptyPlaceholder,
    /// A placeholder name contained interior whitespace or a brace, so it
    /// could not be a valid `{{name}}` reference.
    InvalidVariableName { name: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnterminatedPlaceholder => {
                write!(f, "template has an unterminated `{{{{` placeholder")
            }
            ParseError::EmptyPlaceholder => write!(f, "template has an empty placeholder"),
            ParseError::InvalidVariableName { name } => {
                write!(f, "invalid template variable name `{name}`")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// A problem found while rendering fill-in values into a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The template body referenced a variable with no matching field.
    UnknownVariable { name: String },
    /// A required field had neither a fill-in value nor a default value.
    MissingRequired { field: String },
    /// `Rendered::into_text` was called while an unresolved secret reference
    /// remained; dereferencing belongs to the vault flow.
    UnresolvedSecret { field: String },
    /// The template body failed to parse.
    Parse(ParseError),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::UnknownVariable { name } => {
                write!(f, "template references undefined variable `{name}`")
            }
            RenderError::MissingRequired { field } => {
                write!(f, "required field `{field}` was not filled in")
            }
            RenderError::UnresolvedSecret { field } => {
                write!(f, "secret reference for field `{field}` is unresolved")
            }
            RenderError::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<ParseError> for RenderError {
    fn from(e: ParseError) -> Self {
        RenderError::Parse(e)
    }
}
