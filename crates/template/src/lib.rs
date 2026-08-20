//! Template parsing and rendering for Typvia snippets.
//!
//! The engine turns a template body plus its [`TemplateField`] definitions and
//! user-supplied values into final text. It is pure and storage-agnostic;
//! secret-reference fields are carried through as opaque references and are
//! only resolved to plaintext by the vault flow.
//!
//! [`TemplateField`]: typvia_core::model::TemplateField

mod authoring;
mod error;
mod parse;
mod render;

pub use authoring::{preview, variables};
pub use error::{ParseError, RenderError};
pub use parse::{Segment, parse};
pub use render::{RenderSegment, Rendered, SecretReference, render};
