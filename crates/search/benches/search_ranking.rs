// Benchmark harness code: aborting on setup failure is the desired behavior,
// so the no-expect rule for runtime paths does not apply here.
#![allow(clippy::expect_used)]

//! End-to-end search latency over the real schema.
//!
//! Unlike the fts5_search spike baseline (bare FTS table), this measures the
//! full path: query parsing, FTS candidate retrieval over the migrated
//! schema, and tier classification/sorting in Rust.
//! Targets: < 50ms at 10k snippets, < 150ms at 50k.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, TriggerMode};
use typvia_core::repo::{SnippetRepo, new_id};
use typvia_search::{SearchIndex, Searcher};

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

fn build_db(n: usize) -> Connection {
    let mut conn = open_in_memory().expect("open in-memory db");
    migrate_to_latest(&mut conn).expect("migrate");
    let repo = SnippetRepo::new(&conn);
    let mut rng = Rng(0x5eed_cafe);

    let tx = conn.unchecked_transaction().expect("begin tx");
    for i in 0..n {
        let mut content = String::new();
        for _ in 0..30 {
            content.push_str(rng.pick(WORDS));
            content.push(' ');
        }
        content.push_str(rng.pick(CN));
        let mut s = Snippet {
            id: new_id(),
            workspace_id: "w1".to_string(),
            title: format!("{} {} {i}", rng.pick(WORDS), rng.pick(CN)),
            content: SnippetContent::Plaintext(content),
            snippet_type: SnippetType::Text,
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
            security_level: SecurityLevel::Normal,
            is_favorite: false,
            is_pinned: false,
            is_enabled: true,
            platform_scope: vec![],
            created_at: 1_000,
            updated_at: 1_000,
            last_used_at: Some((rng.next() % 1_000_000) as i64),
            usage_count: rng.next() % 500,
            version: 1,
            deleted_at: None,
            conflict_of: None,
        };
        // A sparse population of triggers, like real vaults.
        if i % 50 == 0 {
            s.trigger = Some(format!(":tr{i}"));
            s.trigger_mode = Some(TriggerMode::Delimiter);
        }
        repo.insert(&s).expect("insert snippet");
    }
    tx.commit().expect("commit");

    SearchIndex::new(&conn).rebuild().expect("rebuild index");
    conn
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_ranking");
    group.sample_size(20);

    for &n in &[10_000usize, 50_000] {
        let conn = build_db(n);
        let searcher = Searcher::new(&conn);

        for (name, query) in [
            ("term", "docker"),
            ("prefix", "dep"),
            ("multi_term", "server deploy"),
            ("cjk", "发票"),
            ("mixed", "docker 部署"),
        ] {
            group.bench_with_input(BenchmarkId::new(name, n), &query, |b, q| {
                b.iter(|| searcher.search(q, 50, 0).expect("search"));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
