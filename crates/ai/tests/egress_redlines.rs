// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Egress red lines:
//!
//! ① Sensitive snippets and fake secrets are zero-hit at the request byte
//!    level, with an injected benign probe first proving the scan reads
//!    real wire bytes.
//! ② No switch, configuration field or feature gate can disable the
//!    egress log — asserted structurally against this crate's sources
//!    (the fail-closed behavior itself is pinned in unit tests).
//! ③ The egress log carries no content or key material, pinned at the
//!    database-file byte level against a real migrated database.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use typvia_ai::{
    AiError, AiProvider, AiRequestClass, AiTransport, ApiKey, CancelToken, ChatMessage,
    ChatRequest, EgressEntry, EgressLog, EgressLogError, HttpAiTransport, OpenAiCompatProvider,
    ProviderConfig, ProviderKind, Role, TransportFailure, TransportRequest, TransportResponse,
    vetted_snippet_text,
};
use typvia_core::db::{migrate_to_latest, open};
use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType};
use typvia_core::repo::AiEgressLogRepo;

/// In-memory sink for tests that only need the gate to pass.
#[derive(Default)]
struct MemoryLog {
    entries: Mutex<Vec<EgressEntry>>,
}

impl EgressLog for MemoryLog {
    fn record(&self, entry: &EgressEntry) -> Result<(), EgressLogError> {
        self.entries
            .lock()
            .map_err(|_| EgressLogError)?
            .push(entry.clone());
        Ok(())
    }
}

