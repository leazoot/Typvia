//! Sensitive index isolation red line (regression-mandatory).
//!
//! Proves at the byte level that a sensitive snippet's body never reaches
//! FTS storage: the plaintext feature string is unmatchable through FTS,
//! absent from every `snippet_fts_*` shadow table, and absent from the whole
//! database file (the stored "ciphertext" is different bytes, so any
//! occurrence anywhere in the file would be a leak).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, Tag};
use typvia_core::repo::{SnippetRepo, TagRepo, new_id};
use typvia_search::{SearchIndex, Searcher};

/// The secret body as the user would type it. Never stored anywhere in
/// these tests: what lands in the database is [`fake_ciphertext`] bytes.
const SECRET_MARKER: &str = "XKCD9931_FAKE_SECRET_FEATURE";

/// Stands in for real encryption: flips the bits so the stored bytes share
/// no substring with the plaintext.
fn fake_ciphertext(plaintext: &str) -> Vec<u8> {
    plaintext.bytes().map(|b| b ^ 0xFF).collect()
}

/// On-disk database that cleans up after itself; file-backed so index
/// storage can be scanned as raw bytes.
struct DiskDb {
    path: PathBuf,
    conn: Connection,
}

impl DiskDb {
    fn create() -> Self {
        let path = std::env::temp_dir().join(format!("typvia-redline-{}.db", new_id()));
        let mut conn = open(&path).unwrap();
        migrate_to_latest(&mut conn).unwrap();
        Self { path, conn }
    }

    /// All bytes of the main database file, WAL flushed in first.
    fn file_bytes(&self) -> Vec<u8> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        fs::read(&self.path).unwrap()
    }
}

impl Drop for DiskDb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("db-wal"));
        let _ = fs::remove_file(self.path.with_extension("db-shm"));
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Concatenated bytes of every TEXT/BLOB value in all `snippet_fts_*`
/// shadow tables — the entirety of FTS index storage, read row by row.
fn fts_storage_bytes(conn: &Connection) -> Vec<u8> {
    let mut names_stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
              WHERE type = 'table' AND name LIKE 'snippet\\_fts\\_%' ESCAPE '\\'",
        )
        .unwrap();
    let tables: Vec<String> = names_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!tables.is_empty(), "no FTS shadow tables found");

    let mut bytes = Vec::new();
    for table in tables {
        // Shadow table names come from sqlite_master, not user input.
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
        let column_count = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..column_count {
                match row.get_ref(i).unwrap() {
                    rusqlite::types::ValueRef::Text(t) => bytes.extend_from_slice(t),
                    rusqlite::types::ValueRef::Blob(b) => bytes.extend_from_slice(b),
                    _ => {}
                }
            }
        }
    }
    bytes
}

