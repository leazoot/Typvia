// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Relative ordering of every ranking level:
//! title exact > trigger > title prefix > tag > content, then recency,
//! then usage frequency; plus query combination semantics.

#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, Tag, TriggerMode};
use typvia_core::repo::{SnippetRepo, TagRepo, new_id};
use typvia_search::{MatchTier, SearchIndex, Searcher};

fn fresh_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    conn
}

fn snippet(title: &str, body: &str) -> Snippet {
    Snippet {
        id: new_id(),
        workspace_id: "w1".to_string(),
        title: title.to_string(),
        content: SnippetContent::Plaintext(body.to_string()),
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
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

fn insert_and_index(conn: &Connection, s: &Snippet) {
    SnippetRepo::new(conn).insert(s).unwrap();
    SearchIndex::new(conn).sync_snippet(&s.id).unwrap();
}

fn hit_ids(conn: &Connection, query: &str) -> Vec<String> {
    Searcher::new(conn)
        .search(query, 50, 0)
        .unwrap()
        .into_iter()
        .map(|h| h.snippet_id)
        .collect()
}

#[test]
fn tiers_rank_exact_trigger_prefix_tag_content() {
    let conn = fresh_db();
    let tags = TagRepo::new(&conn);
    let snippets = SnippetRepo::new(&conn);
    let index = SearchIndex::new(&conn);

    let exact = snippet("deploy", "alpha body");
    let mut trigger = snippet("Server restart", "beta body");
    trigger.trigger = Some(":deploy".to_string());
    trigger.trigger_mode = Some(TriggerMode::Delimiter);
    let prefix = snippet("deployment guide", "gamma body");
    let tagged = snippet("Cluster notes", "delta body");
    let content = snippet("Weekly report", "how we deploy on fridays");

    for s in [&exact, &trigger, &prefix, &tagged, &content] {
        snippets.insert(s).unwrap();
    }
    let tag = Tag {
        id: new_id(),
        name: "deploy".to_string(),
        created_at: 1_000,
    };
    tags.insert(&tag).unwrap();
    snippets
        .batch_add_tag(std::slice::from_ref(&tagged.id), &tag.id)
        .unwrap();
    for s in [&exact, &trigger, &prefix, &tagged, &content] {
        index.sync_snippet(&s.id).unwrap();
    }

    let hits = Searcher::new(&conn).search("deploy", 50, 0).unwrap();
    let order: Vec<&str> = hits.iter().map(|h| h.snippet_id.as_str()).collect();
    assert_eq!(
        order,
        vec![
            exact.id.as_str(),
            trigger.id.as_str(),
            prefix.id.as_str(),
            tagged.id.as_str(),
            content.id.as_str(),
        ]
    );
    let tiers: Vec<MatchTier> = hits.iter().map(|h| h.tier).collect();
    assert_eq!(
        tiers,
        vec![
            MatchTier::TitleExact,
            MatchTier::Trigger,
            MatchTier::TitlePrefix,
            MatchTier::Tag,
            MatchTier::Content,
        ]
    );
}

#[test]
fn title_exact_match_is_case_insensitive() {
    let conn = fresh_db();
    let s = snippet("Deploy", "body");
    insert_and_index(&conn, &s);
    let hits = Searcher::new(&conn).search("deploy", 10, 0).unwrap();
    assert_eq!(hits[0].tier, MatchTier::TitleExact);
}

#[test]
fn recency_orders_within_a_tier() {
    let conn = fresh_db();
    let mut old = snippet("Notes one", "shared deploy words");
    old.last_used_at = Some(1_000);
    let mut fresh = snippet("Notes two", "shared deploy words");
    fresh.last_used_at = Some(9_000);
    let never = snippet("Notes three", "shared deploy words");
    for s in [&old, &fresh, &never] {
        insert_and_index(&conn, s);
    }

    assert_eq!(
        hit_ids(&conn, "deploy"),
        vec![fresh.id.clone(), old.id.clone(), never.id.clone()]
    );
}

#[test]
fn usage_count_orders_when_recency_is_equal() {
    let conn = fresh_db();
    let mut rare = snippet("Alpha note", "shared deploy words");
    rare.last_used_at = Some(5_000);
    rare.usage_count = 2;
    let mut frequent = snippet("Beta note", "shared deploy words");
    frequent.last_used_at = Some(5_000);
    frequent.usage_count = 40;
    for s in [&rare, &frequent] {
        insert_and_index(&conn, s);
    }

    assert_eq!(
        hit_ids(&conn, "deploy"),
        vec![frequent.id.clone(), rare.id.clone()]
    );
}

#[test]
fn multi_term_queries_require_every_term() {
    let conn = fresh_db();
    let both = snippet("Runbook", "docker restart sequence");
    let only_one = snippet("Notes", "docker compose file");
    for s in [&both, &only_one] {
        insert_and_index(&conn, s);
    }

    assert_eq!(hit_ids(&conn, "docker restart"), vec![both.id.clone()]);
}

#[test]
fn last_term_matches_by_prefix_while_typing() {
    let conn = fresh_db();
    let s = snippet("Runbook", "restart nginx gracefully");
    insert_and_index(&conn, &s);

    assert_eq!(hit_ids(&conn, "restart ngi"), vec![s.id.clone()]);
    // Non-final terms stay whole-token: no hit for a bare prefix there.
    assert!(hit_ids(&conn, "rest nginx").is_empty());
}

#[test]
fn cjk_title_prefix_outranks_cjk_content() {
    let conn = fresh_db();
    let title_hit = snippet("发票抬头", "公司税务信息");
    let content_hit = snippet("公司资料", "开具发票需要抬头");
    for s in [&title_hit, &content_hit] {
        insert_and_index(&conn, s);
    }

    let hits = Searcher::new(&conn).search("发票", 10, 0).unwrap();
    assert_eq!(hits[0].snippet_id, title_hit.id);
    assert_eq!(hits[0].tier, MatchTier::TitlePrefix);
    assert_eq!(hits[1].snippet_id, content_hit.id);
    assert_eq!(hits[1].tier, MatchTier::Content);
}

#[test]
fn blank_and_punctuation_queries_return_no_hits() {
    let conn = fresh_db();
    let s = snippet("Anything", "body");
    insert_and_index(&conn, &s);

    for query in ["", "   ", "::"] {
        assert!(hit_ids(&conn, query).is_empty(), "query: {query:?}");
    }
}

#[test]
fn fts_operators_in_queries_are_literal_terms() {
    let conn = fresh_db();
    let with_and = snippet("Songlist", "alpha and beta duet");
    let without_and = snippet("Duolist", "alpha beta duet");
    for s in [&with_and, &without_and] {
        insert_and_index(&conn, s);
    }

    // "AND" is a term, not an operator: only the snippet containing the
    // word "and" matches.
    assert_eq!(hit_ids(&conn, "alpha AND beta"), vec![with_and.id.clone()]);
    // A stray quote must not produce an FTS syntax error.
    assert!(hit_ids(&conn, "say\"hi").is_empty());
}

#[test]
fn pagination_applies_after_ranking() {
    let conn = fresh_db();
    let mut ids = Vec::new();
    for i in 0..5 {
        let mut s = snippet(&format!("Note {i}"), "shared deploy words");
        s.last_used_at = Some(i64::from(10 - i));
        insert_and_index(&conn, &s);
        ids.push(s.id);
    }

    let searcher = Searcher::new(&conn);
    let page1: Vec<String> = searcher
        .search("deploy", 2, 0)
        .unwrap()
        .into_iter()
        .map(|h| h.snippet_id)
        .collect();
    let page2: Vec<String> = searcher
        .search("deploy", 2, 2)
        .unwrap()
        .into_iter()
        .map(|h| h.snippet_id)
        .collect();
    assert_eq!(page1, vec![ids[0].clone(), ids[1].clone()]);
    assert_eq!(page2, vec![ids[2].clone(), ids[3].clone()]);
}
