#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use common::fixture::CapturingReporter;
use outpost_core::ops::analyze::{
    self, AnalyzeOptions, BranchDeleteSafety, Probe, SourcePushHazard,
};
use outpost_core::ops::branch_analysis::{BranchCleanupProvider, MergedPullRequest};
use outpost_core::selector::OutpostSelector;
use outpost_core::{AheadBehind, BranchName, OutpostResult, SourceRepo};

#[test]
fn analyze_reports_basic_outpost_state_and_lock() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    lock_registry_entry(&source, &outpost, Some("release freeze")).expect("lock outpost");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
        },
        None,
    )
    .expect("analyze report");

    assert_eq!(report.outpost_path, canonical(&outpost));
    assert_eq!(report.source_path, canonical(&fixture.source));
    assert_eq!(
        report
            .upstream_remote
            .as_ref()
            .map(|upstream| (upstream.remote.as_str(), upstream.url.as_str())),
        Probe::Known(("origin", fixture.upstream.to_str().expect("utf-8 path")))
    );
    assert_eq!(
        report.branch.as_ref().map(|branch| branch.as_str()),
        Some("main")
    );
    assert!(!report.outpost_dirty);
    assert!(report.locked);
    assert_eq!(report.lock_reason.as_deref(), Some("release freeze"));
    assert_eq!(
        report.outpost_vs_source,
        Probe::Known(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Known(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
    assert_eq!(
        report.upstream_default_branch.as_ref().map(|identity| {
            (
                identity.remote.as_str(),
                identity.branch.as_str(),
                identity.oid.len(),
                identity.oid.chars().all(|ch| ch.is_ascii_hexdigit()),
            )
        }),
        Probe::Known(("origin", "main", 40, true))
    );
    assert_eq!(
        report.source_push_hazard,
        Probe::Known(SourcePushHazard {
            checked_out: true,
            push_would_fail: false,
        })
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            reason: outpost_core::ops::branch_analysis::BranchCleanupSkipReason::BranchCheckedOut,
            ..
        }
    ));
}

#[test]
fn analyze_fetches_and_reports_ahead_behind_relationships() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let outpost = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost, "outpost commit")
        .expect("outpost commit");
    fixture
        .commit_in_source("source commit")
        .expect("source commit");
    fixture
        .commit_in_upstream("main", "upstream commit")
        .expect("upstream commit");
    let source = fixture.source_repo().expect("source repo");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
    )
    .expect("analyze report");

    assert_eq!(
        report.outpost_vs_source,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Known(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
}

#[test]
fn analyze_reports_safe_delete_from_default_ancestor_proof() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
    )
    .expect("analyze report");

    match report.safe_delete {
        BranchDeleteSafety::Yes(candidate) => {
            assert_eq!(candidate.branch, branch);
            assert!(matches!(
                candidate.proof,
                outpost_core::ops::branch_analysis::BranchCleanupProof::AncestorOfDefaultBranch { .. }
            ));
        }
        other => panic!("expected safe-delete yes, got {other:?}"),
    }
}

#[test]
fn analyze_discovers_remote_default_branch_without_local_origin_head() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    assert!(
        !fixture
            .invoker(&fixture.source)
            .run_status(["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"])
            .expect("origin HEAD status"),
        "test setup should not rely on local origin/HEAD"
    );

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
    )
    .expect("analyze report");

    assert_eq!(
        report
            .upstream_default_branch
            .as_ref()
            .map(|identity| (identity.remote.as_str(), identity.branch.as_str())),
        Probe::Known(("origin", "main"))
    );
    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::Yes(candidate) if candidate.branch == branch
    ));
}

