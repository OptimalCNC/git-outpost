#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::status::{
    ConfigProblem, OutpostHeadStatus, SourceHead, SourceLocation, StatusReport, run, run_with,
};
use outpost_core::{BranchName, OutpostError};

#[test]
fn run_rejects_a_non_repository_target_without_mutation() {
    let fixture = AbcFixture::new();
    let target = fixture.root.join("not-a-repository");
    fs::create_dir(&target).expect("target directory");

    let error = run(&target).expect_err("non-repository target should fail");

    assert!(
        matches!(error, OutpostError::NotARepo(ref path) if path == &target),
        "{error:?}"
    );
}

#[test]
fn run_with_rejects_a_missing_target_path() {
    let fixture = AbcFixture::new();
    let target = fixture.root.join("missing");

    let error = run_with(&target, &fixture.git_env).expect_err("missing target should fail");

    assert!(
        matches!(
            error,
            OutpostError::IoAt { ref path, ref source }
            if path == &target
                && matches!(
                    source.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                )
        ),
        "{error:?}"
    );
}

#[test]
fn invalid_metadata_returns_a_typed_degraded_report() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    write_metadata(&fixture, &outpost, "{");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.source, SourceLocation::Unconfigured);
    assert_eq!(report.remote_name, None);
    assert!(matches!(report.head, OutpostHeadStatus::Attached { .. }));
    assert!(matches!(
        report.problems.as_slice(),
        [ConfigProblem::InvalidMetadata { reason }] if !reason.is_empty()
    ));
}

#[test]
fn invalid_metadata_on_detached_head_reports_detached() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost");
    let metadata = format!(
        r#"{{"version":99,"source_repo":{},"remote_name":"local"}}"#,
        serde_json::to_string(&fixture.source).expect("serialize source path")
    );
    write_metadata(&fixture, &outpost, &metadata);

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.head, OutpostHeadStatus::Detached);
    assert!(matches!(
        report.problems.as_slice(),
        [ConfigProblem::InvalidMetadata { reason }] if reason.contains("unsupported metadata version")
    ));
}

#[test]
fn malformed_source_registry_json_is_a_typed_registry_error() {
    let fixture = AbcFixture::new();
    let path = fixture.source_repo().expect("source repo").registry_path();
    fs::create_dir_all(path.parent().expect("registry parent")).expect("registry parent");
    fs::write(&path, "not json").expect("corrupt registry");

    let error = run_with(&fixture.source, &fixture.git_env).expect_err("bad registry");

    assert!(matches!(error, OutpostError::BadRegistry { path: actual, .. } if actual == path));
}

#[test]
fn source_non_head_upstream_is_treated_as_unset() {
    let fixture = AbcFixture::new();
    set_local_config(&fixture, &fixture.source, "branch.main.remote", "origin");
    set_local_config(
        &fixture,
        &fixture.source,
        "branch.main.merge",
        "refs/tags/release",
    );

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.head,
        SourceHead::Attached {
            branch: branch("main"),
            upstream: None,
        }
    );
}

#[test]
fn malformed_upstream_merge_ref_is_rejected_at_the_status_boundary() {
    let fixture = AbcFixture::new();
    set_local_config(&fixture, &fixture.source, "branch.main.remote", "origin");
    set_local_config(
        &fixture,
        &fixture.source,
        "branch.main.merge",
        "refs/heads/invalid ref",
    );

    let error = run_with(&fixture.source, &fixture.git_env).expect_err("invalid merge ref");

    assert!(
        matches!(error, OutpostError::InvalidRefName { name } if name == "refs/heads/invalid ref")
    );
}

#[test]
fn malformed_upstream_remote_name_is_rejected_at_the_status_boundary() {
    let fixture = AbcFixture::new();
    set_local_config(&fixture, &fixture.source, "branch.main.remote", "bad name");
    set_local_config(
        &fixture,
        &fixture.source,
        "branch.main.merge",
        "refs/heads/main",
    );

    let error = run_with(&fixture.source, &fixture.git_env).expect_err("invalid remote name");

    assert!(matches!(error, OutpostError::InvalidRefName { name } if name == "bad name"));
}

#[test]
fn outpost_tracking_remote_mismatch_is_unavailable() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_local_config(&fixture, &outpost, "branch.main.remote", "origin");
    set_local_config(&fixture, &outpost, "branch.main.merge", "refs/heads/main");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.outpost_ahead_behind_source, None);
    assert!(
        report
            .problems
            .contains(&ConfigProblem::OutpostSourceTrackingUnavailable {
                branch: branch("main"),
            })
    );
}

#[test]
fn update_instead_source_config_suppresses_push_would_fail() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_local_config(
        &fixture,
        &fixture.source,
        "receive.denyCurrentBranch",
        "updateInstead",
    );

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(!report.problems.iter().any(|problem| {
        matches!(problem, ConfigProblem::PushWouldFail { branch } if branch.as_str() == "main")
    }));
}

fn write_metadata(fixture: &AbcFixture, outpost: &Path, contents: &str) {
    let git_dir = fixture
        .invoker(outpost)
        .run_capture(["rev-parse", "--git-dir"])
        .expect("git dir");
    let git_dir = PathBuf::from(git_dir);
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        outpost.join(git_dir)
    };
    let path = git_dir.join("outpost/metadata.json");
    fs::create_dir_all(path.parent().expect("metadata parent")).expect("metadata directory");
    fs::write(path, contents).expect("metadata contents");
}

fn set_local_config(fixture: &AbcFixture, repo: &Path, key: &str, value: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", key, value])
        .expect("set config");
}

fn expect_outpost(report: StatusReport) -> outpost_core::ops::status::OutpostStatus {
    match report {
        StatusReport::Outpost(report) => report,
        StatusReport::Source(_) => panic!("expected outpost report"),
    }
}

fn expect_source(report: StatusReport) -> outpost_core::ops::status::SourceStatus {
    match report {
        StatusReport::Source(report) => report,
        StatusReport::Outpost(_) => panic!("expected source report"),
    }
}

fn branch(value: &str) -> BranchName {
    BranchName::parse(value).expect("branch")
}
