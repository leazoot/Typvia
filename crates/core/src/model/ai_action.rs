//! The `AIAction` entity (PRD §15.9).

use super::validation::{ValidationError, require_non_blank};
use super::{AiActionId, TimestampMs};

/// A user-configured AI action definition (PRD §12.11.4 / §15.9).
///
/// `input_source`, `output_mode`, and `permission_scope` are controlled TEXT
/// values whose value sets the PRD does not define; they stay opaque strings
/// until the AI batch fixes them (OQ-R9). The model only requires them to be
/// present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiAction {
    pub id: AiActionId,
    pub name: String,
    /// Instruction template sent to the provider; may reference the input.
    pub prompt_template: String,
    /// References a configured AI provider (BYOK); provider storage is
    /// defined by the AI batch.
    pub provider_id: String,
    /// Model name as understood by the provider.
    pub model: String,
    pub input_source: String,
    pub output_mode: String,
    pub permission_scope: String,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl AiAction {
    /// Validates the action definition before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        require_non_blank("prompt_template", &self.prompt_template)?;
        require_non_blank("provider_id", &self.provider_id)?;
        require_non_blank("model", &self.model)?;
        require_non_blank("input_source", &self.input_source)?;
        require_non_blank("output_mode", &self.output_mode)?;
        require_non_blank("permission_scope", &self.permission_scope)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn action() -> AiAction {
        AiAction {
            id: "a1".to_string(),
            name: "Translate".to_string(),
            prompt_template: "Translate the input into English.".to_string(),
            provider_id: "p1".to_string(),
            model: "llama3".to_string(),
            input_source: "selection".to_string(),
            output_mode: "replace".to_string(),
            permission_scope: "normal_only".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn accepts_a_well_formed_action() {
        assert_eq!(action().validate(), Ok(()));
    }

    #[test]
    fn rejects_blank_prompt_template() {
        let mut a = action();
        a.prompt_template = "".to_string();
        assert_eq!(a.validate().unwrap_err().field, "prompt_template");
    }
}
