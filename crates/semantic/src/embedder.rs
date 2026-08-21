// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The embedder abstraction. The model is replaceable: vectors carry the
//! producing `model_id`, and a backend swap only changes what this trait
//! returns.

/// Why embedding failed; stable classes, never content (log red line).
#[derive(Debug, PartialEq, Eq)]
pub enum EmbedError {
    /// Model files missing/corrupt/unsupported.
    ModelUnavailable,
    /// Inference failed at runtime.
    Inference,
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelUnavailable => f.write_str("embedding model unavailable"),
            Self::Inference => f.write_str("embedding inference failed"),
        }
    }
}

impl std::error::Error for EmbedError {}

/// A local text-embedding backend. Implementations must be offline.
pub trait Embedder: Send {
    /// Stable identifier of the loaded model (stored with each vector).
    fn model_id(&self) -> &str;
    /// Output dimensionality.
    fn dims(&self) -> usize;
    /// Embeds passages (documents) for indexing.
    fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
    /// Embeds one search query.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
}
