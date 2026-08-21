// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Schema-level invariants that must hold regardless of application code:
//! the sensitive-content red line, column pairing rules, and foreign keys.

#![allow(clippy::unwrap_used)]

use rusqlite::{Connection, params};
use typvia_core::db::{migrate_to_latest, open_in_memory};

fn fresh_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    conn
}

fn insert_snippet(
    conn: &Connection,
    id: &str,
    plaintext: Option<&str>,
    ciphertext: Option<&[u8]>,
    security_level: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO snippet (
            id, workspace_id, title, content_plaintext, content_ciphertext,
            type, security_level, created_at, updated_at
        ) VALUES (?1, 'w1', 'title', ?2, ?3, 'text', ?4, 0, 0)",
        params![id, plaintext, ciphertext, security_level],
    )
}

#[test]
fn accepts_a_normal_snippet_with_plaintext() {
    let conn = fresh_db();
    assert_eq!(
        insert_snippet(&conn, "s1", Some("body"), None, "normal").unwrap(),
        1
    );
}

#[test]
fn accepts_a_sensitive_snippet_with_ciphertext_only() {
    let conn = fresh_db();
    let blob: &[u8] = &[0xAA, 0xBB];
    assert_eq!(
        insert_snippet(&conn, "s1", None, Some(blob), "sensitive").unwrap(),
        1
    );
}

#[test]
fn rejects_a_sensitive_snippet_carrying_plaintext() {
    let conn = fresh_db();
    let err = insert_snippet(&conn, "s1", Some("secret body"), None, "sensitive").unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn rejects_a_row_with_both_content_columns_set() {
    let conn = fresh_db();
    let blob: &[u8] = &[0x01];
    let err = insert_snippet(&conn, "s1", Some("body"), Some(blob), "normal").unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn rejects_a_row_with_no_content_column_set() {
    let conn = fresh_db();
    let err = insert_snippet(&conn, "s1", None, None, "normal").unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn rejects_a_trigger_without_a_trigger_mode() {
    let conn = fresh_db();
    let err = conn
        .execute(
            "INSERT INTO snippet (
                id, workspace_id, title, content_plaintext, type,
                security_level, \"trigger\", created_at, updated_at
            ) VALUES ('s1', 'w1', 'title', 'body', 'text', 'normal', ':t', 0, 0)",
            [],
        )
        .unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn rejects_a_snippet_referencing_a_missing_folder() {
    let conn = fresh_db();
    let err = conn
        .execute(
            "INSERT INTO snippet (
                id, workspace_id, title, content_plaintext, type,
                security_level, folder_id, created_at, updated_at
            ) VALUES ('s1', 'w1', 'title', 'body', 'text', 'normal', 'nope', 0, 0)",
            [],
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("FOREIGN KEY"),
        "unexpected error: {err}"
    );
}

#[test]
fn deleting_a_snippet_cascades_to_its_tag_links() {
    let conn = fresh_db();
    insert_snippet(&conn, "s1", Some("body"), None, "normal").unwrap();
    conn.execute(
        "INSERT INTO tag (id, name, created_at) VALUES ('t1', 'shell', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO snippet_tag (snippet_id, tag_id) VALUES ('s1', 't1')",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM snippet WHERE id = 's1'", [])
        .unwrap();

    let links: i64 = conn
        .query_row("SELECT COUNT(*) FROM snippet_tag", [], |row| row.get(0))
        .unwrap();
    assert_eq!(links, 0);
}

#[test]
fn rejects_a_revoked_device_without_revocation_time() {
    let conn = fresh_db();
    let err = conn
        .execute(
            "INSERT INTO device (id, name, platform, public_key, trust_level, created_at)
             VALUES ('d1', 'Mac', 'macos', X'01', 'revoked', 0)",
            [],
        )
        .unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn rejects_a_sync_tombstone_carrying_ciphertext() {
    let conn = fresh_db();
    conn.execute(
        "INSERT INTO device (id, name, platform, public_key, trust_level, created_at)
         VALUES ('d1', 'Mac', 'macos', X'01', 'trusted', 0)",
        [],
    )
    .unwrap();
    let err = conn
        .execute(
            "INSERT INTO sync_record (
                id, entity_type, entity_id, version, ciphertext,
                deleted_at, updated_at, device_id
            ) VALUES ('r1', 'snippet', 's1', 1, X'EE', 100, 0, 'd1')",
            [],
        )
        .unwrap_err();
    assert!(err.to_string().contains("CHECK"), "unexpected error: {err}");
}

#[test]
fn fts_table_indexes_and_matches_content() {
    let conn = fresh_db();
    conn.execute(
        "INSERT INTO snippet_fts (snippet_id, title, content, description, tags, folder_name, \"trigger\")
         VALUES ('s1', 'Docker logs', 'docker logs -f app', '', 'shell', 'Commands', ':dlog')",
        [],
    )
    .unwrap();
    let id: String = conn
        .query_row(
            "SELECT snippet_id FROM snippet_fts WHERE snippet_fts MATCH 'docker'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(id, "s1");
}
