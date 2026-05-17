use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::outpost::Outpost;
use crate::registry::{Registry, RegistryMut};
use crate::{BranchName, GitInvoker, OutpostError, OutpostResult, RefName, UpstreamRef};

pub struct SourceRepo {
    work_tree: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
    git: GitInvoker,
    env: BTreeMap<OsString, OsString>,
}

impl SourceRepo {
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
        let git_common_dir_raw = git
            .run_capture(["rev-parse", "--git-common-dir"])
            .map_err(|err| map_discovery_error(err, &start))?;

        let work_tree = canonicalize_path(Path::new(&work_tree_raw))?;
        let git_dir = canonicalize_git_path(&start, &git_dir_raw)?;
        let git_common_dir = canonicalize_git_path(&start, &git_common_dir_raw)?;
        let git = invoker_at(&work_tree, env);

        Ok(Self {
            work_tree,
            git_dir,
            git_common_dir,
            git,
            env: env.clone(),
        })
    }

    pub fn work_tree(&self) -> &Path {
        &self.work_tree
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn git_common_dir(&self) -> &Path {
        &self.git_common_dir
    }

    pub fn outpost_at(&self, path: &Path) -> OutpostResult<Outpost> {
        Outpost::at_with(path, &self.env)
    }

    pub fn env(&self) -> &BTreeMap<OsString, OsString> {
        &self.env
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn test_invoker(&self) -> &GitInvoker {
        &self.git
    }

    pub fn current_branch(&self) -> OutpostResult<BranchName> {
        current_branch(&self.git, &self.work_tree)
    }

    pub fn checked_out_branches(&self) -> OutpostResult<Vec<BranchName>> {
        let mut branches = Vec::new();
        if let Ok(branch) = self.current_branch() {
            branches.push(branch);
        }

        let output = self.git.run_capture(["worktree", "list", "--porcelain"])?;
        for line in output.lines() {
            if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                let branch = BranchName::parse(branch.to_owned())?;
                if !branches.iter().any(|existing| existing == &branch) {
                    branches.push(branch);
                }
            }
        }
        Ok(branches)
    }

    pub fn checked_out_worktree_for(&self, branch: &BranchName) -> OutpostResult<Option<PathBuf>> {
        let output = self.git.run_capture(["worktree", "list", "--porcelain"])?;
        let mut current_path: Option<PathBuf> = None;
        for line in output.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                current_path = Some(canonicalize_path(Path::new(path))?);
            } else if let Some(value) = line.strip_prefix("branch refs/heads/") {
                if value == branch.as_str() {
                    return Ok(current_path);
                }
            }
        }
        Ok(None)
    }

    pub fn is_dirty(&self) -> OutpostResult<bool> {
        is_dirty(&self.git)
    }

    pub fn upstream_for(&self, branch: &BranchName) -> OutpostResult<Option<UpstreamRef>> {
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

    pub fn branch_exists(&self, branch: &BranchName) -> OutpostResult<bool> {
        let branch_ref = format!("refs/heads/{}", branch.as_str());
        self.git
            .run_status(["rev-parse", "--verify", "--quiet", &branch_ref])
    }

    pub fn registry_path(&self) -> PathBuf {
        self.work_tree.join(".outpost").join("registry.json")
    }

    pub fn registry(&self) -> OutpostResult<Registry> {
        Registry::load(self)
    }

    pub fn registry_mut(&self) -> OutpostResult<RegistryMut<'_>> {
        RegistryMut::load(self)
    }

    pub(crate) fn local_exclude_path(&self) -> PathBuf {
        self.git_dir.join("info").join("exclude")
    }

    pub(crate) fn git(&self) -> &GitInvoker {
        &self.git
    }

    #[cfg(test)]
    pub(crate) fn from_storage_paths(work_tree: &Path, git_dir: &Path) -> OutpostResult<Self> {
        let work_tree = canonicalize_path(work_tree)?;
        let git_dir = canonicalize_path(git_dir)?;
        Ok(Self {
            git_common_dir: git_dir.clone(),
            git: GitInvoker::at(&work_tree),
            env: BTreeMap::new(),
            work_tree,
            git_dir,
        })
    }
}

