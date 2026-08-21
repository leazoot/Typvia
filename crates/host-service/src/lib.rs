// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Shared desktop/mobile IPC orchestration layer: pure-Connection use-case
//! orchestration over the core traits, the wire DTOs, and the stable IPC
//! error codes. This crate knows nothing of Tauri types or platform
//! capabilities — injector- and Espanso-coupled use cases stay in the
//! desktop host.

pub mod ai;
pub mod dto;
pub mod error;
pub mod service;
pub mod snapshot;
