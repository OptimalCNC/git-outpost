use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use outpost_core::{GitInvoker, OutpostError, OutpostResult, SourceRepo};

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
