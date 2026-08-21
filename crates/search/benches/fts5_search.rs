// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Benchmark harness code: aborting on setup failure is the desired behavior,
// so the no-expect rule for runtime paths does not apply here.
#![allow(clippy::expect_used)]

//! FTS5 search performance baseline.
//!
//! Targets: query latency < 50ms at 10k snippets, < 150ms at 50k snippets.
//! The dataset is synthetic but deterministic, mixing English prose, code-like
//! tokens and Chinese fragments to approximate real snippet content.

use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rusqlite::Connection;

/// Deterministic xorshift PRNG so runs are reproducible without extra deps.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

const WORDS: &[&str] = &[
    "server",
    "deploy",
    "invoice",
    "docker",
    "restart",
    "template",
    "address",
    "kubernetes",
    "password",
    "rotate",
    "backup",
    "database",
    "migration",
    "commit",
    "release",
    "webhook",
    "token",
    "billing",
    "shipping",
    "greeting",
    "signature",
    "endpoint",
    "listener",
    "buffer",
    "channel",
    "snippet",
];

const CODE: &[&str] = &[
    "SELECT * FROM users WHERE id = ?",
    "git rebase -i HEAD~3",
    "docker compose up -d",
    "kubectl get pods -A",
    "cargo clippy --workspace",
    "ssh -L 5432:localhost:5432 host",
    "curl -X POST /api/v1/items",
    "npm run build && npm run preview",
];

const CN: &[&str] = &[
    "常用回复",
    "发票抬头",
    "收货地址",
    "部署命令",
    "会议纪要",
    "报价模板",
    "快递信息",
    "重启服务",
];

fn synth_row(rng: &mut Rng, i: usize) -> (String, String, String) {
    let title = format!("{} {} {}", rng.pick(WORDS), rng.pick(WORDS), i);
    let mut content = String::new();
    for _ in 0..30 {
        content.push_str(rng.pick(WORDS));
        content.push(' ');
    }
    content.push_str(rng.pick(CODE));
    content.push(' ');
    content.push_str(rng.pick(CN));
    let tags = format!("{},{}", rng.pick(WORDS), rng.pick(CN));
    (title, content, tags)
}

fn build_db(n: usize) -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(
        "CREATE VIRTUAL TABLE snippet_fts USING fts5(title, content, tags, tokenize='unicode61');",
    )
    .expect("create fts5 table");

    let mut rng = Rng(0x5eed_cafe);
    let t0 = Instant::now();
    {
        let tx_guard = conn.unchecked_transaction().expect("begin tx");
        {
            let mut stmt = tx_guard
                .prepare("INSERT INTO snippet_fts(title, content, tags) VALUES (?1, ?2, ?3)")
                .expect("prepare insert");
            for i in 0..n {
                let (title, content, tags) = synth_row(&mut rng, i);
                stmt.execute((&title, &content, &tags)).expect("insert row");
            }
        }
        tx_guard.commit().expect("commit");
    }
    eprintln!("BUILD n={n}: bulk insert+index took {:?}", t0.elapsed());
    conn
}

fn query_count(conn: &Connection, match_expr: &str) -> usize {
    let mut stmt = conn
        .prepare_cached(
            "SELECT rowid, title FROM snippet_fts WHERE snippet_fts MATCH ?1 \
             ORDER BY bm25(snippet_fts, 5.0, 1.0, 3.0) LIMIT 50",
        )
        .expect("prepare query");
    stmt.query_map([match_expr], |r| r.get::<_, String>(1))
        .expect("run query")
        .count()
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("fts5");
    group.sample_size(30);

    for &n in &[10_000usize, 50_000] {
        let conn = build_db(n);

        // Incremental update cost: single insert + delete, measured outside
        // criterion (one-shot numbers reported to stderr for the spike doc).
        let t0 = Instant::now();
        conn.execute(
            "INSERT INTO snippet_fts(title, content, tags) VALUES ('inc probe', 'incremental update probe row', 'probe')",
            [],
        )
        .expect("incremental insert");
        eprintln!("INCREMENTAL n={n}: single insert took {:?}", t0.elapsed());

        for (name, expr) in [
            ("prefix", "dep*"),
            ("term", "docker"),
            ("multi_term", "server deploy"),
            ("title_prefix", "title:inv*"),
        ] {
            group.bench_with_input(BenchmarkId::new(name, n), &expr, |b, expr| {
                b.iter(|| query_count(&conn, expr))
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
