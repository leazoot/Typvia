// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Managed engine lifecycle: spawn `espanso daemon` as a direct child,
//! supervise it, and never leave orphans behind.
//!
//! The engine is spawned directly (never through `espanso service register`):
//! on macOS the TCC accessibility attribution of a direct child chain belongs
//! to the parent app. The daemon owns config watching and worker restarts;
//! this supervisor owns the daemon:
//!
//! - deliberate stop: SIGTERM to the recorded pid, escalation after a grace
//!   period — never `espanso stop`, whose force fallback kills every espanso
//!   process on the machine including a user-owned install;
//! - crash: bounded backoff restarts, then a degraded terminal state;
//! - host crash: the daemon survives its parent, so every start
//!   first clears a still-live pid recorded in the previous run's pid file,
//!   verifying the process is actually espanso before signalling (pid reuse).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::managed::{ManagedDirs, ManagedDirsError};

/// Pid of the current engine daemon, kept inside the private runtime dir so a
/// later run can clean up after a host crash.
const PID_FILE: &str = "typvia-engine.pid";

/// Restart delays after an engine crash; once exhausted the supervisor stops
/// trying and reports [`EngineState::Failed`] (the status surface presents
/// this as the degraded mode).
const DEFAULT_BACKOFF: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// How long a deliberate stop waits after SIGTERM before escalating.
const DEFAULT_STOP_GRACE: Duration = Duration::from_secs(3);

/// Supervisor-visible engine state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineState {
    /// Never started in this session.
    Idle,
    /// Engine daemon alive.
    Running {
        /// Daemon process id.
        pid: u32,
    },
    /// Engine exited unexpectedly; a restart is pending.
    Retrying {
        /// 1-based restart attempt about to run.
        attempt: u32,
    },
    /// Restart budget exhausted — degraded mode until the next explicit start.
    Failed,
    /// Deliberately stopped.
    Stopped,
}

/// Why the engine could not be started.
#[derive(Debug)]
pub enum EngineError {
    /// The private directory layout could not be prepared.
    Dirs(ManagedDirsError),
    /// The engine binary could not be spawned (structural kind only).
    Spawn(io::ErrorKind),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dirs(error) => write!(f, "{error}"),
            Self::Spawn(kind) => write!(f, "engine spawn failed: {kind}"),
        }
    }
}

impl std::error::Error for EngineError {}

struct Inner {
    state: EngineState,
    /// Bumped by every deliberate start/stop; a monitor thread that observes a
    /// different generation than its own stands down instead of restarting.
    generation: u64,
    child_pid: Option<u32>,
}

/// Supervises one managed engine daemon as a direct child process.
pub struct EngineSupervisor {
    dirs: ManagedDirs,
    binary: PathBuf,
    backoff: Vec<Duration>,
    stop_grace: Duration,
    inner: Arc<Mutex<Inner>>,
}

impl EngineSupervisor {
    /// Build a supervisor for the engine at `binary` over the private `dirs`.
    pub fn new(dirs: ManagedDirs, binary: impl Into<PathBuf>) -> Self {
        Self {
            dirs,
            binary: binary.into(),
            backoff: DEFAULT_BACKOFF.to_vec(),
            stop_grace: DEFAULT_STOP_GRACE,
            inner: Arc::new(Mutex::new(Inner {
                state: EngineState::Idle,
                generation: 0,
                child_pid: None,
            })),
        }
    }

    /// Override restart/stop timing (tests drive crashes without real waits).
    pub fn with_timing(mut self, backoff: &[Duration], stop_grace: Duration) -> Self {
        self.backoff = backoff.to_vec();
        self.stop_grace = stop_grace;
        self
    }

    /// The private directory set this engine runs over.
    pub fn dirs(&self) -> &ManagedDirs {
        &self.dirs
    }

    /// Current state snapshot.
    pub fn state(&self) -> EngineState {
        self.lock().state.clone()
    }

    /// Start the engine unless it is already running or mid-retry.
    pub fn ensure_running(&self) -> Result<(), EngineError> {
        match self.state() {
            EngineState::Running { .. } | EngineState::Retrying { .. } => Ok(()),
            EngineState::Idle | EngineState::Failed | EngineState::Stopped => self.start(),
        }
    }

