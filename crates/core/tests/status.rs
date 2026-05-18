#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::add::{run as add_run, AddCheckout, AddOptions};
use outpost_core::ops::status::{run_with, ConfigProblem};
use outpost_core::{AheadBehind, OutpostError, RemoteName, Reporter, StepKind};

#[test]
fn s01_run_with_from_inside_outpost_reports_canonical_source_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let nested = outpost.join("nested");
    fs::create_dir(&nested).expect("create nested dir");

    let report = run_with(&nested, &fixture.git_env).expect("status report");

    assert_eq!(report.source_path, Some(canonical(&fixture.source)));
}

#[test]
fn s02_run_with_reports_local_remote_name() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("local")
    );
}

#[test]
fn s03_run_with_reports_current_branch_and_detached_head() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let branch_report = run_with(&outpost, &fixture.git_env).expect("branch status report");

    assert_eq!(
        branch_report
            .current_branch
            .as_ref()
            .map(|branch| branch.as_str()),
        Some("main")
    );

    fixture
        .invoker(&outpost)
        .run_check(["checkout", "--detach"])
        .expect("detach HEAD");

    let detached_report = run_with(&outpost, &fixture.git_env).expect("detached status report");

    assert_eq!(detached_report.current_branch, None);
}

#[test]
fn s04_run_with_reports_dirty_state_for_untracked_files() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let clean_report = run_with(&outpost, &fixture.git_env).expect("clean status report");

    assert!(!clean_report.outpost_dirty);

    fs::write(outpost.join("untracked.txt"), "new").expect("write untracked file");

    let dirty_report = run_with(&outpost, &fixture.git_env).expect("dirty status report");

    assert!(dirty_report.outpost_dirty);
}

#[test]
fn s05_run_with_reports_outpost_ahead_behind_source_from_existing_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source_seen = fixture
        .commit_in_source("source seen by outpost")
        .expect("source seen commit");
    update_remote_tracking_ref(&fixture, &outpost, "local", "main", &source_seen);
    fixture
        .commit_in_outpost(&outpost, "outpost commit")
        .expect("outpost commit");
    fixture
        .commit_in_source("source not fetched by status")
        .expect("source unseen commit");
    let remote_ref_before = rev_parse(&fixture, &outpost, "refs/remotes/local/main");

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(
        report.outpost_ahead_behind_source,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        rev_parse(&fixture, &outpost, "refs/remotes/local/main"),
        remote_ref_before
    );
}

#[test]
fn s06_run_with_reports_source_ahead_behind_upstream_from_existing_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_source("source commit")
        .expect("source commit");
    let upstream_seen = fixture
        .commit_in_upstream("main", "upstream seen by source")
        .expect("upstream seen commit");
    set_branch_upstream(&fixture, &fixture.source, "main", "origin");
    update_remote_tracking_ref(&fixture, &fixture.source, "origin", "main", &upstream_seen);
    fixture
        .commit_in_upstream("main", "upstream not fetched by status")
        .expect("upstream unseen commit");
    let remote_ref_before = rev_parse(&fixture, &fixture.source, "refs/remotes/origin/main");

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(
        report.source_ahead_behind_upstream,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        rev_parse(&fixture, &fixture.source, "refs/remotes/origin/main"),
        remote_ref_before
    );
}

#[test]
fn s10_run_with_reports_missing_source_problem() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let moved_source = fixture.root.join("B.moved");
    fs::rename(&fixture.source, &moved_source).expect("move source repo");

    let report = run_with(&outpost, &fixture.git_env).expect("degraded status report");

    assert_eq!(report.source_path, Some(canonical_missing(&fixture.source)));
    assert!(!report.source_present);
    assert!(report
        .problems
        .contains(&ConfigProblem::SourceMissing(canonical_missing(
            &fixture.source
        ))));
}

#[test]
fn s11_run_with_flags_local_remote_mismatch() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    set_remote_url(&fixture, &outpost, "local", &fixture.upstream);

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert!(report
        .problems
        .contains(&ConfigProblem::LocalRemoteMismatch {
            configured: canonical(&fixture.source),
            actual: canonical(&fixture.upstream),
        }));
}

