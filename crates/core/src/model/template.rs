// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The `TemplateField` entity.

use super::enums::TemplateFieldType;
use super::validation::{ValidationError, require_non_blank};
use super::{SnippetId, TemplateFieldId};

/// A fillable field belonging to a template snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateField {
    pub id: TemplateFieldId,
    pub snippet_id: SnippetId,
    /// Variable name referenced from the template body as `{{name}}`.
    pub name: String,
    /// Human-readable label shown in the fill-in form.
    pub label: String,
    pub field_type: TemplateFieldType,
    pub default_value: Option<String>,
    /// Choices for select/dropdown types; must be empty for other types.
    pub options: Vec<String>,
    /// Validation rule expression; concrete syntax is defined with the
    /// template engine (crates/template), stored opaquely here.
    pub validation: Option<String>,
    pub is_required: bool,
    pub sort_order: i32,
    /// Per-platform behavior overrides as an opaque JSON document; its schema
    /// is defined by the template engine task and not interpreted here.
    pub platform_overrides: Option<String>,
}

impl TemplateField {
    /// True for field types whose values come from the `options` list.
    fn is_choice_type(&self) -> bool {
        matches!(
            self.field_type,
            TemplateFieldType::SingleSelect
                | TemplateFieldType::MultiSelect
                | TemplateFieldType::Dropdown
        )
    }

    /// Validates the field definition before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        if self
            .name
            .chars()
            .any(|c| c.is_whitespace() || c == '{' || c == '}')
        {
            return Err(ValidationError::new(
                "name",
                "must not contain whitespace or braces",
            ));
        }
        require_non_blank("label", &self.label)?;

        if self.is_choice_type() {
            if self.options.is_empty() {
                return Err(ValidationError::new(
                    "options",
                    "choice fields require at least one option",
                ));
            }
        } else if !self.options.is_empty() {
            return Err(ValidationError::new(
                "options",
                "non-choice fields must not define options",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn text_field() -> TemplateField {
        TemplateField {
            id: "f1".to_string(),
            snippet_id: "s1".to_string(),
            name: "module_name".to_string(),
            label: "Module name".to_string(),
            field_type: TemplateFieldType::SingleLineText,
            default_value: None,
            options: vec![],
            validation: None,
            is_required: true,
            sort_order: 0,
            platform_overrides: None,
        }
    }

    #[test]
    fn accepts_a_well_formed_text_field() {
        assert_eq!(text_field().validate(), Ok(()));
    }

    #[test]
    fn accepts_a_select_field_with_options() {
        let mut f = text_field();
        f.field_type = TemplateFieldType::SingleSelect;
        f.options = vec!["dev".to_string(), "prod".to_string()];
        assert_eq!(f.validate(), Ok(()));
    }

    #[test]
    fn rejects_select_field_without_options() {
        let mut f = text_field();
        f.field_type = TemplateFieldType::Dropdown;
        assert_eq!(f.validate().unwrap_err().field, "options");
    }

    #[test]
    fn rejects_text_field_with_options() {
        let mut f = text_field();
        f.options = vec!["stray".to_string()];
        assert_eq!(f.validate().unwrap_err().field, "options");
    }

    #[test]
    fn rejects_name_containing_braces_or_whitespace() {
        for bad in ["{{name}}", "my name"] {
            let mut f = text_field();
            f.name = bad.to_string();
            assert_eq!(f.validate().unwrap_err().field, "name", "value: {bad}");
        }
    }
}
