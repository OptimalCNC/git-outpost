use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::{ConfigKey, ConfigStore, ConfigValue};
use crate::error::io_at;
use crate::git::{at_most_one, parse_ls_remote};
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
        let root = repository_path(&git, "--show-toplevel")?;
        Self::at_with(root, env)
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

        let work_tree = repository_path(&git, "--show-toplevel")?;
        let git_dir = repository_path(&git, "--git-dir")?;
        let git_common_dir = repository_path(&git, "--git-common-dir")?;
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

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn git_argv_log_for_tests(&self) -> Vec<Vec<OsString>> {
        self.git.argv_log()
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
        let common = registry
            .entries()
            .iter()
            .filter_map(|entry| entry.path.parent().map(Path::to_path_buf))
            .reduce(|common, parent| common_path_prefix(&common, &parent));
        // Paths on different Windows drives share no prefix.
        Ok(common.filter(|common| !common.as_os_str().is_empty()))
    }

    pub fn env(&self) -> &BTreeMap<OsString, OsString> {
        &self.env
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn test_invoker(&self) -> &GitInvoker {
        &self.git
    }

    pub fn current_branch(&self) -> OutpostResult<BranchName> {
        current_branch(&self.git)
    }

    pub fn checked_out_branches(&self) -> OutpostResult<Vec<BranchName>> {
        let mut branches = Vec::new();
        if let Ok(branch) = self.current_branch() {
            branches.push(branch);
        }

        let output = self
            .git
            .run_capture_bytes(["worktree", "list", "--porcelain", "-z"])?;
        for field in output.split(|&byte| byte == 0) {
            if let Some(name) = field.strip_prefix(b"branch refs/heads/") {
                let branch = BranchName::parse(self.git.output_text(name)?)?;
                if !branches.iter().any(|existing| existing == &branch) {
                    branches.push(branch);
                }
            }
        }
        Ok(branches)
    }

    pub fn checked_out_worktree_for(&self, branch: &BranchName) -> OutpostResult<Option<PathBuf>> {
        let output = self
            .git
            .run_capture_bytes(["worktree", "list", "--porcelain", "-z"])?;
        let mut current_path = None;
        for field in output.split(|&byte| byte == 0) {
            if let Some(path) = field.strip_prefix(b"worktree ") {
                current_path = Some(path);
            } else if field.strip_prefix(b"branch refs/heads/") == Some(branch.as_str().as_bytes())
            {
                return current_path
                    .map(|path| canonicalize_path(self.git.output_path(path)?))
                    .transpose();
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
        let urls = self
            .git
            .run_capture_lines(["remote", "get-url", remote.as_str()])?;
        match urls.as_slice() {
            [url] if !url.is_empty() => Ok(url.clone()),
            _ => Err(self.git.invalid_output("expected one nonempty remote URL")),
        }
    }

    pub fn branch_exists(&self, branch: &BranchName) -> OutpostResult<bool> {
        let branch_ref = format!("refs/heads/{}", branch.as_str());
        self.git
            .run_status(["show-ref", "--verify", "--quiet", &branch_ref])
    }

    pub fn branch_oid(&self, branch: &BranchName) -> OutpostResult<Option<String>> {
        if !self.branch_exists(branch)? {
            return Ok(None);
        }

        self.git.ref_oid(&source_branch_ref(branch)).map(Some)
    }

    pub fn has_commit_oid(&self, oid: &str) -> OutpostResult<bool> {
        let commit = format!("{oid}^{{commit}}");
        self.git
            .run_status(["rev-parse", "--verify", "--quiet", &commit])
    }

    pub fn origin_branch_oid(&self, branch: &BranchName) -> OutpostResult<Option<String>> {
        self.remote_branch_oid(&origin_remote(), branch)
    }

    pub fn remote_branch_oid(
        &self,
        remote: &RemoteName,
        branch: &BranchName,
    ) -> OutpostResult<Option<String>> {
        remote_ref_oid(&self.git, remote, &source_branch_ref(branch))
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
            return Err(self.git.invalid_output(&reference));
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
        let oid = self.git.ref_oid(&remote_tracking_ref)?;

        Ok(Some((branch, oid.trim().to_owned())))
    }

    pub fn fetch_remote_branches(
        &self,
        remote: &RemoteName,
        branches: &[BranchName],
    ) -> OutpostResult<()> {
        let mut unique = std::collections::BTreeSet::new();
        let mut args = vec![OsString::from("fetch"), OsString::from(remote.as_str())];
        for branch in branches {
            if !unique.insert(branch.clone()) {
                continue;
            }
            args.push(OsString::from(format!(
                "+refs/heads/{}:refs/remotes/{}/{}",
                branch.as_str(),
                remote.as_str(),
                branch.as_str()
            )));
        }
        if args.len() == 2 {
            return Ok(());
        }
        self.git.run_check(args)
    }

    pub(crate) fn materialize_remote_branch(
        &self,
        remote: &RemoteName,
        branch: &BranchName,
    ) -> OutpostResult<()> {
        let remote_tracking_ref = format!("refs/remotes/{}/{}", remote.as_str(), branch.as_str());
        let fetch_refspec = format!("+refs/heads/{}:{remote_tracking_ref}", branch.as_str());
        self.git
            .run_check(["fetch", "--no-tags", remote.as_str(), &fetch_refspec])?;

        let fetch_key = format!("remote.{}.fetch", remote.as_str());
        let already_configured =
            if self
                .git
                .run_status(["config", "--local", "--get-all", &fetch_key])?
            {
                self.git
                    .run_capture(["config", "--local", "--get-all", &fetch_key])?
                    .lines()
                    .any(|configured| configured == fetch_refspec)
            } else {
                false
            };
        if !already_configured {
            self.git
                .run_check(["config", "--local", "--add", &fetch_key, &fetch_refspec])?;
        }

        self.git.run_check([
            "-c",
            crate::git::PREFER_EXACT_REF,
            "branch",
            "--track",
            branch.as_str(),
            &remote_tracking_ref,
        ])
    }

    fn remote_head_branch(&self, remote: &RemoteName) -> OutpostResult<Option<BranchName>> {
        let output = self
            .git
            .run_capture(["ls-remote", "--symref", remote.as_str(), "HEAD"])?;
        let records = parse_ls_remote(&output).ok_or_else(|| self.git.invalid_output(&output))?;
        // The pattern also matches the tails of names like refs/remotes/x/HEAD.
        let Some(head) = records
            .iter()
            .find(|record| record.symref && record.name == "HEAD")
        else {
            return Ok(None);
        };
        let branch = head
            .value
            .strip_prefix("refs/heads/")
            .ok_or_else(|| self.git.invalid_output(&output))?;
        BranchName::parse(branch.to_owned()).map(Some)
    }

    pub fn is_ancestor_oid(&self, ancestor: &str, descendant: &str) -> OutpostResult<bool> {
        is_ancestor(&self.git, ancestor, descendant)
    }

    pub fn is_branch_checked_out(&self, branch: &BranchName) -> OutpostResult<bool> {
        Ok(self.checked_out_branches()?.contains(branch))
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
        let observed_url = self.remote_url(remote)?;
        self.delete_observed_remote_branch_if_oid(remote, &observed_url, branch, expected_oid)
    }

    pub(crate) fn verify_remote_deletion_destination(
        &self,
        remote: &RemoteName,
        observed_url: &str,
    ) -> OutpostResult<()> {
        let push_urls = self.git.run_capture_lines([
            "remote",
            "get-url",
            "--push",
            "--all",
            remote.as_str(),
        ])?;
        if self.remote_url(remote)? != observed_url
            || !matches!(push_urls.as_slice(), [url] if url == observed_url)
        {
            return Err(OutpostError::UnverifiedPushDestination {
                remote: remote.as_str().to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn delete_observed_remote_branch_if_oid(
        &self,
        remote: &RemoteName,
        observed_url: &str,
        branch: &BranchName,
        expected_oid: &str,
    ) -> OutpostResult<()> {
        self.verify_remote_deletion_destination(remote, observed_url)?;
        let lease = format!(
            "--force-with-lease=refs/heads/{}:{expected_oid}",
            branch.as_str()
        );
        let delete_refspec = format!(":refs/heads/{}", branch.as_str());
        // Keep the named remote's explicit push URL and fetch mappings. Passing its
        // expanded URL instead can apply pushInsteadOf again and leave stale tracking refs.
        self.git
            .run_check(["push", &lease, "--", remote.as_str(), &delete_refspec])
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
        let fetch_refspec = format!("{local_ref}:{remote_ref}");
        self.git.run_check(["fetch", "origin", &fetch_refspec])?;

        let local_oid = self.git.ref_oid(&local_ref)?;
        let remote_oid = self.git.ref_oid(&remote_ref)?;
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
        self.git_dir.join("outpost").join("registry.json")
    }

    pub fn config_path(&self) -> PathBuf {
        self.git_dir.join("outpost").join("config.json")
    }

    pub fn registry(&self) -> OutpostResult<Registry> {
        Registry::load(self)
    }

    pub fn registry_mut(&self) -> OutpostResult<RegistryMut> {
        RegistryMut::load(self)
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

pub(crate) fn remote_ref_oid(
    git: &GitInvoker,
    remote: &RemoteName,
    reference: &str,
) -> OutpostResult<Option<String>> {
    // ls-remote patterns also match slash-delimited tails of longer ref names.
    let output = git.run_capture(["ls-remote", "--", remote.as_str(), reference])?;
    let records = parse_ls_remote(&output).ok_or_else(|| git.invalid_output(&output))?;
    let exact = records
        .iter()
        .filter(|record| !record.symref && record.name == reference)
        .map(|record| record.value.to_owned());
    at_most_one(exact).ok_or_else(|| git.invalid_output(&output))
}

pub(crate) fn current_branch_or_detached(git: &GitInvoker) -> OutpostResult<Option<BranchName>> {
    match git.run_capture_bytes(["symbolic-ref", "--quiet", "HEAD"]) {
        Ok(bytes) => {
            let reference = git.output_text(bytes.strip_suffix(b"\n").unwrap_or(&bytes))?;
            BranchName::from_git_ref(reference).map(Some)
        }
        Err(OutpostError::GitFailed { code: 1, .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn current_branch(git: &GitInvoker) -> OutpostResult<BranchName> {
    current_branch_or_detached(git)?.ok_or_else(|| OutpostError::BranchNotFound {
        branch: "HEAD".to_owned(),
        repo: git.cwd().to_owned(),
    })
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

pub(crate) fn is_ancestor(
    git: &GitInvoker,
    ancestor: &str,
    descendant: &str,
) -> OutpostResult<bool> {
    git.run_status(["merge-base", "--is-ancestor", ancestor, descendant])
}

pub(crate) fn canonicalize_path(path: &Path) -> OutpostResult<PathBuf> {
    fs::canonicalize(path).map_err(io_at(path))
}

pub(crate) fn repository_path(git: &GitInvoker, option: &str) -> OutpostResult<PathBuf> {
    let path = match git.run_capture_path(["rev-parse", option]) {
        Err(OutpostError::GitFailed { code: 128, .. })
            if option == "--show-toplevel" && git_found_no_repository(git) =>
        {
            return Err(OutpostError::NotARepo(git.cwd().to_owned()));
        }
        result => result?,
    };
    canonicalize_path(&git.cwd().join(path))
}

fn git_found_no_repository(git: &GitInvoker) -> bool {
    // Discovery, invalid configuration and bare repositories all exit 128.
    // Classify the failure in a fixed locale and keep the user's original diagnostic.
    // If this second run disagrees, the failure stays unclassified.
    let classification = git
        .clone()
        .with_env("LC_ALL", "C")
        .run_check(["rev-parse", "--show-toplevel"]);
    matches!(
        classification,
        Err(OutpostError::GitFailed { code: 128, ref stderr, .. }) if reports_no_repository(stderr)
    )
}

fn reports_no_repository(stderr: &str) -> bool {
    // Trace output may precede the fatal message, and the message itself can span lines.
    stderr
        .lines()
        .find(|line| line.starts_with("fatal: "))
        .is_some_and(|line| {
            line == "fatal: not a git repository (or any of the parent directories): .git"
                || line.starts_with("fatal: not a git repository (or any parent up to mount point ")
        })
}

pub(crate) fn source_branch_ref(branch: &BranchName) -> String {
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
        let one = container.join("team-a").join("one");
        let two = container.join("team-b").join("two");
        fs::create_dir_all(&one).expect("one outpost dir");
        fs::create_dir_all(&two).expect("two outpost dir");
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
