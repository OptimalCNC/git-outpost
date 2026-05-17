use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::metadata::{Metadata, RawMetadata};
use crate::source_repo::{
    canonicalize_path, current_branch, invoker_at, is_dirty, read_optional_config, SourceRepo,
};
use crate::{BranchName, GitInvoker, OutpostError, OutpostResult, RefName, UpstreamRef};

pub struct Outpost {
    work_tree: PathBuf,
    git_dir: PathBuf,
    git: GitInvoker,
    metadata: Metadata,
    env: BTreeMap<OsString, OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

impl Outpost {
    pub fn discover(start: &Path) -> OutpostResult<Self> {
        Self::discover_with(start, &BTreeMap::new())
    }

    pub fn discover_with(start: &Path, env: &BTreeMap<OsString, OsString>) -> OutpostResult<Self> {
        let git = invoker_at(start, env);
        let work_tree = git
            .run_capture(["rev-parse", "--show-toplevel"])
            .map_err(|err| map_discovery_error(err, start))?;
        Self::at_with(work_tree, env)
    }

    pub fn at(path: impl Into<PathBuf>) -> OutpostResult<Self> {
        Self::at_with(path, &BTreeMap::new())
    }

    pub fn at_with(
        path: impl Into<PathBuf>,
        env: &BTreeMap<OsString, OsString>,
    ) -> OutpostResult<Self> {
        let start = path.into();
        let git = invoker_at(&start, env);
        let work_tree_raw = git
            .run_capture(["rev-parse", "--show-toplevel"])
            .map_err(|err| map_discovery_error(err, &start))?;
        let git_dir_raw = git
            .run_capture(["rev-parse", "--git-dir"])
            .map_err(|err| map_discovery_error(err, &start))?;
        let work_tree = canonicalize_path(Path::new(&work_tree_raw))?;
        let git_dir = canonicalize_git_path(&start, &git_dir_raw)?;
        let git = invoker_at(&work_tree, env);
        let raw = RawMetadata::read(&git)?;
        let metadata = Metadata::from_raw(&work_tree, raw)?;

        Ok(Self {
            work_tree,
            git_dir,
            git,
            metadata,
            env: env.clone(),
        })
    }

    pub fn work_tree(&self) -> &Path {
        &self.work_tree
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn source_repo(&self) -> OutpostResult<SourceRepo> {
        if !self.metadata.source_repo.exists() {
            return Err(OutpostError::SourceMissing(
                self.metadata.source_repo.clone(),
            ));
        }
        SourceRepo::at_with(&self.metadata.source_repo, &self.env)
    }

    pub fn current_branch(&self) -> OutpostResult<BranchName> {
        current_branch(&self.git, &self.work_tree)
    }

    pub fn is_dirty(&self) -> OutpostResult<bool> {
        is_dirty(&self.git)
    }

    pub fn upstream_tracking(&self) -> OutpostResult<Option<UpstreamRef>> {
        let branch = self.current_branch()?;
        let remote_key = format!("branch.{}.remote", branch.as_str());
        let merge_key = format!("branch.{}.merge", branch.as_str());
        let Some(remote) = read_optional_config(&self.git, &remote_key)? else {
            return Ok(None);
        };
        let Some(merge_ref) = read_optional_config(&self.git, &merge_key)? else {
            return Ok(None);
        };

        Ok(Some(UpstreamRef {
            remote: crate::RemoteName::parse(remote)?,
            merge_ref: RefName::parse(merge_ref)?,
        }))
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn test_invoker(&self) -> &GitInvoker {
        &self.git
    }
}

fn canonicalize_git_path(start: &Path, value: &str) -> OutpostResult<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        canonicalize_path(&path)
    } else {
        canonicalize_path(&start.join(path))
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
    use std::fs;

    use super::*;
    use crate::RemoteName;

    #[test]
    fn outpost_at_rejects_unmanaged_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");

        let Err(err) = Outpost::at(temp.path()) else {
            panic!("unmanaged repo should fail");
        };
        assert!(matches!(err, OutpostError::NotAnOutpost(path) if path == temp.path()));
    }

    #[test]
    fn outpost_at_reads_metadata_and_source_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo(&source);
        init_repo(&outpost);
        let metadata = Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        };
        metadata.write(&GitInvoker::at(&outpost)).unwrap();

        let outpost = Outpost::at(&outpost).expect("managed outpost");

        assert_eq!(outpost.metadata().remote_name.as_str(), "local");
        assert_eq!(
            outpost.source_repo().unwrap().work_tree(),
            fs::canonicalize(&source).unwrap()
        );
    }

    #[test]
    fn outpost_reports_missing_source_repo_from_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo(&source);
        init_repo(&outpost);
        let metadata = Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        };
        metadata.write(&GitInvoker::at(&outpost)).unwrap();
        fs::remove_dir_all(&source).expect("remove source");

        let outpost = Outpost::at(&outpost).expect("managed outpost");
        let Err(err) = outpost.source_repo() else {
            panic!("source should be missing");
        };

        assert!(
            matches!(err, OutpostError::SourceMissing(path) if path == fs::canonicalize(temp.path()).unwrap().join("source"))
        );
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("repo dir");
        GitInvoker::at(path)
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
    }
}
