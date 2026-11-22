#[allow(dead_code)]
mod common;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::analyze::{
    self, AnalyzeOptions, BranchDeleteSafety, Probe, SourcePushHazard,
};
use outpost_core::ops::branch_analysis::{BranchCleanupFinding, BranchCleanupSkipReason};
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
fn stale_tracking_ref_cannot_substitute_for_the_replacement_branch() {
    let fixture = AbcFixture::new();
    let source = fixture.invoker(&fixture.source);
    let upstream = fixture.invoker(&fixture.upstream);
    source
        .run_check(["push", "origin", "main:refs/heads/topic"])
        .expect("publish topic");
    upstream
        .run_check(["branch", "-D", "topic"])
        .expect("retire topic upstream");
    upstream
        .run_check(["branch", "topic/review", "main"])
        .expect("publish replacement branch upstream");
    let remote_tip = fixture
        .commit_in_upstream("topic/review", "remote review change")
        .expect("advance the replacement branch upstream");
    source
        .run_check(["switch", "-c", "topic/review"])
        .expect("start local review branch");
    let local_tip = fixture
        .commit_in_source("local review change")
        .expect("advance the local review branch");
    let outpost = fixture.add_outpost("C").expect("add review outpost");

    let report = run_analysis(&fixture, outpost);

    match report.upstream_branch.map(|identity| identity.oid) {
        Probe::Unavailable(reason) => assert!(!reason.trim().is_empty()),
        observed => assert_eq!(observed, Probe::Known(remote_tip)),
    }
    match report.source_vs_upstream {
        Probe::Unavailable(reason) => assert!(!reason.trim().is_empty()),
        comparison => assert_eq!(
            comparison,
            Probe::Known(AheadBehind {
                ahead: 1,
                behind: 1,
            })
        ),
    }
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 0,
        })
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/topic/review")
            .expect("local review branch survives analysis"),
        local_tip
    );
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
fn an_upstreams_remote_tracking_head_cannot_prove_a_feature_is_merged() {
    let fixture = AbcFixture::new();
    let feature = feature_branch(&fixture, "feature", true);
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(feature))
        .expect("add feature outpost");
    let upstream = fixture.invoker(&fixture.upstream);
    upstream
        .run_check([
            OsStr::new("remote"),
            OsStr::new("add"),
            OsStr::new("team"),
            fixture.source.as_os_str(),
        ])
        .expect("configure upstream's team remote");
    upstream.run_check(["fetch", "team"]).expect("fetch team");
    upstream
        .run_check(["remote", "set-head", "team", "feature"])
        .expect("point team/HEAD at the unmerged feature");
    let main_tip = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("upstream main");
    let feature_tip = fixture
        .rev_parse(&fixture.upstream, "refs/heads/feature")
        .expect("upstream feature");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report
            .upstream_default_branch
            .as_ref()
            .map(|identity| (identity.branch.as_str(), identity.oid.as_str())),
        Probe::Known(("main", main_tip.as_str()))
    );
    assert_eq!(
        report.upstream_branch.map(|identity| identity.oid),
        Probe::Known(feature_tip)
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
fn invalid_worktree_config_preserves_gits_reason_and_independent_comparisons() {
    let mut fixture = AbcFixture::new();
    fixture.git_env.insert("LC_ALL".into(), "C".into());
    fixture.commit_in_source("unpublished work").unwrap();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    set_source_config(&fixture, "receive.denyCurrentBranch", "refuse");
    set_source_config(&fixture, "worktree.guessRemote", "auto");
    let mut git = std::process::Command::new("git");
    outpost_core::isolate_git_test_command(&mut git);
    let native = git
        .args(["worktree", "list"])
        .current_dir(&fixture.source)
        .envs(&fixture.git_env)
        .output()
        .expect("inspect the worktrees with Git");
    assert!(!native.status.success());
    let stderr = String::from_utf8_lossy(&native.stderr);
    let diagnostic = stderr
        .lines()
        .find(|line| line.starts_with("fatal: "))
        .expect("Git reports the invalid worktree setting");

    let report = run_analysis(&fixture, outpost);

    match report.source_push_hazard {
        Probe::Unavailable(reason) => assert!(reason.contains(diagnostic), "{reason}"),
        result => assert_eq!(
            result,
            Probe::Known(SourcePushHazard {
                checked_out: true,
                push_would_fail: true,
            })
        ),
    }
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 0,
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
fn moved_native_worktree_does_not_make_its_branch_safe_to_push_or_delete() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "topic", false);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add topic outpost");
    set_source_config(&fixture, "receive.denyCurrentBranch", "refuse");
    let checkout_name = "portable-checkout";
    let checkout = fixture.root.join(checkout_name);
    fixture
        .invoker(&fixture.source)
        .run_check([
            OsStr::new("worktree"),
            OsStr::new("add"),
            checkout.as_os_str(),
            OsStr::new(branch.as_str()),
        ])
        .expect("check out topic in a native worktree");
    fs::rename(&checkout, fixture.root.join("moved-checkout")).expect("move checkout directory");

    let report = run_analysis(&fixture, outpost);

    match &report.source_push_hazard {
        Probe::Known(hazard) => assert!(hazard.checked_out && hazard.push_would_fail),
        Probe::Unknown(reason) | Probe::Unavailable(reason) => {
            assert!(reason.contains(checkout_name), "{reason}");
        }
    }
    assert!(!matches!(report.safe_delete, BranchDeleteSafety::Yes(_)));
    let explains_topic = report
        .safe_delete_findings
        .iter()
        .any(|finding| match finding {
            BranchCleanupFinding::Warning {
                branch: Some(found),
                message,
            } => found == &branch && message.contains(checkout_name),
            BranchCleanupFinding::Skipped {
                branch: Some(found),
                reason: BranchCleanupSkipReason::BranchCheckedOut,
            } => found == &branch,
            _ => false,
        });
    assert!(explains_topic, "{:?}", report.safe_delete_findings);
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
fn analysis_keeps_the_default_comparison_when_the_observed_feature_is_deleted() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    fixture
        .push_source_branch(&branch)
        .expect("publish feature");
    let observed = fixture
        .commit_in_upstream("feat", "remote-only change")
        .expect("remote commit");
    fixture
        .invoker(&fixture.upstream)
        .run_check(["update-ref", "-d", "refs/heads/feat", &observed])
        .expect("collaborator deletes observed branch");
    let provider = SnapshotProvider {
        snapshot: CleanupEvidenceSnapshot {
            default_branch: Some(ObservedRemoteBranch {
                branch: BranchName::parse("main".to_owned()).expect("main"),
                oid: main_oid(&fixture),
            }),
            upstream_oid: Some(observed.clone()),
            merged_pull_request: None,
        },
    };
    let mut reporter = CapturingReporter::default();

    let report = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert!(matches!(
        report.source_vs_upstream,
        Probe::Unavailable(reason) if reason.contains(&observed)
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
fn analysis_keeps_known_branch_tips_when_a_reference_cache_goes_offline() {
    let fixture = AbcFixture::new();
    let cache = fixture.root.join("object-cache");
    fs::rename(&fixture.source, &cache).expect("use the seeded clone as the object cache");
    fixture
        .invoker(&fixture.root)
        .run_check([
            OsStr::new("clone"),
            OsStr::new("--no-local"),
            OsStr::new("--reference"),
            cache.as_os_str(),
            fixture.upstream.as_os_str(),
            fixture.source.as_os_str(),
        ])
        .expect("clone the source borrowing its existing history");

    let git = fixture.invoker(&fixture.source);
    git.run_check(["switch", "-c", "feature"])
        .expect("create feature");
    let feature_tip = fixture
        .commit_file_in_source("feature work", "feature.txt", "feature\n")
        .expect("feature commit");
    switch_source(&fixture, "main");
    let main_tip = fixture
        .commit_file_in_source("main work", "main.txt", "main\n")
        .expect("default branch commit");
    git.run_check(["push", "-u", "origin", "feature", "main"])
        .expect("publish the two branches");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(BranchName::parse("feature").unwrap()))
        .expect("add the feature outpost");
    fs::rename(&cache, fixture.root.join("relocated-cache")).expect("move the object cache");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.upstream_branch.map(|branch| branch.oid),
        Probe::Known(feature_tip)
    );
    assert_eq!(
        report.upstream_default_branch.map(|branch| branch.oid),
        Probe::Known(main_tip)
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Known(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
    assert!(matches!(
        report.source_vs_upstream_default,
        Probe::Unavailable(_)
    ));
    assert!(
        !matches!(report.safe_delete, BranchDeleteSafety::Yes(_)),
        "incomplete history cannot establish that the feature was merged"
    );
}

#[test]
fn malformed_selector_returns_public_error() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let missing = fs::canonicalize(&fixture.root)
        .expect("canonical fixture root")
        .join("does-not-exist");

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

#[test]
fn an_unrelated_tracking_head_cannot_name_a_detached_upstream_default() {
    let fixture = AbcFixture::new();
    let feature = feature_branch(&fixture, "feature", true);
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(feature))
        .expect("add feature outpost");
    let upstream = fixture.invoker(&fixture.upstream);
    upstream
        .run_check([
            OsStr::new("remote"),
            OsStr::new("add"),
            OsStr::new("team"),
            fixture.source.as_os_str(),
        ])
        .expect("configure upstream's team remote");
    upstream.run_check(["fetch", "team"]).expect("fetch team");
    upstream
        .run_check(["remote", "set-head", "team", "feature"])
        .expect("point team/HEAD at the feature");
    // `ls-remote --symref origin HEAD` also matches refs/remotes/team/HEAD.
    fs::write(fixture.upstream.join("HEAD"), main_oid(&fixture)).expect("detach upstream HEAD");

    let report = run_analysis(&fixture, outpost);

    assert_eq!(
        report.upstream_default_branch,
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
fn an_upstream_head_naming_a_tag_is_not_a_default_branch() {
    let fixture = AbcFixture::new();
    let feature = feature_branch(&fixture, "feature", false);
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(feature))
        .expect("add feature outpost");
    let upstream = fixture.invoker(&fixture.upstream);
    upstream.run_check(["tag", "v1", "main"]).expect("tag");
    upstream
        .run_check(["symbolic-ref", "HEAD", "refs/tags/v1"])
        .expect("point upstream HEAD at the tag");

    let report = run_analysis(&fixture, outpost);

    assert!(matches!(
        &report.upstream_default_branch,
        Probe::Unavailable(reason) if reason.contains("ref: refs/tags/v1")
    ));
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            ..
        }
    ));
    assert!(report.safe_delete_findings.iter().any(|finding| matches!(
        finding,
        BranchCleanupFinding::Warning { message, .. }
            if message.contains("default branch is not under refs/heads")
    )));
}

