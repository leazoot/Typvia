// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Process-level end-to-end test: spawns the real host binary and speaks
//! the exact native-messaging frames a browser would, over a snapshot
//! produced by the same writer the desktop app uses. It covers everything
//! up to the browser boundary; the rest needs a real browser and is verified
//! by hand.

#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use typvia_host_service::dto::SnippetCreateInput;
use typvia_host_service::service;

const NOW: i64 = 1_700_000_000_000;
const SECRET_TITLE: &str = "E2E_REDLINE_TITLE";
const SECRET_BODY: &str = "E2E_REDLINE_BODY_FAKE_sk_51";

fn build_snapshot(dir: &Path) {
    let mut conn = typvia_core::db::open_in_memory().unwrap();
    typvia_core::db::migrate_to_latest(&mut conn).unwrap();
    let create = |title: &str, body: &str, trigger: Option<&str>| SnippetCreateInput {
        title: title.to_string(),
        body: body.to_string(),
        snippet_type: "text".to_string(),
        description: None,
        folder_id: None,
        trigger: trigger.map(str::to_string),
        trigger_mode: trigger.map(|_| "delimiter".to_string()),
        language: None,
    };
    service::snippet_create(
        &conn,
        create("Docker logs", "docker logs -f app", Some(";dlog")),
        NOW,
    )
    .unwrap();
    service::snippet_create(&conn, create("Greeting", "hello {{name}}!", None), NOW).unwrap();
    // A sensitive row for the byte-level red line on every response.
    let mut session = typvia_core::vault::VaultSession::new();
    session
        .initialize(&conn, b"correct horse battery staple", NOW)
        .unwrap();
    service::vault_create_secret(
        &conn,
        &session,
        SnippetCreateInput {
            title: SECRET_TITLE.to_string(),
            body: SECRET_BODY.to_string(),
            snippet_type: "sensitive".to_string(),
            description: None,
            folder_id: None,
            trigger: None,
            trigger_mode: None,
            language: None,
        },
        NOW,
    )
    .unwrap();
    typvia_host_service::snapshot::write_snapshot(&conn, dir, "device-e2e", NOW).unwrap();
}

fn spawn_host(dir: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_typvia-browser-host"))
        .env("TYPVIA_BROWSER_SNAPSHOT_DIR", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn send(child: &mut Child, payload: &[u8]) {
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(&u32::try_from(payload.len()).unwrap().to_le_bytes())
        .unwrap();
    stdin.write_all(payload).unwrap();
    stdin.flush().unwrap();
}

fn receive(child: &mut Child) -> serde_json::Value {
    let stdout = child.stdout.as_mut().unwrap();
    let mut len_bytes = [0u8; 4];
    stdout.read_exact(&mut len_bytes).unwrap();
    let mut payload = vec![0u8; u32::from_le_bytes(len_bytes) as usize];
    stdout.read_exact(&mut payload).unwrap();
    serde_json::from_slice(&payload).unwrap()
}

#[test]
fn full_message_surface_over_real_frames() {
    let dir = tempfile::tempdir().unwrap();
    build_snapshot(dir.path());
    let mut child = spawn_host(dir.path());
    let mut all_responses = Vec::new();
    let mut exchange = |payload: &[u8]| {
        send(&mut child, payload);
        let value = receive(&mut child);
        all_responses.push(value.to_string());
        value
    };

    let hello = exchange(br#"{"type":"hello"}"#);
    assert_eq!(hello["ok"], true);
    assert_eq!(hello["protocol_version"], 1);
    assert_eq!(hello["snapshot_present"], true);
    // Two normal snippets; the sensitive row never becomes a result.
    assert_eq!(hello["snippet_count"], 2);

    let search = exchange(br#"{"type":"search","query":"docker"}"#);
    assert_eq!(search["results"][0]["title"], "Docker logs");
    assert_eq!(search["results"][0]["trigger"], ";dlog");

    let template = exchange(br#"{"type":"search","query":"greeting"}"#);
    assert_eq!(template["results"][0]["variables"][0], "name");
    let id = template["results"][0]["id"].as_str().unwrap().to_string();

    let render = exchange(
        format!(r#"{{"type":"render","snippet_id":"{id}","variables":{{"name":"Ada"}}}}"#)
            .as_bytes(),
    );
    assert_eq!(render["ok"], true);
    assert_eq!(render["text"], "hello Ada!");

    let missing = exchange(format!(r#"{{"type":"render","snippet_id":"{id}"}}"#).as_bytes());
    assert_eq!(missing["code"], "missing_variable");
    assert_eq!(missing["field"], "name");

    let recent = exchange(br#"{"type":"list_recent"}"#);
    assert_eq!(recent["ok"], true);
    assert_eq!(recent["results"].as_array().unwrap().len(), 2);

    let unknown = exchange(br#"{"type":"drop_table"}"#);
    assert_eq!(unknown["code"], "unknown_message");

    let invalid = exchange(b"not json at all");
    assert_eq!(invalid["code"], "unknown_message");

    // Red line: no response byte sequence may contain the sensitive title
    // or body (they exist in the snapshot only as ciphertext).
    let joined = all_responses.join("\n");
    assert!(!joined.contains(SECRET_TITLE));
    assert!(!joined.contains(SECRET_BODY));

    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn an_oversized_frame_gets_an_error_and_the_stream_survives() {
    let dir = tempfile::tempdir().unwrap();
    build_snapshot(dir.path());
    let mut child = spawn_host(dir.path());

    // Declare 1MB + 1 and actually send it, then a valid hello.
    let huge_len: u32 = 1024 * 1024 + 1;
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&huge_len.to_le_bytes()).unwrap();
        stdin.write_all(&vec![b'x'; huge_len as usize]).unwrap();
        stdin.flush().unwrap();
    }
    let error = receive(&mut child);
    assert_eq!(error["code"], "oversized_frame");

    send(&mut child, br#"{"type":"hello"}"#);
    assert_eq!(receive(&mut child)["ok"], true);

    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}

#[test]
fn a_missing_snapshot_answers_with_a_stable_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = spawn_host(dir.path());

    send(&mut child, br#"{"type":"search","query":"x"}"#);
    assert_eq!(receive(&mut child)["code"], "snapshot_unavailable");
    send(&mut child, br#"{"type":"hello"}"#);
    let hello = receive(&mut child);
    assert_eq!(hello["ok"], true);
    assert_eq!(hello["snapshot_present"], false);

    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}
