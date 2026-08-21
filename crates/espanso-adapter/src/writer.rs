// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Atomic write of the generated espanso config.
//!
//! Failure preserves the old config: a generation or write failure must never
//! leave espanso with a half-written config. The caller compiles first —
//! a [`crate::CompileError`] means it never calls here —
//! and this writer stages the bytes in a temporary sibling then renames over the
//! target, so an interrupted write leaves the previous file intact.
//!
//! The target is a single file inside espanso's own `match/` directory
//! (`match/typvia.yml`, see [`crate::EspansoPaths::typvia_config_path`]); Typvia
//! writes only this one file and never touches the rest of the user's espanso
//! setup, which keeps the two installs isolated.

use std::fs;
use std::io;
use std::path::Path;

/// Atomically write `yaml` to `target`, replacing any existing file.
///
/// The parent directory is created if absent. Bytes are staged in a sibling
/// `<name>.tmp` and renamed into place (atomic on the same filesystem); on any
/// failure the temp file is removed and the previous file is left untouched.
pub fn write_config(target: &Path, yaml: &str) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "config target has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let temp = temp_sibling(target)?;
    if let Err(error) = fs::write(&temp, yaml) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

/// `foo.yml` → `foo.yml.tmp`, kept in the same directory so the rename is atomic.
fn temp_sibling(target: &Path) -> io::Result<std::path::PathBuf> {
    let name = target.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "config target has no file name",
        )
    })?;
    let mut temp_name = name.to_os_string();
    temp_name.push(".tmp");
    Ok(target.with_file_name(temp_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_config_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("typvia.yml");
        write_config(&target, "matches: []\n").expect("write");
        assert_eq!(fs::read_to_string(&target).expect("read"), "matches: []\n");
        assert!(!dir.path().join("typvia.yml.tmp").exists());
    }

    #[test]
    fn creates_missing_parent_directory() {
        let root = tempfile::tempdir().expect("temp dir");
        let target = root.path().join("match").join("typvia.yml");
        write_config(&target, "matches: []\n").expect("write");
        assert!(target.exists());
    }

    #[test]
    fn overwrites_previous_config_in_place() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("typvia.yml");
        write_config(&target, "# OLD\n").expect("first write");
        write_config(&target, "# NEW\n").expect("second write");
        assert_eq!(fs::read_to_string(&target).expect("read"), "# NEW\n");
    }
}
