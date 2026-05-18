#![allow(dead_code)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct CliFixture {
    _tmp: tempfile::TempDir,
    pub root: PathBuf,
    pub upstream: PathBuf,
    pub source: PathBuf,
    git_env: Vec<(OsString, OsString)>,
}

impl CliFixture {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let empty_gitconfig = root.join("empty.gitconfig");
        fs::File::create(&empty_gitconfig).expect("empty gitconfig");
        let upstream = root.join("A.git");
        let source = root.join("B");
        let fixture = Self {
            _tmp: tmp,
            root,
            upstream,
            source,
            git_env: hermetic_git_env(&empty_gitconfig),
        };

        fixture.run_git_ok(
            &fixture.root,
            ["init", "--bare", "--initial-branch=main"],
            |cmd| {
                cmd.arg(&fixture.upstream);
            },
        );
        fixture.run_git_ok(&fixture.root, ["clone"], |cmd| {
            cmd.arg(&fixture.upstream).arg(&fixture.source);
        });
        fixture.run_git_ok(
            &fixture.source,
            ["config", "core.autocrlf", "false"],
            |_| {},
        );
        fixture.run_git_ok(
            &fixture.source,
            ["commit", "--allow-empty", "-m", "initial"],
            |_| {},
        );
        fixture.run_git_ok(&fixture.source, ["push", "origin", "main"], |_| {});

        fixture
    }

    pub fn gop(&self) -> Command {
        with_git_env(gop_command(), &self.git_env)
    }

    pub fn git_outpost(&self) -> Command {
        with_git_env(git_outpost_command(), &self.git_env)
    }

    pub fn git_dispatch(&self) -> Command {
        with_git_env(git_dispatch_command(), &self.git_env)
    }

    pub fn git(&self, cwd: &Path) -> Command {
        let mut command = Command::new("git");
        command.current_dir(cwd);
        with_git_env(command, &self.git_env)
    }

    pub fn outpost(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn add_outpost(&self, name: &str) -> PathBuf {
        let outpost = self.outpost(name);
        let output = run(self
            .gop()
            .current_dir(&self.source)
            .arg("add")
            .arg(format!("../{name}"))
            .arg("main"));
        assert_success(&output, "gop add");
        outpost
    }

    pub fn commit_file(&self, repo: &Path, msg: &str, path: &str, content: &str) -> String {
        let absolute = repo.join(path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&absolute, content).expect("write file");

        self.run_git_ok(repo, ["add", path], |_| {});
        self.run_git_ok(repo, ["commit", "-m", msg], |_| {});
        self.git_capture(repo, ["rev-parse", "HEAD"])
    }

    pub fn commit_upstream_file(
        &self,
        branch: &str,
        msg: &str,
        path: &str,
        content: &str,
    ) -> String {
        let scratch = tempfile::tempdir_in(&self.root).expect("scratch");
        let repo = scratch.path().join("upstream-work");
        self.run_git_ok(&self.root, ["clone"], |cmd| {
            cmd.arg(&self.upstream).arg(&repo);
        });
        self.run_git_ok(&repo, ["checkout", branch], |_| {});
        let oid = self.commit_file(&repo, msg, path, content);
        self.run_git_ok(&repo, ["push", "origin", branch], |_| {});
        oid
    }

    pub fn git_capture<const N: usize>(&self, cwd: &Path, args: [&str; N]) -> String {
        let output = run(self.git(cwd).args(args));
        assert_success(&output, "git capture");
        stdout(&output).trim().to_owned()
    }

    fn run_git_ok<const N: usize, F>(&self, cwd: &Path, args: [&str; N], configure: F)
    where
        F: FnOnce(&mut Command),
    {
        let mut command = self.git(cwd);
        command.args(args);
        configure(&mut command);
        let output = run(&mut command);
        assert_success(&output, "git");
    }
}

pub fn command(bin_name: &str) -> Command {
    Command::new(binary_path(bin_name))
}

pub fn git_outpost_command() -> Command {
    command("git-outpost")
}

pub fn gop_command() -> Command {
    command("gop")
}

pub fn git_dispatch_command() -> Command {
    let mut command = Command::new("git");
    command.arg("outpost").env("PATH", path_with_binary_dir());
    command
}

pub fn binary_path(bin_name: &str) -> PathBuf {
    let key = format!("CARGO_BIN_EXE_{bin_name}");
    env::var_os(&key)
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback_binary_path(bin_name))
}

pub fn run(command: &mut Command) -> Output {
    command.output().expect("run command")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

pub fn assert_failure_code(output: &Output, code: i32, label: &str) {
    assert!(
        !output.status.success(),
        "{label} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
    assert_eq!(
        output.status.code(),
        Some(code),
        "{label} exit mismatch\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

pub fn assert_usage_error(output: &Output, flag: &str) {
    assert!(
        !output.status.success(),
        "expected usage error for {flag}, got success"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected clap usage exit for {flag}; stderr:\n{}",
        stderr(output)
    );

    let stderr = stderr(output);
    assert!(
        stderr.contains(flag),
        "expected stderr to mention {flag}; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Usage:"),
        "expected clap usage text for {flag}; stderr:\n{stderr}"
    );
}

fn fallback_binary_path(bin_name: &str) -> PathBuf {
    let mut path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("target")
        .join("debug")
        .join(bin_name);
    path.set_extension(env::consts::EXE_EXTENSION);
    path
}

fn path_with_binary_dir() -> OsString {
    let bin_dir = binary_path("git-outpost")
        .parent()
        .expect("binary directory")
        .to_path_buf();
    let existing = env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin_dir).chain(env::split_paths(&existing));

    env::join_paths(paths).expect("join PATH")
}

fn with_git_env(mut command: Command, env: &[(OsString, OsString)]) -> Command {
    command.envs(env.iter().cloned());
    command
}

fn hermetic_git_env(empty_gitconfig: &Path) -> Vec<(OsString, OsString)> {
    vec![
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
    ]
}
