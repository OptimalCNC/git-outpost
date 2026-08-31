#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::{push, remove};
use outpost_core::selector::OutpostSelector;
use outpost_core::{Outpost, OutpostError, StepKind};

#[test]
fn push_allows_clean_checked_out_source_with_update_instead() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "receive.denyCurrentBranch", "updateInstead"])
        .expect("allow updates to the checked-out source branch");
    let outpost_oid = fixture
        .commit_in_outpost(&outpost_path, "outpost commit")
        .expect("commit in outpost");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = push::run(&outpost, push::PushOptions, &mut reporter)
        .expect("updateInstead permits a clean checked-out source branch");

    assert_eq!(
        report.outpost_to_source,
        push::StepResult::Pushed { commits: 1 }
    );
    assert_eq!(
        report.source_to_origin,
        push::StepResult::Pushed { commits: 1 }
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source branch"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin branch"),
        outpost_oid
    );
    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::OutpostPush, StepKind::SourcePush]
    );
}

#[test]
fn remove_with_disabled_cleanup_reports_skip_and_preserves_source_branch() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feature/disabled-cleanup")
        .expect("create feature branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");

    let report = remove::run_with_cleanup(
        &source,
        remove_options(&outpost_path, false),
        remove::BranchCleanupMode::Disabled,
    )
    .expect("remove with cleanup disabled");

    assert!(!outpost_path.exists());
    assert!(
        source
            .branch_exists(&branch)
            .expect("source branch remains")
    );
    assert_eq!(
        report.branch_cleanup,
        vec![remove::BranchCleanupOutcome::Skipped {
            branch: None,
            reason: remove::BranchCleanupSkipReason::CleanupDisabled,
        }]
    );
}

#[test]
fn remove_locked_outpost_without_reason_reports_empty_reason_and_preserves_state() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    {
        let mut registry = source.registry_mut().expect("mutable registry");
        registry.lock(&outpost_path, None).expect("lock outpost");
        registry.save().expect("save lock");
    }

    let err = remove::run(&source, remove_options(&outpost_path, false))
        .expect_err("locked outpost must not be removed");

    assert!(matches!(err, OutpostError::OutpostLocked { path, reason }
        if path == canonical(&outpost_path) && reason.is_empty()));
    assert!(outpost_path.exists());
    let registry = source.registry().expect("registry after rejected remove");
    assert_eq!(registry.entries().len(), 1);
    assert!(registry.entries()[0].locked);
}

fn open_outpost(fixture: &AbcFixture, path: &Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

fn remove_options(path: &Path, force: bool) -> remove::RemoveOptions {
    remove::RemoveOptions {
        selector: OutpostSelector::from_path(path.to_path_buf()),
        force,
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
