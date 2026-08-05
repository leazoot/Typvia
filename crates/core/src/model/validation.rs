//! Model-level validation primitives.
//!
//! Validation errors identify the field and the violated rule with static
//! strings only, so they can never leak snippet content, secrets, or other
//! user data into logs or IPC error payloads.

use std::fmt;

/// A business-rule violation found while validating a model value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Field (or field pair) that violated the rule.
    pub field: &'static str,
    /// Stable, machine-readable description of the violated rule.
    pub rule: &'static str,
}

impl ValidationError {
    pub(crate) fn new(field: &'static str, rule: &'static str) -> Self {
        Self { field, rule }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "validation failed on {}: {}", self.field, self.rule)
    }
}

impl std::error::Error for ValidationError {}

/// Rejects values that are empty or whitespace-only.
pub(crate) fn require_non_blank(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new(field, "must not be blank"));
    }
    Ok(())
}
