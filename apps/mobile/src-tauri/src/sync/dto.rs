// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Wire shapes for the sync command surface. Serialized camelCase to match
//! the rest of the IPC layer; no key material, no entity content, and no
//! server tokens appear in any field.
//!
//! Field-for-field the desktop host's shapes: both hosts answer the same
//! typed IPC client, so a divergence here would silently break one of them.

use serde::Serialize;

/// The persisted configuration, read straight from `sync_config`.
pub struct SyncSetupState {
    pub server_url: Option<String>,
    pub account_id: Option<String>,
    pub enabled: bool,
    pub key_generation: u32,
    pub pending_backlog: u64,
    /// The post-recovery historical catch-up has not drained yet.
    pub recovery_catchup_pending: bool,
    /// Which sync backend the account is bound to.
    pub transport_kind: String,
}

/// Everything the Sync page and the Settings group need to describe the
/// current state without a second round trip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusDto {
    /// False when the platform secure store is unavailable: sync cannot be
    /// set up here, and the page says so instead of offering a dead button.
    pub available: bool,
    /// An account is bound (even while sync is switched off).
    pub configured: bool,
    pub enabled: bool,
    pub server_url: Option<String>,
    pub account_id: Option<String>,
    pub device_id: String,
    pub device_name: String,
    /// Current K_sync generation; rises on every rotation.
    pub key_generation: u32,
    /// Records still waiting in the outbox (backlog indicator).
    pub pending_backlog: u64,
    /// Conflict copies awaiting a decision.
    pub conflict_count: u32,
    pub last_sync_at: Option<i64>,
    /// A vault exists on this device (recovery codes need its master key).
    pub vault_ready: bool,
    pub vault_unlocked: bool,
    /// When this device last exported a recovery code.
    pub recovery_exported_at: Option<i64>,
    /// A recovery finished but its historical catch-up is still pending;
    /// the rounds resume it automatically.
    pub recovery_catchup_pending: bool,
    /// Which sync backend the account is bound to: "server" | "webdav".
    pub transport_kind: String,
}

/// One row of the device list / one node on the route drawing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeviceDto {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    /// The certificate chain verifies to the pinned trust root.
    pub verified: bool,
    pub is_this_device: bool,
    pub is_root: bool,
}

/// What one sync round did. Counts only — never which entities.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRoundDto {
    pub pushed: u32,
    pub applied: u32,
    pub merged: u32,
    pub conflict_copies: u32,
    pub parked: u32,
    pub skipped: u32,
    pub pending_backlog: u64,
    pub at: i64,
}

/// A started pairing session on the new device: the code to show, plus the
/// account it will join once approved.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStartDto {
    /// `TYPVIA-PAIR.V1.<base64>` — the QR payload and the paste fallback.
    pub code: String,
    pub session_id: String,
}

/// The trusted device's view of a scanned code: the short
/// authentication string plus who is asking to join. The UI must not offer
/// a way past this check — it is what defeats a relaying server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingSasDto {
    pub sas: String,
    /// The name the joining device gave itself, so the user knows which two
    /// screens they are comparing.
    pub device_name: String,
    pub platform: String,
    /// Whether this account has a vault at all — a device can only be
    /// granted vault access when there is one.
    pub vault_ready: bool,
}

/// The new device's view once the offer arrived: the same
/// short authentication string, computed independently, plus the account
/// anchor it would pin. Nothing is installed until the user confirms.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingClaimDto {
    pub sas: String,
    /// Display form of the account's trust-root fingerprint.
    pub root_fingerprint: String,
}

/// A generated recovery code, returned exactly once.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCodeDto {
    /// Hyphen-grouped display form. Never persisted, never logged.
    pub code: String,
    pub account_id: String,
    pub server_url: String,
}
