//! Ingestion of share-inbox documents on iOS and Android. The
//! platform share surface — the iOS share extension in the App Group
//! container, the Android share-target Activity in the app data dir
//! (same-UID direct write; root resolution is `share_inbox_root` in lib.rs)
//! — is write-only into `inbox/` under that root: one JSON document per
//! share, landed tmp+rename, never touching the database.
//! The main app drains the directory at setup, on every return to the
//! foreground and on the frontend's explicit drain command, turning each
//! valid document into a normal snippet through the shared host-service
//! `snippet_create`. Everything below is platform-neutral by design.
//!
//! Pinned schema v1 (written by the extension, read here):
//! `{ "schema_version": 1, "shared_at": <unix ms>, "title"?: string,
//!    "source_hint"?: string, "text": string }`
//! `shared_at` and `source_hint` are informational for the document and not
//! consumed by ingestion (snippet timestamps use the host clock); unknown
//! fields are ignored so v1 keeps forward room.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Deserialize;
use typvia_host_service::dto::SnippetCreateInput;
use typvia_host_service::error::IpcErrorCode;
use typvia_host_service::service;

/// Directory inside the share-inbox root the platform share surface writes
/// into (identical relative layout on both platforms).
pub const INBOX_DIR_NAME: &str = "inbox";
/// The only inbox document schema this host understands.
const SCHEMA_VERSION: u32 = 1;
/// Upper bound for the shared text (UTF-8 bytes); both share surfaces
/// mirror this cap in their Save validation.
pub const SHARE_TEXT_MAX_BYTES: usize = 128 * 1024;
/// Upper bound for a whole inbox file (text cap + JSON envelope headroom);
/// anything larger is quarantined without being read into memory.
const SHARE_FILE_MAX_BYTES: u64 = 256 * 1024;
/// Presentation bound shared with the mobile editor's derived-title rule and
/// the desktop import derivation.
const TITLE_MAX_CHARS: usize = 60;

/// The fields ingestion consumes from a v1 document. Serde ignores the
/// informational fields (`shared_at`, `source_hint`) and anything unknown.
#[derive(Debug, Deserialize)]
struct ShareInboxDocument {
    schema_version: u32,
    title: Option<String>,
    text: String,
}

/// Per-file ingest verdict; drives what happens to the file afterwards.
enum FileOutcome {
    /// Snippet created — remove the file (see the duplicate-window note).
    Created,
    /// The document can never become a snippet — quarantine it.
    Invalid,
    /// Transient storage fault — keep the file for the next pass.
    Retry,
}

/// Drains `inbox/*.json` under `container_dir` into the library and returns
/// how many snippets were created. Best-effort by design: malformed or
/// invalid documents are quarantined (renamed `*.json.rejected`, out of the
/// scan set), storage faults leave the file for the next pass, and nothing
/// here panics or logs (a document holds user content — log red line).
pub fn ingest(conn: &Connection, container_dir: &Path, now: i64) -> u32 {
    let inbox = container_dir.join(INBOX_DIR_NAME);
    let Ok(entries) = std::fs::read_dir(&inbox) else {
        // No inbox directory yet: nothing was ever shared on this device.
        return 0;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        // Only final documents: the writer's `*.json.tmp` files and
        // quarantined `*.json.rejected` files fail the extension check.
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    // Name order (random UUIDs) — deterministic across passes, not temporal.
    files.sort();

    let mut created = 0;
    for path in files {
        match ingest_file(conn, &path, now) {
            FileOutcome::Created => {
                created += 1;
                // DB create first, file removal second: a crash in between
                // re-ingests this document next pass (duplicate snippet).
                // That tiny window is accepted over adding a dedupe column
                // to the schema; the reverse order could lose the share.
                let _ = std::fs::remove_file(&path);
            }
            FileOutcome::Invalid => quarantine(&path),
            FileOutcome::Retry => {}
        }
    }
    created
}

fn ingest_file(conn: &Connection, path: &Path, now: i64) -> FileOutcome {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > SHARE_FILE_MAX_BYTES => return FileOutcome::Invalid,
        Ok(_) => {}
        Err(_) => return FileOutcome::Retry,
    }
    let Ok(bytes) = std::fs::read(path) else {
        return FileOutcome::Retry;
    };
    let Ok(document) = serde_json::from_slice::<ShareInboxDocument>(&bytes) else {
        return FileOutcome::Invalid;
    };
    if document.schema_version != SCHEMA_VERSION {
        return FileOutcome::Invalid;
    }
    if document.text.trim().is_empty() || document.text.len() > SHARE_TEXT_MAX_BYTES {
        return FileOutcome::Invalid;
    }
    let Some(title) = snippet_title(document.title.as_deref(), &document.text) else {
        return FileOutcome::Invalid;
    };
    let input = SnippetCreateInput {
        title,
        body: document.text,
        snippet_type: "text".to_string(),
        description: None,
        folder_id: None,
        trigger: None,
        trigger_mode: None,
        language: None,
    };
    // Plain snippets only (desktop import parity): sensitive detection is an
    // advisory editor service, never a silent vault write on ingest.
    match service::snippet_create(conn, input, now) {
        Ok(_) => FileOutcome::Created,
        Err(error) if error.code == IpcErrorCode::System => FileOutcome::Retry,
        Err(_) => FileOutcome::Invalid,
    }
}

/// Bounded presentation title: the sender's typed title when one was sent,
/// otherwise the first non-empty line of the text — the same ~60-char rule
/// the mobile editor and desktop import use. `None` only when the text has
/// no non-blank line (already rejected by the caller's blank check).
fn snippet_title(typed: Option<&str>, text: &str) -> Option<String> {
    let explicit = typed.and_then(first_non_empty_line);
    let base = explicit.or_else(|| first_non_empty_line(text))?;
    Some(base.chars().take(TITLE_MAX_CHARS).collect())
}

