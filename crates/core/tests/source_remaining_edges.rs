#[allow(dead_code)]
mod common;

use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::path::PathBuf;

use common::fixture::AbcFixture;
use outpost_core::{BranchName, OutpostError, RemoteName, SourceRepo};

#[test]
fn discover_from_nested_path_finds_the_canonical_source_and_preserves_env() {
    let fixture = AbcFixture::new();
    let nested = fixture.source.join("nested");
    fs::create_dir(&nested).expect("nested directory");

    let source = SourceRepo::discover_with(&nested, &fixture.git_env).expect("discover source");

    assert_eq!(
        source.work_tree(),
        fs::canonicalize(&fixture.source).unwrap()
    );
    assert_eq!(source.env(), &fixture.git_env);
}

#[test]
fn discover_missing_path_preserves_the_underlying_io_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-repository");

    let err = SourceRepo::discover(&missing)
        .err()
        .expect("missing path must fail");

    assert!(matches!(err, OutpostError::IoAt { path, .. } if path == missing));
}

#[test]
fn resolve_outpost_destination_handles_bare_absolute_and_relative_paths() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container directory");
    source
        .set_outpost_container(&container)
        .expect("set outpost container");

    let cwd = fixture.root.join("working");
    let absolute = fixture.root.join("absolute-outpost");
    assert_eq!(
        source
            .resolve_outpost_destination(&cwd, Path::new("named"))
            .expect("bare destination"),
        fs::canonicalize(&container).unwrap().join("named")
    );
    assert_eq!(
        source
            .resolve_outpost_destination(&cwd, &absolute)
            .expect("absolute destination"),
        absolute
    );
    assert_eq!(
        source
            .resolve_outpost_destination(&cwd, Path::new("nested/name"))
            .expect("relative destination"),
        cwd.join("nested/name")
    );
}

#[test]
fn suggest_outpost_container_returns_none_for_an_empty_registry() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source.suggest_outpost_container().expect("suggestion"),
        None
    );
}

#[test]
fn outpost_at_and_remote_url_read_existing_repository_metadata() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("outpost").expect("outpost");
    let source = fixture.source_repo().expect("source repo");

    let outpost = source.outpost_at(&outpost_path).expect("open outpost");
    assert_eq!(
        outpost.work_tree(),
        fs::canonicalize(&outpost_path).unwrap()
    );
    assert_eq!(
        source.remote_url(&remote("origin")).expect("origin URL"),
        fixture.upstream.to_string_lossy()
    );
}

#[test]
fn remote_url_propagates_a_missing_remote_error() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    assert!(matches!(
        source
            .remote_url(&remote("missing"))
            .expect_err("missing remote"),
        OutpostError::GitFailed { .. }
    ));
}

#[cfg(unix)]
#[test]
fn resolve_destination_accepts_a_non_utf8_relative_path_as_relative() {
    use std::os::unix::ffi::OsStringExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let cwd = fixture.root.join("working");
    let name = OsString::from_vec(vec![b'n', b'a', 0x80]);
    let path = PathBuf::from(&name);

    assert_eq!(
        source
            .resolve_outpost_destination(&cwd, &path)
            .expect("non-UTF-8 relative destination"),
        cwd.join(path)
    );
}

#[test]
fn fast_forward_updates_the_primary_checked_out_worktree() {
    let fixture = AbcFixture::new();
    let expected = fixture
        .commit_in_upstream("main", "advance main")
        .expect("upstream commit");
    let source = fixture.source_repo().expect("source repo");

    source
        .fast_forward_branch_from_origin(&branch("main"))
        .expect("fast-forward primary worktree");

    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "HEAD")
            .expect("updated primary HEAD"),
        expected
    );
}

#[test]
fn fast_forward_is_a_noop_when_local_and_origin_oids_match() {
    let fixture = AbcFixture::new();
    let expected = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("synchronized primary HEAD");
    let source = fixture.source_repo().expect("source repo");

    source
        .fast_forward_branch_from_origin(&branch("main"))
        .expect("already synchronized branch");

    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "HEAD")
            .expect("unchanged primary HEAD"),
        expected
    );
}

fn branch(value: &str) -> BranchName {
    BranchName::parse(value).expect("branch")
}

fn remote(value: &str) -> RemoteName {
    RemoteName::parse(value).expect("remote")
}
