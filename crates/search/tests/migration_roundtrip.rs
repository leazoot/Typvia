//! Search-index behavior across a schema downgrade/upgrade round trip.
//! Migration 0003 recreates `snippet_fts`, so any round trip crossing
//! version 3 leaves the index empty while every snippet row
//! survives — `SearchIndex::rebuild()` is the documented remediation and
//! must restore search completely.

#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use typvia_core::db::{migrate_to, migrate_to_latest, open_in_memory};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType};
use typvia_core::repo::{SnippetRepo, new_id};
use typvia_search::SearchIndex;
use typvia_search::segment::query_phrase;

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

fn matches(conn: &Connection, term: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT snippet_id FROM snippet_fts WHERE snippet_fts MATCH ?1 ORDER BY snippet_id",
        )
        .unwrap();
    let rows = stmt
        .query_map([query_phrase(term)], |row| row.get::<_, String>(0))
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

#[test]
fn rebuild_restores_search_after_a_round_trip_across_version_three() {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let s = snippet("Deploystep", "release the build");
    SnippetRepo::new(&conn).insert(&s).unwrap();
    SearchIndex::new(&conn).sync_snippet(&s.id).unwrap();
    assert_eq!(matches(&conn, "Deploystep"), vec![s.id.clone()]);

    // Round trip below version 3 and back: the snippet row survives, but
    // the recreated FTS table comes back empty by construction.
    migrate_to(&mut conn, 2).unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let survived: i64 = conn
        .query_row("SELECT COUNT(*) FROM snippet", [], |r| r.get(0))
        .unwrap();
    assert_eq!(survived, 1);
    assert!(matches(&conn, "Deploystep").is_empty());

    // The documented remediation restores search completely.
    let indexed = SearchIndex::new(&conn).rebuild().unwrap();
    assert_eq!(indexed, 1);
    assert_eq!(matches(&conn, "Deploystep"), vec![s.id]);
}