fn first_non_empty_line(value: &str) -> Option<&str> {
    value.lines().map(str::trim).find(|line| !line.is_empty())
}

/// Moves a bad document out of the scan set: `*.json` → `*.json.rejected`,
/// same directory (atomic rename). If the rename fails the file is removed;
/// as a last resort it stays and is re-judged (still invalid) next pass.
fn quarantine(path: &Path) {
    let rejected = path.with_extension("json.rejected");
    if std::fs::rename(path, &rejected).is_err() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_core::repo::{ListScope, SnippetRepo};
    use typvia_host_service::dto::SnippetDto;

    fn migrated_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn write_inbox_file(container: &Path, name: &str, contents: &str) -> PathBuf {
        let inbox = container.join(INBOX_DIR_NAME);
        std::fs::create_dir_all(&inbox).unwrap();
        let path = inbox.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn document_json(text: &str, title: Option<&str>) -> String {
        serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "shared_at": 1_700_000_000_000_i64,
            "title": title,
            "source_hint": "Safari",
            "text": text,
        }))
        .unwrap()
    }

    fn all_snippets(conn: &Connection) -> Vec<SnippetDto> {
        service::snippet_list_page(conn, "all", None, None, 50, 0).unwrap()
    }

    #[test]
    fn valid_document_becomes_a_text_snippet_and_the_file_is_removed() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let body = "Ship it behind a flag\nRevert is cheap.";
        let path = write_inbox_file(dir.path(), "share-a.json", &document_json(body, None));

        let created = ingest(&conn, dir.path(), 1_700_000_001_000);

        assert_eq!(created, 1);
        let rows = all_snippets(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Ship it behind a flag");
        assert_eq!(rows[0].body.as_deref(), Some(body));
        assert_eq!(rows[0].snippet_type, "text");
        assert_eq!(rows[0].security_level, "normal");
        assert!(!path.exists());
    }

    #[test]
    fn a_typed_title_wins_over_the_derived_one() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        write_inbox_file(
            dir.path(),
            "share-a.json",
            &document_json("first line\nrest", Some("  Release notes  ")),
        );

        assert_eq!(ingest(&conn, dir.path(), 1_000), 1);
        assert_eq!(all_snippets(&conn)[0].title, "Release notes");
    }

    #[test]
    fn a_long_derived_title_is_bounded_to_sixty_chars() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let long_line = "x".repeat(90);
        write_inbox_file(dir.path(), "share-a.json", &document_json(&long_line, None));

        assert_eq!(ingest(&conn, dir.path(), 1_000), 1);
        assert_eq!(all_snippets(&conn)[0].title, "x".repeat(60));
    }

    #[test]
    fn malformed_json_is_quarantined_and_later_files_still_ingest() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let bad = write_inbox_file(dir.path(), "share-a.json", "{ not json");
        let good = write_inbox_file(dir.path(), "share-b.json", &document_json("kept", None));

        let created = ingest(&conn, dir.path(), 1_000);

        assert_eq!(created, 1);
        assert_eq!(all_snippets(&conn)[0].title, "kept");
        assert!(!bad.exists());
        assert!(bad.with_extension("json.rejected").exists());
        assert!(!good.exists());
    }

    #[test]
    fn oversized_text_is_quarantined_without_creating_a_snippet() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let oversized = "y".repeat(SHARE_TEXT_MAX_BYTES + 1);
        let path = write_inbox_file(dir.path(), "share-a.json", &document_json(&oversized, None));

        assert_eq!(ingest(&conn, dir.path(), 1_000), 0);
        assert!(all_snippets(&conn).is_empty());
        assert!(!path.exists());
        assert!(path.with_extension("json.rejected").exists());
    }

    #[test]
    fn blank_text_is_quarantined() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let path = write_inbox_file(dir.path(), "share-a.json", &document_json("  \n\t ", None));

        assert_eq!(ingest(&conn, dir.path(), 1_000), 0);
        assert!(all_snippets(&conn).is_empty());
        assert!(path.with_extension("json.rejected").exists());
    }

    #[test]
    fn unknown_schema_version_is_quarantined() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let path = write_inbox_file(
            dir.path(),
            "share-a.json",
            r#"{"schema_version":2,"shared_at":1,"text":"future"}"#,
        );

        assert_eq!(ingest(&conn, dir.path(), 1_000), 0);
        assert!(all_snippets(&conn).is_empty());
        assert!(path.with_extension("json.rejected").exists());
    }

    #[test]
    fn writer_temp_files_are_ignored() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        let tmp = write_inbox_file(dir.path(), "share-a.json.tmp", &document_json("half", None));

        assert_eq!(ingest(&conn, dir.path(), 1_000), 0);
        assert!(all_snippets(&conn).is_empty());
        // A tmp file is the writer's business: never read, never quarantined.
        assert!(tmp.exists());
    }

    #[test]
    fn a_missing_inbox_directory_ingests_nothing() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(ingest(&conn, dir.path(), 1_000), 0);
    }

    #[test]
    fn a_second_pass_over_a_drained_inbox_creates_nothing_new() {
        let conn = migrated_conn();
        let dir = tempfile::tempdir().unwrap();
        write_inbox_file(dir.path(), "share-a.json", &document_json("once", None));

        assert_eq!(ingest(&conn, dir.path(), 1_000), 1);
        assert_eq!(ingest(&conn, dir.path(), 2_000), 0);
        let count = SnippetRepo::new(&conn)
            .count_scoped(ListScope::All, None)
            .unwrap();
        assert_eq!(count, 1);
    }
}
