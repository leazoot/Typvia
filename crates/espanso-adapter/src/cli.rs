// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Process boundary to the espanso CLI.
//!
//! GPL isolation: espanso is only ever driven as a separate process via its
//! command line. Nothing here links, embeds, or reimplements espanso code.
//! The [`EspansoCli`] trait lets the host inject the real system process
//! runner while tests substitute fixed sample output.

use std::io;
use std::process::Command;

/// Captured result of one espanso CLI invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOutput {
    /// Process exit code, or `None` if the process was terminated by a signal.
    ///
    /// A subprocess panic (e.g. `espanso path` before the config directory
    /// exists) surfaces here as a non-zero code, never as a
    /// panic in this process.
    pub code: Option<i32>,
    /// Captured standard output (lossy UTF-8).
    pub stdout: String,
    /// Captured standard error (lossy UTF-8).
    pub stderr: String,
}

/// Runs espanso subcommands as a separate process.
///
/// Implementors must not link any espanso crate; only `exec` of the installed
/// binary plus config-file I/O is permitted. The host injects a
/// concrete implementation so tests can substitute deterministic output.
pub trait EspansoCli {
    /// Run `espanso <args...>` and capture its output.
    ///
    /// Returns `Err` only when the binary cannot be launched at all (e.g. not
    /// installed → [`io::ErrorKind::NotFound`]); a process that runs and exits
    /// non-zero is reported as `Ok` with a non-zero [`CliOutput::code`].
    fn run(&self, args: &[&str]) -> io::Result<CliOutput>;
}

/// [`EspansoCli`] backed by the real `espanso` binary resolved on `PATH`.
///
/// Host binary-path resolution (e.g. a GUI-launched app whose `PATH` omits the
/// Homebrew prefix) is a wiring concern for the desktop host; this default
/// relies on `PATH` and is what the ignored live test exercises.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemEspansoCli;

impl EspansoCli for SystemEspansoCli {
    fn run(&self, args: &[&str]) -> io::Result<CliOutput> {
        let output = Command::new("espanso").args(args).output()?;
        Ok(CliOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
