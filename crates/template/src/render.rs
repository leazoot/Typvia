// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Rendering fill-in values into a parsed template.
//!
//! The engine merges a template body, its [`TemplateField`] definitions, and a
//! map of user-supplied values, then produces the final text. Secret-reference
//! fields are carried through as opaque references, never dereferenced here —
//! resolving them to plaintext is the vault flow's job.

use std::collections::HashMap;

use typvia_core::model::{TemplateField, TemplateFieldType};

use crate::error::RenderError;
use crate::parse::{Segment, parse};

/// An opaque reference to a secret, e.g. a secret snippet id. It is never the
/// secret plaintext; the engine holds only the reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReference(pub String);

/// One piece of a rendered template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderSegment {
    /// Resolved literal text.
    Text(String),
    /// An unresolved secret reference for `field`; the vault flow substitutes
    /// the plaintext after unlocking the vault.
    Secret {
        field: String,
        reference: SecretReference,
    },
}

/// The result of rendering: an ordered mix of text and unresolved secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    segments: Vec<RenderSegment>,
}

impl Rendered {
    /// The rendered segments in order.
    pub fn segments(&self) -> &[RenderSegment] {
        &self.segments
    }

    /// True when at least one unresolved secret reference remains.
    pub fn contains_secret(&self) -> bool {
        self.segments
            .iter()
            .any(|s| matches!(s, RenderSegment::Secret { .. }))
    }

    /// Flattens to final text. Errors with [`RenderError::UnresolvedSecret`] if
    /// any secret reference is still unresolved, so a caller without the vault
    /// can never accidentally emit an empty or placeholder secret.
    pub fn into_text(self) -> Result<String, RenderError> {
        let mut out = String::new();
        for segment in self.segments {
            match segment {
                RenderSegment::Text(t) => out.push_str(&t),
                RenderSegment::Secret { field, .. } => {
                    return Err(RenderError::UnresolvedSecret { field });
                }
            }
        }
        Ok(out)
    }
}

/// Renders `body` using `fields` definitions and the supplied `values`
/// (keyed by field name).
///
/// Rules:
/// - A referenced variable with no matching field is [`RenderError::UnknownVariable`].
/// - The effective value is the non-empty supplied value, else the non-empty
///   default; a required field with neither is [`RenderError::MissingRequired`].
/// - A `secret_ref` field yields a [`RenderSegment::Secret`] carrying the
///   reference, never resolved to plaintext.
pub fn render(
    body: &str,
    fields: &[TemplateField],
    values: &HashMap<String, String>,
) -> Result<Rendered, RenderError> {
    let by_name: HashMap<&str, &TemplateField> =
        fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut segments: Vec<RenderSegment> = Vec::new();
    for segment in parse(body)? {
        match segment {
            Segment::Literal(text) => push_text(&mut segments, text),
            Segment::Variable(name) => {
                let field = by_name
                    .get(name.as_str())
                    .ok_or(RenderError::UnknownVariable { name: name.clone() })?;
                let value = effective_value(field, values)?;
                if field.field_type == TemplateFieldType::SecretRef {
                    segments.push(RenderSegment::Secret {
                        field: name,
                        reference: SecretReference(value),
                    });
                } else {
                    push_text(&mut segments, value);
                }
            }
        }
    }

    Ok(Rendered { segments })
}

/// Appends text, coalescing into a trailing `Text` segment when possible.
fn push_text(segments: &mut Vec<RenderSegment>, text: String) {
    if text.is_empty() {
        return;
    }
    match segments.last_mut() {
        Some(RenderSegment::Text(existing)) => existing.push_str(&text),
        _ => segments.push(RenderSegment::Text(text)),
    }
}

