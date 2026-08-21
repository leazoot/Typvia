// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Local semantic-search kernel: the int8 in-memory vector index with
//! brute-force cosine scan, the `Embedder` abstraction
//! (model-replaceable), and the candle-based BERT-family backend.
//! Everything here is offline; nothing in this crate performs network IO.

mod candle_backend;
mod embedder;
mod index;
mod quant;

pub use candle_backend::{CandleEmbedder, ModelFiles, convert_weights_to_f16};
pub use embedder::{EmbedError, Embedder};
pub use index::{ScanHit, VectorIndex};
pub use quant::QuantizedVector;
