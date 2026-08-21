// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! HTTP implementation of [`AiTransport`] over ureq.
//!
//! Certificate verification is never disabled — this module exposes no
//! insecure-TLS surface at all; plain http exists
//! only toward loopback, where local model daemons listen. The API key
//! goes into the `Authorization` header only, and no request or response
//! content is ever logged (this module logs nothing).

use std::io::Read;

use ureq::Agent;

use crate::config::is_acceptable_url;
use crate::transport::{
    AiTransport, CancelToken, HttpMethod, TransportFailure, TransportRequest, TransportResponse,
};

/// Response body cap; a completion answer is orders of magnitude smaller.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

const READ_CHUNK: usize = 8 * 1024;

/// Blocking transport. One agent is built per call so the configured
/// timeout bounds the whole exchange, including reading the body.
#[derive(Debug, Default)]
pub struct HttpAiTransport;

impl HttpAiTransport {
    pub fn new() -> Self {
        Self
    }
}

impl AiTransport for HttpAiTransport {
    fn execute(
        &self,
        request: &TransportRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<TransportResponse, TransportFailure> {
        if !is_acceptable_url(&request.url) {
            return Err(TransportFailure::InsecureUrl);
        }
        if cancel.is_cancelled() {
            return Err(TransportFailure::Cancelled);
        }
        let agent: Agent = Agent::config_builder()
            // Non-2xx statuses carry provider error semantics; the wire
            // client interprets them, so they are not transport errors.
            .http_status_as_error(false)
            .timeout_global(Some(request.timeout))
            .build()
            .new_agent();

        let result = match request.method {
            HttpMethod::Get => {
                let mut builder = agent.get(&request.url);
                if let Some(key) = request.api_key {
                    builder = builder.header("Authorization", format!("Bearer {}", key.expose()));
                }
                builder.call()
            }
            HttpMethod::Post => {
                let mut builder = agent
                    .post(&request.url)
                    .header("Content-Type", "application/json");
                if let Some(key) = request.api_key {
                    builder = builder.header("Authorization", format!("Bearer {}", key.expose()));
                }
                builder.send(request.body.unwrap_or_default())
            }
        };
        let mut response = result.map_err(map_ureq_error)?;
        let status = response.status().as_u16();

        // The body is read in chunks so cancellation takes effect between
        // chunks while a long completion streams in; the agent timeout
        // still bounds the whole exchange.
        let mut reader = response.body_mut().as_reader();
        let mut body = Vec::new();
        let mut buf = [0u8; READ_CHUNK];
        loop {
            if cancel.is_cancelled() {
                return Err(TransportFailure::Cancelled);
            }
            let n = reader.read(&mut buf).map_err(map_read_error)?;
            if n == 0 {
                break;
            }
            if body.len() + n > MAX_RESPONSE_BYTES {
                return Err(TransportFailure::ResponseTooLarge);
            }
            body.extend_from_slice(&buf[..n]);
        }
        Ok(TransportResponse { status, body })
    }
}

fn map_ureq_error(error: ureq::Error) -> TransportFailure {
    // Renderings of these errors are never surfaced; the variants carry no
    // request or response content anyway.
    match error {
        ureq::Error::Timeout(_) => TransportFailure::Timeout,
        _ => TransportFailure::Unreachable,
    }
}

fn map_read_error(error: std::io::Error) -> TransportFailure {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => TransportFailure::Timeout,
        _ => TransportFailure::Interrupted,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::{Read as _, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::credentials::ApiKey;

    /// One-shot loopback HTTP server; sends `body` split into `chunks`
    /// pieces with `pause` between them, and reports the raw request bytes
    /// it saw.
    fn serve_once(
        body: &'static str,
        chunks: usize,
        pause: Duration,
    ) -> (String, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            // Read the complete request (headers plus declared body):
            // responding and closing while body bytes are still in flight
            // would RST the connection and destroy the client's unread
            // response data.
            loop {
                if let Some(headers_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&request[..headers_end]);
                    let content_length: usize = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if request.len() >= headers_end + 4 + content_length {
                        break;
                    }
                }
                match socket.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => request.extend_from_slice(&buf[..n]),
                    Err(_) => break,
                }
            }
            let _ = tx.send(request);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(header.as_bytes()).unwrap();
            let piece = body.len().div_ceil(chunks);
            for chunk in body.as_bytes().chunks(piece.max(1)) {
                socket.write_all(chunk).unwrap();
                socket.flush().unwrap();
                thread::sleep(pause);
            }
        });
        (url, rx)
    }

    fn request<'a>(
        url: &str,
        method: HttpMethod,
        api_key: Option<&'a ApiKey>,
        body: Option<&'a [u8]>,
        timeout_ms: u64,
    ) -> TransportRequest<'a> {
        TransportRequest {
            method,
            url: url.to_string(),
            api_key,
            body,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    #[test]
    fn round_trips_a_post_and_puts_the_key_only_in_the_authorization_header() {
        let (url, seen) = serve_once(r#"{"ok":true}"#, 1, Duration::ZERO);
        let key = ApiKey::new("sk-FAKE-wire-canary").unwrap();
        let body = br#"{"model":"m"}"#;
        let response = HttpAiTransport::new()
            .execute(
                &request(&url, HttpMethod::Post, Some(&key), Some(body), 2_000),
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, br#"{"ok":true}"#);

        // Byte-level red line at the wire boundary: the key appears
        // exactly once, in the Authorization header line, never in the
        // URL or body.
        let raw = seen.recv().unwrap();
        let hits = raw
            .windows(b"sk-FAKE-wire-canary".len())
            .filter(|w| *w == b"sk-FAKE-wire-canary")
            .count();
        assert_eq!(hits, 1);
        let text = String::from_utf8_lossy(&raw);
        let auth_line = text
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
            .unwrap();
        assert!(auth_line.contains("Bearer sk-FAKE-wire-canary"));
        let (head, body_part) = text.split_once("\r\n\r\n").unwrap();
        assert!(!body_part.contains("sk-FAKE-wire-canary"));
        assert!(!head.lines().next().unwrap().contains("sk-FAKE-wire-canary"));
    }

    #[test]
    fn a_request_without_a_key_sends_no_authorization_header() {
        let (url, seen) = serve_once("{}", 1, Duration::ZERO);
        HttpAiTransport::new()
            .execute(
                &request(&url, HttpMethod::Get, None, None, 2_000),
                &CancelToken::new(),
            )
            .unwrap();
        let raw = String::from_utf8_lossy(&seen.recv().unwrap()).to_ascii_lowercase();
        assert!(!raw.contains("authorization:"));
    }

    #[test]
    fn an_unanswered_request_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let hold = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(1_500));
            drop(socket);
        });
        let outcome = HttpAiTransport::new().execute(
            &request(&url, HttpMethod::Get, None, None, 200),
            &CancelToken::new(),
        );
        assert_eq!(outcome.unwrap_err(), TransportFailure::Timeout);
        hold.join().unwrap();
    }

    #[test]
    fn cancellation_interrupts_a_streaming_body() {
        // 2000 bytes dripped in 20 chunks, 100ms apart: the full body
        // would take ~2s; cancelling after ~150ms must cut the read loop
        // well before that.
        let (url, _seen) = serve_once(
            // 2000-byte payload.
            Box::leak(("x".repeat(2000)).into_boxed_str()),
            20,
            Duration::from_millis(100),
        );
        let cancel = CancelToken::new();
        let canceller = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            canceller.cancel();
        });
        let started = std::time::Instant::now();
        let outcome = HttpAiTransport::new()
            .execute(&request(&url, HttpMethod::Get, None, None, 10_000), &cancel);
        assert_eq!(outcome.unwrap_err(), TransportFailure::Cancelled);
        assert!(started.elapsed() < Duration::from_millis(1_500));
    }

    #[test]
    fn an_already_cancelled_token_prevents_any_connection() {
        let cancel = CancelToken::new();
        cancel.cancel();
        // Nothing listens on this port; a connection attempt would fail
        // differently, proving the pre-send checkpoint fired first.
        let outcome = HttpAiTransport::new().execute(
            &request("http://127.0.0.1:9", HttpMethod::Get, None, None, 500),
            &cancel,
        );
        assert_eq!(outcome.unwrap_err(), TransportFailure::Cancelled);
    }

    #[test]
    fn plaintext_urls_off_loopback_are_refused_without_connecting() {
        let outcome = HttpAiTransport::new().execute(
            &request(
                "http://192.0.2.1:11434/v1/models",
                HttpMethod::Get,
                None,
                None,
                500,
            ),
            &CancelToken::new(),
        );
        assert_eq!(outcome.unwrap_err(), TransportFailure::InsecureUrl);
    }

    #[test]
    fn an_unreachable_host_reports_unreachable() {
        // TEST-NET-1 with a tiny timeout: connect fails or times out; both
        // are acceptable, but nothing may panic or hang.
        let outcome = HttpAiTransport::new().execute(
            &request("http://127.0.0.1:9", HttpMethod::Get, None, None, 300),
            &CancelToken::new(),
        );
        assert!(matches!(
            outcome.unwrap_err(),
            TransportFailure::Unreachable | TransportFailure::Timeout
        ));
    }
}
