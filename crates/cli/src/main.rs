mod cli;

use std::path::Path;
use std::process::ExitCode;

use cli::Cli;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(err.exit_code())
        }
    }
}

fn run() -> outpost_core::OutpostResult<()> {
    let argv: Vec<_> = std::env::args_os().collect();
    let bin = argv
        .first()
        .and_then(|arg| Path::new(arg).file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gop".to_owned());

    let cli = Cli::try_parse_from_with_bin(argv, &bin).unwrap_or_else(|err| err.exit());
    cli.validate_refs()?;

    Ok(())
}
