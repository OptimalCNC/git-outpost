#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use outpost_core::{GitInvoker, OutpostError};
use tempfile::TempDir;

#[test]
fn invoker_applies_its_cwd_and_environment_to_the_child() {
    let fake = FakeGit::new();
    let git = fake.invoker();

    assert_eq!(git.cwd(), fake.cwd.as_path());
    assert_eq!(
        git.run_capture(["context"])
            .expect("fake git should succeed"),
        format!(
            "cwd={}\nenv=per-invoker-value",
            fs::canonicalize(&fake.cwd)
                .expect("canonical working directory")
                .to_string_lossy()
        )
    );
}

#[test]
fn run_capture_returns_trimmed_stdout_on_success() {
    let fake = FakeGit::new();

    assert_eq!(
        fake.invoker()
            .run_capture(["trim"])
            .expect("fake git should succeed"),
        "capture result"
    );
}

#[test]
fn run_check_reports_unspawnable_git_at_its_cwd() {
    let fake = FakeGit::new();

    let error = fake
        .invoker_without_git()
        .run_check(["anything"])
        .expect_err("an empty per-invoker PATH cannot spawn git");

    assert_spawn_failed_at(error, &fake.cwd);
}

#[test]
fn run_capture_lossily_decodes_invalid_utf8_stdout() {
    let fake = FakeGit::new();

    assert_eq!(
        fake.invoker()
            .run_capture(["invalid-stdout"])
            .expect("fake git should succeed"),
        "\u{fffd} stdout"
    );
}

#[test]
fn run_check_lossily_decodes_invalid_utf8_stderr() {
    let fake = FakeGit::new();

    let error = fake
        .invoker()
        .run_check(["invalid-stderr"])
        .expect_err("fake git should fail");

    assert_git_failed(error, r#"["invalid-stderr"]"#, 42, "\u{fffd} stderr");
}

#[test]
fn run_capture_reports_unix_signal_termination() {
    let fake = FakeGit::new();

    let error = fake
        .invoker()
        .run_capture(["terminate"])
        .expect_err("fake git should terminate itself");

    match error {
        OutpostError::GitTerminatedBySignal { args, signal_str } => {
            assert_eq!(args, r#"["terminate"]"#);
            assert_eq!(signal_str, " (signal 15)");
        }
        other => panic!("expected GitTerminatedBySignal, got {other:?}"),
    }
}

fn assert_git_failed(error: OutpostError, args: &str, code: i32, stderr: &str) {
    match error {
        OutpostError::GitFailed {
            args: actual_args,
            code: actual_code,
            stderr: actual_stderr,
        } => {
            assert_eq!(actual_args, args);
            assert_eq!(actual_code, code);
            assert_eq!(actual_stderr, stderr);
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
}

fn assert_spawn_failed_at(error: OutpostError, cwd: &Path) {
    match error {
        OutpostError::IoAt { path, source } => {
            assert_eq!(path, cwd);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("expected IoAt, got {other:?}"),
    }
}

struct FakeGit {
    _temp: TempDir,
    cwd: PathBuf,
    bin: PathBuf,
    empty_bin: PathBuf,
}

impl FakeGit {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary test directory");
        let cwd = temp.path().join("working directory");
        let bin = temp.path().join("bin");
        let empty_bin = temp.path().join("empty-bin");
        fs::create_dir_all(&cwd).expect("working directory");
        fs::create_dir(&bin).expect("fake git directory");
        fs::create_dir(&empty_bin).expect("empty PATH directory");

        let executable = bin.join("git");
        fs::write(&executable, FAKE_GIT_SCRIPT).expect("fake git script");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
            .expect("make fake git executable");

        Self {
            _temp: temp,
            cwd,
            bin,
            empty_bin,
        }
    }

    fn invoker(&self) -> GitInvoker {
        GitInvoker::at(&self.cwd)
            .with_env("PATH", self.bin.clone())
            .with_env("GIT_INVOKER_EDGE_TOKEN", "per-invoker-value")
    }

    fn invoker_without_git(&self) -> GitInvoker {
        GitInvoker::at(&self.cwd).with_env("PATH", self.empty_bin.clone())
    }
}

const FAKE_GIT_SCRIPT: &str = r#"#!/bin/sh
case "$1" in
    context)
        printf 'cwd=%s\nenv=%s\n' "$PWD" "$GIT_INVOKER_EDGE_TOKEN"
        ;;
    trim)
        printf '\n  capture result  \t\n'
        ;;
    invalid-stdout)
        printf '\377 stdout \n'
        ;;
    invalid-stderr)
        printf '\377 stderr \n' >&2
        exit 42
        ;;
    terminate)
        kill -TERM "$$"
        ;;
    *)
        printf 'unexpected fake git action: %s\n' "$1" >&2
        exit 99
        ;;
esac
"#;