/// The value to render for `field`: a non-empty supplied value wins over a
/// non-empty default; a required field with neither is an error, an optional
/// one renders as empty.
fn effective_value(
    field: &TemplateField,
    values: &HashMap<String, String>,
) -> Result<String, RenderError> {
    let supplied = values.get(&field.name).filter(|v| !v.is_empty());
    let default = field.default_value.as_deref().filter(|v| !v.is_empty());
    match supplied.map(String::as_str).or(default) {
        Some(v) => Ok(v.to_string()),
        None if field.is_required => Err(RenderError::MissingRequired {
            field: field.name.clone(),
        }),
        None => Ok(String::new()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn field(name: &str, ty: TemplateFieldType, required: bool) -> TemplateField {
        TemplateField {
            id: format!("f_{name}"),
            snippet_id: "s1".to_string(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: ty,
            default_value: None,
            options: vec![],
            is_required: required,
            validation: None,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn renders_supplied_values() {
        let fields = vec![field("name", TemplateFieldType::SingleLineText, true)];
        let out = render("Hi {{name}}!", &fields, &values(&[("name", "Sam")]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "Hi Sam!");
    }

    #[test]
    fn falls_back_to_the_default_value() {
        let mut f = field("env", TemplateFieldType::SingleLineText, true);
        f.default_value = Some("prod".to_string());
        let out = render("deploy {{env}}", &[f], &values(&[]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "deploy prod");
    }

    #[test]
    fn a_supplied_value_overrides_the_default() {
        let mut f = field("env", TemplateFieldType::SingleLineText, true);
        f.default_value = Some("prod".to_string());
        let out = render("deploy {{env}}", &[f], &values(&[("env", "dev")]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "deploy dev");
    }

    #[test]
    fn an_empty_supplied_value_falls_back_to_the_default() {
        let mut f = field("env", TemplateFieldType::SingleLineText, true);
        f.default_value = Some("prod".to_string());
        let out = render("{{env}}", &[f], &values(&[("env", "")]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "prod");
    }

    #[test]
    fn missing_required_value_is_an_error() {
        let fields = vec![field("name", TemplateFieldType::SingleLineText, true)];
        assert_eq!(
            render("Hi {{name}}", &fields, &values(&[])),
            Err(RenderError::MissingRequired {
                field: "name".to_string()
            })
        );
    }

    #[test]
    fn missing_optional_value_renders_empty() {
        let fields = vec![field("note", TemplateFieldType::SingleLineText, false)];
        let out = render("[{{note}}]", &fields, &values(&[]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "[]");
    }

    #[test]
    fn an_unknown_variable_is_an_error() {
        let fields = vec![field("name", TemplateFieldType::SingleLineText, true)];
        assert_eq!(
            render("{{name}} {{missing}}", &fields, &values(&[("name", "x")])),
            Err(RenderError::UnknownVariable {
                name: "missing".to_string()
            })
        );
    }

    #[test]
    fn escaped_braces_are_not_treated_as_variables() {
        // No field named `name` is defined, yet the escaped form must not be
        // looked up as a variable.
        let out = render(r"literal \{{name}} here", &[], &values(&[]))
            .unwrap()
            .into_text()
            .unwrap();
        assert_eq!(out, "literal {{name}} here");
    }

    #[test]
    fn secret_reference_is_carried_through_not_dereferenced() {
        let fields = vec![field("api_key", TemplateFieldType::SecretRef, true)];
        let rendered = render(
            "Authorization: {{api_key}}",
            &fields,
            &values(&[("api_key", "secret-ref-123")]),
        )
        .unwrap();

        assert!(rendered.contains_secret());
        assert_eq!(
            rendered.segments(),
            &[
                RenderSegment::Text("Authorization: ".to_string()),
                RenderSegment::Secret {
                    field: "api_key".to_string(),
                    reference: SecretReference("secret-ref-123".to_string()),
                },
            ]
        );
        // into_text refuses to emit while a secret is unresolved.
        assert_eq!(
            rendered.into_text(),
            Err(RenderError::UnresolvedSecret {
                field: "api_key".to_string()
            })
        );
    }

    #[test]
    fn a_required_secret_reference_still_needs_a_value() {
        let fields = vec![field("api_key", TemplateFieldType::SecretRef, true)];
        assert_eq!(
            render("{{api_key}}", &fields, &values(&[])),
            Err(RenderError::MissingRequired {
                field: "api_key".to_string()
            })
        );
    }

    #[test]
    fn error_never_carries_the_fill_in_value() {
        // The unknown-variable path is the only render error that echoes a
        // name; assert the *value* never appears in any Display output.
        let fields = vec![field("token", TemplateFieldType::SecretRef, true)];
        let err = render("{{token}}", &fields, &values(&[]))
            .unwrap_err()
            .to_string();
        assert!(!err.contains("secret-ref"), "error leaked a value: {err}");
    }
}
