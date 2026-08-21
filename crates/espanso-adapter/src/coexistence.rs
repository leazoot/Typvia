// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Coexistence with a user-installed espanso.
//!
//! The managed engine must never fight a user-owned espanso instance for the
//! global event stream, and Typvia never touches that instance's lifecycle —
//! `espanso stop`-style commands from Typvia's context can kill it globally or
//! reset its accessibility grant. So the only lever is a gate on *our* side:
//! while the user instance is running the managed engine does not
//! start, and the user chooses between
//!
//! - **take over** — they stop/unregister their instance themselves (guided by
//!   the settings UI); once it is gone the gate opens on its own;
//! - **stand aside** — the managed engine stays off and the status surface
//!   reports the degraded mode honestly.
//!
//! Detection runs the CLI *without* the isolation environment, so
//! `espanso service status` inspects the default (user) runtime directory and
//! never sees Typvia's private instance.

use std::fs;
use std::io;

use crate::cli::EspansoCli;
use crate::managed::ManagedDirs;
use crate::writer::write_config;

/// The user's recorded answer to the conflict prompt, kept as a small file in
/// the private engine directory (`<engine>/coexistence`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoexistenceChoice {
    /// User intends to stop their own instance; keep waiting for it to go.
    Takeover,
    /// Leave the user's instance alone; managed engine stays off.
    StandAside,
}

impl CoexistenceChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Takeover => "takeover",
            Self::StandAside => "stand_aside",
        }
    }

    /// Parse a stored/IPC value; unknown text is `None` (forward compatible).
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "takeover" => Some(Self::Takeover),
            "stand_aside" => Some(Self::StandAside),
            _ => None,
        }
    }
}

fn choice_path(dirs: &ManagedDirs) -> std::path::PathBuf {
    dirs.config_root()
        .parent()
        .map(|root| root.join("coexistence"))
        .unwrap_or_else(|| dirs.config_root().join("coexistence"))
}

/// The recorded choice, if any (unreadable/unknown content reads as none).
pub fn load_choice(dirs: &ManagedDirs) -> Option<CoexistenceChoice> {
    let text = fs::read_to_string(choice_path(dirs)).ok()?;
    CoexistenceChoice::parse(&text)
}

/// Record the user's choice (atomic, like every engine-dir write).
pub fn store_choice(dirs: &ManagedDirs, choice: CoexistenceChoice) -> io::Result<()> {
    write_config(&choice_path(dirs), choice.as_str())
}

/// What the gate decided about starting the managed engine right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineGate {
    /// No conflict — the engine may start.
    Start,
    /// Integration is off (no generated config); nothing to run.
    Disabled,
    /// A user-owned espanso is running and no choice has been made yet — the
    /// UI must ask; the engine stays off (no silent takeover).
    ConflictUnresolved,
    /// User chose takeover but their instance is still running — keep waiting,
    /// engine stays off.
    TakeoverPending,
    /// User chose to leave their instance alone — engine stays off.
    StandingAside,
}

/// Remove the `match/typvia.yml` the earlier, unmanaged integration wrote into the
/// *user's* espanso directory. That file was always Typvia's own artifact (the
/// old enable/disable surface created and removed it); leaving it behind would
/// make a user-owned instance keep expanding a stale trigger set next to the
/// managed engine. Best-effort: no user instance / no legacy file is fine, and
/// nothing else in the user's setup is touched.
pub fn retire_legacy_config<C: EspansoCli>(cli: C) {
    if let Ok(paths) = crate::status::EspansoAdapter::new(cli).paths() {
        let _ = fs::remove_file(paths.typvia_config_path());
    }
}