#[test]
fn s12_run_with_uses_metadata_remote_name_for_custom_remote() {
    let fixture = AbcFixture::new();
    let outpost = add_outpost_with_remote(&fixture, "C", "custom");
    let source_seen = fixture
        .commit_in_source("source seen by custom outpost")
        .expect("source seen commit");
    update_remote_tracking_ref(&fixture, &outpost, "custom", "main", &source_seen);
    fixture
        .commit_in_outpost(&outpost, "custom outpost commit")
        .expect("outpost commit");
    assert!(matches!(
        fixture
            .invoker(&outpost)
            .run_capture(["remote", "get-url", "local"]),
        Err(OutpostError::GitFailed { .. })
    ));

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("custom")
    );
    assert_eq!(
        report.outpost_ahead_behind_source,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert!(!report
        .problems
        .iter()
        .any(|problem| matches!(problem, ConfigProblem::LocalRemoteMismatch { .. })));
}

#[test]
fn run_with_flags_not_in_registry_when_outpost_entry_is_missing() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    remove_from_registry(&fixture, &outpost);

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert!(report.problems.contains(&ConfigProblem::NotInRegistry));
}

#[test]
fn run_with_flags_push_would_fail_when_source_refuses_checked_out_branch_update() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    set_local_config(
        &fixture,
        &fixture.source,
        "receive.denyCurrentBranch",
        "refuse",
    );

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert!(report.problems.contains(&ConfigProblem::PushWouldFail {
        branch: outpost_core::BranchName::parse("main").expect("branch"),
    }));
}

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

fn set_branch_upstream(fixture: &AbcFixture, repo: &Path, branch: &str, remote: &str) {
    let remote_key = format!("branch.{branch}.remote");
    set_local_config(fixture, repo, &remote_key, remote);
    let merge_key = format!("branch.{branch}.merge");
    let merge_ref = format!("refs/heads/{branch}");
    set_local_config(fixture, repo, &merge_key, &merge_ref);
}

fn set_local_config(fixture: &AbcFixture, repo: &Path, key: &str, value: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", key, value])
        .expect("set local config");
}

fn set_remote_url(fixture: &AbcFixture, repo: &Path, remote: &str, url: &Path) {
    fixture
        .invoker(repo)
        .run_check([
            "remote".into(),
            "set-url".into(),
            remote.into(),
            url.as_os_str().to_os_string(),
        ])
        .expect("set remote url");
}

fn remove_from_registry(fixture: &AbcFixture, outpost: &Path) {
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    assert!(registry
        .remove_by_path(outpost)
        .expect("remove registry entry"));
    registry.save().expect("save registry");
}

fn update_remote_tracking_ref(
    fixture: &AbcFixture,
    repo: &Path,
    remote: &str,
    branch: &str,
    oid: &str,
) {
    let ref_name = format!("refs/remotes/{remote}/{branch}");
    let fetch_refspec = format!("refs/heads/{branch}:{ref_name}");
    fixture
        .invoker(repo)
        .run_check(["fetch", remote, &fetch_refspec])
        .expect("fetch remote tracking ref");
    assert_eq!(rev_parse(fixture, repo, &ref_name), oid);
}

fn rev_parse(fixture: &AbcFixture, repo: &Path, rev: &str) -> String {
    fixture
        .invoker(repo)
        .run_capture(["rev-parse", rev])
        .expect("rev-parse")
}

fn add_outpost_with_remote(fixture: &AbcFixture, name: &str, remote_name: &str) -> PathBuf {
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join(name);
    let mut reporter = NoopReporter;
    add_run(
        &source,
        AddOptions {
            destination: destination.clone(),
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse(remote_name).expect("remote name"),
        },
        &mut reporter,
    )
    .expect("add outpost");
    destination
}

struct NoopReporter;

impl Reporter for NoopReporter {
    fn step(&mut self, _kind: StepKind, _message: &str) {}

    fn warn(&mut self, _message: &str) {}
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

fn canonical_missing(path: &Path) -> PathBuf {
    let parent = path.parent().expect("path parent");
    canonical(parent).join(path.file_name().expect("file name"))
}
