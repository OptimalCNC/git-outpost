#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::source::{pull, SourcePullOptions};
use outpost_core::{BranchName, Outpost, OutpostError, OutpostResult, StepKind};

#[test]
fn sp01_source_pull_fast_forwards_unchecked_out_source_branch_without_switching() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/source-refresh")
        .expect("create source branch");
    fixture.push_source_branch(&feature).expect("push feature");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(feature.clone()))
        .expect("add feature outpost");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "main"])
        .expect("switch source back to main");
    let upstream_oid = fixture
        .commit_file_in_upstream(
            feature.as_str(),
            "advance feature upstream",
            "feature.txt",
            "from upstream\n",
        )
        .expect("upstream feature commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = pull(
        &outpost,
        SourcePullOptions {
            branch: feature.clone(),
        },
        &mut reporter,
    )
    .expect("source pull");

    assert!(report.updated);
    assert_eq!(report.branch.as_str(), feature.as_str());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/feature/source-refresh")
            .expect("feature source oid"),
        upstream_oid
    );
    assert_eq!(
        fixture
            .current_branch_name(&fixture.source)
            .expect("source current branch"),
        "main"
    );
}

#[test]
fn sp02_source_pull_updates_checked_out_source_worktree() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let upstream_oid = fixture
        .commit_file_in_upstream(
            "main",
            "advance main upstream",
            "main.txt",
            "from upstream\n",
        )
        .expect("upstream main commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = pull(
        &outpost,
        SourcePullOptions {
            branch: branch("main"),
        },
        &mut reporter,
    )
    .expect("source pull");

    assert!(report.updated);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "HEAD")
            .expect("source HEAD oid"),
        upstream_oid
    );
    assert_eq!(
        fs::read_to_string(fixture.source.join("main.txt")).expect("source worktree file"),
        "from upstream\n"
    );
}

#[test]
fn sp03_source_pull_returns_divergence_when_source_and_origin_diverge() {
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
        pull(
            &outpost,
            SourcePullOptions {
                branch: branch("main"),
            },
            &mut reporter,
        ),
        "source pull should reject divergence",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
}

#[test]
fn sp04_source_pull_missing_branch_returns_branch_not_found() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost = outpost(&fixture, &outpost_path);
    let missing = branch("feature/missing");
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        pull(
            &outpost,
            SourcePullOptions {
                branch: missing.clone(),
            },
            &mut reporter,
        ),
        "source pull should reject missing branch",
    );

    assert!(
        matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == missing.as_str() && repo == canonical_source(&fixture))
    );
    assert!(reporter.steps.is_empty());
}

#[test]
fn sp05_source_pull_records_source_fetch_event() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_upstream("main", "advance upstream")
        .expect("upstream commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    pull(
        &outpost,
        SourcePullOptions {
            branch: branch("main"),
        },
        &mut reporter,
    )
    .expect("source pull");

    assert_eq!(
        reporter.step_kinds().first().copied(),
        Some(StepKind::SourceFetch)
    );
    assert!(
        reporter.warnings.is_empty(),
        "source pull should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

fn outpost(fixture: &AbcFixture, path: &std::path::Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

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
