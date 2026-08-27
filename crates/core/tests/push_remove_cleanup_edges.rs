#[allow(dead_code)]
mod common;

use std::cell::Cell;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::cleanup_evidence::{
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot,
};
use outpost_core::ops::{push, remove};
use outpost_core::selector::OutpostSelector;
use outpost_core::{BranchName, Outpost, OutpostError, OutpostResult, StepKind};

#[test]
fn push_without_new_commits_reports_zero_for_both_hops() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = push::run(&outpost, push::PushOptions, &mut reporter).expect("no-op push");

    assert_eq!(
        report.outpost_to_source,
        push::StepResult::Pushed { commits: 0 }
    );
    assert_eq!(
        report.source_to_origin,
        push::StepResult::Pushed { commits: 0 }
    );
    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::OutpostPush, StepKind::SourcePush]
    );
}

#[test]
fn push_allows_non_checked_out_source_branch_when_update_policy_refuses_checked_out_updates() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feature/not-checked-out")
        .expect("create feature branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add feature outpost");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "receive.denyCurrentBranch", "refuse"])
        .expect("refuse checked-out updates");
    let outpost_oid = fixture
        .commit_in_outpost(&outpost_path, "feature commit")
        .expect("commit in outpost");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = push::run(&outpost, push::PushOptions, &mut reporter).expect("push feature");

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
            .rev_parse(&fixture.source, "refs/heads/feature/not-checked-out")
            .expect("source feature"),
        outpost_oid
    );
}

#[test]
fn push_stops_before_reporting_when_outpost_to_source_fetch_fails() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost commit")
        .expect("commit in outpost");
    let missing_remote = fixture.root.join("missing-source.git");
    fixture
        .invoker(&outpost_path)
        .run_check([
            OsStr::new("remote"),
            OsStr::new("set-url"),
            OsStr::new("local"),
            missing_remote.as_os_str(),
        ])
        .expect("break local remote");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let outpost = open_outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        push::run(&outpost, push::PushOptions, &mut reporter),
        "broken source fetch should fail",
    );

    assert!(matches!(err, OutpostError::GitFailed { .. }));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
}

#[test]
fn remove_noninteractive_deletes_only_the_outpost_and_records_skip() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feature/noninteractive")
        .expect("create branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");

    let report = remove::run_with_cleanup(
        &source,
        remove_options(&outpost_path, false),
        remove::BranchCleanupMode::NonInteractive,
    )
    .expect("remove noninteractively");

    assert!(!outpost_path.exists());
    assert!(source.branch_exists(&branch).expect("branch query"));
    assert_eq!(
        report.branch_cleanup,
        vec![remove::BranchCleanupOutcome::Skipped {
            branch: None,
            reason: remove::BranchCleanupSkipReason::NonInteractive,
        }]
    );
}

#[test]
fn remove_missing_outpost_records_the_selected_cleanup_mode() {
    let disabled = remove_missing_with_mode(MissingMode::Disabled);
    assert_eq!(disabled, remove::BranchCleanupSkipReason::CleanupDisabled);

    let noninteractive = remove_missing_with_mode(MissingMode::NonInteractive);
    assert_eq!(
        noninteractive,
        remove::BranchCleanupSkipReason::NonInteractive
    );

    let prompt = remove_missing_with_mode(MissingMode::Prompt);
    assert_eq!(prompt, remove::BranchCleanupSkipReason::MissingOutpost);
}

#[test]
fn remove_with_cleanup_uses_git_fallback_when_provider_returns_none() {
    let (fixture, branch, outpost_path) = cleanup_ready_fixture("feature/provider-none");
    let source = fixture.source_repo().expect("source repo");
    let provider = NoneProvider::default();
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove_options(&outpost_path, false),
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: Some(&provider),
            prompt: &mut prompt,
        }),
    )
    .expect("remove with provider fallback");

    assert_eq!(provider.calls.get(), 1);
    assert!(!source.branch_exists(&branch).expect("branch query"));
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeletedSourceBranch { branch: deleted }
            if deleted == &branch
    )));
}

