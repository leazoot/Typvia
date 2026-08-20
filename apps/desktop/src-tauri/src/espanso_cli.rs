//! Espanso binary resolution for the desktop host.
//!
//! Release bundles carry their own pinned engine next to the app binary and
//! that copy always wins. Dev builds have no bundled sibling and
//! fall through to `PATH`; a Finder-launched .app inherits launchd's minimal
//! `PATH`, which omits the Homebrew prefix, so the well-known install
//! locations come last as the GUI-launch fallback.

use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use typvia_espanso_adapter::{CliOutput, EspansoCli};

/// [`EspansoCli`] backed by the espanso binary at a host-resolved path.
/// Resolution runs once per process (the install location does not move
/// mid-session) and never blocks a summon path — espanso calls happen on
/// settings/home loads and config syncs only.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResolvedEspansoCli;

impl EspansoCli for ResolvedEspansoCli {
    fn run(&self, args: &[&str]) -> io::Result<CliOutput> {
        let output = Command::new(program()).args(args).output()?;
        Ok(CliOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn program() -> &'static PathBuf {
    static PROGRAM: OnceLock<PathBuf> = OnceLock::new();
    PROGRAM.get_or_init(|| {
        choose(
            bundled_candidate(),
            path_lookup_works(),
            &install_candidates(),
        )
    })
}

/// The engine bundled as a nested helper app
/// (`Contents/Helpers/TypviaEngine.app`, injected by
/// scripts/release/macos_package.sh). A helper bundle — not a
/// bare sibling binary — because the engine's Cocoa runtime registers its
/// enclosing bundle with LaunchServices: inside `Contents/MacOS` it would
/// impersonate Typvia itself (second dock icon; `open` refusing to relaunch).
/// In dev builds no helper exists and resolution falls through to `PATH`.
fn bundled_candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(
        exe.parent()?
            .parent()?
            .join("Helpers/TypviaEngine.app/Contents/MacOS/espanso"),
    )
}

/// The resolved binary path for callers that spawn the engine themselves
/// (the managed-engine supervisor). Same one-shot resolution.
pub(crate) fn resolved_program() -> PathBuf {
    program().clone()
}

/// Whether the bare name resolves via the inherited `PATH`.
fn path_lookup_works() -> bool {
    Command::new("espanso")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Install locations to try when `PATH` cannot see espanso: Homebrew on Apple
/// silicon and Intel, then the official .app bundle's embedded binary.
fn install_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin/espanso"),
        PathBuf::from("/usr/local/bin/espanso"),
        PathBuf::from("/Applications/Espanso.app/Contents/MacOS/espanso"),
    ]
}

/// The bundled engine wins over everything — a release bundle must run its
/// pinned, checksum-verified version, never whatever happens to be on the
/// user's `PATH`. Then `PATH` (dev / terminal launches), then the
/// well-known install locations; a bare name last, so a genuinely missing
/// binary still surfaces through the adapter as `NotInstalled` rather than a
/// different error shape.
fn choose(bundled: Option<PathBuf>, path_has_espanso: bool, candidates: &[PathBuf]) -> PathBuf {
    if let Some(bundled) = bundled.filter(|candidate| candidate.is_file()) {
        return bundled;
    }
    if path_has_espanso {
        return PathBuf::from("espanso");
    }
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("espanso"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_engine_wins_over_path_and_install_locations() {
        let dir = tempfile::tempdir().unwrap();
        let bundled = dir.path().join("espanso");
        std::fs::write(&bundled, b"").unwrap();
        assert_eq!(
            choose(
                Some(bundled.clone()),
                true,
                &[PathBuf::from("/nowhere/espanso")]
            ),
            bundled
        );
    }

    #[test]
    fn path_lookup_wins_when_no_bundled_engine_exists() {
        let missing_bundled = PathBuf::from("/nowhere/bundle/espanso");
        assert_eq!(
            choose(
                Some(missing_bundled),
                true,
                &[PathBuf::from("/nowhere/espanso")]
            ),
            PathBuf::from("espanso")
        );
    }

    #[test]
    fn falls_back_to_the_first_existing_install_location() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("espanso");
        std::fs::write(&present, b"").unwrap();
        let missing = dir.path().join("missing/espanso");
        assert_eq!(
            choose(None, false, &[missing.clone(), present.clone()]),
            present
        );
        // Nothing found anywhere: keep the bare name so the adapter reports
        // NotInstalled through its normal path.
        assert_eq!(choose(None, false, &[missing]), PathBuf::from("espanso"));
    }
}
