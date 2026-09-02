#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::status::{
    ConfigProblem, OutpostHeadStatus, SourceLocation, SourceUpstreamStatus, StatusReport,
    TrackedUpstream, run_with,
};
use outpost_core::{AheadBehind, BranchName, OutpostError};

#[test]
fn missing_outpost_remote_is_tolerated_without_a_false_mismatch() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["remote", "remove", "local"])
        .expect("remove local remote");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.source,
        SourceLocation::Present(canonical(&fixture.source))
    );
    assert_eq!(
        report.remote_name.as_ref().map(|name| name.as_str()),
        Some("local")
    );
    assert_eq!(report.outpost_ahead_behind_source, None);
    assert!(
        !report
            .problems
            .iter()
            .any(|problem| matches!(problem, ConfigProblem::LocalRemoteMismatch { .. }))
    );
}

#[test]
fn relative_outpost_remote_url_is_canonicalized_before_comparison() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["remote", "set-url", "local", "../B"])
        .expect("set relative remote");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(
        !report
            .problems
            .iter()
            .any(|problem| matches!(problem, ConfigProblem::LocalRemoteMismatch { .. }))
    );
}

#[test]
fn absent_outpost_tracking_ref_yields_no_comparison_or_tracking_diagnostic() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["update-ref", "-d", "refs/remotes/local/main"])
        .expect("delete tracking ref");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.outpost_ahead_behind_source, None);
    assert!(!report.problems.iter().any(|problem| {
        matches!(
            problem,
            ConfigProblem::OutpostSourceTrackingUnavailable { .. }
        )
    }));
}

#[test]
fn outpost_reports_local_repository_source_upstream_and_zero_divergence() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_local_config(&fixture, &fixture.source, "branch.main.remote", ".");
    set_local_config(
        &fixture,
        &fixture.source,
        "branch.main.merge",
        "refs/heads/main",
    );

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.head,
        OutpostHeadStatus::Attached {
            branch: BranchName::parse("main").expect("branch"),
            source_upstream: SourceUpstreamStatus::Configured(TrackedUpstream::LocalRepository {
                branch: BranchName::parse("main").expect("branch"),
            },),
        }
    );
    assert_eq!(
        report.source_ahead_behind_upstream,
        Some(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
}

#[test]
fn malformed_outpost_tracking_remote_is_rejected_as_an_invalid_ref() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_local_config(&fixture, &outpost, "branch.main.remote", "bad name");

    let error = run_with(&outpost, &fixture.git_env).expect_err("invalid tracking remote");

    assert!(matches!(error, OutpostError::InvalidRefName { name } if name == "bad name"));
}

#[test]
fn invalid_metadata_report_preserves_dirty_state() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    write_metadata(&fixture, &outpost, "{");
    fs::write(outpost.join("dirty.txt"), "dirty").expect("dirty file");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(report.outpost_dirty);
    assert!(matches!(
        report.head,
        OutpostHeadStatus::Attached {
            source_upstream: outpost_core::ops::status::SourceUpstreamStatus::Unavailable,
            ..
        }
    ));
    assert!(matches!(
        report.problems.as_slice(),
        [ConfigProblem::InvalidMetadata { reason }] if !reason.is_empty()
    ));
}

#[test]
fn configured_source_path_that_is_not_a_repository_propagates_not_a_repo() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    let file = fixture.root.join("source-directory");
    fs::create_dir(&file).expect("source directory");
    let expected = canonical(&file);
    edit_metadata(&outpost, |metadata| {
        metadata["source_repo"] = serde_json::Value::String(file.to_string_lossy().into_owned());
    });

    let error = run_with(&outpost, &fixture.git_env).expect_err("non-repository source");

    assert!(
        matches!(error, OutpostError::NotARepo(ref path) if path == &expected),
        "{error:?}"
    );
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

fn edit_metadata(outpost: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let metadata = outpost_core::Outpost::at(outpost).expect("managed outpost");
    let path = metadata.metadata_path();
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("metadata contents"))
            .expect("metadata JSON");
    edit(&mut value);
    fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize metadata"),
    )
    .expect("write metadata");
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

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