fn sensitive_snippet(title: &str, body_plaintext: &str) -> Snippet {
    Snippet {
        id: new_id(),
        workspace_id: "w1".to_string(),
        title: title.to_string(),
        content: SnippetContent::Ciphertext(fake_ciphertext(body_plaintext)),
        snippet_type: SnippetType::Sensitive,
        description: None,
        folder_id: None,
        trigger: None,
        trigger_mode: None,
        language: None,
        security_level: SecurityLevel::Sensitive,
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

#[test]
fn sensitive_body_is_unreachable_through_fts_queries() {
    let db = DiskDb::create();
    let snippets = SnippetRepo::new(&db.conn);
    let index = SearchIndex::new(&db.conn);

    let s = sensitive_snippet("Vaultentry", SECRET_MARKER);
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    // Not through MATCH, not through raw LIKE over any FTS column.
    let searcher = Searcher::new(&db.conn);
    assert!(searcher.search(SECRET_MARKER, 10, 0).unwrap().is_empty());
    let like_hits: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM snippet_fts
              WHERE title LIKE ?1 OR content LIKE ?1 OR description LIKE ?1
                 OR tags LIKE ?1 OR folder_name LIKE ?1 OR \"trigger\" LIKE ?1
                 OR language LIKE ?1",
            [format!("%{SECRET_MARKER}%")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(like_hits, 0);
    // The snippet itself is findable by title.
    assert_eq!(searcher.search("Vaultentry", 10, 0).unwrap().len(), 1);
}

#[test]
fn sensitive_body_bytes_are_absent_from_index_storage_and_file() {
    let db = DiskDb::create();
    let snippets = SnippetRepo::new(&db.conn);
    let tags = TagRepo::new(&db.conn);
    let index = SearchIndex::new(&db.conn);

    let tag = Tag {
        id: new_id(),
        name: "vaulttag".to_string(),
        created_at: 1_000,
    };
    tags.insert(&tag).unwrap();
    let mut s = sensitive_snippet("Vaultentry", SECRET_MARKER);
    s.description = Some("vaultnote".to_string());
    snippets.insert(&s).unwrap();
    snippets
        .batch_add_tag(std::slice::from_ref(&s.id), &tag.id)
        .unwrap();
    index.sync_snippet(&s.id).unwrap();

    // Positive control: indexed fields do appear in index storage, so the
    // scan is looking at the right bytes.
    let storage = fts_storage_bytes(&db.conn);
    assert!(contains_bytes(&storage, b"vaultentry") || contains_bytes(&storage, b"Vaultentry"));
    assert!(contains_bytes(&storage, b"vaulttag"));
    assert!(contains_bytes(&storage, b"vaultnote"));
    // Red line: the body feature string is nowhere in index storage.
    assert!(!contains_bytes(&storage, SECRET_MARKER.as_bytes()));

    // Stronger: the plaintext appears nowhere in the whole database file —
    // only the (fake) ciphertext bytes are stored.
    let file = db.file_bytes();
    assert!(contains_bytes(&file, b"Vaultentry"));
    assert!(!contains_bytes(&file, SECRET_MARKER.as_bytes()));
}

#[test]
fn converting_a_snippet_to_sensitive_purges_its_old_plaintext_from_the_index() {
    let db = DiskDb::create();
    let snippets = SnippetRepo::new(&db.conn);
    let index = SearchIndex::new(&db.conn);

    // Starts life as a normal snippet with the secret typed in plain text.
    let mut s = sensitive_snippet("Careless", "placeholder");
    s.snippet_type = SnippetType::Text;
    s.security_level = SecurityLevel::Normal;
    s.content = SnippetContent::Plaintext(format!("note with {SECRET_MARKER} inside"));
    snippets.insert(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();
    assert!(contains_bytes(
        &fts_storage_bytes(&db.conn),
        SECRET_MARKER.as_bytes()
    ));

    // The user marks it sensitive; the body becomes ciphertext.
    s.snippet_type = SnippetType::Sensitive;
    s.security_level = SecurityLevel::Sensitive;
    s.content = SnippetContent::Ciphertext(fake_ciphertext(SECRET_MARKER));
    s.version = 2;
    snippets.update(&s).unwrap();
    index.sync_snippet(&s.id).unwrap();

    // Logically unmatchable right away.
    assert!(
        Searcher::new(&db.conn)
            .search(SECRET_MARKER, 10, 0)
            .unwrap()
            .is_empty()
    );

    // Physically purged from index storage after segment merge.
    index.optimize().unwrap();
    assert!(!contains_bytes(
        &fts_storage_bytes(&db.conn),
        SECRET_MARKER.as_bytes()
    ));
}

#[test]
fn search_results_carry_the_security_marker() {
    let db = DiskDb::create();
    let snippets = SnippetRepo::new(&db.conn);
    let index = SearchIndex::new(&db.conn);

    let sensitive = sensitive_snippet("Sharedword vault", SECRET_MARKER);
    let mut normal = sensitive_snippet("Sharedword plain", "unused");
    normal.snippet_type = SnippetType::Text;
    normal.security_level = SecurityLevel::Normal;
    normal.content = SnippetContent::Plaintext("ordinary body".to_string());
    for s in [&sensitive, &normal] {
        snippets.insert(s).unwrap();
        index.sync_snippet(&s.id).unwrap();
    }

    let hits = Searcher::new(&db.conn).search("Sharedword", 10, 0).unwrap();
    assert_eq!(hits.len(), 2);
    for hit in hits {
        if hit.snippet_id == sensitive.id {
            assert!(hit.is_sensitive);
        } else {
            assert!(!hit.is_sensitive);
        }
    }
}
