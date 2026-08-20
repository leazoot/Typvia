//! Adapter error type.
//!
//! Messages carry no snippet content — this crate never handles snippet bodies,
//! only structural information about CLI interaction.

use std::fmt;
use std::io;

/// Failures surfaced by adapter operations.
#[derive(Debug)]
pub enum EspansoError {
    /// The espanso binary could not be launched (not installed / not on `PATH`).
    NotInstalled,
    /// The CLI ran but reported failure (non-zero exit); carries the exit code.
    ///
    /// e.g. `espanso path` before the config directory exists panics inside the
    /// espanso process; the adapter observes that as a
    /// non-zero exit here rather than propagating a panic.
    Cli { code: Option<i32> },
    /// The CLI output could not be parsed into the expected shape.
    Unparseable,
    /// Underlying process I/O failure other than a missing binary.
    Io(io::Error),
}

impl EspansoError {
    /// Map a launch-time I/O error: a missing binary means espanso is not
    /// installed; anything else is a genuine I/O failure.
    pub(crate) fn from_io(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::NotInstalled
        } else {
            Self::Io(error)
        }
    }
}

impl fmt::Display for EspansoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => f.write_str("espanso is not installed"),
            Self::Cli { code: Some(code) } => write!(f, "espanso CLI exited with code {code}"),
            Self::Cli { code: None } => f.write_str("espanso CLI was terminated by a signal"),
            Self::Unparseable => f.write_str("espanso CLI output could not be parsed"),
            Self::Io(error) => write!(f, "espanso CLI I/O error: {error}"),
        }
    }
}

impl std::error::Error for EspansoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
