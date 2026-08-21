// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Espanso integration adapter: config-file generation and CLI interaction only.
//!
//! No code-level dependency on espanso (GPL-3.0) is permitted: espanso is driven
//! exclusively as a separate process through its command-line interface, and
//! Typvia writes its own configuration into an isolated directory. See this
//! crate's `README.md`.
//!
//! This crate never handles snippet bodies; its error and log surfaces carry
//! only structural information about CLI interaction.

mod cli;
mod coexistence;
mod compile;
mod error;
mod import;
mod lifecycle;
mod managed;
mod status;
mod writer;

pub use cli::{CliOutput, EspansoCli, SystemEspansoCli};
pub use coexistence::{
    CoexistenceChoice, EngineGate, engine_gate, load_choice, retire_legacy_config, store_choice,
};
pub use compile::{CompileError, CompiledConfig, compile_snippets};
pub use error::EspansoError;
pub use import::{ImportParseError, ImportedMatch, ParsedMatches, SkippedMatch, parse_matches};
pub use lifecycle::{EngineError, EngineState, EngineSupervisor};
pub use managed::{ManagedDirs, ManagedDirsError};
pub use status::{EspansoAdapter, EspansoPaths, EspansoStatus, EspansoVersion};
pub use writer::write_config;
