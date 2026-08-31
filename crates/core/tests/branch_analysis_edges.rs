#[allow(dead_code)]
mod common;

use std::io;
use std::path::PathBuf;

use common::fixture::AbcFixture;
use outpost_core::ops::branch_analysis::{
    BranchCleanupCandidate, BranchCleanupFinding, BranchCleanupProof, BranchCleanupSkipReason,
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot, MergedPullRequest,
    analyze_branch_cleanup,
};
use outpost_core::ops::cleanup_evidence::ObservedRemoteBranch;
use outpost_core::{BranchName, Outpost, OutpostError, OutpostResult, RemoteName, SourceRepo};

#[test]
fn matching_merged_pull_request_is_a_typed_candidate() {
    let (fixture, source, outpost, branch) = feature_setup_with_commit("feature");
    let source_oid = source_branch_oid(&source, &branch);
    let default_oid = source_branch_oid(&source, &branch_name("main"));
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", default_oid.clone())),
        Some(source_oid.clone()),
        Some(MergedPullRequest {
            id: "#42".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: source_oid.clone(),
        }),
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.findings, Vec::new());
    assert_eq!(
        analysis.candidate,
        Some(BranchCleanupCandidate {
            branch: branch.clone(),
            source_oid: source_oid.clone(),
            upstream_remote: remote_name("origin"),
            upstream_oid: Some(source_oid),
            proof: BranchCleanupProof::MergedPullRequest(MergedPullRequest {
                id: "#42".to_owned(),
                head_ref_name: branch.clone(),
                head_ref_oid: source_branch_oid(&source, &branch),
            }),
        })
    );
    let evidence = analysis.evidence.expect("provider evidence");
    assert_eq!(evidence.request.branch, branch);
    assert_eq!(evidence.request.upstream_remote, remote_name("origin"));
    assert_eq!(
        evidence.request.upstream_url,
        fixture.upstream.to_string_lossy()
    );
}

#[test]
fn ancestor_of_default_branch_is_the_fallback_proof() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let source_oid = source_branch_oid(&source, &branch);
    let default_oid = source_branch_oid(&source, &branch_name("main"));
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", default_oid.clone())),
        None,
        None,
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.findings, Vec::new());
    assert_eq!(
        analysis.candidate,
        Some(BranchCleanupCandidate {
            branch: branch.clone(),
            source_oid,
            upstream_remote: remote_name("origin"),
            upstream_oid: None,
            proof: BranchCleanupProof::AncestorOfDefaultBranch {
                remote: remote_name("origin"),
                default_branch: branch_name("main"),
                default_oid,
            },
        })
    );
    assert_eq!(
        analysis
            .evidence
            .expect("evidence")
            .snapshot
            .default_branch
            .expect("default branch")
            .branch,
        branch_name("main")
    );
}

#[test]
fn no_upstream_tracking_is_a_typed_skip() {
    let (_fixture, source, outpost, branch) = feature_setup();
    unset_outpost_tracking(&outpost, &branch);

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(None, BranchCleanupSkipReason::NoUpstreamTracking)]
    );
}

#[test]
fn detached_outpost_is_a_typed_skip() {
    let (_fixture, source, outpost, _branch) = feature_setup();
    outpost
        .test_invoker()
        .run_check(["checkout", "--detach"])
        .expect("detach outpost");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(None, BranchCleanupSkipReason::DetachedHead)]
    );
}

#[test]
fn outpost_tracking_remote_mismatch_is_typed_skip() {
    let (_fixture, source, outpost, branch) = feature_setup();
    set_outpost_config(
        &outpost,
        &format!("branch.{}.remote", branch.as_str()),
        "origin",
    );

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(
            None,
            BranchCleanupSkipReason::UpstreamRemoteMismatch
        )]
    );
}

#[test]
fn non_branch_outpost_tracking_is_typed_skip() {
    let (_fixture, source, outpost, branch) = feature_setup();
    set_outpost_config(
        &outpost,
        &format!("branch.{}.merge", branch.as_str()),
        "refs/tags/v1",
    );

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(None, BranchCleanupSkipReason::UpstreamNotBranch)]
    );
}

#[test]
fn missing_source_branch_is_typed_skip() {
    let (_fixture, source, outpost, branch) = feature_setup();
    source
        .test_invoker()
        .run_check(["branch", "-D", branch.as_str()])
        .expect("delete source branch");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(
            Some(branch),
            BranchCleanupSkipReason::SourceBranchMissing
        )]
    );
}

