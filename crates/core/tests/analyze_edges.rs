#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::analyze::{
    self, AnalyzeOptions, BranchDeleteSafety, Probe, SourcePushHazard,
};
use outpost_core::ops::branch_analysis::BranchCleanupSkipReason;
use outpost_core::ops::cleanup_evidence::{
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot, ObservedRemoteBranch,
};
use outpost_core::selector::OutpostSelector;
use outpost_core::{AheadBehind, BranchName, OutpostError, OutpostResult, Reporter};

fn run_analysis(
    fixture: &AbcFixture,
    outpost: PathBuf,
) -> outpost_core::ops::analyze::AnalyzeReport {
    let source = fixture.source_repo().expect("source repo");
    analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
    )
    .expect("analyze report")
}

fn run_analysis_with_reporter(
    fixture: &AbcFixture,
    outpost: PathBuf,
    provider: Option<&dyn CleanupEvidenceProvider>,
    reporter: &mut dyn Reporter,
) -> outpost_core::ops::analyze::AnalyzeReport {
    let source = fixture.source_repo().expect("source repo");
    analyze::run_with_reporter(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        provider,
        reporter,
    )
    .expect("analyze report")
}

fn switch_source(fixture: &AbcFixture, branch: &str) {
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", branch])
        .expect("switch source branch");
}

fn feature_branch(fixture: &AbcFixture, name: &str, commit: bool) -> BranchName {
    let branch = fixture
        .create_source_branch(name)
        .expect("create source branch");
    if commit {
        switch_source(fixture, branch.as_str());
        fixture
            .commit_in_source("feature commit")
            .expect("feature commit");
        switch_source(fixture, "main");
    }
    branch
}

fn set_outpost_config(fixture: &AbcFixture, outpost: &Path, key: &str, value: &str) {
    fixture
        .invoker(outpost)
        .run_check(["config", key, value])
        .expect("set outpost config");
}

fn set_source_config(fixture: &AbcFixture, key: &str, value: &str) {
    fixture
        .invoker(&fixture.source)
        .run_check(["config", key, value])
        .expect("set source config");
}

fn source_oid(fixture: &AbcFixture, branch: &BranchName) -> String {
    fixture
        .source_repo()
        .expect("source repo")
        .branch_oid(branch)
        .expect("source branch query")
        .expect("source branch")
}

fn main_oid(fixture: &AbcFixture) -> String {
    let main = BranchName::parse("main".to_owned()).expect("main");
    source_oid(fixture, &main)
}

#[test]
fn detached_head_is_reported_as_unknown_without_failing_analysis() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["switch", "--detach", "HEAD"])
        .expect("detach outpost");
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, None, &mut reporter);

    assert_eq!(report.branch, None);
    assert_eq!(
        report.outpost_vs_source,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.upstream_remote,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.upstream_branch,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.upstream_default_branch,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert_eq!(
        report.source_push_hazard,
        Probe::Unknown("outpost HEAD is detached".to_owned())
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            branch: None,
            reason: BranchCleanupSkipReason::DetachedHead,
        }
    ));
    assert!(report.safe_delete_findings.iter().any(|finding| matches!(
        finding,
        outpost_core::ops::branch_analysis::BranchCleanupFinding::Skipped {
            branch: None,
            reason: BranchCleanupSkipReason::DetachedHead,
        }
    )));
    assert!(
        reporter
            .steps
            .iter()
            .any(|(_, message)| message == "unknown: outpost HEAD is detached")
    );
}

#[test]
fn no_outpost_tracking_is_unknown_but_source_default_relationships_remain_known() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .invoker(&outpost)
        .run_check(["branch", "--unset-upstream"])
        .expect("unset outpost upstream");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.outpost_vs_source,
        Probe::Unknown("outpost has no upstream tracking branch".to_owned())
    );
    assert_eq!(
        report
            .upstream_branch
            .as_ref()
            .map(|identity| (identity.remote.as_str(), identity.branch.as_str())),
        Probe::Known(("origin", "main"))
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::NoUpstreamTracking,
            ..
        }
    ));
    assert_eq!(
        report.source_push_hazard,
        Probe::Known(SourcePushHazard {
            checked_out: true,
            push_would_fail: false,
        })
    );
}

