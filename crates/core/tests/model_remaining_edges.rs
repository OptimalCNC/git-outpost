#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::safety;
use outpost_core::{Outpost, OutpostError, RemoteName};

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn error_of<T>(result: outpost_core::OutpostResult<T>) -> OutpostError {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(error) => error,
    }
}

#[test]
fn outpost_tracking_returns_none_when_only_merge_config_is_missing() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    fixture
        .invoker(&outpost_path)
        .run_check(["config", "--unset-all", "branch.main.merge"])
        .expect("unset merge config");

    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(outpost.upstream_tracking().expect("read tracking"), None);
}

#[test]
fn unpushed_commits_rejects_tracking_a_different_branch() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source");
    fixture
        .invoker(&outpost_path)
        .run_check(["config", "branch.main.merge", "refs/heads/other"])
        .expect("configure different tracked branch");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    let error = outpost
        .unpushed_commits(&source)
        .expect_err("tracking a different branch must be rejected");

    assert!(matches!(
        error,
        OutpostError::NoUpstreamTracking { branch } if branch == "main"
    ));
}

#[test]
fn ahead_behind_propagates_fetch_failure_for_missing_remote() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    fixture
        .invoker(&outpost_path)
        .run_check([
            "remote",
            "set-url",
            "local",
            fixture.root.join("missing-remote").to_str().expect("path"),
        ])
        .expect("point source remote at missing repository");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    let error = outpost
        .ahead_behind_source()
        .expect_err("fetching missing remote must fail");

    assert!(matches!(error, OutpostError::GitFailed { .. }));
}

#[test]
fn registry_load_reports_io_error_when_registry_path_is_directory() {
    let fixture = AbcFixture::new();
    let source_repo = fixture.source_repo().expect("source");
    let registry_path = source_repo.registry_path();
    fs::create_dir_all(&registry_path).expect("registry path directory");

    let error = source_repo
        .registry()
        .expect_err("directory cannot be read as registry JSON");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == registry_path
    ));
}

#[test]
fn registry_save_reports_blocked_state_directory_without_losing_exclude() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let outpost_path = fixture.root.join("C");
    fs::create_dir_all(&outpost_path).expect("outpost path");
    let mut registry = source.registry_mut().expect("registry mut");
    fs::write(source.git_dir().join("outpost"), "blocking file").expect("block state directory");
    registry
        .add(
            outpost_core::RegistryEntry::new(
                outpost_path.clone(),
                RemoteName::parse("local").expect("remote"),
            )
            .expect("entry"),
        )
        .expect("add entry");

    let error = registry
        .save()
        .expect_err("blocked state path must fail save");

    assert!(matches!(error, OutpostError::IoAt { .. }));
    assert!(!source.registry_path().exists());
    let exclude = fs::read_to_string(source.local_exclude_path_for_tests()).expect("exclude");
    assert!(exclude.lines().any(|line| line.trim() == ".outpost/"));
}

#[test]
fn managed_outpost_check_maps_missing_recorded_source_to_unmanaged() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source");
    fs::rename(&fixture.source, fixture.root.join("source-moved")).expect("move source");

    let error = error_of(safety::check_path_is_managed_outpost_of(
        &source,
        &outpost_path,
    ));

    assert!(matches!(
        error,
        OutpostError::RegistryEntryNotManaged(path) if path == canonical(&outpost_path)
    ));
}

#[test]
fn managed_outpost_check_reports_io_for_missing_candidate_path() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let candidate = fixture.root.join("missing");

    let error = error_of(safety::check_path_is_managed_outpost_of(
        &source, &candidate,
    ));

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == candidate && source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn destination_check_reports_io_when_parent_does_not_exist() {
    let fixture = AbcFixture::new();
    let parent = fixture.root.join("missing-parent");

    let error = safety::check_destination_clean(&parent, Path::new("outpost"))
        .expect_err("missing parent cannot resolve destination");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == parent && source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn destination_check_propagates_io_when_parent_is_not_a_directory() {
    let fixture = AbcFixture::new();
    let parent = fixture.root.join("parent-file");
    fs::write(&parent, "not a directory").expect("parent file");

    let error = safety::check_destination_clean(&parent, Path::new("outpost"))
        .expect_err("file parent cannot be used as a working directory");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == parent
    ));
}