#[test]
fn outpost_head_mismatch_is_typed_skip() {
    let (_fixture, source, outpost, branch) = feature_setup();
    commit_in_repo(&outpost, "outpost-only");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(
            Some(branch),
            BranchCleanupSkipReason::OutpostHeadMismatch
        )]
    );
}

#[test]
fn checked_out_source_branch_is_typed_skip() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feat")
        .expect("feature branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", branch.as_str()])
        .expect("checkout feature in source");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(
        analysis.findings,
        vec![skipped(
            Some(branch),
            BranchCleanupSkipReason::BranchCheckedOut
        )]
    );
}

#[test]
fn default_branch_is_skipped_after_evidence_collection() {
    let fixture = AbcFixture::new();
    let other = fixture.create_source_branch("dev").expect("dev branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", other.as_str()])
        .expect("switch source to dev");
    let main = branch_name("main");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(main.clone()))
        .expect("add main outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    let default_oid = source_branch_oid(&source, &main);
    let provider = StubProvider::snapshot(Some(snapshot(Some(("main", default_oid)), None, None)));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis.findings,
        vec![skipped(Some(main), BranchCleanupSkipReason::DefaultBranch)]
    );
    assert!(
        analysis.evidence.is_some(),
        "default branch decision retains evidence"
    );
}

#[test]
fn unknown_default_branch_is_a_typed_skip_and_retains_evidence() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let provider = StubProvider::snapshot(Some(snapshot(None, None, None)));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis.findings,
        vec![skipped(
            Some(branch),
            BranchCleanupSkipReason::DefaultBranchUnknown
        )]
    );
    assert!(
        analysis.evidence.is_some(),
        "unknown default still records snapshot"
    );
}

#[test]
fn mismatched_provider_proof_warns_then_uses_ancestor_fallback() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let source_oid = source_branch_oid(&source, &branch);
    let default_oid = source_branch_oid(&source, &branch_name("main"));
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", default_oid.clone())),
        None,
        Some(MergedPullRequest {
            id: "#bad".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: "0000000000000000000000000000000000000000".to_owned(),
        }),
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis.candidate,
        Some(BranchCleanupCandidate {
            branch: branch.clone(),
            source_oid,
            upstream_remote: remote_name("origin"),
            upstream_oid: None,
            proof: BranchCleanupProof::AncestorOfDefaultBranch {
                remote: remote_name("origin"),
                default_branch: branch_name("main"),
                default_oid,
            },
        })
    );
    assert!(
        matches!(&analysis.findings[..], [BranchCleanupFinding::Warning { branch: Some(found), message }]
        if found == &branch && message == "provider proof did not match the source branch tip")
    );
}

#[test]
fn mismatched_provider_proof_and_divergent_history_yield_no_proof() {
    let (_fixture, source, outpost, branch) = feature_setup_with_commit("feature");
    let default_oid = source_branch_oid(&source, &branch_name("main"));
    let source_oid_before = source_branch_oid(&source, &branch);
    let outpost_oid_before = outpost
        .test_invoker()
        .run_capture(["rev-parse", "HEAD"])
        .expect("outpost head before analysis");
    let registry_before = source.registry().expect("registry").entries().to_vec();
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", default_oid)),
        None,
        Some(MergedPullRequest {
            id: "#bad".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: "0000000000000000000000000000000000000000".to_owned(),
        }),
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.findings.len(), 2);
    assert!(
        matches!(&analysis.findings[0], BranchCleanupFinding::Warning { branch: Some(found), message }
        if found == &branch && message == "provider proof did not match the source branch tip")
    );
    assert_eq!(
        analysis.findings[1],
        skipped(Some(branch.clone()), BranchCleanupSkipReason::NoProof)
    );
    assert_eq!(source_branch_oid(&source, &branch), source_oid_before);
    assert_eq!(
        outpost
            .test_invoker()
            .run_capture(["rev-parse", "HEAD"])
            .expect("outpost head after analysis"),
        outpost_oid_before
    );
    assert_eq!(
        source.registry().expect("registry").entries(),
        registry_before.as_slice()
    );
}