#[test]
fn non_branch_outpost_upstream_is_unknown_and_safe_delete_names_the_reason() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_outpost_config(&fixture, &outpost, "branch.main.merge", "refs/tags/v1");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.outpost_vs_source,
        Probe::Unknown("outpost upstream is not a branch".to_owned())
    );
    assert_eq!(
        report
            .upstream_remote
            .as_ref()
            .map(|upstream| upstream.remote.as_str()),
        Probe::Known("origin")
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::UpstreamNotBranch,
            ..
        }
    ));
}

#[test]
fn outpost_remote_mismatch_is_not_the_same_as_missing_tracking() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", false);
    fixture
        .push_source_branch(&branch)
        .expect("publish feature branch");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    set_outpost_config(&fixture, &outpost, "branch.feat.remote", "origin");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.outpost_vs_source,
        Probe::Unknown("outpost has no upstream tracking branch".to_owned())
    );
    assert_eq!(
        report
            .upstream_branch
            .as_ref()
            .map(|identity| (identity.remote.as_str(), identity.branch.as_str())),
        Probe::Known(("origin", "feat"))
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            branch: None,
            reason: BranchCleanupSkipReason::UpstreamRemoteMismatch,
        }
    ));
}

#[test]
fn source_non_branch_upstream_propagates_unknown_probes() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    set_source_config(&fixture, "branch.feat.remote", "origin");
    set_source_config(&fixture, "branch.feat.merge", "refs/tags/v1");
    set_outpost_config(&fixture, &outpost, "branch.feat.remote", "origin");

    let report = run_analysis(&fixture, outpost);

    let reason = "source upstream is not a branch";
    assert_unknown(report.upstream_remote, reason);
    assert_unknown(report.upstream_branch, reason);
    assert_unknown(report.upstream_default_branch, reason);
    assert_unknown(report.source_vs_upstream, reason);
    assert_unknown(report.source_vs_upstream_default, reason);
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::UpstreamRemoteMismatch,
            ..
        }
    ));
}

fn assert_unknown<T>(probe: Probe<T>, expected: &str) {
    assert!(matches!(probe, Probe::Unknown(reason) if reason == expected));
}

#[test]
fn missing_source_remote_is_unavailable_and_propagates_to_comparisons() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    set_source_config(&fixture, "branch.feat.remote", "ghost");
    set_source_config(&fixture, "branch.feat.merge", "refs/heads/feat");

    let mut reporter = CapturingReporter::default();
    let report = run_analysis_with_reporter(&fixture, outpost, None, &mut reporter);

    assert!(
        matches!(report.upstream_remote, Probe::Unavailable(reason) if reason.contains("ghost"))
    );
    assert!(
        matches!(report.upstream_branch, Probe::Unavailable(reason) if reason.contains("ghost"))
    );
    assert!(matches!(
        report.upstream_default_branch,
        Probe::Unavailable(reason) if reason.contains("ghost")
    ));
    assert!(
        matches!(report.source_vs_upstream, Probe::Unavailable(reason) if reason.contains("ghost"))
    );
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Unavailable(reason) if reason.contains("ghost")
    ));
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            ..
        }
    ));
    assert!(
        reporter
            .steps
            .iter()
            .any(|(_, message)| message.starts_with("unavailable:"))
    );
}

#[test]
fn malformed_source_tracking_remote_is_unavailable_and_safe_delete_is_unknown() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    set_source_config(&fixture, "branch.feat.remote", "bad remote name");
    set_source_config(&fixture, "branch.feat.merge", "refs/heads/feat");

    let report = run_analysis(&fixture, outpost);

    assert!(
        matches!(report.upstream_remote, Probe::Unavailable(reason) if reason.contains("bad remote name"))
    );
    assert!(
        matches!(report.upstream_branch, Probe::Unavailable(reason) if reason.contains("bad remote name"))
    );
    assert!(matches!(
        report.upstream_default_branch,
        Probe::Unavailable(reason) if reason.contains("bad remote name")
    ));
    assert!(
        matches!(report.source_vs_upstream, Probe::Unavailable(reason) if reason.contains("bad remote name"))
    );
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Unavailable(reason) if reason.contains("bad remote name")
    ));
    assert!(
        matches!(report.safe_delete, BranchDeleteSafety::Unknown(reason) if reason.contains("did not produce"))
    );
}

