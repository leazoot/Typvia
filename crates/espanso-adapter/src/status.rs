// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Installation detection, version reading, config-path location and the
//! not-installed / stopped / running state machine.
//!
//! All decisions are driven off the [`EspansoCli`] trait, so unit tests feed
//! fixed sample output and the ignored live test drives the real binary.

use std::path::PathBuf;

use crate::cli::EspansoCli;
use crate::error::EspansoError;

/// A parsed espanso version.
///
/// `raw` is always retained so an unrecognised format still round-trips to the
/// UI; `parts` holds the `(major, minor, patch)` triple when it parses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EspansoVersion {
    raw: String,
    parts: Option<(u32, u32, u32)>,
}

impl EspansoVersion {
    /// Parse the version out of `espanso --version` output. Tolerant of both a
    /// bare `2.4.0` and a prefixed `espanso 2.4.0` form; the first `x.y.z`
    /// token wins.
    pub fn parse(text: &str) -> Self {
        let raw = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_string();
        let parts = raw.split_whitespace().find_map(parse_triple);
        Self { raw, parts }
    }

    /// The raw version line as reported by espanso.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The major version component, when the version parsed.
    pub fn major(&self) -> Option<u32> {
        self.parts.map(|(major, _, _)| major)
    }

    /// The full `(major, minor, patch)` triple, when the version parsed.
    pub fn triple(&self) -> Option<(u32, u32, u32)> {
        self.parts
    }
}

/// Extract a `major.minor.patch` triple from a single token, ignoring a leading
/// non-digit prefix (e.g. `v2.4.0`) and a trailing pre-release suffix
/// (e.g. `2.4.0-beta`).
fn parse_triple(token: &str) -> Option<(u32, u32, u32)> {
    let token = token.trim_start_matches(|c: char| !c.is_ascii_digit());
    let mut parts = token.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_field = parts.next()?;
    let patch_digits: String = patch_field
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let patch = patch_digits.parse().ok()?;
    Some((major, minor, patch))
}

/// The espanso directory paths reported by `espanso path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EspansoPaths {
    /// Configuration root (holds `config/` and `match/`).
    pub config: PathBuf,
    /// Installed packages directory.
    pub packages: PathBuf,
    /// Runtime/cache directory.
    pub runtime: PathBuf,
}

impl EspansoPaths {
    /// Parse the three labelled lines of `espanso path` output. Returns `None`
    /// if any of the expected labels is absent.
    fn parse(stdout: &str) -> Option<Self> {
        let mut config = None;
        let mut packages = None;
        let mut runtime = None;
        for line in stdout.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("Config:") {
                config = Some(PathBuf::from(value.trim()));
            } else if let Some(value) = line.strip_prefix("Packages:") {
                packages = Some(PathBuf::from(value.trim()));
            } else if let Some(value) = line.strip_prefix("Runtime:") {
                runtime = Some(PathBuf::from(value.trim()));
            }
        }
        Some(Self {
            config: config?,
            packages: packages?,
            runtime: runtime?,
        })
    }

    /// The single file Typvia writes its generated matches into:
    /// `<config>/match/typvia.yml`. Typvia writes only this one file and never
    /// touches the rest of the user's espanso setup; espanso loads it via its
    /// `match/` scan.
    pub fn typvia_config_path(&self) -> PathBuf {
        self.config.join("match").join("typvia.yml")
    }
}

/// Espanso availability as seen through the CLI.
///
/// macOS cannot distinguish "installed but not yet authorized" from a plainly
/// stopped service over the CLI: the daemon refuses to start until Accessibility
/// is granted, and that TCC grant belongs to Espanso.app, not to Typvia. Both
/// therefore map to [`EspansoStatus::Stopped`]; the settings UI frames it as
/// "installed — enable / authorize espanso".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EspansoStatus {
    /// The espanso binary was not found.
    NotInstalled,
    /// Binary present, background service not running (covers unauthorized).
    Stopped { version: EspansoVersion },
    /// Service registered and running.
    Running { version: EspansoVersion },
}

/// Detects espanso and reports its status through an injected [`EspansoCli`].
pub struct EspansoAdapter<C: EspansoCli> {
    cli: C,
}

impl<C: EspansoCli> EspansoAdapter<C> {
    /// Build an adapter over a CLI runner.
    pub fn new(cli: C) -> Self {
        Self { cli }
    }

    /// Detect installation, read the version, and classify the service state.
    ///
    /// A binary that cannot be launched is [`EspansoStatus::NotInstalled`]; a
    /// running service (`espanso service status` exits `0`) is
    /// [`EspansoStatus::Running`]; anything else with the binary present is
    /// [`EspansoStatus::Stopped`] (observed: exit `4`, "espanso is not running").
    pub fn status(&self) -> EspansoStatus {
        let version = match self.cli.run(&["--version"]) {
            Ok(output) => EspansoVersion::parse(&output.stdout),
            Err(_) => return EspansoStatus::NotInstalled,
        };
        match self.cli.run(&["service", "status"]) {
            Ok(output) if output.code == Some(0) => EspansoStatus::Running { version },
            _ => EspansoStatus::Stopped { version },
        }
    }

    /// Resolve the espanso config/packages/runtime directories via
    /// `espanso path`.
    ///
    /// A missing config directory makes espanso panic, so this guards it:
    /// a non-zero exit or unparseable output becomes an [`EspansoError`], never
    /// a panic in this process.
    pub fn paths(&self) -> Result<EspansoPaths, EspansoError> {
        let output = self.cli.run(&["path"]).map_err(EspansoError::from_io)?;
        if output.code != Some(0) {
            return Err(EspansoError::Cli { code: output.code });
        }
        EspansoPaths::parse(&output.stdout).ok_or(EspansoError::Unparseable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_version() {
        let version = EspansoVersion::parse("2.4.0\n");
        assert_eq!(version.raw(), "2.4.0");
        assert_eq!(version.triple(), Some((2, 4, 0)));
        assert_eq!(version.major(), Some(2));
    }

    #[test]
    fn parses_prefixed_version() {
        let version = EspansoVersion::parse("espanso 2.4.0\nsome trailing help text");
        assert_eq!(version.triple(), Some((2, 4, 0)));
    }

    #[test]
    fn parses_version_with_prerelease_suffix() {
        let version = EspansoVersion::parse("v2.5.8-beta");
        assert_eq!(version.triple(), Some((2, 5, 8)));
    }

    #[test]
    fn retains_raw_and_leaves_triple_empty_when_unparseable() {
        let version = EspansoVersion::parse("unknown build");
        assert_eq!(version.raw(), "unknown build");
        assert_eq!(version.triple(), None);
        assert_eq!(version.major(), None);
    }

    #[test]
    fn parses_all_three_espanso_paths() {
        let stdout = "Config: /Users/x/Library/Application Support/espanso\n\
             Packages: /Users/x/Library/Application Support/espanso/match/packages\n\
             Runtime: /Users/x/Library/Caches/espanso\n";
        let paths = EspansoPaths::parse(stdout).expect("all three labels present");
        assert_eq!(
            paths.config,
            PathBuf::from("/Users/x/Library/Application Support/espanso")
        );
        assert_eq!(
            paths.packages,
            PathBuf::from("/Users/x/Library/Application Support/espanso/match/packages")
        );
        assert_eq!(
            paths.runtime,
            PathBuf::from("/Users/x/Library/Caches/espanso")
        );
    }

    #[test]
    fn rejects_path_output_missing_a_label() {
        let stdout = "Config: /tmp/espanso\nRuntime: /tmp/cache\n";
        assert_eq!(EspansoPaths::parse(stdout), None);
    }
}