#[test]
fn provider_none_uses_git_fallback_snapshot() {
    let (_fixture, source, outpost, branch) = feature_setup();

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.findings, Vec::new());
    assert_eq!(
        analysis
            .candidate
            .as_ref()
            .map(|candidate| &candidate.branch),
        Some(&branch)
    );
    assert!(
        matches!(analysis.candidate.as_ref().map(|candidate| &candidate.proof),
        Some(BranchCleanupProof::AncestorOfDefaultBranch { default_branch, .. }) if default_branch == &branch_name("main"))
    );
    let evidence = analysis.evidence.expect("git fallback evidence");
    assert_eq!(evidence.snapshot.merged_pull_request, None);
}

#[test]
fn provider_none_fallback_failure_warns_and_marks_default_unknown() {
    let (fixture, source, outpost, branch) = feature_setup();
    source
        .test_invoker()
        .run_check([
            "remote",
            "set-url",
            "origin",
            fixture.root.join("missing-remote").to_str().expect("path"),
        ])
        .expect("set missing remote URL");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert_eq!(analysis.findings.len(), 2);
    assert!(
        matches!(&analysis.findings[0], BranchCleanupFinding::Warning { branch: Some(found), message }
        if found == &branch && message.starts_with("cannot inspect upstream branches: "))
    );
    assert_eq!(
        analysis.findings[1],
        skipped(Some(branch), BranchCleanupSkipReason::DefaultBranchUnknown)
    );
}

#[test]
fn provider_error_warns_but_successful_git_fallback_is_used() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let provider = StubProvider::error();

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis
            .candidate
            .as_ref()
            .map(|candidate| &candidate.branch),
        Some(&branch)
    );
    assert_eq!(analysis.findings.len(), 1);
    assert!(
        matches!(&analysis.findings[0], BranchCleanupFinding::Warning { branch: Some(found), message }
        if found == &branch && message.starts_with("provider branch cleanup probe failed: "))
    );
}

#[test]
fn provider_none_response_falls_back_to_git_snapshot() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let provider = StubProvider::snapshot(None);

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.findings, Vec::new());
    assert!(matches!(
        analysis
            .candidate
            .as_ref()
            .map(|candidate| &candidate.proof),
        Some(BranchCleanupProof::AncestorOfDefaultBranch { .. })
    ));
    assert_eq!(
        analysis.evidence.expect("fallback evidence").request.branch,
        branch
    );
}

#[test]
fn provider_none_fallback_snapshot_without_default_is_unknown() {
    let (fixture, source, outpost, branch) = feature_setup();
    let empty_upstream = fixture.root.join("empty.git");
    fixture
        .invoker(&fixture.root)
        .run_check([
            "init",
            "--bare",
            "--initial-branch=main",
            empty_upstream.to_str().expect("path"),
        ])
        .expect("create empty upstream");
    source
        .test_invoker()
        .run_check([
            "remote",
            "set-url",
            "origin",
            empty_upstream.to_str().expect("path"),
        ])
        .expect("point origin at empty upstream");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(
        analysis.findings,
        vec![skipped(
            Some(branch),
            BranchCleanupSkipReason::DefaultBranchUnknown
        )]
    );
}

#[test]
fn unavailable_default_object_yields_ancestor_proof() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feat")
        .expect("feature branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let advanced_default = fixture
        .commit_in_upstream("main", "advance default")
        .expect("advance upstream default");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", advanced_default.clone())),
        None,
        None,
    )));
    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.findings, Vec::new());
    assert!(
        matches!(analysis.candidate.map(|candidate| candidate.proof),
        Some(BranchCleanupProof::AncestorOfDefaultBranch { default_oid, .. }) if default_oid == advanced_default)
    );
}

#[test]
fn fetch_failure_warns_and_yields_no_proof() {
    let (fixture, source, outpost, branch) = feature_setup();
    let default_oid = fixture
        .commit_in_upstream("main", "advance default")
        .expect("advance upstream default");
    source
        .test_invoker()
        .run_check([
            "remote",
            "set-url",
            "origin",
            fixture.root.join("missing").to_str().expect("path"),
        ])
        .expect("set missing remote URL");
    let provider = StubProvider::snapshot(Some(snapshot(Some(("main", default_oid)), None, None)));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.findings.len(), 2);
    assert!(
        matches!(&analysis.findings[0], BranchCleanupFinding::Warning { branch: Some(found), message }
        if found == &branch && message.starts_with("cannot fetch upstream default branch: "))
    );
    assert_eq!(
        analysis.findings[1],
        skipped(Some(branch), BranchCleanupSkipReason::NoProof)
    );
}