#[test]
fn analyze_uses_source_branch_upstream_remote_when_not_origin() {
    let fixture = AbcFixture::new();
    rename_source_remote(&fixture, "origin", "upstream");
    ensure_remote_head(&fixture, "upstream");
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
    )
    .expect("analyze report");

    assert_eq!(
        report.upstream_branch.as_ref().map(|identity| {
            (
                identity.remote.as_str(),
                identity.branch.as_str(),
                identity.oid.len(),
            )
        }),
        Probe::Known(("upstream", "main", 40))
    );
    assert_eq!(
        report
            .upstream_remote
            .as_ref()
            .map(|upstream| (upstream.remote.as_str(), upstream.url.as_str())),
        Probe::Known(("upstream", fixture.upstream.to_str().expect("utf-8 path")))
    );
    assert_eq!(
        report
            .upstream_default_branch
            .as_ref()
            .map(|identity| (identity.remote.as_str(), identity.branch.as_str())),
        Probe::Known(("upstream", "main"))
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Known(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
}

#[test]
fn analyze_reports_progress_while_collecting_probes() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut reporter = CapturingReporter::default();

    analyze::run_with_reporter(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        None,
        &mut reporter,
    )
    .expect("analyze report");

    let messages = reporter
        .steps
        .iter()
        .map(|(_, message)| message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("resolving outpost"))
            && messages.iter().any(|message| message.contains("resolved:"))
            && messages
                .iter()
                .any(|message| message.contains("comparing outpost and source"))
            && messages
                .iter()
                .any(|message| message.contains("checking upstream remote"))
            && messages
                .iter()
                .any(|message| message.contains("discovering upstream default branch"))
            && messages
                .iter()
                .any(|message| message.contains("comparing source and upstream default"))
            && messages
                .iter()
                .any(|message| message.contains("ahead 0, behind 0"))
            && messages.iter().any(|message| message.contains("main at"))
            && messages
                .iter()
                .any(|message| message.contains("checking safe branch deletion proof"))
            && messages
                .iter()
                .any(|message| message.contains("no:") || message.contains("yes:")),
        "analyze should stream factual progress and result messages, got {messages:?}"
    );
}

#[test]
fn analyze_reports_safe_delete_from_matching_merged_pr_proof() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = source_branch_with_unmerged_commit(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let source_oid = source
        .branch_oid(&branch)
        .expect("source oid")
        .expect("branch oid");
    let provider = FakeProvider {
        proof: Some(MergedPullRequest {
            id: "#123".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: source_oid,
        }),
    };

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost),
        },
        Some(&provider),
    )
    .expect("analyze report");

    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::Yes(candidate)
            if candidate.branch == branch
                && matches!(
                    candidate.proof,
                    outpost_core::ops::branch_analysis::BranchCleanupProof::MergedPullRequest(_)
                )
    ));
}

#[test]
fn analyze_reports_no_safe_delete_without_proof_and_does_not_mutate_state() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = source_branch_with_unmerged_commit(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let registry_before = source.registry().expect("registry").entries().to_vec();
    let source_branch_before = source
        .branch_oid(&branch)
        .expect("source oid")
        .expect("source branch");
    let outpost_head_before = fixture
        .rev_parse(&outpost, "HEAD")
        .expect("outpost head before");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
        },
        None,
    )
    .expect("analyze report");

    assert!(matches!(
        report.safe_delete,
        BranchDeleteSafety::No {
            branch: Some(reported),
            reason: outpost_core::ops::branch_analysis::BranchCleanupSkipReason::NoProof,
        } if reported == branch
    ));
    assert_eq!(
        source.registry().expect("registry").entries(),
        registry_before.as_slice()
    );
    assert_eq!(
        source.branch_oid(&branch).expect("source oid").as_deref(),
        Some(source_branch_before.as_str())
    );
    assert_eq!(
        fixture
            .rev_parse(&outpost, "HEAD")
            .expect("outpost head after"),
        outpost_head_before
    );
    assert!(outpost.exists());
}

fn ensure_origin_head(fixture: &AbcFixture) {
    ensure_remote_head(fixture, "origin");
}

fn ensure_remote_head(fixture: &AbcFixture, remote: &str) {
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "set-head", remote, "main"])
        .expect("set remote head");
}

fn rename_source_remote(fixture: &AbcFixture, old: &str, new: &str) {
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "rename", old, new])
        .expect("rename source remote");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "branch",
            "--set-upstream-to",
            &format!("{new}/main"),
            "main",
        ])
        .expect("set source branch upstream");
}

fn source_branch_with_unmerged_commit(fixture: &AbcFixture, branch: &str) -> BranchName {
    let branch = fixture
        .create_source_branch(branch)
        .expect("create source branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", branch.as_str()])
        .expect("switch source branch");
    fixture
        .commit_in_source("feature commit")
        .expect("feature commit");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "main"])
        .expect("switch source branch");
    branch
}

fn lock_registry_entry(
    source: &SourceRepo,
    path: &Path,
    reason: Option<&str>,
) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    registry.lock(path, reason.map(str::to_owned))?;
    registry.save()
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

struct FakeProvider {
    proof: Option<MergedPullRequest>,
}

impl BranchCleanupProvider for FakeProvider {
    fn merged_pull_request(
        &self,
        _branch: &BranchName,
        _source_oid: &str,
    ) -> OutpostResult<Option<MergedPullRequest>> {
        Ok(self.proof.clone())
    }
}
