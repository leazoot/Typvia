//! Adapter status/paths behavior against fixed sample CLI output.
//!
//! The samples mirror real `espanso 2.4.0` output captured from a live
//! install, so the offline tests track actual behavior.

use std::io;

use typvia_espanso_adapter::{CliOutput, EspansoAdapter, EspansoCli, EspansoError, EspansoStatus};

/// [`EspansoCli`] whose behavior is supplied by a closure over the arguments,
/// so each test scripts exactly the responses it needs.
struct FakeCli<F>(F);

impl<F: Fn(&[&str]) -> io::Result<CliOutput>> EspansoCli for FakeCli<F> {
    fn run(&self, args: &[&str]) -> io::Result<CliOutput> {
        (self.0)(args)
    }
}

fn ok(code: i32, stdout: &str) -> io::Result<CliOutput> {
    Ok(CliOutput {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: String::new(),
    })
}

#[test]
fn reports_not_installed_when_binary_is_missing() {
    let adapter = EspansoAdapter::new(FakeCli(|_args: &[&str]| {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }));
    assert_eq!(adapter.status(), EspansoStatus::NotInstalled);
}

#[test]
fn reports_running_when_service_status_exits_zero() {
    let adapter = EspansoAdapter::new(FakeCli(|args: &[&str]| match args {
        ["--version"] => ok(0, "2.4.0\n"),
        ["service", "status"] => ok(0, "espanso is running\n"),
        other => panic!("unexpected args: {other:?}"),
    }));
    match adapter.status() {
        EspansoStatus::Running { version } => assert_eq!(version.triple(), Some((2, 4, 0))),
        other => panic!("expected Running, got {other:?}"),
    }
}

#[test]
fn reports_stopped_when_service_is_not_running() {
    // Mirrors the real sample: `service status` exits 4 with "espanso is not
    // running". This state also covers "installed but not authorized" on macOS.
    let adapter = EspansoAdapter::new(FakeCli(|args: &[&str]| match args {
        ["--version"] => ok(0, "2.4.0\n"),
        ["service", "status"] => ok(4, "espanso is not running\n"),
        other => panic!("unexpected args: {other:?}"),
    }));
    match adapter.status() {
        EspansoStatus::Stopped { version } => assert_eq!(version.raw(), "2.4.0"),
        other => panic!("expected Stopped, got {other:?}"),
    }
}

#[test]
fn treats_service_status_launch_failure_as_stopped() {
    let adapter = EspansoAdapter::new(FakeCli(|args: &[&str]| match args {
        ["--version"] => ok(0, "2.4.0\n"),
        ["service", "status"] => Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        other => panic!("unexpected args: {other:?}"),
    }));
    assert!(matches!(adapter.status(), EspansoStatus::Stopped { .. }));
}

#[test]
fn resolves_paths_from_path_output() {
    let stdout = "Config: /Users/x/Library/Application Support/espanso\n\
         Packages: /Users/x/Library/Application Support/espanso/match/packages\n\
         Runtime: /Users/x/Library/Caches/espanso\n";
    let adapter = EspansoAdapter::new(FakeCli(move |args: &[&str]| match args {
        ["path"] => ok(0, stdout),
        other => panic!("unexpected args: {other:?}"),
    }));
    let paths = adapter.paths().expect("paths resolve");
    assert!(paths.config.is_absolute());
    assert!(paths.config.ends_with("espanso"));
}

#[test]
fn paths_returns_error_without_panicking_when_config_dir_missing() {
    // `espanso path` panics inside the espanso process when
    // the config directory is absent. The adapter must observe that as a
    // non-zero exit and return an error — never propagate a panic.
    let adapter = EspansoAdapter::new(FakeCli(|args: &[&str]| match args {
        ["path"] => Ok(CliOutput {
            code: Some(101),
            stdout: String::new(),
            stderr: "thread 'main' panicked at 'missing config directory'\n".to_string(),
        }),
        other => panic!("unexpected args: {other:?}"),
    }));
    match adapter.paths() {
        Err(EspansoError::Cli { code }) => assert_eq!(code, Some(101)),
        other => panic!("expected Cli error, got {other:?}"),
    }
}

#[test]
fn paths_returns_not_installed_when_binary_is_missing() {
    let adapter = EspansoAdapter::new(FakeCli(|_args: &[&str]| {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }));
    assert!(matches!(adapter.paths(), Err(EspansoError::NotInstalled)));
}
