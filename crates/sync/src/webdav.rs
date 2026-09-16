// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Minimal WebDAV client for the dumb-storage sync form.
//!
//! Method surface is deliberately tiny — GET, conditional PUT, DELETE and
//! MKCOL; the protocol needs no PROPFIND, because single-writer head
//! counters replace directory listing. Only `https://` endpoints are
//! accepted, plus plain http toward loopback for tests — certificate
//! verification is never disabled and no insecure-TLS surface exists.
//! Credentials travel exclusively in the Authorization header; nothing in
//! this module logs, and no request or response content appears in errors.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use ureq::Agent;

use crate::transport::TransportError;

/// Per-file read cap: a single sealed record stays far below this;
/// anything larger is not a conforming file.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Credentials for the user's WebDAV endpoint. Stored only in the platform
/// secure store (entry `sync.webdav.credentials`) as
/// `basic:<user>:<password>` or `bearer:<token>`; parsed back here. Values
/// must be printable ASCII — the same header-injection guard API keys
/// use.
pub enum WebdavCredentials {
    Basic { username: String, password: String },
    Bearer(String),
}

impl WebdavCredentials {
    /// Builds the secure-store representation of a username and password.
    ///
    /// The counterpart of [`Self::parse`], and here for the same reason the
    /// parser is: the string's shape belongs to this module. A host that spells
    /// it out itself is a host that will still be spelling the old one out
    /// after this changes — and the failure that produces is an account that
    /// refuses a password the user typed correctly.
    ///
    /// - Returns: `None` when neither field was filled, which is a folder that
    ///   needs no credentials — not a folder handed empty ones.
    pub fn basic(username: &str, password: &str) -> Option<String> {
        if username.is_empty() && password.is_empty() {
            return None;
        }
        Some(format!("basic:{username}:{password}"))
    }

    /// Parses the secure-store representation.
    pub fn parse(stored: &str) -> Result<Self, TransportError> {
        let parsed = if let Some(rest) = stored.strip_prefix("basic:") {
            let (username, password) = rest
                .split_once(':')
                .ok_or(TransportError::MalformedResponse)?;
            Self::Basic {
                username: username.to_string(),
                password: password.to_string(),
            }
        } else if let Some(token) = stored.strip_prefix("bearer:") {
            Self::Bearer(token.to_string())
        } else {
            return Err(TransportError::MalformedResponse);
        };
        if !parsed.is_printable_ascii() {
            return Err(TransportError::MalformedResponse);
        }
        Ok(parsed)
    }

    fn is_printable_ascii(&self) -> bool {
        let printable = |s: &str| s.bytes().all(|b| (0x20..0x7f).contains(&b));
        match self {
            Self::Basic { username, password } => printable(username) && printable(password),
            Self::Bearer(token) => printable(token),
        }
    }

    fn header_value(&self) -> String {
        match self {
            Self::Basic { username, password } => {
                let raw = format!("{username}:{password}");
                format!("Basic {}", BASE64.encode(raw.as_bytes()))
            }
            Self::Bearer(token) => format!("Bearer {token}"),
        }
    }
}

/// Outcome of a conditional PUT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutOutcome {
    /// The write landed.
    Done,
    /// The precondition failed: the file already exists (`If-None-Match: *`)
    /// or was modified since the given ETag (`If-Match`).
    PreconditionFailed,
}

/// A fetched file: raw bytes plus the ETag the server reported (if any).
/// Debug prints lengths only — file bytes are ciphertext but the no-content
/// discipline holds everywhere.
#[derive(Clone)]
pub struct FetchedFile {
    pub bytes: Vec<u8>,
    pub etag: Option<String>,
}

impl std::fmt::Debug for FetchedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchedFile")
            .field("len", &self.bytes.len())
            .field("etag", &self.etag)
            .finish()
    }
}

/// Blocking WebDAV client over a validated base URL. Paths given to the
/// methods are relative to the base and must already be percent-safe (the
/// protocol's file names are `[a-z0-9./-]` only).
pub struct WebdavClient {
    agent: Agent,
    base_url: String,
    auth_header: Option<String>,
}

