use std::path::PathBuf;

use crate::selector::{OutpostSelector, resolve_entry};
use crate::{BranchName, Outpost, OutpostError, OutpostResult, SourceRepo, safety};

pub struct RemoveOptions {
    pub selector: OutpostSelector,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveReport {
    pub path: PathBuf,
    pub branch_cleanup: Vec<BranchCleanupOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCleanupCandidate {
    pub branch: BranchName,
    pub source_oid: String,
    pub upstream_oid: Option<String>,
    pub proof: BranchCleanupProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCleanupProof {
    MergedPullRequest(MergedPullRequest),
    AncestorOfDefaultBranch {
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
pub enum BranchCleanupOutcome {
    Skipped {
        branch: Option<BranchName>,
        reason: BranchCleanupSkipReason,
    },
    DeclinedSourceBranch {
        branch: BranchName,
    },
    DeletedSourceBranch {
        branch: BranchName,
    },
    DeclinedUpstreamBranch {
        branch: BranchName,
    },
    DeletedUpstreamBranch {
        branch: BranchName,
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

pub trait BranchCleanupPrompt {
    fn confirm_source_branch_delete(&mut self, candidate: &BranchCleanupCandidate) -> bool;
    fn confirm_upstream_branch_delete(&mut self, candidate: &BranchCleanupCandidate) -> bool;
}

pub struct BranchCleanupOptions<'a> {
    pub provider: Option<&'a dyn BranchCleanupProvider>,
    pub prompt: &'a mut dyn BranchCleanupPrompt,
}

pub enum BranchCleanupMode<'a> {
    Disabled,
    NonInteractive,
    Prompt(BranchCleanupOptions<'a>),
}

pub fn run(source: &SourceRepo, opts: RemoveOptions) -> OutpostResult<()> {
    run_internal(source, opts, BranchCleanupMode::Disabled).map(|_| ())
}

pub fn run_with_cleanup(
    source: &SourceRepo,
    opts: RemoveOptions,
    mode: BranchCleanupMode<'_>,
) -> OutpostResult<RemoveReport> {
    run_internal(source, opts, mode)
}

fn run_internal(
    source: &SourceRepo,
    opts: RemoveOptions,
    mut mode: BranchCleanupMode<'_>,
) -> OutpostResult<RemoveReport> {
    let mut branch_cleanup = Vec::new();
    let entry = resolve_entry(source, &opts.selector)?.entry;
    let report_path = entry.path.clone();

    if entry.locked && !opts.force {
        return Err(OutpostError::OutpostLocked {
            path: entry.path,
            reason: lock_reason(&entry.lock_reason),
        });
    }

    if !entry.path.exists() {
        let mut registry = source.registry_mut()?;
        registry.remove_by_path(&entry.path)?;
        registry.save()?;
        record_mode_skip(
            &mode,
            &mut branch_cleanup,
            BranchCleanupSkipReason::MissingOutpost,
        );
        return Ok(RemoveReport {
            path: report_path,
            branch_cleanup,
        });
    }

    let outpost = safety::check_entry_is_managed_outpost_of(source, &entry)?;
    if !opts.force {
        safety::check_clean(outpost.work_tree(), outpost.git())?;
        safety::check_no_unpushed(&outpost, source)?;
    }

    let candidate = match &mut mode {
        BranchCleanupMode::Disabled => {
            branch_cleanup.push(BranchCleanupOutcome::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::CleanupDisabled,
            });
            None
        }
        BranchCleanupMode::NonInteractive => {
            branch_cleanup.push(BranchCleanupOutcome::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::NonInteractive,
            });
            None
        }
        BranchCleanupMode::Prompt(options) => {
            analyze_branch_cleanup(source, &outpost, options.provider, &mut branch_cleanup)
        }
    };

    let mut registry = source.registry_mut()?;
    registry.remove_by_path(&entry.path)?;
    registry.save()?;
    std::fs::remove_dir_all(&entry.path).map_err(|source| OutpostError::IoAt {
        path: entry.path,
        source,
    })?;

    if let (Some(candidate), BranchCleanupMode::Prompt(options)) = (candidate, mode) {
        perform_branch_cleanup(source, candidate, options.prompt, &mut branch_cleanup);
    }

    Ok(RemoveReport {
        path: report_path,
        branch_cleanup,
    })
}

fn lock_reason(reason: &Option<String>) -> String {
    reason
        .as_ref()
        .map(|reason| format!(": {reason}"))
        .unwrap_or_default()
}

fn record_mode_skip(
    mode: &BranchCleanupMode<'_>,
    branch_cleanup: &mut Vec<BranchCleanupOutcome>,
    missing_prompt_reason: BranchCleanupSkipReason,
) {
    let reason = match mode {
        BranchCleanupMode::Disabled => BranchCleanupSkipReason::CleanupDisabled,
        BranchCleanupMode::NonInteractive => BranchCleanupSkipReason::NonInteractive,
        BranchCleanupMode::Prompt(_) => missing_prompt_reason,
    };
    branch_cleanup.push(BranchCleanupOutcome::Skipped {
        branch: None,
        reason,
    });
}

fn analyze_branch_cleanup(
    source: &SourceRepo,
    outpost: &Outpost,
    provider: Option<&dyn BranchCleanupProvider>,
    outcomes: &mut Vec<BranchCleanupOutcome>,
) -> Option<BranchCleanupCandidate> {
    let upstream = match outpost.upstream_tracking() {
        Ok(Some(upstream)) => upstream,
        Ok(None) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::NoUpstreamTracking,
            });
            return None;
        }
        Err(OutpostError::BranchNotFound { .. }) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: None,
                reason: BranchCleanupSkipReason::DetachedHead,
            });
            return None;
        }
        Err(err) => {
            outcomes.push(warning(None, "cannot inspect outpost upstream", err));
            return None;
        }
    };

    if upstream.remote != outpost.metadata().remote_name {
        outcomes.push(BranchCleanupOutcome::Skipped {
            branch: None,
            reason: BranchCleanupSkipReason::UpstreamRemoteMismatch,
        });
        return None;
    }

    let Some(branch) = upstream.short_branch() else {
        outcomes.push(BranchCleanupOutcome::Skipped {
            branch: None,
            reason: BranchCleanupSkipReason::UpstreamNotBranch,
        });
        return None;
    };
    let branch = match BranchName::parse(branch.to_owned()) {
        Ok(branch) => branch,
        Err(err) => {
            outcomes.push(warning(None, "cannot parse outpost upstream branch", err));
            return None;
        }
    };

    let Some(source_oid) = (match source.branch_oid(&branch) {
        Ok(oid) => oid,
        Err(err) => {
            outcomes.push(warning(
                Some(branch.clone()),
                "cannot inspect source branch",
                err,
            ));
            return None;
        }
    }) else {
        outcomes.push(BranchCleanupOutcome::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::SourceBranchMissing,
        });
        return None;
    };

    let outpost_oid = match outpost.git().run_capture(["rev-parse", "HEAD"]) {
        Ok(oid) => oid,
        Err(err) => {
            outcomes.push(warning(
                Some(branch.clone()),
                "cannot inspect outpost HEAD",
                err,
            ));
            return None;
        }
    };
    if outpost_oid != source_oid {
        outcomes.push(BranchCleanupOutcome::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::OutpostHeadMismatch,
        });
        return None;
    }

    match source.is_branch_checked_out(&branch) {
        Ok(true) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::BranchCheckedOut,
            });
            return None;
        }
        Ok(false) => {}
        Err(err) => {
            outcomes.push(warning(
                Some(branch.clone()),
                "cannot inspect checked-out source branches",
                err,
            ));
            return None;
        }
    }

    let (default_branch, default_oid) = match source.fetch_origin_default_branch() {
        Ok(Some(default)) => default,
        Ok(None) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            return None;
        }
        Err(err) => {
            outcomes.push(warning(
                Some(branch.clone()),
                "cannot inspect upstream default branch",
                err,
            ));
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::DefaultBranchUnknown,
            });
            return None;
        }
    };
    if branch == default_branch {
        outcomes.push(BranchCleanupOutcome::Skipped {
            branch: Some(branch),
            reason: BranchCleanupSkipReason::DefaultBranch,
        });
        return None;
    }

    let upstream_oid = match source.origin_branch_oid(&branch) {
        Ok(oid) => oid,
        Err(err) => {
            outcomes.push(warning(
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
                    upstream_oid,
                    proof: BranchCleanupProof::MergedPullRequest(merged_pr),
                });
            }
            Ok(Some(_)) => {
                outcomes.push(BranchCleanupOutcome::Warning {
                    branch: Some(branch.clone()),
                    message: "provider proof did not match the source branch tip".to_owned(),
                });
            }
            Ok(None) => {}
            Err(err) => {
                outcomes.push(warning(
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
            upstream_oid,
            proof: BranchCleanupProof::AncestorOfDefaultBranch {
                default_branch,
                default_oid,
            },
        }),
        Ok(false) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: Some(branch),
                reason: BranchCleanupSkipReason::NoProof,
            });
            None
        }
        Err(err) => {
            outcomes.push(warning(
                Some(branch.clone()),
                "cannot prove source branch is merged",
                err,
            ));
            None
        }
    }
}

