use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::metadata::RawMetadata;
use crate::outpost::AheadBehind;
use crate::source_repo::{
    SourceRepo, canonicalize_path, invoker_at, is_dirty, read_optional_config,
};
use crate::{BranchName, GitInvoker, OutpostError, OutpostResult, RefName, RemoteName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub outpost_path: PathBuf,
    pub source_path: Option<PathBuf>,
    pub source_present: bool,
    pub remote_name: Option<RemoteName>,
    pub current_branch: Option<BranchName>,
    pub outpost_dirty: bool,
    pub source_ahead_behind_upstream: Option<AheadBehind>,
    pub outpost_ahead_behind_source: Option<AheadBehind>,
    pub problems: Vec<ConfigProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigProblem {
    MissingSourceRepoConfig,
    SourceMissing(PathBuf),
    MissingRemoteNameConfig,
    LocalRemoteMismatch {
        configured: PathBuf,
        actual: PathBuf,
    },
    NoUpstreamTracking {
        branch: BranchName,
    },
    NotInRegistry,
    PushWouldFail {
        branch: BranchName,
    },
}

pub fn run(target_path: &Path) -> OutpostResult<StatusReport> {
    run_with(target_path, &BTreeMap::new())
}

pub fn run_with(
    target_path: &Path,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<StatusReport> {
    let outpost_path = discover_work_tree(target_path, env)?;
    let git = invoker_at(&outpost_path, env);
    let raw = RawMetadata::read(&git)?;

    if raw.managed != Some(true) {
        return Err(OutpostError::NotAnOutpost(outpost_path));
    }

    report_from_raw(outpost_path, raw, &git, env)
}

fn discover_work_tree(
    target_path: &Path,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<PathBuf> {
    let git = invoker_at(target_path, env);
    let work_tree = git
        .run_capture(["rev-parse", "--show-toplevel"])
        .map_err(|err| map_discovery_error(err, target_path))?;
    canonicalize_path(Path::new(&work_tree))
}

fn report_from_raw(
    outpost_path: PathBuf,
    raw: RawMetadata,
    git: &GitInvoker,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<StatusReport> {
    let mut problems = Vec::new();
    let source_path = match raw.source_repo {
        Some(path) => Some(canonicalize_existing_or_missing(&path)?),
        None => {
            problems.push(ConfigProblem::MissingSourceRepoConfig);
            None
        }
    };
    if raw.remote_name.is_none() {
        problems.push(ConfigProblem::MissingRemoteNameConfig);
    }

    let source_present = source_path.as_ref().is_some_and(|path| path.exists());
    if let Some(path) = source_path.as_ref().filter(|_| !source_present) {
        problems.push(ConfigProblem::SourceMissing(path.clone()));
    }
    let remote_name = raw.remote_name;
    let current_branch = current_branch_or_detached(git)?;
    let outpost_dirty = is_dirty(git)?;
    let mut source_ahead_behind_upstream = None;
    let mut outpost_ahead_behind_source = None;

    if let (Some(source_path), Some(remote_name)) = (source_path.as_ref(), remote_name.as_ref()) {
        if source_present {
            check_local_remote(git, &outpost_path, source_path, remote_name, &mut problems)?;
            let source = SourceRepo::at_with(source_path, env)?;
            check_registry(&source, &outpost_path, &mut problems)?;
            if let Some(branch) = current_branch.as_ref() {
                outpost_ahead_behind_source =
                    ahead_behind_outpost_source(git, branch, remote_name, &mut problems)?;
                source_ahead_behind_upstream =
                    ahead_behind_source_upstream(source_path, branch, env, &mut problems)?;
                check_push_would_fail(&source, branch, &mut problems)?;
            }
        }
    }

    Ok(StatusReport {
        outpost_path,
        source_path,
        source_present,
        remote_name,
        current_branch,
        outpost_dirty,
        source_ahead_behind_upstream,
        outpost_ahead_behind_source,
        problems,
    })
}

fn map_discovery_error(err: OutpostError, path: &Path) -> OutpostError {
    match err {
        OutpostError::GitFailed { .. } => OutpostError::NotARepo(path.to_path_buf()),
        other => other,
    }
}

fn canonicalize_existing_or_missing(path: &Path) -> OutpostResult<PathBuf> {
    if path.exists() {
        canonicalize_path(path)
    } else {
        Ok(canonicalize_missing(path))
    }
}

fn canonicalize_missing(path: &Path) -> PathBuf {
    let Some(parent) = path.parent() else {
        return path.to_path_buf();
    };
    match std::fs::canonicalize(parent) {
        Ok(parent) => parent.join(path.file_name().unwrap_or_default()),
        Err(_) => path.to_path_buf(),
    }
}

fn current_branch_or_detached(git: &GitInvoker) -> OutpostResult<Option<BranchName>> {
    match git.run_capture(["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(branch) => BranchName::parse(branch).map(Some),
        Err(OutpostError::GitFailed { code: 1, .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

fn check_local_remote(
    git: &GitInvoker,
    outpost_path: &Path,
    configured: &Path,
    remote_name: &RemoteName,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<()> {
    let actual = match git.run_capture(["remote", "get-url", remote_name.as_str()]) {
        Ok(actual) => canonicalize_remote_path(outpost_path, &actual)?,
        Err(OutpostError::GitFailed { .. }) => return Ok(()),
        Err(err) => return Err(err),
    };

    if actual != configured {
        problems.push(ConfigProblem::LocalRemoteMismatch {
            configured: configured.to_path_buf(),
            actual,
        });
    }

    Ok(())
}

fn check_registry(
    source: &SourceRepo,
    outpost_path: &Path,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<()> {
    if !source
        .registry()?
        .entries()
        .iter()
        .any(|entry| entry.path == outpost_path)
    {
        problems.push(ConfigProblem::NotInRegistry);
    }

    Ok(())
}

fn check_push_would_fail(
    source: &SourceRepo,
    branch: &BranchName,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<()> {
    if read_optional_config(source.git(), "receive.denyCurrentBranch")?.as_deref()
        == Some("updateInstead")
    {
        return Ok(());
    }

    if source
        .checked_out_branches()?
        .iter()
        .any(|checked_out| checked_out == branch)
    {
        problems.push(ConfigProblem::PushWouldFail {
            branch: branch.clone(),
        });
    }

    Ok(())
}

fn ahead_behind_outpost_source(
    git: &GitInvoker,
    branch: &BranchName,
    remote_name: &RemoteName,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<Option<AheadBehind>> {
    let Some(remote_branch) = tracking_branch(git, branch, Some(remote_name), problems)? else {
        return Ok(None);
    };
    let local_ref = format!("refs/heads/{}", branch.as_str());
    let remote_ref = format!("refs/remotes/{}/{remote_branch}", remote_name.as_str());
    ahead_behind_existing_refs(git, &local_ref, &remote_ref)
}

fn ahead_behind_source_upstream(
    source_path: &Path,
    branch: &BranchName,
    env: &BTreeMap<OsString, OsString>,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<Option<AheadBehind>> {
    let source_git = invoker_at(source_path, env);
    let Some(remote_branch) = tracking_branch(&source_git, branch, None, problems)? else {
        return Ok(None);
    };
    let Some(remote_name) = read_branch_remote(&source_git, branch)? else {
        return Ok(None);
    };
    let local_ref = format!("refs/heads/{}", branch.as_str());
    let remote_ref = format!("refs/remotes/{}/{remote_branch}", remote_name.as_str());
    ahead_behind_existing_refs(&source_git, &local_ref, &remote_ref)
}

fn tracking_branch(
    git: &GitInvoker,
    branch: &BranchName,
    expected_remote: Option<&RemoteName>,
    problems: &mut Vec<ConfigProblem>,
) -> OutpostResult<Option<String>> {
    let Some(remote) = read_branch_remote(git, branch)? else {
        push_no_upstream_once(problems, branch);
        return Ok(None);
    };
    if expected_remote.is_some_and(|expected| expected != &remote) {
        push_no_upstream_once(problems, branch);
        return Ok(None);
    }

    let merge_key = format!("branch.{}.merge", branch.as_str());
    let Some(merge_ref) = read_optional_config(git, &merge_key)? else {
        push_no_upstream_once(problems, branch);
        return Ok(None);
    };
    let merge_ref = RefName::parse(merge_ref)?;
    let Some(remote_branch) = merge_ref.as_str().strip_prefix("refs/heads/") else {
        push_no_upstream_once(problems, branch);
        return Ok(None);
    };

    Ok(Some(remote_branch.to_owned()))
}

fn read_branch_remote(git: &GitInvoker, branch: &BranchName) -> OutpostResult<Option<RemoteName>> {
    let remote_key = format!("branch.{}.remote", branch.as_str());
    read_optional_config(git, &remote_key)?
        .map(RemoteName::parse)
        .transpose()
}

fn push_no_upstream_once(problems: &mut Vec<ConfigProblem>, branch: &BranchName) {
    let problem = ConfigProblem::NoUpstreamTracking {
        branch: branch.clone(),
    };
    if !problems.contains(&problem) {
        problems.push(problem);
    }
}

fn ahead_behind_existing_refs(
    git: &GitInvoker,
    local_ref: &str,
    remote_ref: &str,
) -> OutpostResult<Option<AheadBehind>> {
    if !ref_exists(git, local_ref)? || !ref_exists(git, remote_ref)? {
        return Ok(None);
    }

    let range = format!("{local_ref}...{remote_ref}");
    let output = git.run_capture(["rev-list", "--left-right", "--count", &range])?;
    parse_ahead_behind(git.cwd(), &output).map(Some)
}

fn ref_exists(git: &GitInvoker, ref_name: &str) -> OutpostResult<bool> {
    git.run_status(["rev-parse", "--verify", "--quiet", ref_name])
}

fn parse_ahead_behind(repo: &Path, output: &str) -> OutpostResult<AheadBehind> {
    let mut parts = output.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_ahead_behind_output(repo, output))?;
    let behind = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_ahead_behind_output(repo, output))?;
    if parts.next().is_some() {
        return Err(invalid_ahead_behind_output(repo, output));
    }

    Ok(AheadBehind { ahead, behind })
}

fn invalid_ahead_behind_output(repo: &Path, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: repo.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected rev-list output: {output}"),
        ),
    }
}

fn canonicalize_remote_path(outpost_path: &Path, value: &str) -> OutpostResult<PathBuf> {
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        outpost_path.join(path)
    };
    canonicalize_existing_or_missing(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_from_raw_records_missing_metadata_problems() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init repo");

        let report = report_from_raw(
            temp.path().to_path_buf(),
            RawMetadata {
                managed: Some(true),
                source_repo: None,
                remote_name: None,
            },
            &crate::GitInvoker::at(temp.path()),
            &BTreeMap::new(),
        )
        .expect("report");

        assert_eq!(report.source_path, None);
        assert!(!report.source_present);
        assert_eq!(report.remote_name, None);
        assert_eq!(
            report.problems,
            vec![
                ConfigProblem::MissingSourceRepoConfig,
                ConfigProblem::MissingRemoteNameConfig,
            ]
        );
    }
}