/// Decide whether the managed engine may start. `cli` must run **without** the
/// isolation environment so it reports the user instance, not ours.
pub fn engine_gate<C: EspansoCli>(cli: &C, dirs: &ManagedDirs) -> EngineGate {
    if !dirs.typvia_match_path().exists() {
        return EngineGate::Disabled;
    }
    let user_instance_running = cli
        .run(&["service", "status"])
        .is_ok_and(|output| output.code == Some(0));
    if !user_instance_running {
        return EngineGate::Start;
    }
    match load_choice(dirs) {
        None => EngineGate::ConflictUnresolved,
        Some(CoexistenceChoice::Takeover) => EngineGate::TakeoverPending,
        Some(CoexistenceChoice::StandAside) => EngineGate::StandingAside,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliOutput;

    struct FixedCli {
        service_status_code: i32,
    }

    impl EspansoCli for FixedCli {
        fn run(&self, args: &[&str]) -> io::Result<CliOutput> {
            assert_eq!(args, ["service", "status"], "gate only probes the service");
            Ok(CliOutput {
                code: Some(self.service_status_code),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    fn enabled_dirs(root: &std::path::Path) -> ManagedDirs {
        let dirs = ManagedDirs::new(root);
        dirs.ensure().expect("layout");
        write_config(&dirs.typvia_match_path(), "matches: []\n").expect("enable");
        dirs
    }

    #[test]
    fn choice_round_trips_and_unknown_content_reads_as_none() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = ManagedDirs::new(temp.path());
        dirs.ensure().expect("layout");
        assert_eq!(load_choice(&dirs), None);
        store_choice(&dirs, CoexistenceChoice::StandAside).expect("store");
        assert_eq!(load_choice(&dirs), Some(CoexistenceChoice::StandAside));
        store_choice(&dirs, CoexistenceChoice::Takeover).expect("store");
        assert_eq!(load_choice(&dirs), Some(CoexistenceChoice::Takeover));
        fs::write(choice_path(&dirs), "someday_a_new_mode").expect("future value");
        assert_eq!(load_choice(&dirs), None);
    }

    #[test]
    fn gate_is_disabled_without_a_generated_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = ManagedDirs::new(temp.path());
        dirs.ensure().expect("layout");
        let cli = FixedCli {
            service_status_code: 4,
        };
        assert_eq!(engine_gate(&cli, &dirs), EngineGate::Disabled);
    }

    #[test]
    fn gate_opens_when_no_user_instance_is_running() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = enabled_dirs(temp.path());
        let cli = FixedCli {
            service_status_code: 4,
        };
        assert_eq!(engine_gate(&cli, &dirs), EngineGate::Start);
    }

    #[test]
    fn running_user_instance_blocks_until_a_choice_is_made() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = enabled_dirs(temp.path());
        let cli = FixedCli {
            service_status_code: 0,
        };
        assert_eq!(engine_gate(&cli, &dirs), EngineGate::ConflictUnresolved);
    }

    #[test]
    fn takeover_keeps_waiting_while_the_user_instance_lives_then_opens() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = enabled_dirs(temp.path());
        store_choice(&dirs, CoexistenceChoice::Takeover).expect("choice");
        let running = FixedCli {
            service_status_code: 0,
        };
        assert_eq!(engine_gate(&running, &dirs), EngineGate::TakeoverPending);
        let stopped = FixedCli {
            service_status_code: 4,
        };
        assert_eq!(engine_gate(&stopped, &dirs), EngineGate::Start);
    }

    #[test]
    fn retires_the_legacy_config_from_the_user_directory_only() {
        let temp = tempfile::tempdir().expect("temp dir");
        let user_match = temp.path().join("match");
        fs::create_dir_all(&user_match).expect("user match dir");
        let legacy = user_match.join("typvia.yml");
        let neighbour = user_match.join("base.yml");
        fs::write(&legacy, "matches: []\n").expect("legacy");
        fs::write(&neighbour, "matches: []\n").expect("neighbour");

        struct PathsCli {
            config: std::path::PathBuf,
        }
        impl EspansoCli for PathsCli {
            fn run(&self, args: &[&str]) -> io::Result<CliOutput> {
                assert_eq!(args, ["path"]);
                Ok(CliOutput {
                    code: Some(0),
                    stdout: format!(
                        "Config: {}\nPackages: {}/packages\nRuntime: {}/runtime\n",
                        self.config.display(),
                        self.config.display(),
                        self.config.display()
                    ),
                    stderr: String::new(),
                })
            }
        }

        retire_legacy_config(PathsCli {
            config: temp.path().to_path_buf(),
        });
        assert!(!legacy.exists(), "legacy typvia.yml must be retired");
        assert!(
            neighbour.exists(),
            "user-owned match files must never be touched"
        );
    }

    #[test]
    fn stand_aside_leaves_the_user_instance_alone() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = enabled_dirs(temp.path());
        store_choice(&dirs, CoexistenceChoice::StandAside).expect("choice");
        let running = FixedCli {
            service_status_code: 0,
        };
        assert_eq!(engine_gate(&running, &dirs), EngineGate::StandingAside);
    }
}