#[test]
fn missing_remote_branch_is_unknown_when_cleanup_evidence_is_skipped() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    set_outpost_config(&fixture, &outpost, "branch.feat.remote", "origin");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.upstream_branch,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 0
        })
    ));
}

#[test]
fn missing_remote_head_is_an_unknown_default_branch() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    fs::write(fixture.upstream.join("HEAD"), main_oid(&fixture)).expect("detach bare remote HEAD");
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, None, &mut reporter);

    assert_eq!(
        report.upstream_default_branch,
        Probe::Unknown("origin default branch is unknown".to_owned())
    );
    assert_eq!(
        report.upstream_branch,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            ..
        }
    ));
    assert!(report.safe_delete_findings.iter().any(|finding| matches!(
        finding,
        outpost_core::ops::branch_analysis::BranchCleanupFinding::Warning { message, .. }
            if message.contains("cannot inspect upstream branches")
    )));
}

#[test]
fn provider_snapshot_without_default_identity_reports_unknown_default() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let provider = SnapshotProvider {
        snapshot: CleanupEvidenceSnapshot {
            default_branch: None,
            upstream_oid: None,
            merged_pull_request: None,
        },
    };

    let report = run_analysis_with_reporter(
        &fixture,
        outpost,
        Some(&provider),
        &mut CapturingReporter::default(),
    );

    assert_eq!(
        report.upstream_branch,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert_eq!(
        report.upstream_default_branch,
        Probe::Unknown("origin default branch is unknown".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Unknown("origin default branch is unknown".to_owned())
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            ..
        }
    ));
}

#[test]
fn outpost_fetch_failure_is_unavailable_but_analysis_continues() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    let missing_remote = fixture.root.join("missing-outpost-remote");
    fixture
        .invoker(&outpost)
        .run_check([
            "remote",
            "set-url",
            "local",
            missing_remote.to_str().expect("missing remote path"),
        ])
        .expect("break local remote");
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, None, &mut reporter);

    assert!(matches!(
        report.outpost_vs_source,
        Probe::Unavailable(reason) if reason.contains("missing-outpost-remote")
    ));
    assert!(reporter.steps.iter().any(|(_, message)| {
        message.starts_with("unavailable:") && message.contains("missing-outpost-remote")
    }));
}

#[test]
fn source_branch_missing_before_analysis_is_reported_as_unknown() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", false);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    fixture
        .delete_source_branch(&branch)
        .expect("delete source branch");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.source_vs_upstream,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Unknown("source branch is missing".to_owned())
    );
    assert_eq!(
        report.source_push_hazard,
        Probe::Unknown("source branch is missing".to_owned())
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::SourceBranchMissing,
            branch: Some(reported),
        } if reported == branch
    ));
}

#[test]
fn source_push_hazard_distinguishes_refuse_and_update_instead() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_source_config(&fixture, "receive.denyCurrentBranch", "refuse");

    let refuse = run_analysis(&fixture, outpost.clone());
    assert_eq!(
        refuse.source_push_hazard,
        Probe::Known(SourcePushHazard {
            checked_out: true,
            push_would_fail: true,
        })
    );

    set_source_config(&fixture, "receive.denyCurrentBranch", "updateInstead");
    let update_instead = run_analysis(&fixture, outpost);
    assert_eq!(
        update_instead.source_push_hazard,
        Probe::Known(SourcePushHazard {
            checked_out: true,
            push_would_fail: false,
        })
    );
}

#[test]
fn source_push_hazard_is_known_safe_for_a_branch_not_checked_out() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", false);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch))
        .expect("add outpost");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.source_push_hazard,
        Probe::Known(SourcePushHazard {
            checked_out: false,
            push_would_fail: false,
        })
    );
}

#[test]
fn outpost_head_mismatch_is_a_safe_delete_skip() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    fixture
        .commit_in_outpost(&outpost, "outpost-only commit")
        .expect("outpost commit");

    let report = run_analysis(&fixture, outpost);

    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::OutpostHeadMismatch,
            ..
        }
    ));
}

#[test]
fn default_branch_is_not_safe_to_delete_when_source_checks_out_another_branch() {
    let fixture = AbcFixture::new();
    let feature = feature_branch(&fixture, "feat", false);
    switch_source(&fixture, feature.as_str());
    let outpost = fixture
        .add_outpost_on_branch(
            "C",
            Some(BranchName::parse("main".to_owned()).expect("main")),
        )
        .expect("add main outpost");

    let report = run_analysis(&fixture, outpost);

    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::DefaultBranch,
            branch: Some(branch),
        } if branch.as_str() == "main"
    ));
}

