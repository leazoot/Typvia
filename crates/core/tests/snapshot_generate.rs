//! Keyboard snapshot generation: content policy, ordering, and the snapshot
//! red line — sensitive plaintext must never reach the serialized bytes.

#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use typvia_core::db::{migrate_to_latest, open_in_memory};
use typvia_core::model::{
    Folder, KeyboardSnapshot, SNAPSHOT_VERSION, SecurityLevel, SnapshotSnippet, Snippet,
    SnippetContent, SnippetType, TriggerMode,
};
use typvia_core::repo::{FolderRepo, SnippetRepo};
use typvia_core::snapshot::generate;
use typvia_core::vault::VaultSession;

const NOW: i64 = 1_700_000_000_000;
const DEVICE: &str = "device-1";

fn fresh_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate_to_latest(&mut conn).unwrap();
    conn
}

fn snippet(id: &str, title: &str, body: &str) -> Snippet {
    Snippet {
        id: id.to_string(),
        workspace_id: "w1".to_string(),
        title: title.to_string(),
        content: SnippetContent::Plaintext(body.to_string()),
        snippet_type: SnippetType::Command,
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

fn folder(id: &str, name: &str, sort_order: i32) -> Folder {
    Folder {
        id: id.to_string(),
        parent_id: None,
        name: name.to_string(),
        sort_order,
        created_at: 1_000,
        updated_at: 1_000,
    }
}

/// Creates a sensitive snippet through the real vault path: the body is
/// encrypted by the unlocked session (AAD-bound to the snippet id), exactly
/// as the host-service vault_create_secret flow stores it.
fn insert_secret(
    conn: &Connection,
    session: &VaultSession,
    id: &str,
    title: &str,
    trigger: &str,
    body: &str,
) {
    let envelope = session.encrypt_content(conn, id, body.as_bytes()).unwrap();
    let mut s = snippet(id, title, "");
    s.content = SnippetContent::Ciphertext(envelope);
    s.snippet_type = SnippetType::Sensitive;
    s.security_level = SecurityLevel::Sensitive;
    s.trigger = Some(trigger.to_string());
    s.trigger_mode = Some(TriggerMode::Delimiter);
    SnippetRepo::new(conn).insert(&s).unwrap();
}

fn entry_ids(snapshot: &KeyboardSnapshot) -> Vec<&str> {
    snapshot
        .snippets
        .iter()
        .map(|entry| match entry {
            SnapshotSnippet::Normal { id, .. } | SnapshotSnippet::Sensitive { id, .. } => {
                id.as_str()
            }
        })
        .collect()
}

#[test]
fn an_empty_library_yields_a_valid_empty_snapshot() {
    let conn = fresh_db();
    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(snapshot.snapshot_version, SNAPSHOT_VERSION);
    assert_eq!(snapshot.generated_at, NOW);
    assert_eq!(snapshot.device_id, DEVICE);
    assert!(snapshot.snippets.is_empty());
    assert!(snapshot.recent_ids.is_empty());
    assert!(snapshot.favorite_ids.is_empty());
    assert!(snapshot.folder_metadata.is_empty());
    assert_eq!(snapshot.validate(), Ok(()));
}

#[test]
fn trashed_and_disabled_snippets_stay_out_of_the_snapshot() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    repo.insert(&snippet("s1", "Kept", "kept body")).unwrap();
    let mut trashed = snippet("s2", "Trashed", "trashed body");
    trashed.deleted_at = Some(NOW);
    repo.insert(&trashed).unwrap();
    let mut disabled = snippet("s3", "Disabled", "disabled body");
    disabled.is_enabled = false;
    repo.insert(&disabled).unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(entry_ids(&snapshot), vec!["s1"]);
}

#[test]
fn a_normal_entry_carries_body_trigger_and_folder_fields() {
    let conn = fresh_db();
    FolderRepo::new(&conn)
        .insert(&folder("f1", "Shell", 0))
        .unwrap();
    let mut s = snippet("s1", "Docker logs", "docker logs -f app");
    s.trigger = Some(":dlog".to_string());
    s.trigger_mode = Some(TriggerMode::Delimiter);
    s.folder_id = Some("f1".to_string());
    s.is_favorite = true;
    SnippetRepo::new(&conn).insert(&s).unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(
        snapshot.snippets,
        vec![SnapshotSnippet::Normal {
            id: "s1".to_string(),
            title: "Docker logs".to_string(),
            snippet_type: "command".to_string(),
            trigger: Some(":dlog".to_string()),
            trigger_mode: Some("delimiter".to_string()),
            folder_id: Some("f1".to_string()),
            is_favorite: true,
            body: "docker logs -f app".to_string(),
        }]
    );
}

#[test]
fn a_sensitive_entry_exposes_only_id_and_encrypted_metadata_keys() {
    let conn = fresh_db();
    let mut session = VaultSession::new();
    session.initialize(&conn, b"a vault password", NOW).unwrap();
    insert_secret(&conn, &session, "s1", "Prod key", ":prod", "secret body");

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    let value = serde_json::to_value(&snapshot).unwrap();
    let entry = value["snippets"][0].as_object().unwrap();
    let mut keys: Vec<&str> = entry.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["encrypted_metadata", "id"]);
    assert_eq!(entry["id"], "s1");
    assert!(!entry["encrypted_metadata"].as_array().unwrap().is_empty());
}

