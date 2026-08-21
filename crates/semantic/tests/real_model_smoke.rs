// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Real-model smoke test: loads an actual multilingual-e5-small directory
//! and checks that similar sentences outrank dissimilar ones in both
//! languages, recording the single-query latency. Gated behind
//! `TYPVIA_E5_DIR` so CI and normal
//! local runs never pull a 470MB model — run explicitly with:
//! `TYPVIA_E5_DIR=/path/to/e5-small cargo test -p typvia-semantic --test real_model_smoke -- --ignored`

#![allow(clippy::unwrap_used)]

use typvia_semantic::{CandleEmbedder, Embedder, ModelFiles, VectorIndex};

fn model_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("TYPVIA_E5_DIR").map(std::path::PathBuf::from)
}

#[test]
#[ignore = "needs a locally downloaded model; set TYPVIA_E5_DIR"]
fn concept_queries_rank_the_right_snippet_first_in_both_languages() {
    let Some(dir) = model_dir() else {
        panic!("set TYPVIA_E5_DIR to the model directory");
    };
    // The product serves locally-converted f16 weights; prefer that
    // layout when present so this smoke covers the real serving path.
    let f16 = dir.join("model.f16.safetensors");
    let weights = if f16.is_file() {
        f16
    } else {
        dir.join("model.safetensors")
    };
    let embedder = CandleEmbedder::load(
        &ModelFiles {
            config: dir.join("config.json"),
            tokenizer: dir.join("tokenizer.json"),
            weights,
        },
        "multilingual-e5-small",
    )
    .unwrap();
    assert_eq!(embedder.dims(), 384);

    let passages = vec![
        "Tail container logs\ndocker logs -f app".to_string(),
        "Greeting email opener\nHope this finds you well.".to_string(),
        "SSH into the staging box\nssh deploy@staging.internal".to_string(),
        "会议纪要模板\n今天的会议讨论了以下事项：".to_string(),
    ];
    let vectors = embedder.embed_passages(&passages).unwrap();
    let index = VectorIndex::build(
        embedder.dims(),
        vectors
            .into_iter()
            .enumerate()
            .map(|(i, v)| (format!("s{i}"), v)),
    );

    // English concept query — no lexical overlap with the body.
    let started = std::time::Instant::now();
    let query = embedder.embed_query("看容器日志的命令").unwrap();
    let query_latency = started.elapsed();
    let hits = index.top_k(&query, 4, -1.0);
    assert_eq!(hits[0].id, "s0", "docker-logs snippet must rank first");

    // Chinese concept query for the Chinese passage.
    let query = embedder.embed_query("记录会议内容的模板").unwrap();
    let hits = index.top_k(&query, 4, -1.0);
    assert_eq!(hits[0].id, "s3", "meeting-notes snippet must rank first");

    // Cross-check: an English query too.
    let query = embedder.embed_query("connect to a remote server").unwrap();
    let hits = index.top_k(&query, 4, -1.0);
    assert_eq!(hits[0].id, "s2", "ssh snippet must rank first");

    // Latency evidence (test output only).
    println!("single query embed latency: {query_latency:?}");
    assert!(
        query_latency.as_millis() < 2_000,
        "query embedding is pathologically slow: {query_latency:?}"
    );
}
