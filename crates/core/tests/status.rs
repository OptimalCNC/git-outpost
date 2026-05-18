#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::status::{run_with, ConfigProblem};
use outpost_core::OutpostError;

#[test]
fn s07_run_with_accepts_explicit_outpost_target_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let cwd = std::env::current_dir().expect("current dir");
    assert!(!cwd.starts_with(&outpost));

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(report.outpost_path, canonical(&outpost));
}

#[test]
fn s08_unmanaged_repo_returns_not_an_outpost() {
    let fixture = AbcFixture::new();

    let err = expect_error(
        run_with(&fixture.source, &fixture.git_env),
        "unmanaged source repo should fail",
    );

    assert!(matches!(
        err,
        OutpostError::NotAnOutpost(path) if path == canonical(&fixture.source)
    ));
}

#[test]
fn s09_missing_source_repo_config_is_reported_as_problem() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.sourceRepo");

    let report = run_with(&outpost, &fixture.git_env).expect("degraded status report");

    assert!(report
        .problems
        .contains(&ConfigProblem::MissingSourceRepoConfig));
}

#[test]
fn s13_missing_source_repo_config_keeps_degraded_report_available() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.sourceRepo");

    let report = run_with(&outpost, &fixture.git_env).expect("degraded status report");

    assert_eq!(report.source_path, None);
    assert!(!report.source_present);
    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("local")
    );
    assert!(report
        .problems
        .contains(&ConfigProblem::MissingSourceRepoConfig));
}

fn unset_local_config(fixture: &AbcFixture, repo: &Path, key: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", "--unset", key])
        .expect("unset local config");
}

fn expect_error<T>(result: outpost_core::OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
