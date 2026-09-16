// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Private directory layout for the managed espanso engine.
//!
//! Typvia runs espanso as a fully managed subprocess: the engine reads its
//! configuration, packages and runtime state exclusively from Typvia-private
//! directories selected via the `ESPANSO_CONFIG_DIR` / `ESPANSO_PACKAGE_DIR` /
//! `ESPANSO_RUNTIME_DIR` environment variables. The user's own espanso
//! installation and configuration are never read or written — every path this
//! module produces is derived from the single root handed in by the host.
//!
//! Layout under `<app data>/engine/`:
//!
//! ```text
//! engine/
//!   config/          ← ESPANSO_CONFIG_DIR (espanso requires config/ + match/)
//!     config/default.yml   managed baseline (no icon, no notifications)
//!     match/typvia.yml     generated matches (writer.rs, compile.rs red line)
//!   packages/        ← ESPANSO_PACKAGE_DIR
//!   runtime/         ← ESPANSO_RUNTIME_DIR (sockets live here; see SUN_LEN)
//!     kvs/           first-run markers pre-seeded so no GUI wizard appears
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::writer::write_config;

/// Managed baseline `config/default.yml`. Regenerated whenever it drifts, so
/// the engine can never end up with an icon or notifications enabled.
///
/// Expansions paste instead of typing the replacement key by key, matching
/// how Typvia itself inserts. Editors that handle a backspace slower than a
/// character (Typora, other web-view editors) otherwise apply some of the
/// trigger's backspaces after the replacement: the trigger's first characters
/// survive and the replacement loses as many from its end. espanso restores
/// the clipboard afterwards, and only non-sensitive snippets are ever compiled
/// into its matches.
const DEFAULT_CONFIG: &str = "\
# Managed by Typvia. This file is regenerated on engine start; edits are lost.
show_icon: false
show_notifications: false
backend: Clipboard
";

/// First-run marker files espanso keeps in `<runtime>/kvs/`. Pre-seeding them
/// to `true` suppresses the welcome / wizard windows a fresh install would
/// otherwise open.
const KVS_MARKERS: [&str; 3] = [
    "has_displayed_welcome",
    "has_completed_wizard",
    "has_selected_auto_start_option",
];

/// espanso binds `espansodaemonv2.sock` / `espansoworkerv2.sock` inside the
/// runtime directory; the whole socket path must fit a `sockaddr_un`
/// (104 bytes on macOS, the tightest platform we target). Longest name plus
/// the joining separator:
const SOCKET_NAME_BUDGET: usize = "/espansoworkerv2.sock".len();
const SUN_PATH_MAX: usize = 104;

/// Why [`ManagedDirs::ensure`] refused or failed to build the layout.
#[derive(Debug)]
pub enum ManagedDirsError {
    /// The runtime directory is too deep for espanso's unix sockets: binding
    /// would fail inside the engine with `SUN_LEN` panics. The
    /// host must place the app data root at a shorter path.
    RuntimePathTooLong {
        /// Byte length of the offending runtime directory path.
        len: usize,
        /// Maximum byte length that still leaves room for the socket name.
        max: usize,
    },
    /// Filesystem error while creating the layout (structural info only).
    Io(io::Error),
}

impl std::fmt::Display for ManagedDirsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimePathTooLong { len, max } => write!(
                f,
                "engine runtime path is {len} bytes; sockets require at most {max}"
            ),
            Self::Io(error) => write!(f, "engine directory setup failed: {}", error.kind()),
        }
    }
}

impl std::error::Error for ManagedDirsError {}

impl From<io::Error> for ManagedDirsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The private directory set for one managed engine instance.
///
/// Constructing this performs no I/O; [`ManagedDirs::ensure`] materialises the
/// layout idempotently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedDirs {
    root: PathBuf,
}

impl ManagedDirs {
    /// Derive the engine layout from the host's app data directory.
    pub fn new(app_data_root: impl Into<PathBuf>) -> Self {
        Self {
            root: app_data_root.into().join("engine"),
        }
    }

    /// `ESPANSO_CONFIG_DIR`: holds `config/` and `match/`.
    pub fn config_root(&self) -> PathBuf {
        self.root.join("config")
    }

    /// `ESPANSO_PACKAGE_DIR`.
    pub fn package_dir(&self) -> PathBuf {
        self.root.join("packages")
    }

    /// `ESPANSO_RUNTIME_DIR`: sockets, logs, kvs markers.
    pub fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    /// The managed baseline configuration file.
    pub fn default_config_path(&self) -> PathBuf {
        self.config_root().join("config").join("default.yml")
    }

    /// The generated match file Typvia writes (compile.rs / writer.rs).
    pub fn typvia_match_path(&self) -> PathBuf {
        self.config_root().join("match").join("typvia.yml")
    }

