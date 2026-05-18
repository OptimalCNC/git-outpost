use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use outpost_core::OutpostError;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    Core(OutpostError),
    WrongContext {
        command: &'static str,
        expected: &'static str,
        cwd: PathBuf,
    },
    MissingOutpostPath {
        command: &'static str,
        cwd: PathBuf,
    },
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Core(err) => err.exit_code(),
            Self::WrongContext { .. } | Self::MissingOutpostPath { .. } => 2,
        }
    }
}

impl From<OutpostError> for CliError {
    fn from(err: OutpostError) -> Self {
        Self::Core(err)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(err) => err.fmt(f),
            Self::WrongContext {
                command,
                expected,
                cwd,
            } => write!(
                f,
                "{command} must be run from {expected}; effective cwd is {}",
                cwd.display()
            ),
            Self::MissingOutpostPath { command, cwd } => write!(
                f,
                "{command} requires <outpost> when run from source repository {}",
                cwd.display()
            ),
        }
    }
}

pub fn report(err: CliError) -> ExitCode {
    eprintln!("error: {err}");
    ExitCode::from(err.exit_code())
}
