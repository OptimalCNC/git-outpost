use crate::{BranchName, Outpost, OutpostError, OutpostResult, RemoteName, SourceRepo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCleanupAnalysis {
    pub candidate: Option<BranchCleanupCandidate>,
    pub findings: Vec<BranchCleanupFinding>,
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
pub struct MergedPullRequest {
    pub id: String,
    pub head_ref_name: BranchName,
    pub head_ref_oid: String,
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

pub trait BranchCleanupProvider {
    fn merged_pull_request(
        &self,
        branch: &BranchName,
        source_oid: &str,
    ) -> OutpostResult<Option<MergedPullRequest>>;
}

pub fn analyze_branch_cleanup(
    source: &SourceRepo,
    outpost: &Outpost,
    provider: Option<&dyn BranchCleanupProvider>,
) -> BranchCleanupAnalysis {
    let mut findings = Vec::new();
    let candidate = analyze_candidate(source, outpost, provider, &mut findings);
    BranchCleanupAnalysis {
        candidate,
        findings,
    }
}

fn analyze_candidate(
    source: &SourceRepo,
    outpost: &Outpost,
    provider: Option<&dyn BranchCleanupProvider>,
    findings: &mut Vec<BranchCleanupFinding>,
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

    let (default_branch, default_oid) = match source.fetch_remote_default_branch(&upstream_remote) {
        Ok(Some(default)) => default,
        Ok(None) => {
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            return None;
        }
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect upstream default branch",
                err,
            ));
            findings.push(BranchCleanupFinding::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            return None;
        }
    };
    if branch == default_branch {
        findings.push(BranchCleanupFinding::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::DefaultBranch,
        });
        return None;
    }

    let upstream_oid = match source.remote_branch_oid(&upstream_remote, &branch) {
        Ok(oid) => oid,
        Err(err) => {
            findings.push(warning(
                Some(branch.clone()),
                "cannot inspect upstream branch",
                err,
            ));
            None
        }
    };

    if let Some(provider) = provider {
        match provider.merged_pull_request(&branch, &source_oid) {
            Ok(Some(merged_pr))
                if merged_pr.head_ref_name == branch && merged_pr.head_ref_oid == source_oid =>
            {
                return Some(BranchCleanupCandidate {
                    branch,
                    source_oid,
                    upstream_remote,
                    upstream_oid,
                    proof: BranchCleanupProof::MergedPullRequest(merged_pr),
                });
            }
            Ok(Some(_)) => {
                findings.push(BranchCleanupFinding::Warning {
                    branch: Some(branch.clone()),
                    message: "provider proof did not match the source branch tip".to_owned(),
                });
            }
            Ok(None) => {}
            Err(err) => {
                findings.push(warning(
                    Some(branch.clone()),
                    "provider branch cleanup probe failed",
                    err,
                ));
            }
        }
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
