use std::path::PathBuf;
use std::process::ExitCode;

use outpost_core::OutpostError;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    Outpost(OutpostError),
    ShellCdRequiresIntegration { outpost: Option<PathBuf> },
}

impl From<OutpostError> for CliError {
    fn from(value: OutpostError) -> Self {
        Self::Outpost(value)
    }
}

pub fn report(err: CliError) -> ExitCode {
    match err {
        CliError::Outpost(err) => {
            eprintln!("error: {err}");
            ExitCode::from(err.exit_code())
        }
        CliError::ShellCdRequiresIntegration { outpost } => {
            eprintln!("error: `gop cd` is provided by shell integration");
            eprintln!();
            eprintln!("A binary command cannot change your current shell directory.");
            if let Some(outpost) = outpost {
                eprintln!("Requested target: {}", outpost.display());
            }
            eprintln!();
            eprintln!("For persistent setup, run one of:");
            eprintln!("  gop shell install bash");
            eprintln!("  gop shell install zsh");
            eprintln!();
            eprintln!("For the current shell only, run one of:");
            eprintln!("  eval \"$(gop shell init bash)\"");
            eprintln!("  eval \"$(gop shell init zsh)\"");
            ExitCode::from(2)
        }
    }
}
