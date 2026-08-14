use crate::ops::cleanup_evidence::GitCleanupEvidenceProvider;
use crate::{BranchName, Outpost, OutpostError, RemoteName, SourceRepo};

pub use crate::ops::cleanup_evidence::{
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot, MergedPullRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCleanupAnalysis {
    pub candidate: Option<BranchCleanupCandidate>,
    pub findings: Vec<BranchCleanupFinding>,
    pub evidence: Option<CleanupEvidenceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvidenceObservation {
    pub request: CleanupEvidenceRequest,
    pub snapshot: CleanupEvidenceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCleanupCandidate {
    pub branch: BranchName,
    pub source_oid: String,
    pub upstream_remote: RemoteName,
    pub upstream_oid: Option<String>,
    pub proof: BranchCleanupProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCleanupProof {
    MergedPullRequest(MergedPullRequest),
    AncestorOfDefaultBranch {
        remote: RemoteName,
        default_branch: BranchName,
        default_oid: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCleanupFinding {
    Skipped {
        branch: Option<BranchName>,
        reason: BranchCleanupSkipReason,
    },
    Warning {
        branch: Option<BranchName>,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchCleanupSkipReason {
    CleanupDisabled,
    NonInteractive,
    MissingOutpost,
    DetachedHead,
    NoUpstreamTracking,
    UpstreamRemoteMismatch,
    UpstreamNotBranch,
    SourceBranchMissing,
    OutpostHeadMismatch,
    BranchCheckedOut,
    DefaultBranch,
    DefaultBranchUnknown,
    NoProof,
}

pub fn analyze_branch_cleanup(
    source: &SourceRepo,
    outpost: &Outpost,
    provider: Option<&dyn CleanupEvidenceProvider>,
) -> BranchCleanupAnalysis {
    let mut findings = Vec::new();
    let mut evidence = None;
    let candidate = analyze_candidate(source, outpost, provider, &mut findings, &mut evidence);
    BranchCleanupAnalysis {
        candidate,
        findings,
        evidence,
    }
}

fn analyze_candidate(
    source: &SourceRepo,
    outpost: &Outpost,
    provider: Option<&dyn CleanupEvidenceProvider>,
    findings: &mut Vec<BranchCleanupFinding>,
    evidence: &mut Option<CleanupEvidenceObservation>,
) -> Option<BranchCleanupCandidate> {
    let upstream = match outpost.upstream_tracking() {
        Ok(Some(upstream)) => upstream,
        Ok(None) => {
            findings.push(BranchCleanupFinding::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::NoUpstreamTracking,
            });
            return None;
        }
        Err(OutpostError::BranchNotFound { .. }) => {
            findings.push(BranchCleanupFinding::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::DetachedHead,
            });
            return None;
        }
        Err(err) => {
            findings.push(warning(None, "cannot inspect outpost upstream", err));
            return None;
        }
    };

    if upstream.remote != outpost.metadata().remote_name {
        findings.push(BranchCleanupFinding::Skipped {
            branch: None,
            reason: BranchCleanupSkipReason::UpstreamRemoteMismatch,
        });
        return None;
    }

    let Some(branch) = upstream.short_branch() else {
        findings.push(BranchCleanupFinding::Skipped {
            branch: None,
            reason: BranchCleanupSkipReason::UpstreamNotBranch,
        });
        return None;
    };
    let branch = match BranchName::parse(branch.to_owned()) {
        Ok(branch) => branch,
        Err(err) => {
            findings.push(warning(None, "cannot parse outpost upstream branch", err));
            return None;
        }
    };

    let Some(source_oid) = (match source.branch_oid(&branch) {
        Ok(oid) => oid,
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect source branch",
                err,
            ));
            return None;
        }
    }) else {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::SourceBranchMissing,
        });
        return None;
    };

    let outpost_oid = match outpost.git().run_capture(["rev-parse", "HEAD"]) {
        Ok(oid) => oid,
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect outpost HEAD",
                err,
            ));
            return None;
        }
    };
    if outpost_oid != source_oid {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::OutpostHeadMismatch,
        });
        return None;
    }

    match source.is_branch_checked_out(&branch) {
        Ok(true) => {
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::BranchCheckedOut,
            });
            return None;
        }
        Ok(false) => {}
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect checked-out source branches",
                err,
            ));
            return None;
        }
    }

    let upstream_remote = source_upstream_remote(source, &branch, findings)?;
    let upstream_url = match source.remote_url(&upstream_remote) {
        Ok(url) => url,
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect upstream remote URL",
                err,
            ));
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            return None;
        }
    };
    let request = CleanupEvidenceRequest {
        upstream_remote: upstream_remote.clone(),
        upstream_url,
        branch: branch.clone(),
        source_oid: source_oid.clone(),
    };
    let snapshot = collect_cleanup_evidence(source, &request, provider, findings)?;
    *evidence = Some(CleanupEvidenceObservation { request, snapshot });
    let observation = evidence
        .as_ref()
        .expect("cleanup evidence was stored before evaluation");
    let Some(default) = observation.snapshot.default_branch.as_ref() else {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::DefaultBranchUnknown,
        });
        return None;
    };
    let default_branch = default.branch.clone();
    let default_oid = default.oid.clone();
    if branch == default_branch {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::DefaultBranch,
        });
        return None;
    }

    let upstream_oid = observation.snapshot.upstream_oid.clone();

    if let Some(merged_pr) = observation.snapshot.merged_pull_request.clone() {
        if merged_pr.head_ref_name == branch && merged_pr.head_ref_oid == source_oid {
            return Some(BranchCleanupCandidate {
                branch,
                source_oid,
                upstream_remote,
                upstream_oid,
                proof: BranchCleanupProof::MergedPullRequest(merged_pr),
            });
        }
        findings.push(BranchCleanupFinding::Warning {
            branch: Some(branch.clone()),
            message: "provider proof did not match the source branch tip".to_owned(),
        });
    }

    if !ensure_commit_available(
        source,
        &upstream_remote,
        &default_branch,
        &default_oid,
        &branch,
        findings,
    ) {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::NoProof,
        });
        return None;
    }

    match source.is_ancestor_oid(&source_oid, &default_oid) {
        Ok(true) => Some(BranchCleanupCandidate {
            branch,
            source_oid,
            upstream_remote: upstream_remote.clone(),
            upstream_oid,
            proof: BranchCleanupProof::AncestorOfDefaultBranch {
                remote: upstream_remote,
                default_branch,
                default_oid,
            },
        }),
        Ok(false) => {
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::NoProof,
            });
            None
        }
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot prove source branch is merged",
                err,
            ));
            None
        }
    }
}

