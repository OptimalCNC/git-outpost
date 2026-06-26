use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::{ConfigKey, ConfigStore, ConfigValue};
use crate::outpost::Outpost;
use crate::registry::{Registry, RegistryMut};
use crate::{
    BranchName, GitInvoker, OutpostError, OutpostResult, RefName, RemoteName, UpstreamRef,
};

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

    pub fn config(&self) -> ConfigStore<'_> {
        ConfigStore::new(self)
    }

    pub fn outpost_container(&self) -> OutpostResult<Option<PathBuf>> {
        match self.config().get(ConfigKey::OutpostContainer)? {
            Some(ConfigValue::OutpostContainer(path)) => Ok(Some(path)),
            None => Ok(None),
        }
    }

    pub fn set_outpost_container(&self, path: &Path) -> OutpostResult<PathBuf> {
        match self.config().set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(path.to_path_buf()),
        )? {
            ConfigValue::OutpostContainer(path) => Ok(path),
        }
    }

    pub fn unset_outpost_container(&self) -> OutpostResult<()> {
        self.config().unset(ConfigKey::OutpostContainer)
    }

    pub fn resolve_outpost_destination(&self, cwd: &Path, path: &Path) -> OutpostResult<PathBuf> {
        if bare_outpost_name(path) {
            let Some(container) = self.outpost_container()? else {
                return Err(OutpostError::OutpostContainerNotConfigured {
                    name: path.to_string_lossy().into_owned(),
                    suggestion: self.suggest_outpost_container().ok().flatten(),
                });
            };
            Ok(container.join(path))
        } else if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(cwd.join(path))
        }
    }

    pub fn suggest_outpost_container(&self) -> OutpostResult<Option<PathBuf>> {
        let registry = self.registry()?;
        let mut parents = registry
            .entries()
            .iter()
            .filter_map(|entry| entry.path.parent().map(Path::to_path_buf));
        let Some(mut common) = parents.next() else {
            return Ok(None);
        };

        for parent in parents {
            common = common_path_prefix(&common, &parent);
            if common.as_os_str().is_empty() {
                return Ok(None);
            }
        }

        Ok(Some(common))
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

    pub fn remote_url(&self, remote: &RemoteName) -> OutpostResult<String> {
        self.git.run_capture(["remote", "get-url", remote.as_str()])
    }

    pub fn branch_exists(&self, branch: &BranchName) -> OutpostResult<bool> {
        let branch_ref = format!("refs/heads/{}", branch.as_str());
        self.git
            .run_status(["rev-parse", "--verify", "--quiet", &branch_ref])
    }

    pub fn branch_oid(&self, branch: &BranchName) -> OutpostResult<Option<String>> {
        if !self.branch_exists(branch)? {
            return Ok(None);
        }

        rev_parse(&self.git, &source_branch_ref(branch)).map(|oid| Some(oid.trim().to_owned()))
    }

    pub fn origin_branch_oid(&self, branch: &BranchName) -> OutpostResult<Option<String>> {
        self.remote_branch_oid(&origin_remote(), branch)
    }

    pub fn remote_branch_oid(
        &self,
        remote: &RemoteName,
        branch: &BranchName,
    ) -> OutpostResult<Option<String>> {
        let remote_ref = source_branch_ref(branch);
        let output = self
            .git
            .run_capture(["ls-remote", remote.as_str(), &remote_ref])?;
        if output.is_empty() {
            return Ok(None);
        }

        let mut fields = output.split_whitespace();
        let oid = fields
            .next()
            .ok_or_else(|| invalid_git_output(&self.git, &output))?;
        let name = fields
            .next()
            .ok_or_else(|| invalid_git_output(&self.git, &output))?;
        if fields.next().is_some() || name != remote_ref {
            return Err(invalid_git_output(&self.git, &output));
        }

        Ok(Some(oid.to_owned()))
    }

    pub fn origin_default_branch(&self) -> OutpostResult<Option<BranchName>> {
        self.remote_default_branch(&origin_remote())
    }

    pub fn remote_default_branch(&self, remote: &RemoteName) -> OutpostResult<Option<BranchName>> {
        let head_ref = format!("refs/remotes/{}/HEAD", remote.as_str());
        if !self
            .git
            .run_status(["symbolic-ref", "--quiet", &head_ref])?
        {
            return Ok(None);
        }

        let reference = self
            .git
            .run_capture(["symbolic-ref", "--quiet", &head_ref])?;
        let remote_prefix = format!("refs/remotes/{}/", remote.as_str());
        let Some(branch) = reference.strip_prefix(&remote_prefix) else {
            return Err(invalid_git_output(&self.git, &reference));
        };

        BranchName::parse(branch.to_owned()).map(Some)
    }

    pub fn fetch_origin_default_branch(&self) -> OutpostResult<Option<(BranchName, String)>> {
        self.fetch_remote_default_branch(&origin_remote())
    }

    pub fn fetch_remote_default_branch(
        &self,
        remote: &RemoteName,
    ) -> OutpostResult<Option<(BranchName, String)>> {
        let branch = match self.remote_default_branch(remote)? {
            Some(branch) => Some(branch),
            None => self.remote_head_branch(remote)?,
        };
        let Some(branch) = branch else {
            return Ok(None);
        };

        let remote_tracking_ref = format!("refs/remotes/{}/{}", remote.as_str(), branch.as_str());
        let fetch_refspec = format!("+{}:{remote_tracking_ref}", source_branch_ref(&branch));
        self.git
            .run_check(["fetch", remote.as_str(), &fetch_refspec])?;
        let oid = rev_parse(&self.git, &remote_tracking_ref)?;

        Ok(Some((branch, oid.trim().to_owned())))
    }

    fn remote_head_branch(&self, remote: &RemoteName) -> OutpostResult<Option<BranchName>> {
        let output = self
            .git
            .run_capture(["ls-remote", "--symref", remote.as_str(), "HEAD"])?;
        for line in output.lines() {
            let Some(rest) = line.strip_prefix("ref: ") else {
                continue;
            };
            let mut fields = rest.split_whitespace();
            let Some(reference) = fields.next() else {
                return Err(invalid_git_output(&self.git, &output));
            };
            let Some(name) = fields.next() else {
                return Err(invalid_git_output(&self.git, &output));
            };
            if fields.next().is_some() {
                return Err(invalid_git_output(&self.git, &output));
            }
            if name != "HEAD" {
                continue;
            }
            let Some(branch) = reference.strip_prefix("refs/heads/") else {
                return Err(invalid_git_output(&self.git, &output));
            };
            return BranchName::parse(branch.to_owned()).map(Some);
        }
        Ok(None)
    }

    pub fn is_ancestor_oid(&self, ancestor: &str, descendant: &str) -> OutpostResult<bool> {
        is_ancestor(&self.git, ancestor, descendant)
    }

    pub fn is_branch_checked_out(&self, branch: &BranchName) -> OutpostResult<bool> {
        self.checked_out_worktree_for(branch)
            .map(|path| path.is_some())
    }

    pub fn delete_branch_if_oid(
        &self,
        branch: &BranchName,
        expected_oid: &str,
    ) -> OutpostResult<()> {
        self.git
            .run_check(["update-ref", "-d", &source_branch_ref(branch), expected_oid])
    }

    pub fn delete_origin_branch_if_oid(
        &self,
        branch: &BranchName,
        expected_oid: &str,
    ) -> OutpostResult<()> {
        self.delete_remote_branch_if_oid(&origin_remote(), branch, expected_oid)
    }

    pub fn delete_remote_branch_if_oid(
        &self,
        remote: &RemoteName,
        branch: &BranchName,
        expected_oid: &str,
    ) -> OutpostResult<()> {
        let lease = format!(
            "--force-with-lease=refs/heads/{}:{expected_oid}",
            branch.as_str()
        );
        let delete_refspec = format!(":refs/heads/{}", branch.as_str());
        self.git
            .run_check(["push", &lease, remote.as_str(), &delete_refspec])
    }

    pub fn fast_forward_branch_from_origin(&self, branch: &BranchName) -> OutpostResult<()> {
        if !self.branch_exists(branch)? {
            return Err(OutpostError::BranchNotFound {
                branch: branch.as_str().to_owned(),
                repo: self.work_tree.clone(),
            });
        }

        let local_ref = format!("refs/heads/{}", branch.as_str());
        let remote_ref = format!("refs/remotes/origin/{}", branch.as_str());
        let fetch_refspec = format!("{}:{remote_ref}", branch.as_str());
        self.git.run_check(["fetch", "origin", &fetch_refspec])?;

        let local_oid = rev_parse(&self.git, &local_ref)?;
        let remote_oid = rev_parse(&self.git, &remote_ref)?;
        if local_oid == remote_oid || is_ancestor(&self.git, &remote_oid, &local_oid)? {
            return Ok(());
        }
        if !is_ancestor(&self.git, &local_oid, &remote_oid)? {
            return Err(OutpostError::Divergence {
                branch: branch.as_str().to_owned(),
            });
        }

        if let Some(worktree) = self.checked_out_worktree_for(branch)? {
            let git = invoker_at(&worktree, &self.env);
            git.run_check(["merge", "--ff-only", &remote_ref])?;
        } else {
            self.git
                .run_check(["update-ref", &local_ref, &remote_oid, &local_oid])?;
        }

        Ok(())
    }

    pub fn registry_path(&self) -> PathBuf {
        self.work_tree.join(".outpost").join("registry.json")
    }

    pub fn config_path(&self) -> PathBuf {
        self.work_tree.join(".outpost").join("config.json")
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

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn local_exclude_path_for_tests(&self) -> PathBuf {
        self.local_exclude_path()
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

pub(crate) fn rev_parse(git: &GitInvoker, reference: &str) -> OutpostResult<String> {
    git.run_capture(["rev-parse", reference])
}

pub(crate) fn is_ancestor(
    git: &GitInvoker,
    ancestor: &str,
    descendant: &str,
) -> OutpostResult<bool> {
    git.run_status(["merge-base", "--is-ancestor", ancestor, descendant])
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

fn source_branch_ref(branch: &BranchName) -> String {
    format!("refs/heads/{}", branch.as_str())
}

fn common_path_prefix(left: &Path, right: &Path) -> PathBuf {
    let mut prefix = PathBuf::new();
    for (left, right) in left.components().zip(right.components()) {
        if left != right {
            break;
        }
        push_component(&mut prefix, left);
    }
    prefix
}

fn push_component(path: &mut PathBuf, component: Component<'_>) {
    path.push(component.as_os_str());
}

fn bare_outpost_name(path: &Path) -> bool {
    if path.is_absolute() || path.to_str().is_none() {
        return false;
    }
    let mut components = path.components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn origin_remote() -> RemoteName {
    RemoteName::parse("origin").expect("origin is a valid remote name")
}

fn invalid_git_output(git: &GitInvoker, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: git.cwd().to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected git output: {output}"),
        ),
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
    fn outpost_container_config_set_and_read_canonical_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let container = temp.path().join("outposts");
        fs::create_dir(&container).expect("container dir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
        let source = SourceRepo::at(temp.path()).expect("source repo");

        let configured = source
            .set_outpost_container(&container)
            .expect("set outpost container");

        let expected = fs::canonicalize(&container).expect("canonical container");
        assert_eq!(configured, expected);
        assert_eq!(
            source
                .outpost_container()
                .expect("read outpost container")
                .expect("configured container"),
            expected
        );
    }

    #[test]
    fn suggested_outpost_container_uses_common_registered_parent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let container = temp.path().join("outposts");
        let one = container.join("one");
        let two = container.join("two");
        fs::create_dir(&container).expect("container dir");
        fs::create_dir(&one).expect("one outpost dir");
        fs::create_dir(&two).expect("two outpost dir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
        let source = SourceRepo::at(temp.path()).expect("source repo");
        let mut registry = source.registry_mut().expect("registry mut");
        registry
            .add(
                crate::RegistryEntry::new(one, RemoteName::parse("local").expect("remote"))
                    .expect("entry one"),
            )
            .expect("add one");
        registry
            .add(
                crate::RegistryEntry::new(two, RemoteName::parse("local").expect("remote"))
                    .expect("entry two"),
            )
            .expect("add two");
        registry.save().expect("save registry");

        assert_eq!(
            source
                .suggest_outpost_container()
                .expect("suggest outpost container"),
            Some(fs::canonicalize(&container).expect("canonical container"))
        );
    }

    #[test]
    fn missing_outpost_container_error_does_not_require_readable_registry_for_suggestion() {
        let temp = tempfile::tempdir().expect("tempdir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init");
        let source = SourceRepo::at(temp.path()).expect("source repo");
        fs::create_dir_all(source.registry_path().parent().unwrap()).expect("registry dir");
        fs::write(source.registry_path(), "{not json").expect("bad registry");

        let err = source
            .resolve_outpost_destination(temp.path(), Path::new("C"))
            .expect_err("missing container should fail before add");

        assert!(matches!(
            err,
            OutpostError::OutpostContainerNotConfigured {
                name,
                suggestion: None,
            } if name == "C"
        ));
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
        assert!(
            !source
                .branch_exists(&BranchName::parse("missing").unwrap())
                .unwrap()
        );
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
