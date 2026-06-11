use std::path::PathBuf;

use crate::ops::branch_analysis::{self, BranchCleanupFinding};
use crate::selector::{OutpostSelector, resolve_entry};
use crate::{BranchName, OutpostError, OutpostResult, RemoteName, SourceRepo, safety};

pub use crate::ops::branch_analysis::{
    BranchCleanupCandidate, BranchCleanupProof, BranchCleanupProvider, BranchCleanupSkipReason,
    MergedPullRequest,
};

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
        remote: RemoteName,
        branch: BranchName,
    },
    DeletedUpstreamBranch {
        remote: RemoteName,
        branch: BranchName,
    },
    Warning {
        branch: Option<BranchName>,
        message: String,
    },
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
    outpost: &crate::Outpost,
    provider: Option<&dyn BranchCleanupProvider>,
    outcomes: &mut Vec<BranchCleanupOutcome>,
) -> Option<BranchCleanupCandidate> {
    let analysis = branch_analysis::analyze_branch_cleanup(source, outpost, provider);
    outcomes.extend(
        analysis
            .findings
            .into_iter()
            .map(BranchCleanupOutcome::from),
    );
    analysis.candidate
}

impl From<BranchCleanupFinding> for BranchCleanupOutcome {
    fn from(finding: BranchCleanupFinding) -> Self {
        match finding {
            BranchCleanupFinding::Skipped { branch, reason } => {
                BranchCleanupOutcome::Skipped { branch, reason }
            }
            BranchCleanupFinding::Warning { branch, message } => {
                BranchCleanupOutcome::Warning { branch, message }
            }
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
    match source.remote_branch_oid(&candidate.upstream_remote, &candidate.branch) {
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
            remote: candidate.upstream_remote,
            branch: candidate.branch,
        });
        return;
    }

    match source.delete_remote_branch_if_oid(
        &candidate.upstream_remote,
        &candidate.branch,
        expected_upstream_oid,
    ) {
        Ok(()) => outcomes.push(BranchCleanupOutcome::DeletedUpstreamBranch {
            remote: candidate.upstream_remote,
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
