// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Per-vector int8 quantization: the resident index stores ~1/4 of the
//! f32 footprint. Vectors are L2 normalized before quantization, so the
//! int8 dot product (rescaled) approximates cosine similarity directly.

/// One quantized vector: symmetric int8 with a per-vector scale.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantizedVector {
    pub scale: f32,
    pub values: Vec<i8>,
}

/// L2-normalizes in place; an all-zero vector stays zero.
pub fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

/// Quantizes a (normalized) vector to symmetric int8.
pub fn quantize(vector: &[f32]) -> QuantizedVector {
    let max_abs = vector.iter().fold(0.0f32, |acc, v| acc.max(v.abs()));
    if max_abs <= f32::EPSILON {
        return QuantizedVector {
            scale: 0.0,
            values: vec![0; vector.len()],
        };
    }
    let scale = max_abs / 127.0;
    let values = vector
        .iter()
        .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    QuantizedVector { scale, values }
}

/// Approximate cosine of two quantized (pre-normalized) vectors: the int8
/// dot product accumulated in i32, rescaled by both scales. Test-only
/// reference — the index inlines the same arithmetic over row slices.
#[cfg(test)]
pub fn cosine(a: &QuantizedVector, b: &QuantizedVector) -> f32 {
    debug_assert_eq!(a.values.len(), b.values.len());
    let dot: i32 = a
        .values
        .iter()
        .zip(&b.values)
        .map(|(x, y)| i32::from(*x) * i32::from(*y))
        .sum();
    dot as f32 * a.scale * b.scale
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn normalized(values: &[f32]) -> Vec<f32> {
        let mut v = values.to_vec();
        normalize(&mut v);
        v
    }

    fn reference_cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn quantized_cosine_tracks_the_f32_reference_closely() {
        let a = normalized(&[0.2, -0.7, 0.5, 0.1, 0.9, -0.3]);
        let b = normalized(&[0.1, -0.6, 0.6, 0.0, 0.8, -0.2]);
        let c = normalized(&[-0.9, 0.2, -0.1, 0.7, -0.4, 0.5]);

        let qa = quantize(&a);
        let qb = quantize(&b);
        let qc = quantize(&c);

        assert!((cosine(&qa, &qb) - reference_cosine(&a, &b)).abs() < 0.02);
        assert!((cosine(&qa, &qc) - reference_cosine(&a, &c)).abs() < 0.02);
        // Ordering is preserved: similar pair beats dissimilar pair.
        assert!(cosine(&qa, &qb) > cosine(&qa, &qc));
    }

    #[test]
    fn a_vector_is_most_similar_to_itself() {
        let a = normalized(&[0.3, 0.1, -0.5, 0.8]);
        let qa = quantize(&a);
        assert!((cosine(&qa, &qa) - 1.0).abs() < 0.02);
    }

    #[test]
    fn a_zero_vector_quantizes_to_zero_without_dividing() {
        let q = quantize(&[0.0; 8]);
        assert_eq!(q.scale, 0.0);
        assert!(q.values.iter().all(|v| *v == 0));
        assert_eq!(cosine(&q, &q), 0.0);
    }
}
