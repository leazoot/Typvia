// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! One row of the AI egress audit log. Device-local, append-only: every AI
//! request that actually left this device gets a row saying when, to which
//! provider, of which class, and how many body bytes — and nothing else. The
//! table has no column capable of holding prompt content, response content or
//! key material; logging full AI requests is forbidden.

use super::{AiProviderId, AiRequestClass, TimestampMs};

/// A recorded egress event, as read back from storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiEgressRecord {
    /// Monotonic append id (AUTOINCREMENT: never reused, audit semantics).
    pub id: i64,
    pub occurred_at: TimestampMs,
    /// May dangle after the provider is deleted — the log outlives the
    /// provider on purpose (no FK; deleting a provider must not erase the
    /// audit trail).
    pub provider_id: AiProviderId,
    pub request_class: AiRequestClass,
    /// Request body bytes that left the device (0 for body-less probes).
    pub request_bytes: i64,
}
