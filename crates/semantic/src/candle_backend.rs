// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! candle-based BERT-family embedder: loads a local model directory
//! (config.json + tokenizer.json + safetensors), mean-pools the last
//! hidden state under the attention mask and L2 normalizes — the e5
//! recipe, with its "query: " / "passage: " prefixes.
//! Fully offline: this module never touches the network.

use std::path::PathBuf;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::Tokenizer;

use crate::embedder::{EmbedError, Embedder};

/// Token cap per text: snippets are short; truncation keeps worst-case
/// latency bounded.
const MAX_TOKENS: usize = 256;

/// The three files a model directory must provide.
#[derive(Debug, Clone)]
pub struct ModelFiles {
    pub config: PathBuf,
    pub tokenizer: PathBuf,
    pub weights: PathBuf,
}

/// A loaded local embedding model.
pub struct CandleEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    dims: usize,
    model_id: String,
}

impl CandleEmbedder {
    /// Loads the model from disk. Any missing/corrupt file is
    /// `ModelUnavailable` — the caller falls back to lexical-only search.
    pub fn load(files: &ModelFiles, model_id: &str) -> Result<Self, EmbedError> {
        let device = Device::Cpu;
        let config_bytes =
            std::fs::read(&files.config).map_err(|_| EmbedError::ModelUnavailable)?;
        let config: Config =
            serde_json::from_slice(&config_bytes).map_err(|_| EmbedError::ModelUnavailable)?;
        let mut tokenizer =
            Tokenizer::from_file(&files.tokenizer).map_err(|_| EmbedError::ModelUnavailable)?;
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|_| EmbedError::ModelUnavailable)?;
        // Weights are memory-mapped read-only; the file is checksum-verified
        // at download time by the host.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                std::slice::from_ref(&files.weights),
                DType::F32,
                &device,
            )
        }
        .map_err(|_| EmbedError::ModelUnavailable)?;
        let dims = config.hidden_size;
        let model = BertModel::load(vb, &config).map_err(|_| EmbedError::ModelUnavailable)?;
        Ok(Self {
            model,
            tokenizer,
            device,
            dims,
            model_id: model_id.to_string(),
        })
    }

    /// Embeds one batch of already-prefixed texts.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|_| EmbedError::Inference)?;
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(1)
            .max(1);
        let mut ids = Vec::with_capacity(texts.len() * max_len);
        let mut mask = Vec::with_capacity(texts.len() * max_len);
        for encoding in &encodings {
            let token_ids = encoding.get_ids();
            ids.extend(token_ids.iter().copied());
            mask.extend(std::iter::repeat_n(1u32, token_ids.len()));
            let pad = max_len - token_ids.len();
            ids.extend(std::iter::repeat_n(0u32, pad));
            mask.extend(std::iter::repeat_n(0u32, pad));
        }
        let shape = (texts.len(), max_len);
        let input_ids =
            Tensor::from_vec(ids, shape, &self.device).map_err(|_| EmbedError::Inference)?;
        let attention_mask =
            Tensor::from_vec(mask, shape, &self.device).map_err(|_| EmbedError::Inference)?;
        let token_type_ids = input_ids.zeros_like().map_err(|_| EmbedError::Inference)?;
        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|_| EmbedError::Inference)?;
        mean_pool_normalized(&hidden, &attention_mask).map_err(|_| EmbedError::Inference)
    }
}

/// Masked mean pooling over the sequence dimension, then L2 normalization
/// per row — returned as plain f32 rows.
fn mean_pool_normalized(
    hidden: &Tensor,
    attention_mask: &Tensor,
) -> Result<Vec<Vec<f32>>, candle_core::Error> {
    let mask = attention_mask
        .to_dtype(DType::F32)?
        .unsqueeze(2)?
        .broadcast_as(hidden.shape())?;
    let summed = hidden.mul(&mask)?.sum(1)?;
    let counts = mask.sum(1)?.clamp(1e-9, f64::INFINITY)?;
    let mean = summed.div(&counts)?;
    let rows = mean.to_vec2::<f32>()?;
    Ok(rows
        .into_iter()
        .map(|mut row| {
            crate::quant::normalize(&mut row);
            row
        })
        .collect())
}

/// Converts a safetensors weights file to f16 storage (about half the
/// disk footprint). Inference is unchanged: the loader always upcasts
/// storage to f32. The output is written whole; callers replace the
/// original only after this succeeds (failed conversion keeps the
/// verified original in place).
pub fn convert_weights_to_f16(
    input: &std::path::Path,
    output: &std::path::Path,
) -> Result<(), EmbedError> {
    let device = Device::Cpu;
    let tensors =
        candle_core::safetensors::load(input, &device).map_err(|_| EmbedError::ModelUnavailable)?;
    let mut converted = std::collections::HashMap::new();
    for (name, tensor) in tensors {
        let half = tensor
            .to_dtype(DType::F16)
            .map_err(|_| EmbedError::Inference)?;
        converted.insert(name, half);
    }
    candle_core::safetensors::save(&converted, output).map_err(|_| EmbedError::Inference)
}

impl Embedder for CandleEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        self.embed(&prefixed)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let rows = self.embed(&[format!("query: {text}")])?;
        rows.into_iter().next().ok_or(EmbedError::Inference)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn f16_conversion_halves_storage_and_round_trips_values() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("w.safetensors");
        let output = dir.path().join("w.f16.safetensors");
        let device = Device::Cpu;
        let tensor = Tensor::from_vec(vec![0.25f32, -1.5, 3.0, 0.0], (2, 2), &device).unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert("weight".to_string(), tensor);
        candle_core::safetensors::save(&map, &input).unwrap();

        convert_weights_to_f16(&input, &output).unwrap();

        // Roughly half the tensor payload (headers add a few bytes).
        let in_len = std::fs::metadata(&input).unwrap().len();
        let out_len = std::fs::metadata(&output).unwrap().len();
        assert!(out_len < in_len);
        // Loading back as f32 reproduces the exactly-representable values.
        let loaded = candle_core::safetensors::load(&output, &device).unwrap();
        let values: Vec<f32> = loaded["weight"]
            .to_dtype(DType::F32)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1()
            .unwrap();
        assert_eq!(values, vec![0.25, -1.5, 3.0, 0.0]);
    }

    #[test]
    fn missing_model_files_are_unavailable_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let files = ModelFiles {
            config: dir.path().join("config.json"),
            tokenizer: dir.path().join("tokenizer.json"),
            weights: dir.path().join("model.safetensors"),
        };
        let error = match CandleEmbedder::load(&files, "m") {
            Ok(_) => panic!("load must fail without model files"),
            Err(error) => error,
        };
        assert_eq!(error, EmbedError::ModelUnavailable);
    }
}