    /// Prepare directories, clear any residue of a crashed previous run, spawn
    /// the daemon and begin supervising it.
    pub fn start(&self) -> Result<(), EngineError> {
        self.dirs.ensure().map_err(EngineError::Dirs)?;
        let pid_file = self.dirs.runtime_dir().join(PID_FILE);
        clear_residual_engine(&pid_file, self.stop_grace);

        let mut inner = self.lock();
        inner.generation += 1;
        let generation = inner.generation;
        let child = self
            .spawn_engine()
            .map_err(|error| EngineError::Spawn(error.kind()))?;
        let pid = child.id();
        inner.state = EngineState::Running { pid };
        inner.child_pid = Some(pid);
        drop(inner);
        let _ = fs::write(&pid_file, pid.to_string());

        self.supervise(child, generation, pid_file);
        Ok(())
    }

    /// Deliberately stop the engine; suppresses any pending restart.
    pub fn stop(&self) {
        let mut inner = self.lock();
        inner.generation += 1;
        let pid = inner.child_pid.take();
        inner.state = EngineState::Stopped;
        drop(inner);
        if let Some(pid) = pid {
            terminate_with_grace(pid, self.stop_grace);
        }
        let _ = fs::remove_file(self.dirs.runtime_dir().join(PID_FILE));
    }

    fn spawn_engine(&self) -> io::Result<Child> {
        let mut command = Command::new(&self.binary);
        command
            .arg("daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (name, value) in self.dirs.env() {
            command.env(name, value);
        }
        command.spawn()
    }