#[test]
fn unavailable_default_oid_warns_after_fetching() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", "HEAD~not-a-number".to_owned())),
        None,
        None,
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.findings.len(), 2);
    assert_eq!(
        analysis.findings[0],
        warning(
            &branch,
            "observed upstream default commit is unavailable after fetch"
        )
    );
    assert_eq!(
        analysis.findings[1],
        skipped(Some(branch), BranchCleanupSkipReason::NoProof)
    );
}

#[test]
fn fetched_default_object_still_missing_warns_and_yields_no_proof() {
    let (_fixture, source, outpost, branch) = feature_setup();
    let provider = StubProvider::snapshot(Some(snapshot(
        Some((
            "main",
            "0000000000000000000000000000000000000000".to_owned(),
        )),
        None,
        None,
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.candidate, None);
    assert_eq!(
        analysis.findings,
        vec![
            warning(
                &branch,
                "observed upstream default commit is unavailable after fetch"
            ),
            skipped(Some(branch), BranchCleanupSkipReason::NoProof),
        ]
    );
}

#[test]
fn source_upstream_remote_defaults_to_origin_when_tracking_is_absent() {
    let (_fixture, source, outpost, branch) = feature_setup();
    unset_source_tracking(&source, &branch);
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", source_branch_oid(&source, &branch_name("main")))),
        None,
        None,
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis
            .candidate
            .as_ref()
            .map(|candidate| &candidate.upstream_remote),
        Some(&remote_name("origin"))
    );
}

#[test]
fn source_upstream_remote_is_taken_from_branch_configuration() {
    let (fixture, source, outpost, branch) = feature_setup();
    source
        .test_invoker()
        .run_check([
            "remote",
            "add",
            "mirror",
            fixture.upstream.to_str().expect("path"),
        ])
        .expect("add mirror remote");
    set_source_config(
        &source,
        &format!("branch.{}.remote", branch.as_str()),
        "mirror",
    );
    set_source_config(
        &source,
        &format!("branch.{}.merge", branch.as_str()),
        &format!("refs/heads/{}", branch),
    );
    let provider = StubProvider::snapshot(Some(snapshot(
        Some(("main", source_branch_oid(&source, &branch_name("main")))),
        None,
        None,
    )));

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(
        analysis
            .candidate
            .as_ref()
            .map(|candidate| &candidate.upstream_remote),
        Some(&remote_name("mirror"))
    );
    assert_eq!(
        analysis.evidence.expect("evidence").request.upstream_remote,
        remote_name("mirror")
    );
}

#[test]
fn invalid_source_upstream_configuration_is_a_warning() {
    let (_fixture, source, outpost, branch) = feature_setup();
    set_source_config(
        &source,
        &format!("branch.{}.remote", branch.as_str()),
        "bad remote name",
    );
    set_source_config(
        &source,
        &format!("branch.{}.merge", branch.as_str()),
        &format!("refs/heads/{}", branch),
    );

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert!(
        matches!(&analysis.findings[..], [BranchCleanupFinding::Warning { branch: Some(found), message }]
        if found == &branch && message.starts_with("cannot inspect source branch upstream: "))
    );
}

#[test]
fn malformed_outpost_upstream_branch_is_a_warning() {
    let (_fixture, source, outpost, branch) = feature_setup();
    set_outpost_config(
        &outpost,
        &format!("branch.{}.merge", branch.as_str()),
        "refs/heads/-bad",
    );

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert!(
        matches!(&analysis.findings[..], [BranchCleanupFinding::Warning { branch: None, message }]
        if message.starts_with("cannot parse outpost upstream branch: "))
    );
}

#[test]
fn invalid_outpost_tracking_configuration_is_a_warning() {
    let (_fixture, source, outpost, branch) = feature_setup();
    set_outpost_config(
        &outpost,
        &format!("branch.{}.remote", branch.as_str()),
        "bad remote name",
    );

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert!(
        matches!(&analysis.findings[..], [BranchCleanupFinding::Warning { branch: None, message }]
        if message.starts_with("cannot inspect outpost upstream: "))
    );
}

#[test]
fn missing_remote_url_is_a_warning_and_default_unknown() {
    let (_fixture, source, outpost, branch) = feature_setup();
    source
        .test_invoker()
        .run_check(["remote", "remove", "origin"])
        .expect("remove source remote");
    let provider = StubProvider::snapshot(None);

    let analysis = analyze_branch_cleanup(&source, &outpost, Some(&provider));

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert_eq!(analysis.findings.len(), 2);
    assert!(
        matches!(&analysis.findings[0], BranchCleanupFinding::Warning { branch: Some(found), message }
        if found == &branch && message.starts_with("cannot inspect upstream remote URL: "))
    );
    assert_eq!(
        analysis.findings[1],
        skipped(Some(branch), BranchCleanupSkipReason::DefaultBranchUnknown)
    );
}

fn feature_setup() -> (AbcFixture, SourceRepo, Outpost, BranchName) {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feat")
        .expect("feature branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    (fixture, source, outpost, branch)
}

fn feature_setup_with_commit(message: &str) -> (AbcFixture, SourceRepo, Outpost, BranchName) {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feat")
        .expect("feature branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", branch.as_str()])
        .expect("switch source branch");
    fixture.commit_in_source(message).expect("feature commit");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "main"])
        .expect("switch source branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    (fixture, source, outpost, branch)
}

