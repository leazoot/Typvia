// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Typvia mobile FFI surface: the UniFFI layer the native iOS app and its
//! extensions call into.
//!
//! Two surfaces, deliberately unequal.
//!
//! [`service`] is the main app's channel: a connection-owning object that
//! forwards to the shared host-service use cases. Every rule — validation,
//! transaction boundaries, encryption order, index policy — stays on the Rust
//! side of this boundary; the platform layer above it composes screens.
//!
//! [`snapshot`] is what a restricted process gets: parse and read a snapshot
//! document, nothing else. The keyboard extension and the widget never open
//! the database.
//!
//! Dependency direction: mobile-ffi → host-service → {core, search, template}.
//! It runs one way, and no platform type crosses into those crates.

pub mod ai;
pub mod error;
pub mod model;
mod secure_store;
pub mod service;
pub mod snapshot;
pub mod sync;
pub mod vault;

uniffi::setup_scaffolding!();
