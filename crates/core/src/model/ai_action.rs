// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The `AIAction` entity.

use serde::{Deserialize, Serialize};

use super::validation::{ValidationError, require_non_blank};
use super::{
    AiActionId, AiActionInputSource, AiActionOutputMode, AiActionPermissionScope, AiProviderId,
    TimestampMs,
};

/// A user-configured AI action definition.
///
/// `input_source`, `output_mode`, and `permission_scope` carry fixed
/// value sets.
#[derive(Debug, Clone, PartialEq)]
pub struct AiAction {
    pub id: AiActionId,
    pub name: String,
    /// Instruction template sent to the provider; may reference the input.
    pub prompt_template: String,
    /// References a configured [`super::AiProvider`] row; `None` means the
    /// action has no provider chosen yet (built-in seeds ship this way,
    /// and synced actions may dangle on this device). Execution surfaces
    /// it as a "provider not configured" user error.
    pub provider_id: Option<AiProviderId>,
    /// Model name as understood by the provider; `None` falls back to the
    /// chosen provider's default model at execution time.
    pub model: Option<String>,
    pub input_source: AiActionInputSource,
    pub output_mode: AiActionOutputMode,
    pub permission_scope: AiActionPermissionScope,
    pub params: AiActionParams,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

/// Provider call parameters, stored as a JSON object. Unknown keys
/// survive read-modify-write round trips so a newer build's parameters
/// are never silently dropped by an older one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AiActionParams {
    /// Sampling temperature, same 0..=2 rule as the crates/ai request layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Keys this build does not know about, preserved verbatim.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl AiActionParams {
    /// Parses the stored JSON object form; anything that is not a JSON
    /// object (or carries an ill-typed known key) is rejected.
    pub fn from_json(raw: &str) -> Result<Self, ValidationError> {
        let parsed: Self = serde_json::from_str(raw)
            .map_err(|_| ValidationError::new("params", "must be a JSON object"))?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Serializes back to the stored JSON object form.
    pub fn to_json(&self) -> String {
        // Invariant: a struct of Option + Map cannot fail to serialize.
        #[allow(clippy::expect_used)]
        serde_json::to_string(self).expect("params serialization is infallible")
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(t) = self.temperature
            && !(0.0..=2.0).contains(&t)
        {
            return Err(ValidationError::new(
                "params.temperature",
                "must be between 0 and 2",
            ));
        }
        Ok(())
    }
}

impl AiAction {
    /// Validates the action definition before it enters storage.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_blank("name", &self.name)?;
        require_non_blank("prompt_template", &self.prompt_template)?;
        if let Some(provider_id) = &self.provider_id {
            require_non_blank("provider_id", provider_id)?;
        }
        if let Some(model) = &self.model {
            require_non_blank("model", model)?;
        }
        self.params.validate()
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
            provider_id: Some("p1".to_string()),
            model: Some("llama3".to_string()),
            input_source: AiActionInputSource::Selection,
            output_mode: AiActionOutputMode::Replace,
            permission_scope: AiActionPermissionScope::NormalOnly,
            params: AiActionParams::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn accepts_a_well_formed_action() {
        assert_eq!(action().validate(), Ok(()));
    }

    #[test]
    fn accepts_an_unconfigured_action() {
        let mut a = action();
        a.provider_id = None;
        a.model = None;
        assert_eq!(a.validate(), Ok(()));
    }

    #[test]
    fn rejects_blank_prompt_template() {
        let mut a = action();
        a.prompt_template = "".to_string();
        assert_eq!(a.validate().unwrap_err().field, "prompt_template");
    }

    #[test]
    fn rejects_a_blank_present_provider_id() {
        let mut a = action();
        a.provider_id = Some("  ".to_string());
        assert_eq!(a.validate().unwrap_err().field, "provider_id");
    }

    #[test]
    fn params_round_trip_preserves_unknown_keys() {
        let parsed = AiActionParams::from_json(r#"{"temperature":0.4,"top_p":0.9}"#).unwrap();
        assert_eq!(parsed.temperature, Some(0.4));
        let json = parsed.to_json();
        let reparsed = AiActionParams::from_json(&json).unwrap();
        assert_eq!(
            reparsed.extra.get("top_p").and_then(|v| v.as_f64()),
            Some(0.9)
        );
    }

    #[test]
    fn params_reject_non_objects_and_bad_temperatures() {
        assert!(AiActionParams::from_json("[]").is_err());
        assert!(AiActionParams::from_json("not json").is_err());
        let err = AiActionParams::from_json(r#"{"temperature":3.5}"#).unwrap_err();
        assert_eq!(err.field, "params.temperature");
    }
}
