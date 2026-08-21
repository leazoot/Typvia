// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! The `SyncTransport` trait.
//!
//! Business code never speaks HTTP directly: every server interaction goes
//! through this trait (backend rule: external dependencies behind traits).
//! v1 ships exactly one implementation — [`crate::HttpTransport`]; WebDAV
//! and later transports share this surface. Authenticated calls take the
//! session token explicitly; the engine owns the token lifecycle and
//! re-runs the challenge exchange once when a token expires.

use std::fmt;

use crate::cert::{DeviceCertificate, RootStatement};
use crate::record::WireRecord;

/// Session token from the challenge exchange. A transport-layer admission
/// credential only — it carries no cryptographic trust, but it still stays
/// out of Debug output and logs.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(pub(crate) String);

impl SessionToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

/// `GET /v1/handshake` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeInfo {
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub server_version: String,
}

/// One accepted record of a push batch; `duplicate` marks an idempotent
/// re-push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushResultItem {
    pub id: String,
    pub server_seq: i64,
    pub duplicate: bool,
}

/// Current head version of a conflicted entity (409 response body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictHead {
    pub entity_type: String,
    pub entity_id: String,
    pub head_version: u64,
}

/// Outcome of a push: the batch was accepted whole, or rejected whole with
/// the current heads (the server push is atomic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    Accepted(Vec<PushResultItem>),
    VersionConflict(Vec<ConflictHead>),
}

/// One pulled record with its server cursor value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulledRecord {
    pub record: WireRecord,
    pub server_seq: u64,
}

/// One page of an incremental pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullPage {
    pub records: Vec<PulledRecord>,
    pub has_more: bool,
}

/// One directory entry of `GET /v1/devices`. `cert_chain_json` is the
/// client-defined chain encoding relayed opaquely by the server; parsing
/// and verification stay client-side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub ed25519_pub: Vec<u8>,
    pub x25519_pub: Vec<u8>,
    pub cert_chain_json: Vec<u8>,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}

/// A revocation statement entry; the statement itself is verified
/// client-side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryRevocation {
    pub device_id: String,
    pub revoked_at: i64,
}

/// The device directory: root statement, devices with chains, and
/// revocations. All trust decisions happen in the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDirectory {
    pub root_statement_json: Vec<u8>,
    pub devices: Vec<DirectoryDevice>,
    pub revocations: Vec<DirectoryRevocation>,
}

/// `POST /v1/pair/begin` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingSession {
    pub session_id: String,
    pub expires_in_seconds: u64,
}

/// `POST /v1/pair/{session}/claim` outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairClaim {
    /// The trusted device has not posted its offer yet.
    Pending,
    /// The single-use relay payload (certificate chain + sealed bundle).
    Ready(Vec<u8>),
}

/// One sealed device-to-device key update message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyUpdate {
    pub seq: i64,
    pub payload: Vec<u8>,
    pub created_at: i64,
}

/// `POST /v1/root` outcome: phase one hands out a challenge, phase two
/// completes the re-root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReRootOutcome {
    Challenge(Vec<u8>),
    Done,
}

/// Transport failures, pre-classified for the engine. Variants carry
/// stable server error codes and structural facts — never record content,
/// tokens, or key material.
#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
    /// The server URL is not usable: only https is accepted, plus plain
    /// http toward loopback for tests (certificate verification is never
    /// disabled, so this is the only http carve-out).
    InvalidServerUrl,
    /// Connection, DNS, TLS, or I/O failure (message from the HTTP stack).
    Network(String),
    /// The session token was rejected; re-run the challenge exchange once.
    SessionExpired,
    /// No protocol version overlap with the server: surface to the user,
    /// never downgrade silently.
    ProtocolUnsupported,
    /// The server asked for a retry later (RATE_LIMITED).
    RateLimited,
    /// Any other stable-code API error.
    Api { code: String, status: u16 },
    /// The response body did not match the documented shape.
    MalformedResponse,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidServerUrl => f.write_str("server url must be https (or http loopback)"),
            Self::Network(msg) => write!(f, "network failure: {msg}"),
            Self::SessionExpired => f.write_str("session expired"),
            Self::ProtocolUnsupported => f.write_str("no protocol version overlap with server"),
            Self::RateLimited => f.write_str("rate limited by server"),
            Self::Api { code, status } => write!(f, "server error {code} (http {status})"),
            Self::MalformedResponse => f.write_str("server response is malformed"),
        }
    }
}

