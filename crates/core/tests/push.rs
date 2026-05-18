#[allow(dead_code)]
mod common;

use std::fs;
use std::path::Path;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::push::{run, PushOptions, StepResult};
use outpost_core::{Outpost, OutpostError, OutpostResult, StepKind};

#[test]
fn pu01_push_sends_outpost_branch_to_source_then_origin() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn pu02_push_records_outpost_push_and_source_push_events() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::OutpostPush, StepKind::SourcePush]
    );
    assert!(
        reporter.warnings.is_empty(),
        "push should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

#[test]
fn pu03_push_from_outpost_only_branch_returns_ambiguous_branch_creation() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check(["switch", "-c", "feature/outpost-only"])
        .expect("create outpost-only branch");
    fixture
        .commit_in_outpost(&outpost_path, "outpost-only side")
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject outpost-only branch creation",
    );

    assert!(
        matches!(err, OutpostError::AmbiguousBranchCreation { branch } if branch == "feature/outpost-only")
    );
    assert!(reporter.steps.is_empty());
    assert!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/feature/outpost-only")
            .is_err(),
        "source branch should not be created"
    );
}

#[test]
fn pu04_push_when_source_diverged_from_outpost_returns_divergence() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    fixture
        .commit_file_in_source("source side", "source.txt", "from source\n")
        .expect("source commit");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject outpost/source divergence",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

#[test]
fn pu05_push_dirty_outpost_succeeds() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "committed outpost side",
            "committed.txt",
            "committed\n",
        )
        .expect("outpost commit");
    fs::write(outpost_path.join("dirty.txt"), "dirty\n").expect("dirty outpost");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn pu06_push_with_missing_source_returns_source_missing() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    let expected_source = fs::canonicalize(&fixture.source).expect("canonical source");
    let moved_source = fixture.root.join("B.moved");
    fs::rename(&fixture.source, &moved_source).expect("move source repo");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject missing source",
    );

    assert!(matches!(err, OutpostError::SourceMissing(path) if path == expected_source));
    assert!(reporter.steps.is_empty());
}

#[test]
fn pu07_push_uses_custom_remote_for_outpost_to_source_and_origin_for_source_to_upstream() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture
        .add_outpost_with_remote("C", "custom")
        .expect("add custom outpost");
    assert!(
        fixture
            .invoker(&outpost_path)
            .run_capture(["remote", "get-url", "local"])
            .is_err(),
        "custom outpost should not rely on a local remote"
    );
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "custom outpost side",
            "custom.txt",
            "custom\n",
        )
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn pu08_push_into_dirty_checked_out_source_branch_surfaces_update_instead_git_failed() {
    let fixture = AbcFixture::new();
    fixture
        .commit_file_in_source("baseline tracked file", "tracked.txt", "base\n")
        .expect("source baseline commit");
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    fs::write(fixture.source.join("tracked.txt"), "dirty source\n").expect("dirty source");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should surface Git's updateInstead failure",
    );

    match err {
        OutpostError::GitFailed { stderr, .. } => {
            assert!(
                !stderr.is_empty(),
                "GitFailed should preserve stderr from updateInstead failure"
            );
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostPush]);
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

#[test]
fn pu09_push_with_deny_current_branch_refuse_returns_push_into_checked_out_branch() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "--local", "receive.denyCurrentBranch", "refuse"])
        .expect("set denyCurrentBranch");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject checked-out source branch when denyCurrentBranch refuses",
    );

    assert!(
        matches!(err, OutpostError::PushIntoCheckedOutBranch { source, branch } if source == canonical_source(&fixture) && branch == "main")
    );
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
}

#[test]
fn pu10_push_on_detached_head_returns_no_upstream_tracking_head_before_push() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost HEAD");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject detached HEAD",
    );

    assert!(matches!(err, OutpostError::NoUpstreamTracking { branch } if branch == "HEAD"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

fn outpost(fixture: &AbcFixture, path: &Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

fn assert_pushed_commits(result: &StepResult, expected: u32) {
    match result {
        StepResult::Pushed { commits } => assert_eq!(*commits, expected),
    }
}

fn canonical_source(fixture: &AbcFixture) -> std::path::PathBuf {
    fs::canonicalize(&fixture.source).expect("canonical source")
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}