    /// The isolation environment for spawning the engine, in
    /// `(variable, value)` form.
    pub fn env(&self) -> [(&'static str, PathBuf); 3] {
        [
            ("ESPANSO_CONFIG_DIR", self.config_root()),
            ("ESPANSO_PACKAGE_DIR", self.package_dir()),
            ("ESPANSO_RUNTIME_DIR", self.runtime_dir()),
        ]
    }

    /// Create the full layout, (re)write the managed baseline config when it
    /// drifts, and seed the first-run markers. Idempotent: an already-correct
    /// layout is left byte-for-byte untouched, so espanso's config watcher sees
    /// no spurious changes.
    pub fn ensure(&self) -> Result<(), ManagedDirsError> {
        let runtime = self.runtime_dir();
        let runtime_len = runtime.as_os_str().len();
        let max = SUN_PATH_MAX - SOCKET_NAME_BUDGET;
        if runtime_len > max {
            return Err(ManagedDirsError::RuntimePathTooLong {
                len: runtime_len,
                max,
            });
        }

        fs::create_dir_all(self.config_root().join("config"))?;
        fs::create_dir_all(self.config_root().join("match"))?;
        fs::create_dir_all(self.package_dir())?;
        let kvs = runtime.join("kvs");
        fs::create_dir_all(&kvs)?;

        write_if_changed(&self.default_config_path(), DEFAULT_CONFIG)?;
        for marker in KVS_MARKERS {
            write_if_changed(&kvs.join(marker), "true")?;
        }
        Ok(())
    }

    /// Whether the user explicitly turned the engine off. The engine is on by
    /// default — it should just work; only this marker keeps the
    /// launch bootstrap from generating the config and starting it.
    pub fn is_disabled(&self) -> bool {
        self.disabled_marker_path().exists()
    }

    /// Record or clear the explicit off switch.
    pub fn set_disabled(&self, disabled: bool) -> io::Result<()> {
        let marker = self.disabled_marker_path();
        if disabled {
            fs::create_dir_all(&self.root)?;
            fs::write(marker, "off")
        } else {
            match fs::remove_file(marker) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            }
        }
    }

    fn disabled_marker_path(&self) -> PathBuf {
        self.root.join("disabled")
    }
}

/// Atomically write `content` unless the file already holds exactly it.
fn write_if_changed(target: &Path, content: &str) -> io::Result<()> {
    if fs::read_to_string(target).is_ok_and(|current| current == content) {
        return Ok(());
    }
    write_config(target, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs_in(root: &Path) -> ManagedDirs {
        ManagedDirs::new(root)
    }

    #[test]
    fn derives_all_paths_under_the_engine_root() {
        let dirs = ManagedDirs::new("/data/app");
        let root = Path::new("/data/app/engine");
        assert_eq!(dirs.config_root(), root.join("config"));
        assert_eq!(dirs.package_dir(), root.join("packages"));
        assert_eq!(dirs.runtime_dir(), root.join("runtime"));
        assert_eq!(
            dirs.default_config_path(),
            root.join("config/config/default.yml")
        );
        assert_eq!(
            dirs.typvia_match_path(),
            root.join("config/match/typvia.yml")
        );
        for (_, value) in dirs.env() {
            assert!(value.starts_with(root), "isolation escape: {value:?}");
        }
    }

    #[test]
    fn env_carries_the_three_isolation_variables() {
        let dirs = ManagedDirs::new("/data/app");
        let names: Vec<&str> = dirs.env().iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "ESPANSO_CONFIG_DIR",
                "ESPANSO_PACKAGE_DIR",
                "ESPANSO_RUNTIME_DIR"
            ]
        );
    }

    #[test]
    fn ensure_builds_layout_baseline_config_and_markers() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = dirs_in(temp.path());
        dirs.ensure().expect("ensure");

        let config = fs::read_to_string(dirs.default_config_path()).expect("default.yml");
        assert!(config.contains("show_icon: false"));
        assert!(config.contains("show_notifications: false"));
        assert!(config.contains("backend: Clipboard"));
        assert!(dirs.config_root().join("match").is_dir());
        assert!(dirs.package_dir().is_dir());
        for marker in KVS_MARKERS {
            let value = fs::read_to_string(dirs.runtime_dir().join("kvs").join(marker))
                .unwrap_or_else(|_| panic!("marker {marker} missing"));
            assert_eq!(value, "true");
        }
    }

    #[test]
    fn ensure_is_idempotent_and_restores_drifted_baseline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = dirs_in(temp.path());
        dirs.ensure().expect("first ensure");
        fs::write(dirs.default_config_path(), "show_icon: true\n").expect("drift");
        dirs.ensure().expect("second ensure");
        let config = fs::read_to_string(dirs.default_config_path()).expect("default.yml");
        assert_eq!(config, DEFAULT_CONFIG);
        assert!(
            !dirs
                .default_config_path()
                .with_extension("yml.tmp")
                .exists()
        );
    }

    #[test]
    fn ensure_rejects_a_runtime_path_too_deep_for_sockets() {
        let temp = tempfile::tempdir().expect("temp dir");
        let deep = temp.path().join("a".repeat(120));
        let dirs = dirs_in(&deep);
        match dirs.ensure() {
            Err(ManagedDirsError::RuntimePathTooLong { len, max }) => {
                assert!(len > max, "reported lengths must justify the refusal");
            }
            other => panic!("expected RuntimePathTooLong, got {other:?}"),
        }
        assert!(!deep.exists(), "no partial layout on refusal");
    }

    #[test]
    fn engine_is_on_by_default_and_the_off_switch_round_trips() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = dirs_in(temp.path());
        assert!(!dirs.is_disabled(), "default must be on");
        dirs.set_disabled(true).expect("turn off");
        assert!(dirs.is_disabled());
        dirs.set_disabled(false).expect("turn on");
        assert!(!dirs.is_disabled());
        dirs.set_disabled(false).expect("idempotent on");
    }

    #[test]
    fn ensure_leaves_generated_match_file_alone() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dirs = dirs_in(temp.path());
        dirs.ensure().expect("first ensure");
        fs::write(dirs.typvia_match_path(), "matches: []\n").expect("seed match");
        dirs.ensure().expect("second ensure");
        let yaml = fs::read_to_string(dirs.typvia_match_path()).expect("typvia.yml");
        assert_eq!(yaml, "matches: []\n");
    }
}
