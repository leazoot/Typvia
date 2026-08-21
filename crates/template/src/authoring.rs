// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Authoring-time helpers for the Template Builder.
//!
//! These are lenient counterparts to [`crate::render`]: where `render` is
//! strict (a missing required value or an unknown variable is an error, ready
//! for real injection), the builder needs to show *something* resolved while
//! the author is still typing.

use std::collections::HashMap;

use typvia_core::model::{TemplateField, TemplateFieldType};

use crate::error::ParseError;
use crate::parse::{Segment, parse};

/// Masking shown for a secret-reference value in an authoring preview. The
/// engine never holds secret plaintext, so the preview shows a fixed mask.
const SECRET_MASK: &str = "••••••";

/// The distinct variable names referenced by `body`, in first-seen order.
///
/// The Builder uses this to decide which field to show for each `{{name}}` in
/// the prose. Returns a [`ParseError`] for a malformed placeholder.
pub fn variables(body: &str) -> Result<Vec<String>, ParseError> {
    let mut seen: Vec<String> = Vec::new();
    for segment in parse(body)? {
        if let Segment::Variable(name) = segment
            && !seen.contains(&name)
        {
            seen.push(name);
        }
    }
    Ok(seen)
}

/// Renders a lenient authoring preview: filled values win over defaults, an
/// unfilled field shows a `‹label›` placeholder, a secret reference shows a
/// fixed mask, and a variable with no matching field is kept literal so the
/// author can see it is undefined. Never errors on missing/unknown values.
pub fn preview(
    body: &str,
    fields: &[TemplateField],
    values: &HashMap<String, String>,
) -> Result<String, ParseError> {
    let by_name: HashMap<&str, &TemplateField> =
        fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut out = String::new();
    for segment in parse(body)? {
        match segment {
            Segment::Literal(text) => out.push_str(&text),
            Segment::Variable(name) => match by_name.get(name.as_str()) {
                None => {
                    out.push_str("{{");
                    out.push_str(&name);
                    out.push_str("}}");
                }
                Some(field) if field.field_type == TemplateFieldType::SecretRef => {
                    out.push_str(SECRET_MASK);
                }
                Some(field) => out.push_str(&resolved_or_placeholder(field, values)),
            },
        }
    }
    Ok(out)
}

/// Non-empty filled value, else non-empty default, else a `‹label›` marker.
fn resolved_or_placeholder(field: &TemplateField, values: &HashMap<String, String>) -> String {
    let supplied = values.get(&field.name).filter(|v| !v.is_empty());
    if let Some(v) = supplied {
        return v.clone();
    }
    if let Some(default) = field.default_value.as_deref().filter(|v| !v.is_empty()) {
        return default.to_string();
    }
    format!("‹{}›", field.label)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn field(name: &str, ty: TemplateFieldType) -> TemplateField {
        TemplateField {
            id: format!("f_{name}"),
            snippet_id: "s1".to_string(),
            name: name.to_string(),
            label: name.to_string(),
            field_type: ty,
            default_value: None,
            options: vec![],
            is_required: false,
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
    fn variables_are_returned_in_first_seen_order_without_duplicates() {
        assert_eq!(
            variables("{{b}} {{a}} {{b}}").unwrap(),
            vec!["b".to_string(), "a".to_string()]
        );
    }

    #[test]
    fn preview_uses_filled_values_over_defaults() {
        let mut f = field("env", TemplateFieldType::SingleLineText);
        f.default_value = Some("prod".to_string());
        assert_eq!(
            preview("{{env}}", &[f], &values(&[("env", "dev")])).unwrap(),
            "dev"
        );
    }

    #[test]
    fn preview_falls_back_to_default_then_label_placeholder() {
        let mut with_default = field("env", TemplateFieldType::SingleLineText);
        with_default.default_value = Some("prod".to_string());
        assert_eq!(
            preview("{{env}}", &[with_default], &values(&[])).unwrap(),
            "prod"
        );

        let bare = field("module", TemplateFieldType::SingleLineText);
        assert_eq!(
            preview("{{module}}", &[bare], &values(&[])).unwrap(),
            "‹module›"
        );
    }

    #[test]
    fn preview_masks_secret_references() {
        let secret = field("api_key", TemplateFieldType::SecretRef);
        let out = preview(
            "key: {{api_key}}",
            &[secret],
            &values(&[("api_key", "ref-1")]),
        )
        .unwrap();
        assert_eq!(out, "key: ••••••");
        assert!(!out.contains("ref-1"), "preview leaked a secret reference");
    }

    #[test]
    fn preview_keeps_an_undefined_variable_literal() {
        assert_eq!(
            preview("{{ghost}}", &[], &values(&[])).unwrap(),
            "{{ghost}}"
        );
    }
}
