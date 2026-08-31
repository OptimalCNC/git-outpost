#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use outpost_core::{GitInvoker, OutpostError};
#[cfg(unix)]
use tempfile::TempDir;

#[test]
fn run_capture_maps_a_missing_working_directory_to_io_at() {
    let temp = tempfile::tempdir().expect("temporary test directory");
    let missing = temp.path().join("missing-working-directory");

    let error = GitInvoker::at(&missing)
        .run_capture(["anything"])
        .expect_err("a missing cwd cannot spawn git");

    match error {
        OutpostError::IoAt { path, source } => {
            assert_eq!(path, missing);
            assert!(matches!(
                source.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ));
        }
        other => panic!("expected IoAt, got {other:?}"),
    }
}

#[test]
#[cfg(unix)]
fn git_error_display_escapes_non_utf8_and_control_argv_bytes() {
    let fake = FakeGit::new();
    let unusual = OsString::from_vec(vec![b'a', 0xff, b'\n', b'"', b'\\']);

    let error = fake
        .invoker()
        .run_check([OsString::from("fail"), unusual])
        .expect_err("the shim should return its failure status");

    assert_eq!(
        error.to_string(),
        r#"git command failed: `git ["fail", "a\xff\n\"\\"]` (exit 42): fake stderr"#
    );
}

#[test]
fn container_error_display_shell_quotes_a_path_with_spaces_and_apostrophe() {
    let error = OutpostError::OutpostContainerNotConfigured {
        name: "C".to_owned(),
        suggestion: Some(PathBuf::from("/tmp/out post's")),
    };

    assert_eq!(
        error.to_string(),
        "outpost container is not configured for bare outpost name C; run `gop config set outpost-container '/tmp/out post'\\''s'`"
    );
}

#[cfg(unix)]
struct FakeGit {
    _temp: TempDir,
    bin: PathBuf,
    cwd: PathBuf,
}

#[cfg(unix)]
impl FakeGit {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary test directory");
        let root = temp.path();
        let bin = root.join("bin");
        let cwd = root.join("working directory");
        fs::create_dir(&bin).expect("fake git directory");
        fs::create_dir(&cwd).expect("working directory");

        let executable = bin.join("git");
        fs::write(&executable, FAKE_GIT_SCRIPT).expect("fake git script");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
            .expect("make fake git executable");

        Self {
            _temp: temp,
            bin,
            cwd,
        }
    }

    fn invoker(&self) -> GitInvoker {
        self.invoker_at(&self.cwd)
    }

    fn invoker_at(&self, cwd: &Path) -> GitInvoker {
        GitInvoker::at(cwd).with_env("PATH", self.bin.clone())
    }
}

#[cfg(unix)]
const FAKE_GIT_SCRIPT: &str = r#"#!/bin/sh
case "$1" in
    fail)
        printf '\n  fake stderr  \t\n' >&2
        exit 42
        ;;
    *)
        printf 'unexpected fake git action: %s\n' "$1" >&2
        exit 99
        ;;
esac
"#;
