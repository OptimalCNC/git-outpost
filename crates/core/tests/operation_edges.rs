#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::add::{AddCheckout, AddOptions};
use outpost_core::ops::{add, lock, merge, prune, pull, rebase, source, unlock};
use outpost_core::selector::OutpostSelector;
use outpost_core::{
    BranchName, Outpost, OutpostError, OutpostResult, RemoteName, SourceRemoteRef, StepKind,
};

#[test]
fn add_propagates_destination_parent_canonicalization_failure() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let parent = fixture.root.join("not-a-directory");
    fs::write(&parent, "file").expect("parent file");
    let destination = parent.join("C");

    let err = expect_error(
        add::run(
            &source,
            AddOptions {
                destination: destination.clone(),
                checkout: AddCheckout::CheckoutExisting {
                    target_branch: None,
                },
                remote_name: RemoteName::parse("local").expect("remote name"),
            },
            &mut CapturingReporter::default(),
        ),
        "file parent should fail",
    );

    assert!(matches!(err, OutpostError::IoAt { path, .. } if path == parent));
    assert!(!destination.exists());
}

#[test]
fn move_rejects_empty_destination_without_moving_outpost() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        outpost_core::ops::r#move::run(
            &source,
            outpost_core::ops::r#move::MoveOptions {
                selector: OutpostSelector::from_path(outpost_path.clone()),
                new_path: PathBuf::new(),
                force: false,
            },
        ),
        "empty destination should fail",
    );

    assert!(matches!(err, OutpostError::IoAt { path, .. } if path == Path::new("")));
    assert!(outpost_path.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost_path));
}

#[test]
fn pull_reports_no_updates_when_source_and_outpost_are_current() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let head_before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = pull::run(&outpost, pull::PullOptions, &mut reporter).expect("no-op pull");

    assert!(!report.source_updated);
    assert!(!report.outpost_updated);
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        head_before
    );
    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::SourceFetch, StepKind::OutpostFetch]
    );
}

#[test]
fn pull_propagates_source_fetch_failure_and_leaves_outpost_unchanged() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let head_before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "remote",
            "set-url",
            "origin",
            fixture.root.join("missing.git").to_str().unwrap(),
        ])
        .expect("break source origin");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        pull::run(&outpost, pull::PullOptions, &mut reporter),
        "source fetch should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        head_before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::SourceFetch]);
}

#[test]
fn source_pull_reports_no_update_when_branch_is_current() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let mut reporter = CapturingReporter::default();

    let report = source::pull(
        &open_outpost(&fixture, &outpost_path),
        source::SourcePullOptions {
            branch: branch("main"),
        },
        &mut reporter,
    )
    .expect("no-op source pull");

    assert!(!report.updated);
    assert_eq!(report.branch.as_str(), "main");
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::SourceFetch]);
}

#[test]
fn source_pull_propagates_origin_fetch_failure() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "remote",
            "set-url",
            "origin",
            fixture.root.join("missing.git").to_str().unwrap(),
        ])
        .expect("break source origin");
    let source_head = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source head");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        source::pull(
            &open_outpost(&fixture, &outpost_path),
            source::SourcePullOptions {
                branch: branch("main"),
            },
            &mut reporter,
        ),
        "source fetch should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_head
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::SourceFetch]);
}

#[test]
fn merge_is_successful_no_op_when_source_ref_is_current() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    let mut reporter = CapturingReporter::default();

    let report = merge::run(
        &open_outpost(&fixture, &outpost_path),
        merge::MergeOptions {
            source_ref: source_ref("local/main"),
        },
        &mut reporter,
    )
    .expect("no-op merge");

    assert_eq!(report.source_ref, source_ref("local/main"));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostFetch]);
}

#[test]
fn merge_propagates_missing_source_ref_and_preserves_head() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        merge::run(
            &open_outpost(&fixture, &outpost_path),
            merge::MergeOptions {
                source_ref: source_ref("local/missing"),
            },
            &mut reporter,
        ),
        "missing source ref should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostFetch]);
}

