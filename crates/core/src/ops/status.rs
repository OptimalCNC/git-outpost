use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::metadata::RawMetadata;
use crate::outpost::AheadBehind;
use crate::source_repo::{canonicalize_path, invoker_at, is_dirty};
use crate::{BranchName, OutpostError, OutpostResult, RemoteName};

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

    report_from_raw(outpost_path, raw, &git)
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
    git: &crate::GitInvoker,
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

    Ok(StatusReport {
        outpost_path,
        source_path,
        source_present,
        remote_name: raw.remote_name,
        current_branch: current_branch_or_detached(git)?,
        outpost_dirty: is_dirty(git)?,
        source_ahead_behind_upstream: None,
        outpost_ahead_behind_source: None,
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

fn current_branch_or_detached(git: &crate::GitInvoker) -> OutpostResult<Option<BranchName>> {
    match git.run_capture(["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(branch) => BranchName::parse(branch).map(Some),
        Err(OutpostError::GitFailed { code: 1, .. }) => Ok(None),
        Err(err) => Err(err),
    }
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
