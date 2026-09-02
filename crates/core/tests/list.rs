#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::AbcFixture;
use outpost_core::ops::list::{OutpostHead, OutpostState, run};
use outpost_core::{OutpostError, RegistryEntry, RemoteName, SourceRepo};

#[test]
fn list_empty_source_returns_no_summaries() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    assert!(summaries.is_empty());
}

#[test]
fn list_reports_three_added_outpost_paths() {
    let fixture = AbcFixture::new();
    let one = fixture.add_outpost("C1").expect("add C1");
    let two = fixture.add_outpost("C2").expect("add C2");
    let three = fixture.add_outpost("C3").expect("add C3");
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    let paths = summaries
        .iter()
        .map(|summary| summary.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![canonical(&one), canonical(&two), canonical(&three)]
    );
}

#[test]
fn list_includes_short_unique_outpost_id_prefixes() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C1").expect("add C1");
    fixture.add_outpost("C2").expect("add C2");
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 2);
    for summary in summaries {
        assert_eq!(summary.display_id.len(), 5);
        assert!(summary.display_id.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(
            summary
                .display_id
                .chars()
                .all(|ch| !ch.is_ascii_uppercase())
        );
    }
}

#[test]
fn list_reports_current_branch_for_each_outpost() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let expected_oid = fixture
        .invoker(&outpost)
        .run_capture(["rev-parse", "HEAD"])
        .expect("outpost HEAD");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    match &summaries[0].state {
        OutpostState::Present {
            head_oid,
            head: OutpostHead::Attached(branch),
        } => {
            assert_eq!(head_oid, &expected_oid);
            assert_eq!(branch.as_str(), "main");
        }
        state => panic!("expected attached outpost, got {state:?}"),
    }
}

#[test]
fn list_does_not_fetch_or_change_source_tracking_ref() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let tracking_ref = "refs/remotes/local/main";
    let before = fixture
        .invoker(&outpost)
        .run_capture(["rev-parse", tracking_ref])
        .expect("tracking ref before list");
    let source_head = fixture
        .commit_in_source("source commit")
        .expect("commit in source");
    assert_ne!(before, source_head);
    let source = fixture.source_repo().expect("source repo");

    run(&source).expect("list summaries");

    let after = fixture
        .invoker(&outpost)
        .run_capture(["rev-parse", tracking_ref])
        .expect("tracking ref after list");
    assert_eq!(after, before);
}

#[test]
fn list_reports_missing_registered_outpost() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove outpost");
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].path, canonical_missing(&outpost));
    assert_eq!(summaries[0].state, OutpostState::Missing);
}

#[test]
fn list_reports_not_managed_registered_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let managed = outpost_core::Outpost::at(&outpost).expect("managed outpost");
    std::fs::remove_file(managed.metadata_path()).expect("remove metadata");
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].state, OutpostState::NotManaged);
}

#[test]
fn list_reports_wrong_source_outpost_as_not_managed() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove original outpost");
    let other = AbcFixture::new();
    let other_outpost = other.add_outpost("C").expect("add other C");
    fs::rename(&other_outpost, &outpost).expect("move wrong-source outpost");
    let source = fixture.source_repo().expect("source repo");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].state, OutpostState::NotManaged);
}

#[test]
fn list_reports_registry_remote_mismatch_as_not_managed() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    assert!(registry.remove_by_path(&outpost).expect("remove entry"));
    registry
        .add(
            RegistryEntry::new(
                outpost,
                RemoteName::parse("different").expect("remote name"),
            )
            .expect("registry entry"),
        )
        .expect("add mismatched entry");
    registry.save().expect("save registry");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].state, OutpostState::NotManaged);
}

#[test]
fn list_outside_source_repo_returns_not_a_repo() {
    let temp = tempfile::tempdir().expect("tempdir");

    let err = expect_error(
        SourceRepo::discover(temp.path()),
        "outside repo should fail",
    );

    assert!(matches!(err, OutpostError::NotARepo(path) if path == temp.path()));
}

#[test]
fn list_includes_lock_reason_from_registry() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .lock(&outpost, Some("release freeze".to_owned()))
        .expect("lock outpost");
    registry.save().expect("save registry");

    let summaries = run(&source).expect("list summaries");

    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].locked);
    assert_eq!(summaries[0].lock_reason.as_deref(), Some("release freeze"));
}

fn expect_error<T>(result: outpost_core::OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}

fn canonical(path: &std::path::Path) -> std::path::PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn canonical_missing(path: &std::path::Path) -> std::path::PathBuf {
    let parent = path.parent().expect("path parent");
    canonical(parent).join(path.file_name().expect("file name"))
}
