#[allow(dead_code)]
mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{Metadata, Outpost, OutpostError, OutpostStateStore, RemoteName};

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn metadata(source_repo: PathBuf) -> Metadata {
    Metadata {
        source_repo,
        remote_name: RemoteName::parse("local").expect("remote"),
    }
}

#[test]
fn metadata_initialization_reports_a_state_parent_file() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost_path, &fixture.git_env).expect("opened outpost");
    let state_parent = opened
        .metadata_path()
        .parent()
        .expect("metadata parent")
        .to_path_buf();
    fs::remove_dir_all(&state_parent).expect("remove metadata state directory");
    fs::write(&state_parent, "not a directory").expect("state parent file");

    let error = opened
        .state_store()
        .initialize_metadata(&metadata(canonical(&fixture.source)))
        .expect_err("state parent file must block metadata initialization");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == state_parent
                && matches!(source.kind(), std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::NotADirectory)
    ));
}

#[test]
fn metadata_replace_reports_a_destination_directory() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost_path, &fixture.git_env).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove metadata");
    fs::create_dir(&metadata_path).expect("metadata directory");

    let error = metadata(canonical(&fixture.source))
        .write(&fixture.invoker(&outpost_path))
        .expect_err("metadata directory must block replacement");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == metadata_path
    ));
}

#[cfg(unix)]
#[test]
fn metadata_write_reports_an_unresolvable_absolute_source() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let blocker = fixture.root.join("source-blocker");
    let source_path = blocker.join("child");
    fs::write(&blocker, "not a directory").expect("source blocker");

    let error = metadata(source_path.clone())
        .write(&fixture.invoker(&outpost_path))
        .expect_err("unresolvable source path must fail");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == source_path
                && matches!(source.kind(), std::io::ErrorKind::NotADirectory | std::io::ErrorKind::InvalidInput)
    ));
}

#[test]
fn metadata_write_accepts_an_absolute_git_dir_report() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost_path, &fixture.git_env).expect("opened outpost");
    let git = fixture.invoker(&outpost_path).with_env(
        OsString::from("GIT_DIR"),
        opened.git_dir().as_os_str().to_os_string(),
    );

    metadata(canonical(&fixture.source))
        .write(&git)
        .expect("absolute git-dir report must resolve");
    assert!(opened.metadata_path().is_file());
}

#[cfg(target_os = "linux")]
#[test]
fn metadata_write_reports_an_existing_non_utf8_source_path() {
    use std::os::unix::ffi::OsStringExt;

    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost_path, &fixture.git_env).expect("opened outpost");
    let source_path = fixture
        .root
        .join(OsString::from_vec(b"source-\xff".to_vec()));
    fs::create_dir(&source_path).expect("non-UTF-8 source directory");

    let error = metadata(source_path)
        .write(&fixture.invoker(&outpost_path))
        .expect_err("non-UTF-8 source path must fail JSON serialization");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == opened.metadata_path()
                && source.kind() == std::io::ErrorKind::Other
    ));
}