fn chat(content: &str) -> ChatRequest {
    ChatRequest {
        messages: vec![ChatMessage {
            role: Role::User,
            content: content.to_string(),
        }],
        temperature: None,
        max_tokens: None,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

/// Loopback capture server: appends every request's raw bytes to a shared
/// buffer and answers each with a fixed completion, until it reads a
/// request starting with `SHUTDOWN`. Reads each request completely
/// (headers + Content-Length body) before responding, so captures are
/// byte-complete.
#[allow(clippy::unwrap_used)]
fn capture_server() -> (String, Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>) {
    const ANSWER: &str = r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            let body_start = loop {
                let n = stream.read(&mut chunk).unwrap();
                if n == 0 {
                    break request.len();
                }
                request.extend_from_slice(&chunk[..n]);
                if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            if request.starts_with(b"SHUTDOWN") {
                return;
            }
            let headers = String::from_utf8_lossy(&request[..body_start]).to_lowercase();
            let content_length = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while request.len() < body_start + content_length {
                let n = stream.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..n]);
            }
            sink.lock().unwrap().extend_from_slice(&request);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{ANSWER}",
                ANSWER.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (base_url, captured, handle)
}

#[test]
#[allow(clippy::unwrap_used)]
fn sensitive_content_and_fake_secrets_are_zero_hit_at_the_wire_byte_level() {
    const BENIGN_PROBE: &str = "T106-BENIGN-WIRE-PROBE";
    const FAKE_SECRET: &str = "AKIAFAKEFAKEFAKEFAKE";
    const SENSITIVE_MARKER: &str = "T106-SENSITIVE-BODY-MARKER";

    let (base_url, captured, handle) = capture_server();
    let config = ProviderConfig::new(
        "p-wire",
        ProviderKind::CustomBaseUrl,
        Some(&base_url),
        "test-model",
    )
    .unwrap();
    let shutdown_addr = base_url
        .trim_start_matches("http://")
        .trim_end_matches("/v1")
        .to_string();
    let provider = OpenAiCompatProvider::new(
        config,
        None,
        HttpAiTransport::new(),
        Arc::new(MemoryLog::default()),
    )
    .unwrap();

    // Probe self-proof: a benign request's content IS captured on the wire,
    // so the zero-hit assertions below scan bytes that really flow.
    provider
        .complete(&chat(BENIGN_PROBE), &CancelToken::new())
        .unwrap();
    assert!(find(&captured.lock().unwrap(), BENIGN_PROBE.as_bytes()));

    // A fake secret in the content is refused before any I/O.
    let refused = provider
        .complete(
            &chat(&format!("summarize {FAKE_SECRET} for me")),
            &CancelToken::new(),
        )
        .unwrap_err();
    assert!(matches!(refused, AiError::SuspectedSecretBlocked { .. }));

    // A sensitive snippet cannot even yield text for a request: its
    // in-memory form is ciphertext-only and the vetting entry refuses it.
    let sensitive = Snippet {
        id: "s1".to_string(),
        workspace_id: "w1".to_string(),
        title: "Signing key".to_string(),
        content: SnippetContent::Ciphertext(SENSITIVE_MARKER.as_bytes().to_vec()),
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
        created_at: 0,
        updated_at: 0,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    };
    assert_eq!(
        vetted_snippet_text(&sensitive).unwrap_err(),
        AiError::SensitiveSnippetBlocked
    );

    // Stop the server and scan everything that ever crossed the wire.
    std::net::TcpStream::connect(&shutdown_addr)
        .unwrap()
        .write_all(b"SHUTDOWN\r\n\r\n")
        .unwrap();
    handle.join().unwrap();
    let bytes = captured.lock().unwrap();
    // Exactly one request went out — the refusals never opened a connection.
    assert_eq!(count(&bytes, b"POST "), 1);
    // Red line: the fake secret and the sensitive body are nowhere in the
    // captured wire bytes.
    assert!(!find(&bytes, FAKE_SECRET.as_bytes()));
    assert!(!find(&bytes, SENSITIVE_MARKER.as_bytes()));
}

/// Transport double answering every request with a fixed completion whose
/// content is a distinctive canary (for the database byte scan).
struct CannedTransport;

const RESPONSE_CANARY: &str = "T106-RESPONSE-CANARY-55";

impl AiTransport for CannedTransport {
    fn execute(
        &self,
        _request: &TransportRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<TransportResponse, TransportFailure> {
        let body = format!(
            r#"{{"choices":[{{"message":{{"role":"assistant","content":"{RESPONSE_CANARY}"}}}}]}}"#
        );
        Ok(TransportResponse {
            status: 200,
            body: body.into_bytes(),
        })
    }
}

/// The host-shaped sink: entries land in the real `ai_egress_log` table
/// through the core repository, with the timestamp stamped at write time.
struct DbEgressLog {
    conn: Mutex<rusqlite::Connection>,
    now: i64,
}

impl EgressLog for DbEgressLog {
    fn record(&self, entry: &EgressEntry) -> Result<(), EgressLogError> {
        let conn = self.conn.lock().map_err(|_| EgressLogError)?;
        let bytes = i64::try_from(entry.request_bytes).map_err(|_| EgressLogError)?;
        AiEgressLogRepo::new(&conn)
            .append(self.now, &entry.provider_id, entry.request_class, bytes)
            .map_err(|_| EgressLogError)
    }
}

#[test]
#[allow(clippy::unwrap_used)]
fn the_egress_log_database_carries_no_content_or_key_bytes() {
    const PROMPT_CANARY: &str = "T106-PROMPT-CANARY-88";
    const KEY_CANARY: &str = "sk-FAKE-t106-db-key";

    let dir = std::env::temp_dir().join(format!("typvia-egress-redline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("library.db");
    let _ = std::fs::remove_file(&db_path);

    let mut conn = open(&db_path).unwrap();
    migrate_to_latest(&mut conn).unwrap();
    let sink = Arc::new(DbEgressLog {
        conn: Mutex::new(conn),
        now: 1_700_000_000_000,
    });

    let config = ProviderConfig::new("p-log", ProviderKind::Ollama, None, "llama3").unwrap();
    let key = ApiKey::new(KEY_CANARY).unwrap();
    let provider =
        OpenAiCompatProvider::new(config, Some(key), CannedTransport, sink.clone()).unwrap();

    let outcome = provider
        .complete(&chat(PROMPT_CANARY), &CancelToken::new())
        .unwrap();
    assert_eq!(outcome.content, RESPONSE_CANARY);
    provider.check_connectivity(&CancelToken::new()).unwrap();
    drop(provider);

    let db = Arc::try_unwrap(sink)
        .map_err(|_| "sink still shared")
        .unwrap();
    let conn = db.conn.into_inner().unwrap();
    {
        // The log holds exactly the request metadata, newest first.
        let repo = AiEgressLogRepo::new(&conn);
        let records = repo.list(10, 0).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].request_class, AiRequestClass::Connectivity);
        assert_eq!(records[0].request_bytes, 0);
        assert_eq!(records[1].request_class, AiRequestClass::Completion);
        assert_eq!(records[1].provider_id, "p-log");
        assert!(records[1].request_bytes > PROMPT_CANARY.len() as i64);
    }
    // Fold WAL content back into the main file before scanning.
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(conn);

    let bytes = std::fs::read(&db_path).unwrap();
    // Scanner self-proof: metadata that DID go to the database is found.
    assert!(find(&bytes, b"p-log"));
    assert!(find(&bytes, b"completion"));
    // Red line: neither prompt content, nor the provider's answer, nor the
    // API key exists anywhere in the database file.
    assert!(!find(&bytes, PROMPT_CANARY.as_bytes()));
    assert!(!find(&bytes, RESPONSE_CANARY.as_bytes()));
    assert!(!find(&bytes, KEY_CANARY.as_bytes()));

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
#[allow(clippy::unwrap_used)]
fn no_switch_or_feature_gate_can_disable_the_egress_log() {
    // Code-face assertions. The type system already
    // guarantees a provider cannot exist without a sink (the constructor's
    // only signature demands one) and unit tests pin the fail-closed
    // behavior; these structural checks keep future edits from introducing
    // an escape hatch.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let mut sources = String::new();
    for entry in std::fs::read_dir(format!("{manifest_dir}/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push_str(&std::fs::read_to_string(&path).unwrap());
        }
    }
    // No conditional compilation exists in this crate's sources, so the
    // gate cannot be compiled out.
    assert!(!sources.contains("#[cfg(feature"));

    let provider_src = std::fs::read_to_string(format!("{manifest_dir}/src/provider.rs")).unwrap();
    // The sink is a mandatory field — never optional.
    assert!(provider_src.contains("egress_log: Arc<dyn EgressLog>"));
    assert!(!provider_src.contains("Option<Arc<dyn EgressLog>"));

    // The crate declares no feature flags at all.
    let cargo_toml = std::fs::read_to_string(format!("{manifest_dir}/Cargo.toml")).unwrap();
    assert!(!cargo_toml.contains("[features]"));
}