fn perform_branch_cleanup(
    source: &SourceRepo,
    candidate: BranchCleanupCandidate,
    prompt: &mut dyn BranchCleanupPrompt,
    outcomes: &mut Vec<BranchCleanupOutcome>,
) {
    if !prompt.confirm_source_branch_delete(&candidate) {
        outcomes.push(BranchCleanupOutcome::DeclinedSourceBranch {
            branch: candidate.branch,
        });
        return;
    }

    if let Err(err) = source.delete_branch_if_oid(&candidate.branch, &candidate.source_oid) {
        outcomes.push(warning(
            Some(candidate.branch),
            "source branch was not deleted",
            err,
        ));
        return;
    }
    outcomes.push(BranchCleanupOutcome::DeletedSourceBranch {
        branch: candidate.branch.clone(),
    });

    let Some(expected_upstream_oid) = candidate.upstream_oid.as_deref() else {
        return;
    };
    match source.origin_branch_oid(&candidate.branch) {
        Ok(Some(current_oid)) if current_oid == expected_upstream_oid => {}
        Ok(_) => {
            outcomes.push(BranchCleanupOutcome::Warning {
                branch: Some(candidate.branch),
                message: "upstream branch changed or disappeared before deletion".to_owned(),
            });
            return;
        }
        Err(err) => {
            outcomes.push(warning(
                Some(candidate.branch),
                "cannot re-check upstream branch before deletion",
                err,
            ));
            return;
        }
    }

    if !prompt.confirm_upstream_branch_delete(&candidate) {
        outcomes.push(BranchCleanupOutcome::DeclinedUpstreamBranch {
            branch: candidate.branch,
        });
        return;
    }

    match source.delete_origin_branch_if_oid(&candidate.branch, expected_upstream_oid) {
        Ok(()) => outcomes.push(BranchCleanupOutcome::DeletedUpstreamBranch {
            branch: candidate.branch,
        }),
        Err(err) => outcomes.push(warning(
            Some(candidate.branch),
            "upstream branch was not deleted",
            err,
        )),
    }
}

fn warning(branch: Option<BranchName>, context: &str, err: OutpostError) -> BranchCleanupOutcome {
    BranchCleanupOutcome::Warning {
        branch,
        message: format!("{context}: {err}"),
    }
}
