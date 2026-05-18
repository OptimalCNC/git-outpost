use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use outpost_core::{BranchName, OutpostResult, RemoteName, SourceRemoteRef};

const ROOT_AFTER_HELP: &str =
    "Command-specific long flags: --remote-name, --reason, --verbose, --force, --dry-run";

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Manage self-contained Git outposts.",
    disable_help_subcommand = true,
    subcommand_required = true,
    after_help = ROOT_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Run as if Git Outpost was started in <path>.
    #[arg(short = 'C', global = true, value_name = "PATH")]
    pub cd: Option<PathBuf>,

    /// Disable colored output. Also honors NO_COLOR.
    #[arg(long, global = true)]
    pub no_color: bool,
}

impl Cli {
    pub fn try_parse_from_with_bin<I, T>(args: I, bin: &str) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let matches = Self::command()
            .bin_name(bin.to_owned())
            .try_get_matches_from(args)?;

        Self::from_arg_matches(&matches)
    }

    pub fn validate_refs(&self) -> OutpostResult<()> {
        self.command.validate_refs()
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a self-contained outpost.
    Add(AddArgs),

    /// Keep the current outpost branch current.
    Pull(PullArgs),

    /// Operate on source repository branches.
    Source(SourceArgs),

    /// Merge a source remote ref into the current outpost branch.
    Merge(MergeArgs),

    /// Rebase the current outpost branch onto a source remote ref.
    Rebase(RebaseArgs),

    /// Push the current outpost branch through the source repository.
    Push(PushArgs),

    /// List registered outposts.
    List(ListArgs),

    /// Lock a managed outpost.
    Lock(LockArgs),

    /// Unlock a managed outpost.
    Unlock(UnlockArgs),

    /// Move a managed outpost directory.
    Move(MoveArgs),

    /// Remove a managed outpost.
    Remove(RemoveArgs),

    /// Prune stale registry entries.
    Prune(PruneArgs),

    /// Summarize the current managed outpost.
    Status(StatusArgs),
}

impl Command {
    fn validate_refs(&self) -> OutpostResult<()> {
        match self {
            Command::Add(args) => args.validate_refs(),
            Command::Source(args) => args.validate_refs(),
            Command::Merge(args) => args.validate_refs(),
            Command::Rebase(args) => args.validate_refs(),
            Command::Pull(_)
            | Command::Push(_)
            | Command::List(_)
            | Command::Lock(_)
            | Command::Unlock(_)
            | Command::Move(_)
            | Command::Remove(_)
            | Command::Prune(_)
            | Command::Status(_) => Ok(()),
        }
    }
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Optional existing source branch or -b target branch.
    #[arg(value_name = "TARGET-BRANCH")]
    pub target_branch: Option<String>,

    /// Create a new source branch from <target-branch>.
    #[arg(short = 'b', value_name = "NEW-BRANCH")]
    pub new_branch: Option<String>,

    /// Remote name for the source inside the outpost.
    #[arg(long, default_value = "local", value_name = "NAME")]
    pub remote_name: String,
}

impl AddArgs {
    fn validate_refs(&self) -> OutpostResult<()> {
        if let Some(new_branch) = &self.new_branch {
            BranchName::parse(new_branch.clone())?;
        }
        if let Some(target_branch) = &self.target_branch {
            BranchName::parse(target_branch.clone())?;
        }
        RemoteName::parse(self.remote_name.clone())?;
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Include lock reasons and extended annotations.
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct LockArgs {
    #[arg(long, value_name = "STRING")]
    pub reason: Option<String>,

    #[arg(value_name = "OUTPOST")]
    pub outpost_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct UnlockArgs {
    #[arg(value_name = "OUTPOST")]
    pub outpost_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MoveArgs {
    #[arg(value_name = "OUTPOST")]
    pub outpost_path: PathBuf,

    #[arg(value_name = "NEW-PATH")]
    pub new_path: PathBuf,

    /// Ignore dirty-tree and lock guards.
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    #[arg(value_name = "OUTPOST")]
    pub outpost_path: PathBuf,

    /// Ignore dirty, unpushed, and lock guards.
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PruneArgs {
    /// Report actions without modifying the registry.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Print each pruned registry entry.
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct StatusArgs;

#[derive(Debug, Args)]
pub struct PullArgs;

#[derive(Debug, Args)]
pub struct PushArgs;

#[derive(Debug, Args)]
pub struct MergeArgs {
    #[arg(value_name = "SOURCE-REF")]
    pub source_ref: String,
}

impl MergeArgs {
    fn validate_refs(&self) -> OutpostResult<()> {
        SourceRemoteRef::parse(self.source_ref.clone()).map(|_| ())
    }
}

#[derive(Debug, Args)]
pub struct RebaseArgs {
    #[arg(value_name = "SOURCE-REF")]
    pub source_ref: String,
}

impl RebaseArgs {
    fn validate_refs(&self) -> OutpostResult<()> {
        SourceRemoteRef::parse(self.source_ref.clone()).map(|_| ())
    }
}

#[derive(Debug, Args)]
pub struct SourceArgs {
    #[command(subcommand)]
    pub command: SourceCommand,
}

impl SourceArgs {
    fn validate_refs(&self) -> OutpostResult<()> {
        self.command.validate_refs()
    }
}

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Refresh a source branch from origin.
    Pull(SourcePullArgs),
}

impl SourceCommand {
    fn validate_refs(&self) -> OutpostResult<()> {
        match self {
            SourceCommand::Pull(args) => args.validate_refs(),
        }
    }
}

#[derive(Debug, Args)]
pub struct SourcePullArgs {
    #[arg(value_name = "SOURCE-BRANCH")]
    pub source_branch: String,
}

impl SourcePullArgs {
    fn validate_refs(&self) -> OutpostResult<()> {
        BranchName::parse(self.source_branch.clone()).map(|_| ())
    }
}