fn source_branch_oid(source: &SourceRepo, branch: &BranchName) -> String {
    source
        .branch_oid(branch)
        .expect("branch query")
        .expect("branch exists")
}

fn branch_name(name: &str) -> BranchName {
    BranchName::parse(name.to_owned()).expect("branch name")
}

fn remote_name(name: &str) -> RemoteName {
    RemoteName::parse(name.to_owned()).expect("remote name")
}

fn snapshot(
    default: Option<(&str, String)>,
    upstream_oid: Option<String>,
    merged_pull_request: Option<MergedPullRequest>,
) -> CleanupEvidenceSnapshot {
    CleanupEvidenceSnapshot {
        default_branch: default.map(|(branch, oid)| ObservedRemoteBranch {
            branch: branch_name(branch),
            oid,
        }),
        upstream_oid,
        merged_pull_request,
    }
}

fn skipped(branch: Option<BranchName>, reason: BranchCleanupSkipReason) -> BranchCleanupFinding {
    BranchCleanupFinding::Skipped { branch, reason }
}

fn warning(branch: &BranchName, prefix: &str) -> BranchCleanupFinding {
    BranchCleanupFinding::Warning {
        branch: Some(branch.clone()),
        message: prefix.to_owned(),
    }
}

fn set_outpost_config(outpost: &Outpost, key: &str, value: &str) {
    outpost
        .test_invoker()
        .run_check(["config", key, value])
        .expect("set outpost config");
}

fn set_source_config(source: &SourceRepo, key: &str, value: &str) {
    source
        .test_invoker()
        .run_check(["config", key, value])
        .expect("set source config");
}

fn unset_outpost_tracking(outpost: &Outpost, branch: &BranchName) {
    outpost
        .test_invoker()
        .run_check([
            "config",
            "--unset",
            &format!("branch.{}.remote", branch.as_str()),
        ])
        .expect("unset outpost remote tracking");
}

fn unset_source_tracking(source: &SourceRepo, branch: &BranchName) {
    let _ = source.test_invoker().run_check([
        "config",
        "--unset",
        &format!("branch.{}.remote", branch.as_str()),
    ]);
    let _ = source.test_invoker().run_check([
        "config",
        "--unset",
        &format!("branch.{}.merge", branch.as_str()),
    ]);
}

fn commit_in_repo(repo: &Outpost, message: &str) {
    repo.test_invoker()
        .run_check(["commit", "--allow-empty", "-m", message])
        .expect("commit in outpost");
}

struct StubProvider {
    response: ProviderResponse,
}

enum ProviderResponse {
    Snapshot(Option<CleanupEvidenceSnapshot>),
    Error,
}

impl StubProvider {
    fn snapshot(snapshot: Option<CleanupEvidenceSnapshot>) -> Self {
        Self {
            response: ProviderResponse::Snapshot(snapshot),
        }
    }

    fn error() -> Self {
        Self {
            response: ProviderResponse::Error,
        }
    }
}

impl CleanupEvidenceProvider for StubProvider {
    fn snapshot(
        &self,
        _request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        match &self.response {
            ProviderResponse::Snapshot(snapshot) => Ok(snapshot.clone()),
            ProviderResponse::Error => Err(OutpostError::IoAt {
                path: PathBuf::from("provider"),
                source: io::Error::other("provider failed"),
            }),
        }
    }
}
