use std::path::{Path, PathBuf};

use crate::ops::branch_analysis::{self, BranchCleanupFinding};
use crate::selector::{OutpostSelector, resolve_entry};
use crate::{BranchName, OutpostError, OutpostResult, RemoteName, SourceRepo, safety};

pub use crate::ops::branch_analysis::{
    BranchCleanupCandidate, BranchCleanupProof, CleanupEvidenceProvider, MergedPullRequest,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchCleanupSkipReason {
    CleanupDisabled,
    NonInteractive,
    MissingOutpost,
    /// Branch analysis found no safe cleanup candidate.
    Analysis(branch_analysis::BranchCleanupSkipReason),
}

impl From<branch_analysis::BranchCleanupSkipReason> for BranchCleanupSkipReason {
    fn from(reason: branch_analysis::BranchCleanupSkipReason) -> Self {
        Self::Analysis(reason)
    }
}

pub trait BranchCleanupPrompt {
    fn confirm_source_branch_delete(&mut self, candidate: &BranchCleanupCandidate) -> bool;
    fn confirm_upstream_branch_delete(&mut self, candidate: &BranchCleanupCandidate) -> bool;
}

pub struct BranchCleanupOptions<'a> {
    pub provider: Option<&'a dyn CleanupEvidenceProvider>,
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
        safety::check_removable_work(&outpost, source)?;
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
    std::fs::remove_dir_all(&entry.path).map_err(directory_not_removed(&entry.path))?;

    if let (Some((candidate, observed_url)), BranchCleanupMode::Prompt(options)) = (candidate, mode)
    {
        perform_branch_cleanup(
            source,
            candidate,
            &observed_url,
            options.prompt,
            &mut branch_cleanup,
        );
    }

    Ok(RemoveReport {
        path: report_path,
        branch_cleanup,
    })
}

/// The registry entry is already gone when directory deletion fails.
fn directory_not_removed(path: &Path) -> impl FnOnce(std::io::Error) -> OutpostError + '_ {
    |source| OutpostError::OutpostDirectoryNotRemoved {
        path: path.to_owned(),
        source,
    }
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
    provider: Option<&dyn CleanupEvidenceProvider>,
    outcomes: &mut Vec<BranchCleanupOutcome>,
) -> Option<(BranchCleanupCandidate, String)> {
    let analysis = branch_analysis::analyze_branch_cleanup(source, outpost, provider);
    outcomes.extend(
        analysis
            .findings
            .into_iter()
            .map(BranchCleanupOutcome::from),
    );
    analysis.candidate.zip(
        analysis
            .evidence
            .map(|observation| observation.request.upstream_url),
    )
}

impl From<BranchCleanupFinding> for BranchCleanupOutcome {
    fn from(finding: BranchCleanupFinding) -> Self {
        match finding {
            BranchCleanupFinding::Skipped { branch, reason } => BranchCleanupOutcome::Skipped {
                branch,
                reason: reason.into(),
            },
            BranchCleanupFinding::Warning { branch, message } => {
                BranchCleanupOutcome::Warning { branch, message }
            }
        }
    }
}

fn perform_branch_cleanup(
    source: &SourceRepo,
    candidate: BranchCleanupCandidate,
    observed_url: &str,
    prompt: &mut dyn BranchCleanupPrompt,
    outcomes: &mut Vec<BranchCleanupOutcome>,
) {
    if !prompt.confirm_source_branch_delete(&candidate) {
        outcomes.push(BranchCleanupOutcome::DeclinedSourceBranch {
            branch: candidate.branch,
        });
        return;
    }

    match source.is_branch_checked_out(&candidate.branch) {
        Ok(true) => {
            outcomes.push(BranchCleanupOutcome::Skipped {
                branch: Some(candidate.branch),
                reason: branch_analysis::BranchCleanupSkipReason::BranchCheckedOut.into(),
            });
            return;
        }
        Err(err) => {
            outcomes.push(warning(
                Some(candidate.branch),
                "cannot inspect checked-out source branches",
                err,
            ));
            return;
        }
        Ok(false) => {}
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

    if let Err(err) =
        source.verify_remote_deletion_destination(&candidate.upstream_remote, observed_url)
    {
        outcomes.push(warning(
            Some(candidate.branch),
            "upstream branch was not deleted",
            err,
        ));
        return;
    }
    if !prompt.confirm_upstream_branch_delete(&candidate) {
        outcomes.push(BranchCleanupOutcome::DeclinedUpstreamBranch {
            remote: candidate.upstream_remote,
            branch: candidate.branch,
        });
        return;
    }

    match source.delete_observed_remote_branch_if_oid(
        &candidate.upstream_remote,
        observed_url,
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
