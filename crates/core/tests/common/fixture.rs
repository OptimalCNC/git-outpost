use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use outpost_core::ops::add::{run as add_run, AddCheckout, AddOptions};
use outpost_core::{
    BranchName, GitInvoker, OutpostError, OutpostResult, RemoteName, Reporter, SourceRepo, StepKind,
};

pub struct AbcFixture {
    _tmp: tempfile::TempDir,
    pub root: PathBuf,
    pub upstream: PathBuf,
    pub source: PathBuf,
    pub git_env: BTreeMap<OsString, OsString>,
}

impl AbcFixture {
    pub fn new() -> Self {
        Self::try_new().expect("build A/B fixture")
    }

    pub fn try_new() -> OutpostResult<Self> {
        let tmp = tempfile::tempdir().map_err(|source| OutpostError::IoAt {
            path: std::env::temp_dir(),
            source,
        })?;
        let root = tmp.path().to_path_buf();
        let empty_gitconfig = root.join("empty.gitconfig");
        fs::File::create(&empty_gitconfig).map_err(|source| OutpostError::IoAt {
            path: empty_gitconfig.clone(),
            source,
        })?;

        let git_env = hermetic_git_env(&empty_gitconfig);
        let upstream = root.join("A.git");
        let source = root.join("B");
        let fixture = Self {
            _tmp: tmp,
            root,
            upstream,
            source,
            git_env,
        };

        fixture.invoker(&fixture.root).run_check([
            os("init"),
            os("--bare"),
            os("--initial-branch=main"),
            fixture.upstream.as_os_str(),
        ])?;
        fixture.invoker(&fixture.root).run_check([
            os("clone"),
            fixture.upstream.as_os_str(),
            fixture.source.as_os_str(),
        ])?;
        fixture.invoker(&fixture.source).run_check([
            os("config"),
            os("core.autocrlf"),
            os("false"),
        ])?;
        fixture.invoker(&fixture.source).run_check([
            os("commit"),
            os("--allow-empty"),
            os("-m"),
            os("initial"),
        ])?;
        fixture
            .invoker(&fixture.source)
            .run_check([os("push"), os("origin"), os("main")])?;

        Ok(fixture)
    }

    pub fn invoker(&self, cwd: &Path) -> GitInvoker {
        self.git_env
            .iter()
            .fold(GitInvoker::at(cwd), |git, (key, val)| {
                git.with_env(key.clone(), val.clone())
            })
    }

    pub fn source_repo(&self) -> OutpostResult<SourceRepo> {
        SourceRepo::at_with(&self.source, &self.git_env)
    }

    pub fn commit_in_source(&self, msg: &str) -> OutpostResult<String> {
        self.invoker(&self.source).run_check([
            os("commit"),
            os("--allow-empty"),
            os("-m"),
            OsStr::new(msg),
        ])?;
        self.invoker(&self.source)
            .run_capture([os("rev-parse"), os("HEAD")])
    }

    #[allow(dead_code)]
    pub fn commit_file_in_source(
        &self,
        msg: &str,
        path: &str,
        content: &str,
    ) -> OutpostResult<String> {
        commit_file(self, &self.source, msg, path, content)
    }

    #[allow(dead_code)]
    pub fn create_source_branch(&self, branch: &str) -> OutpostResult<BranchName> {
        let branch = BranchName::parse(branch.to_owned())?;
        self.invoker(&self.source)
            .run_check([os("branch"), OsStr::new(branch.as_str())])?;
        Ok(branch)
    }

    #[allow(dead_code)]
    pub fn add_outpost(&self, name: &str) -> OutpostResult<PathBuf> {
        self.add_outpost_on_branch(name, None)
    }

    #[allow(dead_code)]
    pub fn add_outpost_on_branch(
        &self,
        name: &str,
        target_branch: Option<BranchName>,
    ) -> OutpostResult<PathBuf> {
        self.add_outpost_with_remote_on_branch(name, "local", target_branch)
    }

    #[allow(dead_code)]
    pub fn add_outpost_with_remote(&self, name: &str, remote_name: &str) -> OutpostResult<PathBuf> {
        self.add_outpost_with_remote_on_branch(name, remote_name, None)
    }

    #[allow(dead_code)]
    pub fn add_outpost_with_remote_on_branch(
        &self,
        name: &str,
        remote_name: &str,
        target_branch: Option<BranchName>,
    ) -> OutpostResult<PathBuf> {
        let source = self.source_repo()?;
        let destination = self.root.join(name);
        let mut reporter = SilentReporter;
        add_run(
            &source,
            AddOptions {
                destination: destination.clone(),
                checkout: AddCheckout::CheckoutExisting { target_branch },
                remote_name: RemoteName::parse(remote_name)?,
            },
            &mut reporter,
        )?;
        Ok(destination)
    }

    #[allow(dead_code)]
    pub fn dirty_outpost(&self, name: &str) -> OutpostResult<PathBuf> {
        let outpost = self.add_outpost(name)?;
        fs::write(outpost.join("x.txt"), "dirty").map_err(|source| OutpostError::IoAt {
            path: outpost.join("x.txt"),
            source,
        })?;
        Ok(outpost)
    }

    #[allow(dead_code)]
    pub fn commit_in_outpost(&self, outpost: &Path, msg: &str) -> OutpostResult<String> {
        self.invoker(outpost).run_check([
            os("commit"),
            os("--allow-empty"),
            os("-m"),
            OsStr::new(msg),
        ])?;
        self.invoker(outpost)
            .run_capture([os("rev-parse"), os("HEAD")])
    }

