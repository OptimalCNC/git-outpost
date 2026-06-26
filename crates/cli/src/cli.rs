use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use outpost_core::ops;
use outpost_core::{BranchName, ConfigKey, OutpostResult, RemoteName, SourceRemoteRef, SourceRepo};

const ROOT_AFTER_HELP: &str = "Command-specific long flags: --remote-name, --reason, --verbose, --force, --no-branch-cleanup, --dry-run";

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

    /// Analyze a managed outpost and related branch state.
    Analyze(AnalyzeArgs),

    /// Read and write source repository configuration.
    Config(ConfigArgs),
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
            | Command::Status(_)
            | Command::Analyze(_)
            | Command::Config(_) => Ok(()),
        }
    }
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(value_name = "PATH|NAME", required_unless_present = "new_branch")]
    pub path: Option<PathBuf>,

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

    pub fn to_options(
        &self,
        cwd: &Path,
        source: &SourceRepo,
    ) -> OutpostResult<ops::add::AddOptions> {
        let new_branch = self.new_branch.clone().map(BranchName::parse).transpose()?;
        let target_branch = self
            .target_branch
            .clone()
            .map(BranchName::parse)
            .transpose()?;
        let destination_arg = match (&self.path, &new_branch) {
            (Some(path), _) => path.clone(),
            (None, Some(new_branch)) => derived_outpost_name(new_branch),
            (None, None) => unreachable!("clap requires <path-or-name> when -b is absent"),
        };
        let checkout = match new_branch {
            Some(name) => ops::add::AddCheckout::NewBranch {
                name,
                target_branch,
            },
            None => ops::add::AddCheckout::CheckoutExisting { target_branch },
        };

        Ok(ops::add::AddOptions {
            destination: source.resolve_outpost_destination(cwd, &destination_arg)?,
            checkout,
            remote_name: RemoteName::parse(self.remote_name.clone())?,
        })
    }
}

fn derived_outpost_name(branch: &BranchName) -> PathBuf {
    PathBuf::from(
        branch
            .as_str()
            .rsplit('/')
            .next()
            .expect("branch names are non-empty"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_outpost_name_uses_final_slash_component() {
        let cases = [
            ("feature/foo", "foo"),
            ("users/alice/fix-1", "fix-1"),
            ("foo", "foo"),
        ];

        for (branch, expected) in cases {
            let branch = BranchName::parse(branch).expect("valid branch");
            assert_eq!(derived_outpost_name(&branch), PathBuf::from(expected));
        }
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

    /// Do not prompt to delete a safely merged source or upstream branch.
    #[arg(long)]
    pub no_branch_cleanup: bool,
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
pub struct AnalyzeArgs {
    #[arg(value_name = "OUTPOST")]
    pub outpost_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[command(after_help = "Config keys: outpost-container")]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Store a source repository config value.
    Set(ConfigSetArgs),

    /// Print a source repository config value.
    Get(ConfigGetArgs),

    /// Remove a source repository config value.
    Unset(ConfigUnsetArgs),

    /// Print set source repository config values.
    List,

    /// Print source repository config storage and all known keys.
    Show,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    #[arg(value_parser = parse_config_key)]
    pub key: ConfigKey,

    #[arg(value_name = "PATH")]
    pub value: PathBuf,
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    #[arg(value_parser = parse_config_key)]
    pub key: ConfigKey,
}

#[derive(Debug, Args)]
pub struct ConfigUnsetArgs {
    #[arg(value_parser = parse_config_key)]
    pub key: ConfigKey,
}

fn parse_config_key(value: &str) -> Result<ConfigKey, String> {
    ConfigKey::parse(value).map_err(|err| err.to_string())
}

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

    pub fn to_options(&self) -> OutpostResult<ops::merge::MergeOptions> {
        Ok(ops::merge::MergeOptions {
            source_ref: SourceRemoteRef::parse(self.source_ref.clone())?,
        })
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

    pub fn to_options(&self) -> OutpostResult<ops::rebase::RebaseOptions> {
        Ok(ops::rebase::RebaseOptions {
            source_ref: SourceRemoteRef::parse(self.source_ref.clone())?,
        })
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

    pub fn to_options(&self) -> OutpostResult<ops::source::SourcePullOptions> {
        Ok(ops::source::SourcePullOptions {
            branch: BranchName::parse(self.source_branch.clone())?,
        })
    }
}
