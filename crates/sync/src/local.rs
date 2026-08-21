// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Local-device registration: keep the secure-store identity and the
//! device table row in step.
//!
//! The device id is NOT minted here: the host layer already keeps one
//! stable per-install id in `<data_dir>/device-id` for the keyboard
//! snapshot pipeline, and the caller passes that same id in — one device,
//! one id, across snapshot and sync.

use rusqlite::Connection;
use typvia_core::model::{Device, Platform, TimestampMs, TrustLevel};
use typvia_core::repo::DeviceRepo;
use typvia_crypto::SecureStore;

use crate::error::EnsureLocalDeviceError;
use crate::identity::DeviceIdentity;

/// Stable facts about this install, supplied by the host layer.
pub struct LocalDeviceConfig<'a> {
    /// The per-install device id (`<data_dir>/device-id`).
    pub device_id: &'a str,
    /// User-visible device name.
    pub name: &'a str,
    pub platform: Platform,
}

/// The registered local device: its key material plus the directory row.
/// (`Debug` stays safe: the identity's own impl redacts all secrets.)
#[derive(Debug)]
pub struct LocalDevice {
    pub identity: DeviceIdentity,
    pub device: Device,
}

/// Ensures this install has a device identity and a matching device row.
///
/// First run: generates the key pairs (OsRng), persists the private halves
/// to the secure store, and inserts the device row with the Ed25519 public
/// key. Later runs: loads the identity back, checks the row still carries
/// the same public key (a mismatch is surfaced, never merged), and bumps
/// `last_seen_at`. A failure between the store write and the row insert
/// heals on the next call — the persisted identity is loaded and only the
/// missing row is created.
pub fn ensure_local_device(
    conn: &Connection,
    store: &dyn SecureStore,
    config: &LocalDeviceConfig<'_>,
    now: TimestampMs,
) -> Result<LocalDevice, EnsureLocalDeviceError> {
    let identity = match DeviceIdentity::load(store)? {
        Some(identity) => identity,
        None => {
            let identity = DeviceIdentity::generate();
            identity.persist(store)?;
            identity
        }
    };

    let repo = DeviceRepo::new(conn);
    let device = match repo.get(config.device_id)? {
        Some(existing) => {
            if existing.public_key != identity.ed25519_public() {
                return Err(EnsureLocalDeviceError::IdentityMismatch);
            }
            repo.touch_last_seen(config.device_id, now)?;
            Device {
                last_seen_at: Some(now),
                ..existing
            }
        }
        None => {
            let device = Device {
                id: config.device_id.to_string(),
                name: config.name.to_string(),
                platform: config.platform,
                public_key: identity.ed25519_public().to_vec(),
                trust_level: TrustLevel::Trusted,
                last_seen_at: Some(now),
                created_at: now,
                revoked_at: None,
            };
            repo.insert(&device)?;
            device
        }
    };

    Ok(LocalDevice { identity, device })
}