impl std::error::Error for TransportError {}

/// The server API surface. One method per endpoint; ordering, batching,
/// retries and persistence are the engine's job.
/// `Send` so a host can own the engine behind a mutex and drive a round off
/// the UI thread; the engine itself is still single-threaded per call.
pub trait SyncTransport: Send {
    /// `GET /v1/handshake`.
    fn handshake(&self) -> Result<HandshakeInfo, TransportError>;

    /// `POST /v1/accounts`: registers the first device's root statement;
    /// returns the server-assigned account id.
    fn create_account(&self, root: &RootStatement) -> Result<String, TransportError>;

    /// `POST /v1/auth/challenge` (challenge exchange, step 1).
    fn auth_challenge(&self, device_id: &str) -> Result<Vec<u8>, TransportError>;

    /// `POST /v1/auth/session` (challenge exchange, step 2); `signature` covers
    /// `"typvia.auth.v1" || challenge || device_id`.
    fn auth_session(
        &self,
        device_id: &str,
        signature: &[u8; 64],
    ) -> Result<SessionToken, TransportError>;

    /// `POST /v1/records`: atomic batch push.
    fn push_records(
        &self,
        token: &SessionToken,
        records: &[WireRecord],
    ) -> Result<PushOutcome, TransportError>;

    /// `GET /v1/records?since=&limit=`: ascending incremental pull.
    fn pull_records(
        &self,
        token: &SessionToken,
        since: u64,
        limit: u32,
    ) -> Result<PullPage, TransportError>;

    /// `GET /v1/devices`.
    fn device_directory(&self, token: &SessionToken) -> Result<DeviceDirectory, TransportError>;

    /// `POST /v1/devices/{id}/revoke`; `signature` covers
    /// `"typvia.revoke.v1" || device_id || revoked_at(8B LE)`.
    fn revoke_device(
        &self,
        token: &SessionToken,
        device_id: &str,
        revoked_at: i64,
        signature: &[u8; 64],
    ) -> Result<(), TransportError>;

    /// `POST /v1/pair/begin` (pairing step 1; no session yet).
    fn pair_begin(&self, account_id: &str) -> Result<PairingSession, TransportError>;

    /// `POST /v1/pair/{session}/offer` (pairing steps 6-7, trusted side):
    /// the new device's certificate — which the server verifies against the
    /// admitting session device and registers in the same transaction —
    /// plus the sealed key bundle and its signature.
    fn pair_offer(
        &self,
        token: &SessionToken,
        session_id: &str,
        certificate: &DeviceCertificate,
        sealed_bundle: &[u8],
        bundle_signature: &[u8; 64],
    ) -> Result<(), TransportError>;

    /// `POST /v1/pair/{session}/claim` (pairing step 7, new device).
    fn pair_claim(&self, session_id: &str) -> Result<PairClaim, TransportError>;

    /// `POST /v1/keys/updates`: sealed message to one device.
    fn put_key_update(
        &self,
        token: &SessionToken,
        target_device_id: &str,
        payload: &[u8],
    ) -> Result<i64, TransportError>;

    /// `GET /v1/keys/updates?since=`: this device's messages.
    fn list_key_updates(
        &self,
        token: &SessionToken,
        since: i64,
    ) -> Result<Vec<KeyUpdate>, TransportError>;

    /// `PUT /v1/recovery`: stores the recovery blob and rootproof public
    /// key (public material only).
    fn put_recovery_blob(
        &self,
        token: &SessionToken,
        blob: &[u8],
        rootproof_pub: &[u8],
    ) -> Result<(), TransportError>;

    /// `GET /v1/recovery?account_id=` (anonymous + rate limited).
    fn get_recovery_blob(&self, account_id: &str) -> Result<Vec<u8>, TransportError>;

    /// `POST /v1/root`: without `proof_signature` requests the one-time
    /// challenge; with it executes the root replacement.
    fn re_root(
        &self,
        account_id: &str,
        proof_signature: Option<&[u8; 64]>,
        new_root: Option<&RootStatement>,
    ) -> Result<ReRootOutcome, TransportError>;
}