pub(crate) fn invoker_at(cwd: &Path, env: &BTreeMap<OsString, OsString>) -> GitInvoker {
    env.iter().fold(GitInvoker::at(cwd), |git, (key, val)| {
        git.with_env(key.clone(), val.clone())
    })
}

pub(crate) fn current_branch(git: &GitInvoker, repo: &Path) -> OutpostResult<BranchName> {
    let name = git
        .run_capture(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_err(|err| match err {
            OutpostError::GitFailed { .. } => OutpostError::BranchNotFound {
                branch: "HEAD".to_owned(),
                repo: repo.to_path_buf(),
            },
            other => other,
        })?;
    BranchName::parse(name)
}

pub(crate) fn is_dirty(git: &GitInvoker) -> OutpostResult<bool> {
    Ok(!git
        .run_capture(["status", "--porcelain=v1", "--untracked-files=normal"])?
        .is_empty())
}

pub(crate) fn read_optional_config(git: &GitInvoker, key: &str) -> OutpostResult<Option<String>> {
    if git.run_status(["config", "--local", "--get", key])? {
        git.run_capture(["config", "--local", "--get", key])
            .map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) fn canonicalize_path(path: &Path) -> OutpostResult<PathBuf> {
    fs::canonicalize(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
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

    #[test]
    fn source_at_canonicalizes_paths_and_reads_current_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
        let source = SourceRepo::at(temp.path()).expect("source repo");

        assert_eq!(source.work_tree(), fs::canonicalize(temp.path()).unwrap());
        assert_eq!(
            source.git_dir(),
            fs::canonicalize(temp.path().join(".git")).unwrap()
        );
        assert_eq!(
            source.git_common_dir(),
            fs::canonicalize(temp.path().join(".git")).unwrap()
        );
        assert_eq!(source.current_branch().unwrap().as_str(), "main");
        assert!(!source.is_dirty().unwrap());
    }

    #[test]
    fn source_discover_rejects_non_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let Err(err) = SourceRepo::discover(temp.path()) else {
            panic!("non repo should fail");
        };

        assert!(matches!(err, OutpostError::NotARepo(path) if path == temp.path()));
    }

    #[test]
    fn source_dirty_detects_untracked_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
        fs::write(temp.path().join("new.txt"), "dirty").expect("write untracked");

        let source = SourceRepo::at(temp.path()).expect("source repo");
        assert!(source.is_dirty().unwrap());
    }

    #[test]
    fn source_branch_helpers_read_local_heads_upstream_and_worktrees() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sibling = tempfile::tempdir().expect("worktree parent");
        let feature_worktree = sibling.path().join("feature-worktree");
        let git = GitInvoker::at(temp.path());
        git.run_check(["init", "--initial-branch=main"])
            .expect("init");
        git.run_check(["config", "user.name", "Test User"])
            .expect("user name");
        git.run_check(["config", "user.email", "test@example.com"])
            .expect("user email");
        git.run_check(["commit", "--allow-empty", "-m", "initial"])
            .expect("initial commit");
        git.run_check(["branch", "feature"])
            .expect("feature branch");
        git.run_check(["config", "--local", "branch.main.remote", "origin"])
            .expect("remote config");
        git.run_check(["config", "--local", "branch.main.merge", "refs/heads/main"])
            .expect("merge config");
        git.run_check([
            "worktree",
            "add",
            feature_worktree.to_str().unwrap(),
            "feature",
        ])
        .expect("add worktree");

        let source = SourceRepo::at(temp.path()).expect("source repo");
        let main = BranchName::parse("main").unwrap();
        let feature = BranchName::parse("feature").unwrap();

        assert!(source.branch_exists(&main).unwrap());
        assert!(!source
            .branch_exists(&BranchName::parse("missing").unwrap())
            .unwrap());
        assert_eq!(
            source
                .upstream_for(&main)
                .unwrap()
                .expect("main upstream")
                .merge_ref
                .as_str(),
            "refs/heads/main"
        );
        assert_eq!(
            source.checked_out_worktree_for(&feature).unwrap(),
            Some(fs::canonicalize(&feature_worktree).unwrap())
        );
        let checked_out = source.checked_out_branches().unwrap();
        assert!(checked_out.iter().any(|branch| branch == &main));
        assert!(checked_out.iter().any(|branch| branch == &feature));
    }
}