impl WebdavClient {
    pub fn new(
        base_url: &str,
        credentials: Option<&WebdavCredentials>,
    ) -> Result<Self, TransportError> {
        let base_url = base_url.trim_end_matches('/').to_string();
        if !crate::http::is_acceptable_url(&base_url) {
            return Err(TransportError::InvalidServerUrl);
        }
        let config = Agent::config_builder()
            .http_status_as_error(false)
            // MKCOL is a WebDAV method (RFC 4918); ureq gates non-RFC-9110
            // verbs behind this switch.
            .allow_non_standard_methods(true)
            .build();
        Ok(Self {
            agent: config.new_agent(),
            base_url,
            auth_header: credentials.map(WebdavCredentials::header_value),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn run(
        &self,
        method: &str,
        path: &str,
        preconditions: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<(u16, Vec<u8>, Option<String>), TransportError> {
        let mut builder = ureq::http::Request::builder()
            .method(method)
            .uri(self.url(path));
        if let Some(auth) = &self.auth_header {
            builder = builder.header("Authorization", auth);
        }
        for (name, value) in preconditions {
            builder = builder.header(*name, *value);
        }
        let request = builder
            .body(body.unwrap_or(&[]).to_vec())
            .map_err(|_| TransportError::InvalidServerUrl)?;
        let mut response = self
            .agent
            .run(request)
            .map_err(|e| TransportError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_FILE_BYTES)
            .read_to_vec()
            .map_err(|e| TransportError::Network(e.to_string()))?;
        Ok((status, bytes, etag))
    }

    /// Fetches a file; `Ok(None)` when it does not exist.
    pub fn get(&self, path: &str) -> Result<Option<FetchedFile>, TransportError> {
        let (status, bytes, etag) = self.run("GET", path, &[], None)?;
        match status {
            200 => Ok(Some(FetchedFile { bytes, etag })),
            404 => Ok(None),
            _ => Err(status_error(status)),
        }
    }

    /// Creates a file that must not exist yet (`If-None-Match: *`) — the
    /// exclusive-create primitive every "claim by creation" write uses.
    pub fn put_exclusive(&self, path: &str, body: &[u8]) -> Result<PutOutcome, TransportError> {
        let (status, _, _) = self.run("PUT", path, &[("If-None-Match", "*")], Some(body))?;
        put_outcome(status)
    }

    /// Replaces a file only if it still carries `etag` (`If-Match`) — the
    /// optimistic-concurrency primitive for shared files like the device
    /// directory.
    pub fn put_if_match(
        &self,
        path: &str,
        etag: &str,
        body: &[u8],
    ) -> Result<PutOutcome, TransportError> {
        let (status, _, _) = self.run("PUT", path, &[("If-Match", etag)], Some(body))?;
        put_outcome(status)
    }

    /// Unconditional overwrite — only for single-writer files this device
    /// owns (its own `head.json`).
    pub fn put_overwrite(&self, path: &str, body: &[u8]) -> Result<(), TransportError> {
        let (status, _, _) = self.run("PUT", path, &[], Some(body))?;
        match status {
            200..=299 => Ok(()),
            _ => Err(status_error(status)),
        }
    }

    /// Deletes a file; a missing file is not an error (mailbox cleanup is
    /// idempotent).
    pub fn delete(&self, path: &str) -> Result<(), TransportError> {
        let (status, _, _) = self.run("DELETE", path, &[], None)?;
        match status {
            200..=299 | 404 => Ok(()),
            _ => Err(status_error(status)),
        }
    }

    /// Creates one collection; an already-existing collection is fine.
    pub fn mkcol(&self, path: &str) -> Result<(), TransportError> {
        let (status, _, _) = self.run("MKCOL", path, &[], None)?;
        match status {
            200..=299 | 405 | 301 => Ok(()),
            _ => Err(status_error(status)),
        }
    }

    /// Creates every collection along `path` (parents first — WebDAV
    /// requires existing parents).
    pub fn mkcol_all(&self, path: &str) -> Result<(), TransportError> {
        let mut prefix = String::new();
        for segment in path.trim_matches('/').split('/') {
            if segment.is_empty() {
                continue;
            }
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            self.mkcol(&prefix)?;
        }
        Ok(())
    }

    /// One-time onboarding probe: the endpoint must honour
    /// `If-None-Match: *` — a second exclusive create of the same file has
    /// to fail. Endpoints that strip preconditions are refused outright
    /// (honest degradation: no best-effort mode exists). The probe file is
    /// deleted either way.
    pub fn probe_preconditions(&self, nonce: &str) -> Result<(), TransportError> {
        let path = format!("typvia-sync/.probe-{nonce}");
        self.mkcol_all("typvia-sync")?;
        let first = self.put_exclusive(&path, b"probe")?;
        let second = self.put_exclusive(&path, b"probe");
        let cleanup = self.delete(&path);
        if first != PutOutcome::Done {
            // Leftover from an aborted probe: the delete above cleared it;
            // ask the caller to retry with a fresh nonce.
            return Err(TransportError::Network(
                "probe file already present".to_string(),
            ));
        }
        match second? {
            PutOutcome::PreconditionFailed => cleanup,
            PutOutcome::Done => Err(TransportError::Api {
                code: "PRECONDITIONS_UNSUPPORTED".to_string(),
                status: 0,
            }),
        }
    }
}

fn put_outcome(status: u16) -> Result<PutOutcome, TransportError> {
    match status {
        200..=299 => Ok(PutOutcome::Done),
        412 | 409 => Ok(PutOutcome::PreconditionFailed),
        _ => Err(status_error(status)),
    }
}

fn status_error(status: u16) -> TransportError {
    match status {
        401 | 403 => TransportError::Api {
            code: "WEBDAV_AUTH".to_string(),
            status,
        },
        429 => TransportError::RateLimited,
        _ => TransportError::Api {
            code: "WEBDAV_STATUS".to_string(),
            status,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_parse_both_forms_and_reject_the_rest() {
        let basic = WebdavCredentials::parse("basic:alice:FAKE_PW_NOT_A_SECRET").unwrap();
        assert_eq!(
            basic.header_value(),
            format!("Basic {}", BASE64.encode("alice:FAKE_PW_NOT_A_SECRET"))
        );
        let bearer = WebdavCredentials::parse("bearer:FAKE_TOKEN_NOT_A_SECRET").unwrap();
        assert_eq!(bearer.header_value(), "Bearer FAKE_TOKEN_NOT_A_SECRET");
        // No scheme prefix: not a credential this parser accepts.
        assert!(WebdavCredentials::parse("alice:FAKE_PW_NOT_A_SECRET").is_err());
        // Non-printable bytes would smuggle header injection: refused.
        assert!(WebdavCredentials::parse("bearer:bad\ntoken").is_err());
    }

    #[test]
    fn only_https_or_loopback_http_is_accepted() {
        assert!(WebdavClient::new("https://dav.example/base", None).is_ok());
        assert!(WebdavClient::new("http://127.0.0.1:8080/base", None).is_ok());
        assert!(WebdavClient::new("http://dav.example/base", None).is_err());
        assert!(WebdavClient::new("ftp://dav.example", None).is_err());
    }
}
