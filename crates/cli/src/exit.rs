use std::process::ExitCode;

use outpost_core::{OutpostError, OutpostResult};

pub type CliResult<T> = OutpostResult<T>;

pub fn report(err: OutpostError) -> ExitCode {
    eprintln!("error: {err}");
    ExitCode::from(err.exit_code())
}
