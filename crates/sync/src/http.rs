// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! HTTP implementation of [`SyncTransport`] against the Go sync server.
//!
//! Certificate verification is never disabled — this module exposes no
//! insecure-TLS surface at all; the only plain-http
//! carve-out is loopback for tests. Byte fields travel as standard base64
//! (the server's Go `[]byte` JSON convention). Session tokens go into the
//! Authorization header only, never URLs, and no request or response
//! content is ever logged (this module logs nothing).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::cert::{DeviceCertificate, RootStatement};
use crate::directory::{certificate_to_json, root_statement_to_json};
use crate::record::WireRecord;
use crate::transport::{
    ConflictHead, DeviceDirectory, DirectoryDevice, DirectoryRevocation, HandshakeInfo, KeyUpdate,
    PairClaim, PairingSession, PullPage, PulledRecord, PushOutcome, PushResultItem, ReRootOutcome,
    SessionToken, SyncTransport, TransportError,
};

/// Protocol version this client speaks.
pub const PROTOCOL_VERSION: u32 = 1;

const PROTOCOL_HEADER: &str = "X-Typvia-Protocol";
/// Response body cap: a full pull page of maximum-size envelopes in base64
/// stays far below this; anything larger is not a conforming response.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// Blocking HTTP transport over a validated base URL.
pub struct HttpTransport {
    agent: Agent,
    base_url: String,
}

impl HttpTransport {
    /// Builds a transport for `server_url`. Only `https://` is accepted,
    /// plus `http://` toward loopback for tests; everything else —
    /// including any way of relaxing certificate verification — does not
    /// exist in this API.
    pub fn new(server_url: &str) -> Result<Self, TransportError> {
        let base_url = server_url.trim_end_matches('/').to_string();
        if !is_acceptable_url(&base_url) {
            return Err(TransportError::InvalidServerUrl);
        }
        let config = Agent::config_builder()
            // Non-2xx responses carry the stable error envelope; they are
            // parsed, not turned into opaque transport errors.
            .http_status_as_error(false)
            .build();
        Ok(Self {
            agent: config.new_agent(),
            base_url,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        token: Option<&SessionToken>,
    ) -> Result<T, TransportError> {
        let mut request = self
            .agent
            .get(self.url(path))
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string());
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {}", token.0));
        }
        let response = request.call().map_err(map_ureq_error)?;
        read_json_response(response)
    }

    fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        token: Option<&SessionToken>,
        body: &impl Serialize,
    ) -> Result<T, TransportError> {
        self.send_json(path, token, body, false)
    }

    fn put_json<T: DeserializeOwned>(
        &self,
        path: &str,
        token: Option<&SessionToken>,
        body: &impl Serialize,
    ) -> Result<T, TransportError> {
        self.send_json(path, token, body, true)
    }

    fn send_json<T: DeserializeOwned>(
        &self,
        path: &str,
        token: Option<&SessionToken>,
        body: &impl Serialize,
        put: bool,
    ) -> Result<T, TransportError> {
        let payload = serde_json::to_string(body).map_err(|_| TransportError::MalformedResponse)?;
        let url = self.url(path);
        let mut request = if put {
            self.agent.put(url)
        } else {
            self.agent.post(url)
        };
        request = request
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .header("Content-Type", "application/json");
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {}", token.0));
        }
        let response = request.send(payload.as_bytes()).map_err(map_ureq_error)?;
        read_json_response(response)
    }
}

/// Shared URL policy for every network module in this crate: https only,
/// plus plain http toward loopback for tests.
pub(crate) fn is_acceptable_url(url: &str) -> bool {
    if let Some(rest) = url.strip_prefix("https://") {
        return !rest.is_empty();
    }
    if let Some(rest) = url.strip_prefix("http://") {
        // Bracketed IPv6 hosts contain ':' inside the brackets.
        let host = if rest.starts_with('[') {
            rest.split(']').next().map(|h| format!("{h}]"))
        } else {
            rest.split(['/', ':']).next().map(str::to_string)
        };
        return matches!(host.as_deref(), Some("localhost" | "127.0.0.1" | "[::1]"));
    }
    false
}

fn map_ureq_error(error: ureq::Error) -> TransportError {
    // With status-as-error disabled, remaining errors are transport-level
    // (connect, TLS, I/O, protocol). Their rendering carries no request or
    // response content.
    TransportError::Network(error.to_string())
}

