#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::pull::{PullOptions, run};
use outpost_core::{BranchName, Outpost, OutpostError, OutpostResult, StepKind};

#[test]
fn p01_pull_fast_forwards_source_from_origin_then_outpost_from_source() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let upstream_oid = fixture
        .commit_file_in_upstream(
            "main",
            "advance upstream",
            "upstream.txt",
            "from upstream\n",
        )
        .expect("upstream commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PullOptions, &mut reporter).expect("pull");

    assert!(report.source_updated);
    assert!(report.outpost_updated);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        upstream_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/heads/main")
            .expect("outpost main"),
        upstream_oid
    );
}

#[test]
fn p02_pull_fast_forwards_outpost_from_source_without_touching_origin() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let upstream_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("upstream before");
    let source_oid = fixture
        .commit_file_in_source("advance source", "source.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PullOptions, &mut reporter).expect("pull");

    assert!(!report.source_updated);
    assert!(report.outpost_updated);
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/heads/main")
            .expect("outpost main"),
        source_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("upstream after"),
        upstream_before
    );
}

#[test]
fn p03_pull_returns_divergence_when_source_and_origin_diverge() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_source("source side", "source.txt", "from source\n")
        .expect("source commit");
    fixture
        .commit_file_in_upstream("main", "origin side", "origin.txt", "from origin\n")
        .expect("upstream commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PullOptions, &mut reporter),
        "pull should reject source/origin divergence",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert!(
        !reporter
            .step_kinds()
            .iter()
            .any(|kind| *kind == StepKind::OutpostFetch)
    );
}

#[test]
fn p04_pull_returns_divergence_when_outpost_and_source_diverge() {
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
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PullOptions, &mut reporter),
        "pull should reject outpost/source divergence",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert_eq!(reporter.step_kinds(), vec![StepKind::SourceFetch]);
}

#[test]
fn p05_pull_with_missing_source_returns_source_missing() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let expected_source = fs::canonicalize(&fixture.source).expect("canonical source");
    let moved_source = fixture.root.join("B.moved");
    fs::rename(&fixture.source, &moved_source).expect("move source repo");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PullOptions, &mut reporter),
        "pull should reject missing source",
    );

    assert!(matches!(err, OutpostError::SourceMissing(path) if path == expected_source));
    assert!(reporter.steps.is_empty());
}

#[test]
fn p06_pull_on_detached_head_returns_no_upstream_tracking_head() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost HEAD");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PullOptions, &mut reporter),
        "pull should reject detached HEAD",
    );

    assert!(matches!(err, OutpostError::NoUpstreamTracking { branch } if branch == "HEAD"));
    assert!(reporter.steps.is_empty());
}

#[test]
fn p07_pull_uses_custom_source_remote_name() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture
        .add_outpost_with_remote("C", "custom")
        .expect("add custom outpost");
    assert!(matches!(
        fixture
            .invoker(&outpost_path)
            .run_capture(["remote", "get-url", "local"]),
        Err(OutpostError::GitFailed { .. })
    ));
    let source_oid = fixture
        .commit_file_in_source("advance source", "custom.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PullOptions, &mut reporter).expect("pull");

    assert!(!report.source_updated);
    assert!(report.outpost_updated);
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/heads/main")
            .expect("outpost main"),
        source_oid
    );
}

#[test]
fn p08_pull_records_source_fetch_and_outpost_fetch_events() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_upstream("main", "advance upstream")
        .expect("upstream commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(&outpost, PullOptions, &mut reporter).expect("pull");

    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::SourceFetch, StepKind::OutpostFetch]
    );
    assert!(
        reporter.warnings.is_empty(),
        "pull should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

#[test]
fn p09_pull_missing_matching_source_branch_returns_branch_not_found_before_outpost_ff() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/missing-source")
        .expect("create source branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(feature.clone()))
        .expect("add feature outpost");
    let outpost_before = fixture
        .rev_parse(&outpost_path, "HEAD")
        .expect("outpost before");
    fixture
        .delete_source_branch(&feature)
        .expect("delete source branch");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PullOptions, &mut reporter),
        "pull should reject missing source branch",
    );

    assert!(
        matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == feature.as_str() && repo == canonical_source(&fixture))
    );
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "HEAD")
            .expect("outpost after"),
        outpost_before
    );
}

fn outpost(fixture: &AbcFixture, path: &std::path::Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

#[allow(dead_code)]
fn branch(name: &str) -> BranchName {
    BranchName::parse(name).expect("branch name")
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