#[cfg(unix)]
#[test]
fn a_namespaced_upstream_that_repeats_head_gives_no_deletion_proof() {
    let fixture = AbcFixture::new();
    // The feature is already merged: without the repeated HEAD it would be deletable.
    let feature = feature_branch(&fixture, "feature", false);
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(feature))
        .expect("add feature outpost");
    let upstream = fixture.invoker(&fixture.upstream);
    let main_tip = main_oid(&fixture);
    for name in ["main", "feature"] {
        let reference = format!("refs/namespaces/team/refs/heads/{name}");
        upstream
            .run_check(["update-ref", &reference, &main_tip])
            .expect("namespace branch");
    }
    upstream
        .run_check([
            "symbolic-ref",
            "refs/namespaces/team/HEAD",
            "refs/namespaces/team/refs/heads/main",
        ])
        .expect("namespace HEAD");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "config",
            "remote.origin.uploadpack",
            "GIT_NAMESPACE=team git-upload-pack",
        ])
        .expect("serve origin from the team namespace");

    let symbolic = run_analysis(&fixture, outpost.clone());
    upstream
        .run_check([
            "update-ref",
            "--no-deref",
            "refs/namespaces/team/HEAD",
            &main_tip,
        ])
        .expect("detach namespace HEAD");
    let detached = run_analysis(&fixture, outpost);

    for (report, duplicate) in [
        (symbolic, "duplicate default branch:"),
        (detached, "duplicate default branch OID:"),
    ] {
        assert!(matches!(
            report.safe_delete,
            BranchDeleteSafety::No {
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
                ..
            }
        ));
        assert!(
            report.safe_delete_findings.iter().any(|finding| matches!(
                finding,
                BranchCleanupFinding::Warning { message, .. } if message.contains(duplicate)
            )),
            "{:?}",
            report.safe_delete_findings
        );
    }
}

#[test]
fn silent_analysis_returns_the_report_whose_warnings_it_discards() {
    let fixture = AbcFixture::new();
    let branch = feature_branch(&fixture, "feat", true);
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    fixture
        .push_source_branch(&branch)
        .expect("publish feature");
    let observed = fixture
        .commit_in_upstream("feat", "remote-only change")
        .expect("remote commit");
    fixture
        .invoker(&fixture.upstream)
        .run_check(["update-ref", "-d", "refs/heads/feat", &observed])
        .expect("collaborator deletes observed branch");
    let provider = SnapshotProvider {
        snapshot: CleanupEvidenceSnapshot {
            default_branch: Some(ObservedRemoteBranch {
                branch: BranchName::parse("main".to_owned()).expect("main"),
                oid: main_oid(&fixture),
            }),
            upstream_oid: Some(observed),
            merged_pull_request: None,
        },
    };
    let source = fixture.source_repo().expect("source repo");

    let silent = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
        },
        Some(&provider),
    )
    .expect("silent analysis");
    let mut reporter = CapturingReporter::default();
    let reported = run_analysis_with_reporter(&fixture, outpost, Some(&provider), &mut reporter);

    assert!(!reporter.warnings.is_empty());
    assert_eq!(silent, reported);
}