    #[allow(dead_code)]
    pub fn commit_file_in_outpost(
        &self,
        outpost: &Path,
        msg: &str,
        path: &str,
        content: &str,
    ) -> OutpostResult<String> {
        commit_file(self, outpost, msg, path, content)
    }

    pub fn commit_in_upstream(&self, branch: &str, msg: &str) -> OutpostResult<String> {
        let scratch = tempfile::tempdir_in(&self.root).map_err(|source| OutpostError::IoAt {
            path: self.root.clone(),
            source,
        })?;
        let repo = scratch.path().join("upstream-work");

        self.invoker(&self.root).run_check([
            os("clone"),
            self.upstream.as_os_str(),
            repo.as_os_str(),
        ])?;
        let git = self.invoker(&repo);
        git.run_check([os("checkout"), OsStr::new(branch)])?;
        git.run_check([os("commit"), os("--allow-empty"), os("-m"), OsStr::new(msg)])?;
        let oid = git.run_capture([os("rev-parse"), os("HEAD")])?;
        git.run_check([os("push"), os("origin"), OsStr::new(branch)])?;

        Ok(oid)
    }

    #[allow(dead_code)]
    pub fn commit_file_in_upstream(
        &self,
        branch: &str,
        msg: &str,
        path: &str,
        content: &str,
    ) -> OutpostResult<String> {
        let scratch = tempfile::tempdir_in(&self.root).map_err(|source| OutpostError::IoAt {
            path: self.root.clone(),
            source,
        })?;
        let repo = scratch.path().join("upstream-work");

        self.invoker(&self.root).run_check([
            os("clone"),
            self.upstream.as_os_str(),
            repo.as_os_str(),
        ])?;
        let git = self.invoker(&repo);
        git.run_check([os("checkout"), OsStr::new(branch)])?;
        let oid = commit_file(self, &repo, msg, path, content)?;
        git.run_check([os("push"), os("origin"), OsStr::new(branch)])?;

        Ok(oid)
    }

    #[allow(dead_code)]
    pub fn push_source_branch(&self, branch: &BranchName) -> OutpostResult<()> {
        let refspec = format!(
            "refs/heads/{}:refs/heads/{}",
            branch.as_str(),
            branch.as_str()
        );
        self.invoker(&self.source)
            .run_check([os("push"), os("origin"), OsStr::new(&refspec)])
    }

    #[allow(dead_code)]
    pub fn delete_source_branch(&self, branch: &BranchName) -> OutpostResult<()> {
        self.invoker(&self.source)
            .run_check([os("branch"), os("-D"), OsStr::new(branch.as_str())])
    }

    #[allow(dead_code)]
    pub fn rev_parse(&self, repo: &Path, rev: &str) -> OutpostResult<String> {
        self.invoker(repo)
            .run_capture([os("rev-parse"), OsStr::new(rev)])
    }

    #[allow(dead_code)]
    pub fn current_branch_name(&self, repo: &Path) -> OutpostResult<String> {
        self.invoker(repo).run_capture([
            os("symbolic-ref"),
            os("--quiet"),
            os("--short"),
            os("HEAD"),
        ])
    }
}

fn commit_file(
    fixture: &AbcFixture,
    repo: &Path,
    msg: &str,
    path: &str,
    content: &str,
) -> OutpostResult<String> {
    let absolute = repo.join(path);
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&absolute, content).map_err(|source| OutpostError::IoAt {
        path: absolute.clone(),
        source,
    })?;

    let git = fixture.invoker(repo);
    git.run_check([os("add"), OsStr::new(path)])?;
    git.run_check([os("commit"), os("-m"), OsStr::new(msg)])?;
    git.run_capture([os("rev-parse"), os("HEAD")])
}

fn hermetic_git_env(empty_gitconfig: &Path) -> BTreeMap<OsString, OsString> {
    BTreeMap::from([
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            empty_gitconfig.as_os_str().to_os_string(),
        ),
        (
            OsString::from("GIT_CONFIG_SYSTEM"),
            empty_gitconfig.as_os_str().to_os_string(),
        ),
        (
            OsString::from("GIT_AUTHOR_NAME"),
            OsString::from("Test Author"),
        ),
        (
            OsString::from("GIT_AUTHOR_EMAIL"),
            OsString::from("test@example.com"),
        ),
        (
            OsString::from("GIT_COMMITTER_NAME"),
            OsString::from("Test Committer"),
        ),
        (
            OsString::from("GIT_COMMITTER_EMAIL"),
            OsString::from("test@example.com"),
        ),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
    ])
}

fn os(value: &'static str) -> &'static OsStr {
    OsStr::new(value)
}

#[allow(dead_code)]
struct SilentReporter;

impl Reporter for SilentReporter {
    fn step(&mut self, _kind: StepKind, _message: &str) {}

    fn warn(&mut self, _message: &str) {}
}

#[derive(Default)]
pub struct CapturingReporter {
    pub steps: Vec<(StepKind, String)>,
    pub warnings: Vec<String>,
}

impl CapturingReporter {
    #[allow(dead_code)]
    pub fn step_kinds(&self) -> Vec<StepKind> {
        self.steps.iter().map(|(kind, _)| *kind).collect()
    }
}

impl Reporter for CapturingReporter {
    fn step(&mut self, kind: StepKind, message: &str) {
        self.steps.push((kind, message.to_owned()));
    }

    fn warn(&mut self, message: &str) {
        self.warnings.push(message.to_owned());
    }
}
