use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::metadata::RawMetadata;
use crate::outpost::AheadBehind;
use crate::source_repo::{canonicalize_path, invoker_at};
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

    Ok(report_from_raw(outpost_path, raw))
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

fn report_from_raw(outpost_path: PathBuf, raw: RawMetadata) -> StatusReport {
    let mut problems = Vec::new();
    if raw.source_repo.is_none() {
        problems.push(ConfigProblem::MissingSourceRepoConfig);
    }
    if raw.remote_name.is_none() {
        problems.push(ConfigProblem::MissingRemoteNameConfig);
    }

    let source_present = raw.source_repo.as_ref().is_some_and(|path| path.exists());

    StatusReport {
        outpost_path,
        source_path: raw.source_repo,
        source_present,
        remote_name: raw.remote_name,
        current_branch: None,
        outpost_dirty: false,
        source_ahead_behind_upstream: None,
        outpost_ahead_behind_source: None,
        problems,
    }
}

fn map_discovery_error(err: OutpostError, path: &Path) -> OutpostError {
    match err {
        OutpostError::GitFailed { .. } => OutpostError::NotARepo(path.to_path_buf()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_from_raw_records_missing_metadata_problems() {
        let report = report_from_raw(
            PathBuf::from("/outpost"),
            RawMetadata {
                managed: Some(true),
                source_repo: None,
                remote_name: None,
            },
        );

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