fn ensure_commit_available(
    source: &SourceRepo,
    remote: &RemoteName,
    default_branch: &BranchName,
    default_oid: &str,
    candidate_branch: &BranchName,
    findings: &mut Vec<BranchCleanupFinding>,
) -> bool {
    match source.has_commit_oid(default_oid) {
        Ok(true) => return true,
        Ok(false) => {}
        Err(err) => {
            findings.push(warning(
                Some(candidate_branch.clone()),
                "cannot inspect upstream default commit",
                err,
            ));
            return false;
        }
    }

    if let Err(err) = source.fetch_remote_branches(remote, std::slice::from_ref(default_branch)) {
        findings.push(warning(
            Some(candidate_branch.clone()),
            "cannot fetch upstream default branch",
            err,
        ));
        return false;
    }

    match source.has_commit_oid(default_oid) {
        Ok(true) => true,
        Ok(false) => {
            findings.push(BranchCleanupFinding::Warning {
                branch: Some(candidate_branch.clone()),
                message: "observed upstream default commit is unavailable after fetch".to_owned(),
            });
            false
        }
        Err(err) => {
            findings.push(warning(
                Some(candidate_branch.clone()),
                "cannot inspect fetched upstream default commit",
                err,
            ));
            false
        }
    }
}

fn collect_cleanup_evidence(
    source: &SourceRepo,
    request: &CleanupEvidenceRequest,
    provider: Option<&dyn CleanupEvidenceProvider>,
    findings: &mut Vec<BranchCleanupFinding>,
) -> Option<CleanupEvidenceSnapshot> {
    if let Some(provider) = provider {
        match provider.snapshot(request) {
            Ok(Some(snapshot)) => return Some(snapshot),
            Ok(None) => {}
            Err(err) => findings.push(warning(
                Some(request.branch.clone()),
                "provider branch cleanup probe failed",
                err,
            )),
        }
    }

    let fallback = GitCleanupEvidenceProvider::new(source);
    match CleanupEvidenceProvider::snapshot(&fallback, request) {
        Ok(Some(snapshot)) => Some(snapshot),
        Ok(None) => None,
        Err(err) => {
            findings.push(warning(
                Some(request.branch.clone()),
                "cannot inspect upstream branches",
                err,
            ));
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(request.branch.clone()),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            None
        }
    }
}

fn source_upstream_remote(
    source: &SourceRepo,
    branch: &BranchName,
    findings: &mut Vec<BranchCleanupFinding>,
) -> Option<RemoteName> {
    match source.upstream_for(branch) {
        Ok(Some(upstream)) => Some(upstream.remote),
        Ok(None) => Some(origin_remote()),
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect source branch upstream",
                err,
            ));
            None
        }
    }
}

fn origin_remote() -> RemoteName {
    RemoteName::parse("origin").expect("origin is a valid remote name")
}

fn warning(branch: Option<BranchName>, context: &str, err: OutpostError) -> BranchCleanupFinding {
    BranchCleanupFinding::Warning {
        branch,
        message: format!("{context}: {err}"),
    }
}
