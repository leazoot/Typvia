// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Resident vector index: contiguous int8 storage, brute-force top-k
//! cosine scan. No ANN structure — at the 50k-snippet ceiling a full scan
//! is well inside budget, and the result is exact and deterministic (ties
//! break by id).

use crate::quant::{QuantizedVector, normalize, quantize};

/// One scan hit above the score threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanHit {
    pub id: String,
    pub score: f32,
}

/// In-memory index over pre-normalized, int8-quantized vectors.
pub struct VectorIndex {
    dims: usize,
    ids: Vec<String>,
    scales: Vec<f32>,
    /// Row-major contiguous int8 values, `ids.len() * dims` long.
    values: Vec<i8>,
}

impl VectorIndex {
    /// Builds the index from `(id, f32 vector)` pairs. Vectors are
    /// normalized then quantized; entries with a wrong dimension are
    /// skipped (a stale row from another model must not poison the scan).
    pub fn build(dims: usize, entries: impl IntoIterator<Item = (String, Vec<f32>)>) -> Self {
        let mut ids = Vec::new();
        let mut scales = Vec::new();
        let mut values = Vec::new();
        for (id, mut vector) in entries {
            if vector.len() != dims {
                continue;
            }
            normalize(&mut vector);
            let q = quantize(&vector);
            ids.push(id);
            scales.push(q.scale);
            values.extend_from_slice(&q.values);
        }
        Self {
            dims,
            ids,
            scales,
            values,
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Approximate resident bytes (int8 matrix + scales; ids excluded).
    pub fn resident_bytes(&self) -> usize {
        self.values.len() + self.scales.len() * 4
    }

    /// Exact top-k by cosine against `query`, entries below `min_score`
    /// dropped. Deterministic: score descending, then id ascending.
    pub fn top_k(&self, query: &[f32], k: usize, min_score: f32) -> Vec<ScanHit> {
        if query.len() != self.dims || k == 0 {
            return Vec::new();
        }
        let mut q = query.to_vec();
        normalize(&mut q);
        let query_q: QuantizedVector = quantize(&q);
        let mut hits: Vec<ScanHit> = Vec::new();
        for (row, id) in self.ids.iter().enumerate() {
            let start = row * self.dims;
            let dot: i32 = self.values[start..start + self.dims]
                .iter()
                .zip(&query_q.values)
                .map(|(x, y)| i32::from(*x) * i32::from(*y))
                .sum();
            let score = dot as f32 * self.scales[row] * query_q.scale;
            if score >= min_score {
                hits.push(ScanHit {
                    id: id.clone(),
                    score,
                });
            }
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        hits.truncate(k);
        hits
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn index() -> VectorIndex {
        VectorIndex::build(
            4,
            vec![
                ("docker".to_string(), vec![1.0, 0.1, 0.0, 0.0]),
                ("kubectl".to_string(), vec![0.9, 0.3, 0.1, 0.0]),
                ("greeting".to_string(), vec![0.0, 0.0, 1.0, 0.2]),
                ("unrelated".to_string(), vec![-1.0, 0.0, 0.0, 0.5]),
            ],
        )
    }

    #[test]
    fn top_k_orders_by_similarity_and_respects_k() {
        let hits = index().top_k(&[1.0, 0.2, 0.0, 0.0], 2, 0.0);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "docker");
        assert_eq!(hits[1].id, "kubectl");
    }

    #[test]
    fn min_score_drops_weak_matches() {
        let hits = index().top_k(&[1.0, 0.2, 0.0, 0.0], 10, 0.5);
        assert!(hits.iter().all(|h| h.score >= 0.5));
        assert!(!hits.iter().any(|h| h.id == "unrelated"));
    }

    #[test]
    fn ties_break_deterministically_by_id() {
        let index = VectorIndex::build(
            2,
            vec![
                ("b".to_string(), vec![1.0, 0.0]),
                ("a".to_string(), vec![1.0, 0.0]),
            ],
        );
        let hits = index.top_k(&[1.0, 0.0], 2, 0.0);
        assert_eq!(hits[0].id, "a");
        assert_eq!(hits[1].id, "b");
    }

    #[test]
    fn wrong_dimension_entries_and_queries_are_rejected() {
        let index = VectorIndex::build(
            3,
            vec![
                ("ok".to_string(), vec![1.0, 0.0, 0.0]),
                ("stale".to_string(), vec![1.0, 0.0]),
            ],
        );
        assert_eq!(index.len(), 1);
        assert!(index.top_k(&[1.0, 0.0], 5, 0.0).is_empty());
    }

    #[test]
    fn resident_bytes_reflect_the_int8_matrix() {
        let index = index();
        assert_eq!(index.resident_bytes(), 4 * 4 + 4 * 4);
    }

    #[test]
    #[ignore = "memory/latency evidence run; execute with --release -- --ignored"]
    fn fifty_thousand_vectors_stay_inside_memory_and_scan_budget() {
        // Deterministic pseudo-vectors, no RNG dependency.
        let entries = (0..50_000).map(|i| {
            let mut v = vec![0.0f32; 384];
            for (j, value) in v.iter_mut().enumerate() {
                *value = (((i * 31 + j * 7) % 997) as f32 / 997.0) - 0.5;
            }
            (format!("s{i}"), v)
        });
        let index = VectorIndex::build(384, entries);
        assert_eq!(index.len(), 50_000);
        let resident_mb = index.resident_bytes() as f64 / 1_000_000.0;

        let query: Vec<f32> = (0..384).map(|j| ((j % 13) as f32 / 13.0) - 0.5).collect();
        let started = std::time::Instant::now();
        let hits = index.top_k(&query, 24, -1.0);
        let elapsed = started.elapsed();
        assert_eq!(hits.len(), 24);

        println!("resident: {resident_mb:.1}MB, full scan: {elapsed:?}");
        assert!(
            resident_mb < 25.0,
            "resident {resident_mb:.1}MB exceeds plan"
        );
        assert!(
            elapsed.as_millis() < 50,
            "full scan {elapsed:?} exceeds the search budget"
        );
    }
}