#[test]
fn provider_none_snapshot_falls_back_to_git_evidence() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let provider = NullProvider;

    let mut reporter = CapturingReporter::default();
    let report = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert_eq!(
        report.upstream_branch,
        Probe::Unknown("origin/feat is missing".to_owned())
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::NoProof,
            ..
        }
    ));
}

#[test]
fn provider_error_is_warned_and_git_fallback_still_produces_report() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch))
        .expect("add outpost");
    let provider = ErrorProvider;
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert!(report.safe_delete_findings.iter().any(|finding| matches!(
        finding,
        outpost_core::ops::branch_analysis::BranchCleanupFinding::Warning { message, .. }
            if message.contains("provider branch cleanup probe failed")
    )));
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::NoProof,
            ..
        }
    ));
}

#[test]
fn invalid_provider_oid_is_unavailable_and_hydration_failure_is_warned() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let provider = SnapshotProvider {
        snapshot: CleanupEvidenceSnapshot {
            default_branch: Some(ObservedRemoteBranch {
                branch: BranchName::parse("main".to_owned()).expect("main"),
                oid: main_oid(&fixture),
            }),
            upstream_oid: Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_owned()),
            merged_pull_request: None,
        },
    };
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert!(matches!(
        report.source_vs_upstream,
        Probe::Unavailable(reason) if reason.contains("observed remote commit deadbeef")
    ));
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 0
        })
    ));
    assert!(
        reporter
            .warnings
            .iter()
            .any(|warning| warning.contains("fetch"))
    );
}

#[test]
fn invalid_provider_oid_syntax_is_an_unavailable_probe_error() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch))
        .expect("add outpost");
    let provider = SnapshotProvider {
        snapshot: CleanupEvidenceSnapshot {
            default_branch: Some(ObservedRemoteBranch {
                branch: BranchName::parse("main".to_owned()).expect("main"),
                oid: "not an object name".to_owned(),
            }),
            upstream_oid: Some("not an object name".to_owned()),
            merged_pull_request: None,
        },
    };
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert!(matches!(
        report.source_vs_upstream,
        Probe::Unavailable(reason) if reason.contains("not an object name")
    ));
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Unavailable(reason) if reason.contains("not an object name")
    ));
    assert!(reporter.steps.iter().any(|(_, message)| {
        message.contains("unavailable:") && message.contains("not an object name")
    }));
}

#[test]
fn malformed_selector_returns_public_error() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let missing = fixture.root.join("does-not-exist");

    let error = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(missing.clone()),
        },
        None,
    )
    .expect_err("missing selector should fail");

    assert!(matches!(error, OutpostError::RegistryEntryNotFound(path) if path == missing));
}

#[test]
fn probe_mapping_preserves_unknown_and_unavailable_reasons() {
    let known = Probe::Known(7_u32);
    assert_eq!(known.as_ref(), Probe::Known(&7_u32));
    assert_eq!(known.map(|value| value + 1), Probe::Known(8_u32));

    let unknown = Probe::<u32>::Unknown("not observed".to_owned());
    assert_eq!(unknown.as_ref(), Probe::Unknown("not observed".to_owned()));
    assert_eq!(
        unknown.map(|value| value + 1),
        Probe::Unknown("not observed".to_owned())
    );

    let unavailable = Probe::<u32>::Unavailable("git failed".to_owned());
    assert_eq!(
        unavailable.as_ref(),
        Probe::Unavailable("git failed".to_owned())
    );
    assert_eq!(
        unavailable.map(|value| value + 1),
        Probe::Unavailable("git failed".to_owned())
    );
}

struct NullProvider;

impl CleanupEvidenceProvider for NullProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        Ok(None)
    }
}

struct ErrorProvider;

impl CleanupEvidenceProvider for ErrorProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        Err(OutpostError::InvalidRefName {
            name: "provider failure".to_owned(),
        })
    }
}

struct SnapshotProvider {
    snapshot: CleanupEvidenceSnapshot,
}

impl CleanupEvidenceProvider for SnapshotProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        Ok(Some(self.snapshot.clone()))
    }
}