/// The stable error envelope.
#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
    #[serde(default)]
    conflicts: Vec<ConflictHeadBody>,
}

#[derive(Deserialize)]
struct ConflictHeadBody {
    entity_type: String,
    entity_id: String,
    head_version: i64,
}

/// Classified non-2xx outcome: either a push version conflict (a business
/// outcome the engine handles) or a transport error.
fn read_json_response<T: DeserializeOwned>(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<T, TransportError> {
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(map_ureq_error)?;
    if (200..300).contains(&status) {
        return serde_json::from_slice(&bytes).map_err(|_| TransportError::MalformedResponse);
    }
    let envelope: ErrorBody =
        serde_json::from_slice(&bytes).map_err(|_| TransportError::MalformedResponse)?;
    Err(classify_api_error(status, envelope))
}

fn classify_api_error(status: u16, envelope: ErrorBody) -> TransportError {
    match envelope.error.code.as_str() {
        "SESSION_EXPIRED" | "AUTH_CHALLENGE_EXPIRED" => TransportError::SessionExpired,
        "PROTOCOL_UNSUPPORTED" => TransportError::ProtocolUnsupported,
        "RATE_LIMITED" => TransportError::RateLimited,
        "VERSION_CONFLICT" => {
            // Carried through the error path structurally; push_records
            // turns it into a PushOutcome before callers see it.
            TransportError::Api {
                code: "VERSION_CONFLICT".to_string(),
                status,
            }
        }
        code => TransportError::Api {
            code: code.to_string(),
            status,
        },
    }
}

// ----- record JSON forms (server recordBody / RecordOut) -----

#[derive(Serialize)]
struct RecordBody<'a> {
    id: &'a str,
    entity_type: &'a str,
    entity_id: &'a str,
    version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ciphertext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_at: Option<i64>,
    updated_at: i64,
    device_id: &'a str,
    key_id: i64,
    signature: String,
}

fn record_to_body(record: &WireRecord) -> Result<RecordBody<'_>, TransportError> {
    Ok(RecordBody {
        id: &record.id,
        entity_type: record.entity_type.as_str(),
        entity_id: &record.entity_id,
        version: i64::try_from(record.version).map_err(|_| TransportError::MalformedResponse)?,
        ciphertext: if record.is_tombstone() {
            None
        } else {
            Some(BASE64.encode(&record.ciphertext))
        },
        deleted_at: record.deleted_at,
        updated_at: record.updated_at,
        device_id: &record.device_id,
        key_id: i64::from(record.key_id),
        signature: BASE64.encode(record.signature),
    })
}

#[derive(Deserialize)]
struct RecordOutBody {
    id: String,
    entity_type: String,
    entity_id: String,
    version: i64,
    #[serde(default)]
    ciphertext: Option<String>,
    #[serde(default)]
    deleted_at: Option<i64>,
    updated_at: i64,
    device_id: String,
    key_id: i64,
    signature: String,
    server_seq: i64,
}

fn record_from_body(body: RecordOutBody) -> Result<PulledRecord, TransportError> {
    let ciphertext = match body.ciphertext {
        Some(b64) => BASE64
            .decode(b64)
            .map_err(|_| TransportError::MalformedResponse)?,
        None => Vec::new(),
    };
    let signature: [u8; 64] = BASE64
        .decode(body.signature)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(TransportError::MalformedResponse)?;
    Ok(PulledRecord {
        record: WireRecord {
            id: body.id,
            entity_type: body
                .entity_type
                .parse()
                .map_err(|_| TransportError::MalformedResponse)?,
            entity_id: body.entity_id,
            version: u64::try_from(body.version).map_err(|_| TransportError::MalformedResponse)?,
            ciphertext,
            deleted_at: body.deleted_at,
            updated_at: body.updated_at,
            device_id: body.device_id,
            key_id: u32::try_from(body.key_id).map_err(|_| TransportError::MalformedResponse)?,
            signature,
        },
        server_seq: u64::try_from(body.server_seq)
            .map_err(|_| TransportError::MalformedResponse)?,
    })
}

fn decode_b64(value: &str) -> Result<Vec<u8>, TransportError> {
    BASE64
        .decode(value)
        .map_err(|_| TransportError::MalformedResponse)
}