#[test]
fn recent_ids_hold_only_used_snippets_most_recent_first() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut a = snippet("s1", "A", "a");
    a.last_used_at = Some(NOW - 10);
    repo.insert(&a).unwrap();
    let mut b = snippet("s2", "B", "b");
    b.last_used_at = Some(NOW - 5);
    repo.insert(&b).unwrap();
    repo.insert(&snippet("s3", "Never used", "c")).unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(snapshot.recent_ids, vec!["s2", "s1"]);
}

#[test]
fn favorite_ids_follow_the_starred_display_order() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut a = snippet("s1", "A", "a");
    a.is_favorite = true;
    a.updated_at = 2_000;
    repo.insert(&a).unwrap();
    let mut b = snippet("s2", "B", "b");
    b.is_favorite = true;
    b.updated_at = 3_000;
    repo.insert(&b).unwrap();
    repo.insert(&snippet("s3", "Not starred", "c")).unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(snapshot.favorite_ids, vec!["s2", "s1"]);
}

#[test]
fn folder_metadata_lists_every_folder() {
    let conn = fresh_db();
    let folders = FolderRepo::new(&conn);
    folders.insert(&folder("f1", "Shell", 1)).unwrap();
    folders.insert(&folder("f2", "Email", 0)).unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    let names: Vec<(&str, i32)> = snapshot
        .folder_metadata
        .iter()
        .map(|f| (f.name.as_str(), f.sort_order))
        .collect();
    assert_eq!(names, vec![("Email", 0), ("Shell", 1)]);
}

#[test]
fn identical_database_state_yields_identical_snapshots() {
    let conn = fresh_db();
    let repo = SnippetRepo::new(&conn);
    let mut a = snippet("s1", "A", "a");
    a.is_favorite = true;
    a.last_used_at = Some(NOW - 1);
    repo.insert(&a).unwrap();
    repo.insert(&snippet("s2", "B", "b")).unwrap();
    FolderRepo::new(&conn)
        .insert(&folder("f1", "Shell", 0))
        .unwrap();

    let first = generate(&conn, DEVICE, NOW).unwrap();
    let second = generate(&conn, DEVICE, NOW).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn an_empty_device_id_is_rejected_before_the_snapshot_leaves_core() {
    let conn = fresh_db();
    let error = generate(&conn, "", NOW).unwrap_err();
    assert!(error.to_string().contains("device_id"));
}

/// Snapshot red line (regression suite): a
/// sensitive snippet's title, trigger, and body — encrypted through the real
/// vault path — must be absent from the serialized snapshot at byte level,
/// while a normal snippet's body marker stays present (proving this test
/// would catch a leak). Markers are deliberately fake (testing.md).
#[test]
fn red_line_sensitive_plaintext_never_reaches_snapshot_bytes() {
    const TITLE_MARKER: &str = "TITLE_MARKER_FAKE_7431";
    const TRIGGER_MARKER: &str = ":TRIGGER_MARKER_FAKE_7431";
    const BODY_MARKER: &str = "AKIA_FAKE_BODY_MARKER_7431";
    const NORMAL_MARKER: &str = "NORMAL_BODY_CANARY_2960";

    let conn = fresh_db();
    let mut session = VaultSession::new();
    session.initialize(&conn, b"a vault password", NOW).unwrap();
    insert_secret(
        &conn,
        &session,
        "secret-1",
        TITLE_MARKER,
        TRIGGER_MARKER,
        BODY_MARKER,
    );
    SnippetRepo::new(&conn)
        .insert(&snippet("normal-1", "Normal", NORMAL_MARKER))
        .unwrap();

    let snapshot = generate(&conn, DEVICE, NOW).unwrap();
    let json = serde_json::to_string(&snapshot).unwrap();
    let bytes = json.as_bytes();

    for marker in [TITLE_MARKER, TRIGGER_MARKER, BODY_MARKER] {
        let absent = bytes
            .windows(marker.len())
            .all(|window| window != marker.as_bytes());
        assert!(absent, "sensitive marker {marker} leaked into snapshot");
    }
    // The canary proves the assertion above would catch a plaintext leak.
    assert!(json.contains(NORMAL_MARKER), "canary missing: {json}");

    // The sensitive entry is present as id + non-empty opaque envelope only.
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = value["snippets"].as_array().unwrap();
    let secret = entries
        .iter()
        .find(|entry| entry["id"] == "secret-1")
        .unwrap();
    let mut keys: Vec<&str> = secret
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["encrypted_metadata", "id"]);
    assert!(!secret["encrypted_metadata"].as_array().unwrap().is_empty());
}
