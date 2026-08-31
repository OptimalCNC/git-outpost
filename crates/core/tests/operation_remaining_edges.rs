#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::add::{AddCheckout, AddOptions};
use outpost_core::ops::{add, list, prune, pull};
use outpost_core::selector::OutpostSelector;
use outpost_core::{BranchName, Outpost, OutpostError, OutpostResult, RemoteName, StepKind};

#[test]
fn list_reports_detached_outpost_without_branch_or_comparison() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost");
    let source = fixture.source_repo().expect("source repo");

    let summaries = list::run(&source).expect("list detached outpost");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].path, canonical(&outpost_path));
    assert!(summaries[0].current_branch.is_none());
    assert!(summaries[0].ahead_behind.is_none());
    assert_eq!(summaries[0].state, list::OutpostState::Clean);
}

#[test]
fn list_suppresses_comparison_when_outpost_has_no_tracking() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost_path)
        .run_check(["config", "--local", "--unset", "branch.main.remote"])
        .expect("remove tracking remote");
    let source = fixture.source_repo().expect("source repo");

    let summaries = list::run(&source).expect("list untracked outpost");

    assert_eq!(
        summaries[0].current_branch.as_ref().map(BranchName::as_str),
        Some("main")
    );
    assert!(summaries[0].ahead_behind.is_none());
    assert_eq!(summaries[0].state, list::OutpostState::Clean);
}

#[test]
fn add_new_branch_propagates_source_branch_collision_after_clone() {
    let fixture = AbcFixture::new();
    let source_branch = fixture
        .create_source_branch("feature/existing")
        .expect("create existing branch");
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    let err = expect_error(
        add::run(
            &source,
            AddOptions {
                destination: destination.clone(),
                checkout: AddCheckout::NewBranch {
                    name: source_branch.clone(),
                    target_branch: Some(branch("main")),
                },
                remote_name: remote("local"),
            },
            &mut CapturingReporter::default(),
        ),
        "duplicate source branch should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert!(source.branch_exists(&source_branch).expect("branch exists"));
    assert!(
        !source
            .registry()
            .expect("registry")
            .entries()
            .iter()
            .any(|entry| entry.path == canonical(&destination))
    );
}

#[test]
fn move_rejects_destination_with_missing_parent_before_rename() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("missing-parent").join("D");

    let err = expect_error(
        outpost_core::ops::r#move::run(
            &source,
            outpost_core::ops::r#move::MoveOptions {
                selector: OutpostSelector::from_path(outpost_path.clone()),
                new_path: destination,
                force: false,
            },
        ),
        "missing destination parent should fail",
    );

    assert!(
        matches!(err, OutpostError::IoAt { path, .. } if path == fixture.root.join("missing-parent"))
    );
    assert!(outpost_path.exists());
    assert_eq!(
        source.registry().expect("registry").entries()[0].path,
        canonical(&outpost_path)
    );
}

#[test]
fn move_locked_without_reason_reports_empty_lock_reason() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry");
    registry
        .lock(&outpost_path, None)
        .expect("lock without reason");
    registry.save().expect("save lock");

    let err = expect_error(
        outpost_core::ops::r#move::run(
            &source,
            outpost_core::ops::r#move::MoveOptions {
                selector: OutpostSelector::from_path(outpost_path.clone()),
                new_path: fixture.root.join("D"),
                force: false,
            },
        ),
        "locked move should fail",
    );

    assert!(
        matches!(err, OutpostError::OutpostLocked { path, reason } if path == canonical(&outpost_path) && reason.is_empty())
    );
    assert!(outpost_path.exists());
}

#[test]
fn prune_keeps_registered_unmanaged_git_directory() {
    let fixture = AbcFixture::new();
    let unmanaged = fixture.root.join("unmanaged");
    fixture
        .invoker(&fixture.root)
        .run_check([
            std::ffi::OsStr::new("init"),
            std::ffi::OsStr::new("--initial-branch=main"),
            unmanaged.as_os_str(),
        ])
        .expect("initialize unmanaged repository");
    let source = fixture.source_repo().expect("source repo");
    register_path(&source, &unmanaged);

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune unmanaged directory");

    assert!(report.removed_entries.is_empty());
    assert!(report.orphaned_source_missing.is_empty());
    assert!(report.locked_entries.is_empty());
    assert_eq!(
        source.registry().expect("registry").entries()[0].path,
        canonical(&unmanaged)
    );
}

#[test]
fn prune_propagates_invalid_metadata_for_existing_registered_directory() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let outpost = Outpost::at(&outpost_path).expect("open outpost");
    fs::write(outpost.metadata_path(), "not metadata").expect("corrupt metadata");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        prune::run(
            &source,
            prune::PruneOptions {
                dry_run: false,
                verbose: false,
            },
        ),
        "bad metadata should be returned",
    );

    assert!(
        matches!(err, OutpostError::BadMetadata { outpost, .. } if outpost == canonical(&outpost_path))
    );
}

#[test]
fn pull_reports_no_outpost_update_when_outpost_is_ahead_only() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("head before");
    fixture
        .commit_in_outpost(&outpost_path, "outpost-only")
        .expect("outpost commit");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    let mut reporter = CapturingReporter::default();

    let report = pull::run(&outpost, pull::PullOptions, &mut reporter).expect("pull ahead-only");

    assert!(!report.source_updated);
    assert!(!report.outpost_updated);
    assert_ne!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("head after"),
        before
    );
    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::SourceFetch, StepKind::OutpostFetch]
    );
}

fn branch(name: &str) -> BranchName {
    BranchName::parse(name).expect("branch")
}

fn remote(name: &str) -> RemoteName {
    RemoteName::parse(name).expect("remote")
}

fn register_path(source: &outpost_core::SourceRepo, path: &Path) {
    let mut registry = source.registry_mut().expect("registry");
    registry
        .add(outpost_core::RegistryEntry::new(path.to_path_buf(), remote("local")).expect("entry"))
        .expect("register path");
    registry.save().expect("save registry");
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
