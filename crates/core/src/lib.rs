// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Core business logic for Typvia: data model, storage, and use-case orchestration.

pub mod ai_action_seed;
pub mod app_rules;
pub mod backup;
pub mod db;
pub mod import;
pub mod model;
pub mod repo;
pub mod sensitive;
pub mod snapshot;
pub mod sync_hooks;
pub mod vault;