impl SyncTransport for HttpTransport {
    fn handshake(&self) -> Result<HandshakeInfo, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            protocol_min: u32,
            protocol_max: u32,
            server_version: String,
        }
        let body: Body = self.get_json("/v1/handshake", None)?;
        if PROTOCOL_VERSION < body.protocol_min || PROTOCOL_VERSION > body.protocol_max {
            return Err(TransportError::ProtocolUnsupported);
        }
        Ok(HandshakeInfo {
            protocol_min: body.protocol_min,
            protocol_max: body.protocol_max,
            server_version: body.server_version,
        })
    }

    fn create_account(&self, root: &RootStatement) -> Result<String, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            account_id: String,
        }
        let root_json =
            root_statement_to_json(root).map_err(|_| TransportError::MalformedResponse)?;
        let root_value: serde_json::Value =
            serde_json::from_slice(&root_json).map_err(|_| TransportError::MalformedResponse)?;
        let body: Body = self.post_json(
            "/v1/accounts",
            None,
            &serde_json::json!({ "root": root_value }),
        )?;
        Ok(body.account_id)
    }

    fn auth_challenge(&self, device_id: &str) -> Result<Vec<u8>, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            challenge: String,
        }
        let body: Body = self.post_json(
            "/v1/auth/challenge",
            None,
            &serde_json::json!({ "device_id": device_id }),
        )?;
        decode_b64(&body.challenge)
    }

    fn auth_session(
        &self,
        device_id: &str,
        signature: &[u8; 64],
    ) -> Result<SessionToken, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            session_token: String,
        }
        let body: Body = self.post_json(
            "/v1/auth/session",
            None,
            &serde_json::json!({
                "device_id": device_id,
                "sig": BASE64.encode(signature),
            }),
        )?;
        Ok(SessionToken::new(body.session_token))
    }

    fn push_records(
        &self,
        token: &SessionToken,
        records: &[WireRecord],
    ) -> Result<PushOutcome, TransportError> {
        #[derive(Serialize)]
        struct Request<'a> {
            records: Vec<RecordBody<'a>>,
        }
        #[derive(Deserialize)]
        struct Response {
            results: Vec<ResultBody>,
        }
        #[derive(Deserialize)]
        struct ResultBody {
            id: String,
            server_seq: i64,
            #[serde(default)]
            duplicate: bool,
        }

        let bodies: Result<Vec<RecordBody<'_>>, TransportError> =
            records.iter().map(record_to_body).collect();
        let payload = serde_json::to_string(&Request { records: bodies? })
            .map_err(|_| TransportError::MalformedResponse)?;
        let response = self
            .agent
            .post(self.url("/v1/records"))
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token.0))
            .send(payload.as_bytes())
            .map_err(map_ureq_error)?;

        // 409 with conflicts is a business outcome, not a failure.
        let mut response = response;
        let status = response.status().as_u16();
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(map_ureq_error)?;
        if (200..300).contains(&status) {
            let parsed: Response =
                serde_json::from_slice(&bytes).map_err(|_| TransportError::MalformedResponse)?;
            return Ok(PushOutcome::Accepted(
                parsed
                    .results
                    .into_iter()
                    .map(|r| PushResultItem {
                        id: r.id,
                        server_seq: r.server_seq,
                        duplicate: r.duplicate,
                    })
                    .collect(),
            ));
        }
        let envelope: ErrorBody =
            serde_json::from_slice(&bytes).map_err(|_| TransportError::MalformedResponse)?;
        if envelope.error.code == "VERSION_CONFLICT" {
            let heads = envelope
                .error
                .conflicts
                .into_iter()
                .map(|c| {
                    Ok(ConflictHead {
                        entity_type: c.entity_type,
                        entity_id: c.entity_id,
                        head_version: u64::try_from(c.head_version)
                            .map_err(|_| TransportError::MalformedResponse)?,
                    })
                })
                .collect::<Result<Vec<_>, TransportError>>()?;
            return Ok(PushOutcome::VersionConflict(heads));
        }
        Err(classify_api_error(status, envelope))
    }

    fn pull_records(
        &self,
        token: &SessionToken,
        since: u64,
        limit: u32,
    ) -> Result<PullPage, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            records: Vec<RecordOutBody>,
            has_more: bool,
        }
        let body: Body = self.get_json(
            &format!("/v1/records?since={since}&limit={limit}"),
            Some(token),
        )?;
        let records = body
            .records
            .into_iter()
            .map(record_from_body)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PullPage {
            records,
            has_more: body.has_more,
        })
    }

    fn device_directory(&self, token: &SessionToken) -> Result<DeviceDirectory, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            root_statement: serde_json::Value,
            devices: Vec<DeviceBody>,
            revocations: Vec<RevocationBody>,
        }
        #[derive(Deserialize)]
        struct DeviceBody {
            id: String,
            name: String,
            platform: String,
            ed25519_pub: String,
            x25519_pub: String,
            cert_chain: serde_json::Value,
            created_at: i64,
            #[serde(default)]
            revoked_at: Option<i64>,
        }
        #[derive(Deserialize)]
        struct RevocationBody {
            device_id: String,
            revoked_at: i64,
        }

        let body: Body = self.get_json("/v1/devices", Some(token))?;
        let root_statement_json = serde_json::to_vec(&body.root_statement)
            .map_err(|_| TransportError::MalformedResponse)?;
        let devices = body
            .devices
            .into_iter()
            .map(|d| {
                Ok(DirectoryDevice {
                    ed25519_pub: decode_b64(&d.ed25519_pub)?,
                    x25519_pub: decode_b64(&d.x25519_pub)?,
                    cert_chain_json: serde_json::to_vec(&d.cert_chain)
                        .map_err(|_| TransportError::MalformedResponse)?,
                    id: d.id,
                    name: d.name,
                    platform: d.platform,
                    created_at: d.created_at,
                    revoked_at: d.revoked_at,
                })
            })
            .collect::<Result<Vec<_>, TransportError>>()?;
        Ok(DeviceDirectory {
            root_statement_json,
            devices,
            revocations: body
                .revocations
                .into_iter()
                .map(|r| DirectoryRevocation {
                    device_id: r.device_id,
                    revoked_at: r.revoked_at,
                })
                .collect(),
        })
    }

    fn revoke_device(
        &self,
        token: &SessionToken,
        device_id: &str,
        revoked_at: i64,
        signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(rename = "status")]
            _status: String,
        }
        let _: Body = self.post_json(
            &format!("/v1/devices/{device_id}/revoke"),
            Some(token),
            &serde_json::json!({
                "revoked_at": revoked_at,
                "signature": BASE64.encode(signature),
            }),
        )?;
        Ok(())
    }

    fn pair_begin(&self, account_id: &str) -> Result<PairingSession, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            session_id: String,
            expires_in: u64,
        }
        let body: Body = self.post_json(
            "/v1/pair/begin",
            None,
            &serde_json::json!({ "account_id": account_id }),
        )?;
        Ok(PairingSession {
            session_id: body.session_id,
            expires_in_seconds: body.expires_in,
        })
    }

    fn pair_offer(
        &self,
        token: &SessionToken,
        session_id: &str,
        certificate: &DeviceCertificate,
        sealed_bundle: &[u8],
        bundle_signature: &[u8; 64],
    ) -> Result<(), TransportError> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(rename = "status")]
            _status: String,
        }
        let cert_json =
            certificate_to_json(certificate).map_err(|_| TransportError::MalformedResponse)?;
        let cert_value: serde_json::Value =
            serde_json::from_slice(&cert_json).map_err(|_| TransportError::MalformedResponse)?;
        let _: Body = self.post_json(
            &format!("/v1/pair/{session_id}/offer"),
            Some(token),
            &serde_json::json!({
                "certificate": cert_value,
                "sealed_bundle": BASE64.encode(sealed_bundle),
                "bundle_signature": BASE64.encode(bundle_signature),
            }),
        )?;
        Ok(())
    }

    fn pair_claim(&self, session_id: &str) -> Result<PairClaim, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            status: String,
            #[serde(default)]
            payload: Option<String>,
        }
        let body: Body = self.post_json(
            &format!("/v1/pair/{session_id}/claim"),
            None,
            &serde_json::json!({}),
        )?;
        match (body.status.as_str(), body.payload) {
            ("pending", _) => Ok(PairClaim::Pending),
            ("ready", Some(payload)) => Ok(PairClaim::Ready(decode_b64(&payload)?)),
            _ => Err(TransportError::MalformedResponse),
        }
    }

    fn put_key_update(
        &self,
        token: &SessionToken,
        target_device_id: &str,
        payload: &[u8],
    ) -> Result<i64, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            seq: i64,
        }
        let body: Body = self.post_json(
            "/v1/keys/updates",
            Some(token),
            &serde_json::json!({
                "target_device_id": target_device_id,
                "payload": BASE64.encode(payload),
            }),
        )?;
        Ok(body.seq)
    }

    fn list_key_updates(
        &self,
        token: &SessionToken,
        since: i64,
    ) -> Result<Vec<KeyUpdate>, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            updates: Vec<UpdateBody>,
        }
        #[derive(Deserialize)]
        struct UpdateBody {
            seq: i64,
            payload: String,
            created_at: i64,
        }
        let body: Body = self.get_json(&format!("/v1/keys/updates?since={since}"), Some(token))?;
        body.updates
            .into_iter()
            .map(|u| {
                Ok(KeyUpdate {
                    seq: u.seq,
                    payload: decode_b64(&u.payload)?,
                    created_at: u.created_at,
                })
            })
            .collect()
    }

    fn put_recovery_blob(
        &self,
        token: &SessionToken,
        blob: &[u8],
        rootproof_pub: &[u8],
    ) -> Result<(), TransportError> {
        #[derive(Deserialize)]
        struct Body {
            #[serde(rename = "status")]
            _status: String,
        }
        let _: Body = self.put_json(
            "/v1/recovery",
            Some(token),
            &serde_json::json!({
                "blob": BASE64.encode(blob),
                "rootproof_pub": BASE64.encode(rootproof_pub),
            }),
        )?;
        Ok(())
    }

    fn get_recovery_blob(&self, account_id: &str) -> Result<Vec<u8>, TransportError> {
        #[derive(Deserialize)]
        struct Body {
            blob: String,
        }
        let body: Body = self.get_json(&format!("/v1/recovery?account_id={account_id}"), None)?;
        decode_b64(&body.blob)
    }

    fn re_root(
        &self,
        account_id: &str,
        proof_signature: Option<&[u8; 64]>,
        new_root: Option<&RootStatement>,
    ) -> Result<ReRootOutcome, TransportError> {
        let mut request = serde_json::json!({ "account_id": account_id });
        if let Some(signature) = proof_signature {
            request["proof_sig"] = serde_json::Value::String(BASE64.encode(signature));
        }
        if let Some(root) = new_root {
            let root_json =
                root_statement_to_json(root).map_err(|_| TransportError::MalformedResponse)?;
            request["new_root"] = serde_json::from_slice(&root_json)
                .map_err(|_| TransportError::MalformedResponse)?;
        }
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            challenge: Option<String>,
            #[serde(default)]
            status: Option<String>,
        }
        let body: Body = self.post_json("/v1/root", None, &request)?;
        match (body.challenge, body.status.as_deref()) {
            (Some(challenge), _) => Ok(ReRootOutcome::Challenge(decode_b64(&challenge)?)),
            (None, Some("re-rooted")) => Ok(ReRootOutcome::Done),
            _ => Err(TransportError::MalformedResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_urls_are_accepted() {
        assert!(HttpTransport::new("https://sync.example.com").is_ok());
        assert!(HttpTransport::new("https://sync.example.com/").is_ok());
    }

    #[test]
    fn plain_http_is_loopback_only() {
        assert!(HttpTransport::new("http://127.0.0.1:8787").is_ok());
        assert!(HttpTransport::new("http://localhost:8787").is_ok());
        assert!(HttpTransport::new("http://[::1]:8787").is_ok());
        assert!(matches!(
            HttpTransport::new("http://sync.example.com"),
            Err(TransportError::InvalidServerUrl)
        ));
        assert!(matches!(
            HttpTransport::new("http://192.168.1.10:8787"),
            Err(TransportError::InvalidServerUrl)
        ));
    }

    #[test]
    fn other_schemes_are_rejected() {
        for url in ["ftp://x", "ws://x", "file:///tmp/db", "sync.example.com"] {
            assert!(matches!(
                HttpTransport::new(url),
                Err(TransportError::InvalidServerUrl)
            ));
        }
    }

    #[test]
    fn session_token_debug_is_redacted() {
        let token = SessionToken::new("FAKE_TOKEN_VALUE".to_string());
        let rendered = format!("{token:?}");
        assert!(!rendered.contains("FAKE_TOKEN_VALUE"));
    }
}