#[test]
fn remove_with_cleanup_warns_then_falls_back_when_provider_fails() {
    let (fixture, branch, outpost_path) = cleanup_ready_fixture("feature/provider-error");
    let source = fixture.source_repo().expect("source repo");
    let provider = ErrorProvider;
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove_options(&outpost_path, false),
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: Some(&provider),
            prompt: &mut prompt,
        }),
    )
    .expect("remove after provider failure");

    assert!(!source.branch_exists(&branch).expect("branch query"));
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Warning {
            branch: Some(warned),
            ..
        } if warned == &branch
    )));
}

#[test]
fn remove_with_cleanup_deletes_source_and_upstream_branches_after_both_confirmations() {
    let (fixture, branch, outpost_path) = cleanup_ready_fixture("feature/delete-both");
    fixture
        .push_source_branch(&branch)
        .expect("publish feature branch");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], [true]);

    let report = remove::run_with_cleanup(
        &source,
        remove_options(&outpost_path, false),
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove branches");

    assert!(!source.branch_exists(&branch).expect("source branch query"));
    assert_eq!(
        source
            .origin_branch_oid(&branch)
            .expect("origin branch query"),
        None
    );
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeletedUpstreamBranch {
            remote,
            branch: deleted,
        } if remote.as_str() == "origin" && deleted == &branch
    )));
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

enum MissingMode {
    Disabled,
    NonInteractive,
    Prompt,
}

fn remove_missing_with_mode(mode: MissingMode) -> remove::BranchCleanupSkipReason {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost_path).expect("remove outpost before operation");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([], []);
    let mode = match mode {
        MissingMode::Disabled => remove::BranchCleanupMode::Disabled,
        MissingMode::NonInteractive => remove::BranchCleanupMode::NonInteractive,
        MissingMode::Prompt => remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    };

    let report = remove::run_with_cleanup(&source, remove_options(&outpost_path, false), mode)
        .expect("remove missing outpost");

    assert!(source.registry().expect("registry").entries().is_empty());
    assert_eq!(report.branch_cleanup.len(), 1);
    match report.branch_cleanup.into_iter().next().expect("one skip") {
        remove::BranchCleanupOutcome::Skipped {
            branch: None,
            reason,
        } => reason,
        other => panic!("expected mode skip, got {other:?}"),
    }
}

fn cleanup_ready_fixture(branch_name: &str) -> (AbcFixture, BranchName, PathBuf) {
    let fixture = AbcFixture::new();
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "set-head", "origin", "main"])
        .expect("set origin HEAD");
    let branch = fixture
        .create_source_branch(branch_name)
        .expect("create cleanup branch");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add cleanup outpost");
    (fixture, branch, outpost)
}

#[derive(Default)]
struct NoneProvider {
    calls: Cell<usize>,
}

impl CleanupEvidenceProvider for NoneProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        self.calls.set(self.calls.get() + 1);
        Ok(None)
    }
}

struct ErrorProvider;

impl CleanupEvidenceProvider for ErrorProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        Err(OutpostError::IoAt {
            path: PathBuf::from("test cleanup provider"),
            source: io::Error::other("provider unavailable"),
        })
    }
}

struct TestPrompt {
    source_responses: VecDeque<bool>,
    upstream_responses: VecDeque<bool>,
}

impl TestPrompt {
    fn new<const S: usize, const U: usize>(source: [bool; S], upstream: [bool; U]) -> Self {
        Self {
            source_responses: VecDeque::from(source),
            upstream_responses: VecDeque::from(upstream),
        }
    }
}

impl remove::BranchCleanupPrompt for TestPrompt {
    fn confirm_source_branch_delete(
        &mut self,
        _candidate: &remove::BranchCleanupCandidate,
    ) -> bool {
        self.source_responses.pop_front().unwrap_or(false)
    }

    fn confirm_upstream_branch_delete(
        &mut self,
        _candidate: &remove::BranchCleanupCandidate,
    ) -> bool {
        self.upstream_responses.pop_front().unwrap_or(false)
    }
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}