    /// Reap the child and restart it with bounded backoff while its generation
    /// stays current.
    fn supervise(&self, mut child: Child, generation: u64, pid_file: PathBuf) {
        let inner = Arc::clone(&self.inner);
        let backoff = self.backoff.clone();
        let dirs = self.dirs.clone();
        let binary = self.binary.clone();
        thread::spawn(move || {
            let mut attempt = 0usize;
            loop {
                let _ = child.wait();
                {
                    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.generation != generation {
                        return; // deliberate stop or a newer start owns the engine
                    }
                    guard.child_pid = None;
                    if attempt >= backoff.len() {
                        guard.state = EngineState::Failed;
                        let _ = fs::remove_file(&pid_file);
                        return;
                    }
                    guard.state = EngineState::Retrying {
                        attempt: attempt as u32 + 1,
                    };
                }
                thread::sleep(backoff[attempt]);
                attempt += 1;
                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                if guard.generation != generation {
                    return;
                }
                let mut command = Command::new(&binary);
                command
                    .arg("daemon")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                for (name, value) in dirs.env() {
                    command.env(name, value);
                }
                match command.spawn() {
                    Ok(next) => {
                        let pid = next.id();
                        guard.state = EngineState::Running { pid };
                        guard.child_pid = Some(pid);
                        drop(guard);
                        let _ = fs::write(&pid_file, pid.to_string());
                        child = next;
                    }
                    Err(_) => {
                        guard.state = EngineState::Failed;
                        let _ = fs::remove_file(&pid_file);
                        return;
                    }
                }
            }
        });
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for EngineSupervisor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Terminate a still-live engine recorded by a previous (crashed) run. The pid
/// is only signalled when its command line actually names espanso, so a
/// recycled pid can never make Typvia kill an unrelated process. The pid file
/// is removed in every case.
fn clear_residual_engine(pid_file: &Path, grace: Duration) {
    if let Some(pid) = fs::read_to_string(pid_file)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        && process_is_alive(pid)
        && process_command_names_espanso(pid)
    {
        terminate_with_grace(pid, grace);
    }
    let _ = fs::remove_file(pid_file);
}

/// SIGTERM, wait up to `grace`, then SIGKILL. The espanso worker notices the
/// daemon's death on its own and exits (its `--monitor-daemon` design), so
/// signalling the daemon tears down the whole chain.
fn terminate_with_grace(pid: u32, grace: Duration) {
    signal_process(pid, TERM);
    let deadline = Instant::now() + grace;
    while process_is_alive(pid) {
        if Instant::now() >= deadline {
            signal_process(pid, KILL);
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// `ps -o command= -p <pid>` names espanso. Used only as a pid-reuse guard for
/// residual cleanup; `ps` is POSIX and this never runs on a hot path.
fn process_command_names_espanso(pid: u32) -> bool {
    Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains("espanso"))
}

#[cfg(unix)]
const TERM: i32 = libc::SIGTERM;
#[cfg(unix)]
const KILL: i32 = libc::SIGKILL;

#[cfg(unix)]
fn signal_process(pid: u32, signal: i32) {
    // SAFETY: plain kill(2) on a pid this module recorded itself; the espanso
    // name check above guards residual pids against reuse.
    unsafe {
        libc::kill(pid as libc::pid_t, signal);
    }
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    // SAFETY: signal 0 probes existence without delivering anything.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
const TERM: i32 = 0;
#[cfg(not(unix))]
const KILL: i32 = 0;

#[cfg(not(unix))]
fn signal_process(_pid: u32, _signal: i32) {
    // Windows engine management is not implemented yet; until then the
    // supervisor compiles but cannot signal.
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    /// A stub engine script named `espanso` (so the residual-cleanup name guard
    /// matches it) whose behaviour is the given shell body.
    fn stub_engine(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("espanso");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write stub");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    fn fast(supervisor: EngineSupervisor) -> EngineSupervisor {
        supervisor.with_timing(
            &[Duration::from_millis(10), Duration::from_millis(10)],
            Duration::from_millis(300),
        )
    }

    fn wait_until(mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "condition not reached in time");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn start_runs_the_engine_and_stop_terminates_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let binary = stub_engine(temp.path(), "sleep 30");
        let supervisor = fast(EngineSupervisor::new(ManagedDirs::new(temp.path()), binary));
        supervisor.start().expect("start");

        let pid = match supervisor.state() {
            EngineState::Running { pid } => pid,
            other => panic!("expected Running, got {other:?}"),
        };
        let pid_file = supervisor.dirs().runtime_dir().join(PID_FILE);
        assert_eq!(
            fs::read_to_string(&pid_file).expect("pid file").trim(),
            pid.to_string()
        );

        supervisor.stop();
        assert_eq!(supervisor.state(), EngineState::Stopped);
        wait_until(|| !process_is_alive(pid));
        assert!(!pid_file.exists());
    }

    #[test]
    fn crashing_engine_retries_then_degrades_to_failed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let binary = stub_engine(temp.path(), "exit 1");
        let supervisor = fast(EngineSupervisor::new(ManagedDirs::new(temp.path()), binary));
        supervisor.start().expect("start");
        wait_until(|| supervisor.state() == EngineState::Failed);
        assert!(!supervisor.dirs().runtime_dir().join(PID_FILE).exists());
    }

    #[test]
    fn deliberate_stop_suppresses_the_restart_loop() {
        let temp = tempfile::tempdir().expect("temp dir");
        let binary = stub_engine(temp.path(), "sleep 30");
        let supervisor = fast(EngineSupervisor::new(ManagedDirs::new(temp.path()), binary));
        supervisor.start().expect("start");
        supervisor.stop();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(supervisor.state(), EngineState::Stopped);
    }

    #[test]
    fn start_clears_a_residual_engine_from_a_previous_run() {
        let temp = tempfile::tempdir().expect("temp dir");
        let binary = stub_engine(temp.path(), "sleep 30");
        let dirs = ManagedDirs::new(temp.path());
        dirs.ensure().expect("layout");

        // A "previous run": an engine process nothing supervises any more.
        // (In production the pid belongs to a dead parent; here it is a child
        // of the test process, so it must be reaped before liveness asserts —
        // kill(pid, 0) still "sees" an unreaped zombie.)
        let mut orphan = Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("orphan");
        let orphan_pid = orphan.id();
        fs::write(dirs.runtime_dir().join(PID_FILE), orphan_pid.to_string()).expect("pid file");

        let supervisor = fast(EngineSupervisor::new(dirs, binary));
        supervisor.start().expect("start");
        let status = orphan.wait().expect("orphan reaped");
        assert!(
            !status.success(),
            "residual engine must have been terminated, not exited cleanly"
        );
        assert!(!process_is_alive(orphan_pid));
        match supervisor.state() {
            EngineState::Running { pid } => assert_ne!(pid, orphan_pid),
            other => panic!("expected Running, got {other:?}"),
        }
        supervisor.stop();
    }

    #[test]
    fn residual_cleanup_refuses_to_kill_a_non_espanso_pid() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut bystander = Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("bystander");
        let pid_file = temp.path().join(PID_FILE);
        fs::write(&pid_file, bystander.id().to_string()).expect("pid file");

        clear_residual_engine(&pid_file, Duration::from_millis(100));

        assert!(
            process_is_alive(bystander.id()),
            "an unrelated process must never be signalled"
        );
        assert!(!pid_file.exists(), "stale pid file is still discarded");
        bystander.kill().expect("cleanup");
        let _ = bystander.wait();
    }
}