#[test]
fn merge_reports_conflict_without_advancing_outpost_head() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost_commit = fixture
        .commit_file_in_outpost(&outpost_path, "outpost side", "conflict.txt", "outpost\n")
        .expect("outpost commit");
    fixture
        .commit_file_in_source("source side", "conflict.txt", "source\n")
        .expect("source commit");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        merge::run(
            &open_outpost(&fixture, &outpost_path),
            merge::MergeOptions {
                source_ref: source_ref("local/main"),
            },
            &mut reporter,
        ),
        "merge conflict should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        outpost_commit
    );
    assert!(
        fs::read_to_string(outpost_path.join("conflict.txt"))
            .unwrap()
            .contains("<<<<<<<")
    );
}

#[test]
fn rebase_is_successful_no_op_when_source_ref_is_current() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    let mut reporter = CapturingReporter::default();

    let report = rebase::run(
        &open_outpost(&fixture, &outpost_path),
        rebase::RebaseOptions {
            source_ref: source_ref("local/main"),
        },
        &mut reporter,
    )
    .expect("no-op rebase");

    assert_eq!(report.source_ref, source_ref("local/main"));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostFetch]);
}

#[test]
fn rebase_propagates_missing_source_ref_and_preserves_head() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        rebase::run(
            &open_outpost(&fixture, &outpost_path),
            rebase::RebaseOptions {
                source_ref: source_ref("local/missing"),
            },
            &mut reporter,
        ),
        "missing source ref should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        before
    );
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostFetch]);
}

#[test]
fn rebase_reports_conflict_and_leaves_conflict_state() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(&outpost_path, "outpost side", "conflict.txt", "outpost\n")
        .expect("outpost commit");
    fixture
        .commit_file_in_source("source side", "conflict.txt", "source\n")
        .expect("source commit");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        rebase::run(
            &open_outpost(&fixture, &outpost_path),
            rebase::RebaseOptions {
                source_ref: source_ref("local/main"),
            },
            &mut reporter,
        ),
        "rebase conflict should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert!(
        fs::read_to_string(outpost_path.join("conflict.txt"))
            .unwrap()
            .contains("<<<<<<<")
    );
}

#[test]
fn prune_keeps_registered_file_paths_without_classifying_them_as_orphans() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let path = fixture.root.join("registered-file");
    fs::write(&path, "not an outpost").expect("registered file");
    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .add(
            outpost_core::RegistryEntry::new(
                path.clone(),
                RemoteName::parse("local").expect("remote name"),
            )
            .expect("registry entry"),
        )
        .expect("register file");
    registry.save().expect("save registry");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune registered file");

    assert!(report.removed_entries.is_empty());
    assert!(report.orphaned_source_missing.is_empty());
    assert!(report.locked_entries.is_empty());
    assert_eq!(single_entry(&source).path, canonical(&path));
}

#[test]
fn lock_is_idempotent_and_unlock_is_idempotent() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let selector = OutpostSelector::from_path(outpost_path.clone());

    lock::run(
        &source,
        lock::LockOptions {
            selector: selector.clone(),
            reason: Some("keep".to_owned()),
        },
    )
    .expect("first lock");
    lock::run(
        &source,
        lock::LockOptions {
            selector,
            reason: Some("keep".to_owned()),
        },
    )
    .expect("second lock");
    assert_eq!(source.registry().expect("registry").entries().len(), 1);
    assert!(single_entry(&source).locked);
    assert_eq!(single_entry(&source).lock_reason.as_deref(), Some("keep"));

    let selector = OutpostSelector::from_path(outpost_path);
    unlock::run(
        &source,
        unlock::UnlockOptions {
            selector: selector.clone(),
        },
    )
    .expect("first unlock");
    unlock::run(&source, unlock::UnlockOptions { selector }).expect("second unlock");
    assert!(!single_entry(&source).locked);
    assert!(single_entry(&source).lock_reason.is_none());
}

fn open_outpost(fixture: &AbcFixture, path: &Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

fn branch(name: &str) -> BranchName {
    BranchName::parse(name).expect("branch name")
}

fn source_ref(value: &str) -> SourceRemoteRef {
    SourceRemoteRef::parse(value).expect("source ref")
}

fn single_entry(source: &outpost_core::SourceRepo) -> outpost_core::RegistryEntry {
    let registry = source.registry().expect("registry");
    assert_eq!(registry.entries().len(), 1);
    registry.entries()[0].clone()
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}
