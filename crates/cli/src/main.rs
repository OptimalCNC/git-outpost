mod cli;
mod exit;
mod output;
mod reporter_impls;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::{Cli, Command, SourceCommand};
use exit::CliResult;
use outpost_core::{Outpost, OutpostError, SourceRepo, ops};
use reporter_impls::StderrReporter;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => exit::report(err),
    }
}

fn run() -> CliResult<()> {
    let argv: Vec<_> = std::env::args_os().collect();
    let bin = argv
        .first()
        .and_then(|arg| Path::new(arg).file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gop".to_owned());

    let cli = Cli::try_parse_from_with_bin(argv, &bin).unwrap_or_else(|err| err.exit());
    cli.validate_refs()?;
    dispatch(cli)
}

fn dispatch(cli: Cli) -> CliResult<()> {
    let cwd = effective_cwd(cli.cd)?;
    let mut reporter = StderrReporter;

    match cli.command {
        Command::Add(args) => {
            let source = require_source("add", &cwd)?;
            let outpost = ops::add::run(&source, args.to_options(&cwd)?, &mut reporter)?;
            output::print_added(&outpost);
        }
        Command::Pull(_) => {
            let outpost = require_outpost("pull", &cwd)?;
            let report = ops::pull::run(&outpost, ops::pull::PullOptions, &mut reporter)?;
            output::print_pull(&report);
        }
        Command::Source(args) => match args.command {
            SourceCommand::Pull(args) => {
                let outpost = require_outpost("source pull", &cwd)?;
                let report = ops::source::pull(&outpost, args.to_options()?, &mut reporter)?;
                output::print_source_pull(&report);
            }
        },
        Command::Merge(args) => {
            let outpost = require_outpost("merge", &cwd)?;
            let report = ops::merge::run(&outpost, args.to_options()?, &mut reporter)?;
            output::print_merge(&report);
        }
        Command::Rebase(args) => {
            let outpost = require_outpost("rebase", &cwd)?;
            let report = ops::rebase::run(&outpost, args.to_options()?, &mut reporter)?;
            output::print_rebase(&report);
        }
        Command::Push(_) => {
            let outpost = require_outpost("push", &cwd)?;
            let report = ops::push::run(&outpost, ops::push::PushOptions, &mut reporter)?;
            output::print_push(&report);
        }
        Command::List(args) => {
            let source = list_source(&cwd)?;
            let summaries = ops::list::run(&source)?;
            output::print_list(&summaries, args.verbose);
        }
        Command::Lock(args) => {
            let (source, path) = contextual_outpost_path("lock", &cwd, args.outpost_path)?;
            ops::lock::run(
                &source,
                ops::lock::LockOptions {
                    path: path.clone(),
                    reason: args.reason,
                },
            )?;
            println!("locked {}", path.display());
        }
        Command::Unlock(args) => {
            let (source, path) = contextual_outpost_path("unlock", &cwd, args.outpost_path)?;
            ops::unlock::run(&source, ops::unlock::UnlockOptions { path: path.clone() })?;
            println!("unlocked {}", path.display());
        }
        Command::Move(args) => {
            let source = require_source("move", &cwd)?;
            let path = resolve_path_arg(&cwd, args.outpost_path);
            let new_path = resolve_path_arg(&cwd, args.new_path);
            ops::r#move::run(
                &source,
                ops::r#move::MoveOptions {
                    path: path.clone(),
                    new_path: new_path.clone(),
                    force: args.force,
                },
            )?;
            println!("moved {} -> {}", path.display(), new_path.display());
        }
        Command::Remove(args) => {
            let source = require_source("remove", &cwd)?;
            let path = resolve_path_arg(&cwd, args.outpost_path);
            ops::remove::run(
                &source,
                ops::remove::RemoveOptions {
                    path: path.clone(),
                    force: args.force,
                },
            )?;
            println!("removed {}", path.display());
        }
        Command::Prune(args) => {
            let source = require_source("prune", &cwd)?;
            let report = ops::prune::run(
                &source,
                ops::prune::PruneOptions {
                    dry_run: args.dry_run,
                    verbose: args.verbose,
                },
            )?;
            output::print_prune(&report, args.verbose);
        }
        Command::Status(_) => {
            let report = ops::status::run(&cwd)?;
            output::print_status(&report);
        }
    }

    Ok(())
}

fn effective_cwd(cd: Option<PathBuf>) -> CliResult<PathBuf> {
    let current = std::env::current_dir().map_err(|source| OutpostError::IoAt {
        path: PathBuf::from("."),
        source,
    })?;
    let cwd = match cd {
        Some(path) if path.is_absolute() => path,
        Some(path) => current.join(path),
        None => current,
    };
    Ok(cwd)
}

fn resolve_path_arg(cwd: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

enum Context {
    Source(SourceRepo),
    Outpost(Outpost),
}

fn classify(cwd: &Path) -> CliResult<Context> {
    let source = SourceRepo::discover(cwd)?;
    match Outpost::at(source.work_tree()) {
        Ok(outpost) => Ok(Context::Outpost(outpost)),
        Err(OutpostError::NotAnOutpost(_)) => Ok(Context::Source(source)),
        Err(err) => Err(err),
    }
}

fn require_source(command: &'static str, cwd: &Path) -> CliResult<SourceRepo> {
    match classify(cwd)? {
        Context::Source(source) => Ok(source),
        Context::Outpost(_) => Err(OutpostError::WrongContext {
            command,
            expected: "a source repository",
            cwd: cwd.to_path_buf(),
        }),
    }
}

fn require_outpost(command: &'static str, cwd: &Path) -> CliResult<Outpost> {
    match classify(cwd)? {
        Context::Outpost(outpost) => Ok(outpost),
        Context::Source(_) => Err(OutpostError::WrongContext {
            command,
            expected: "a managed outpost",
            cwd: cwd.to_path_buf(),
        }),
    }
}

fn list_source(cwd: &Path) -> CliResult<SourceRepo> {
    match classify(cwd)? {
        Context::Source(source) => Ok(source),
        Context::Outpost(outpost) => outpost.source_repo().map_err(Into::into),
    }
}

fn contextual_outpost_path(
    command: &'static str,
    cwd: &Path,
    path: Option<PathBuf>,
) -> CliResult<(SourceRepo, PathBuf)> {
    match classify(cwd)? {
        Context::Source(source) => match path {
            Some(path) => Ok((source, resolve_path_arg(cwd, path))),
            None => Err(OutpostError::MissingOutpostPath {
                command,
                cwd: cwd.to_path_buf(),
            }),
        },
        Context::Outpost(outpost) => {
            let path = path
                .map(|path| resolve_path_arg(cwd, path))
                .unwrap_or_else(|| outpost.work_tree().to_path_buf());
            Ok((outpost.source_repo()?, path))
        }
    }
}
