use crate::gh;
use outpost_core::AheadBehind;
use outpost_core::BranchName;
use outpost_core::ops;
use outpost_core::ops::status::ConfigProblem;

pub fn print_added(outpost: &outpost_core::Outpost) {
    println!("added {}", outpost.work_tree().display());
}

pub fn print_list(summaries: &[ops::list::OutpostSummary], verbose: bool) {
    for summary in summaries {
        let branch = summary
            .current_branch
            .as_ref()
            .map(|branch| branch.as_str())
            .unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}{}",
            summary.path.display(),
            branch,
            list_state(summary.state),
            format_ahead_behind(summary.ahead_behind),
            lock_suffix(summary.locked)
        );
        if verbose {
            if let Some(reason) = &summary.lock_reason {
                println!("  lock-reason: {reason}");
            }
        }
    }
}

pub fn print_status(report: &ops::status::StatusReport) {
    println!("outpost: {}", report.outpost_path.display());
    match &report.source_path {
        Some(path) => println!("source: {}", path.display()),
        None => println!("source: -"),
    }
    println!("source-present: {}", report.source_present);
    match &report.remote_name {
        Some(remote) => println!("remote: {}", remote.as_str()),
        None => println!("remote: -"),
    }
    match &report.current_branch {
        Some(branch) => println!("branch: {}", branch.as_str()),
        None => println!("branch: detached"),
    }
    println!(
        "outpost-state: {}",
        if report.outpost_dirty {
            "dirty"
        } else {
            "clean"
        }
    );
    println!(
        "outpost-vs-source: {}",
        format_ahead_behind(report.outpost_ahead_behind_source)
    );
    println!(
        "source-vs-upstream: {}",
        format_ahead_behind(report.source_ahead_behind_upstream)
    );

    if report.problems.is_empty() {
        println!("health: ok");
    } else {
        println!("health: problems");
        for problem in &report.problems {
            println!("  - {}", format_problem(problem));
        }
    }
}

pub fn print_pull(report: &ops::pull::PullReport) {
    println!(
        "source: {}",
        if report.source_updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
    println!(
        "outpost: {}",
        if report.outpost_updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
}

pub fn print_source_pull(report: &ops::source::SourcePullReport) {
    println!(
        "source {}: {}",
        report.branch.as_str(),
        if report.updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
}

pub fn print_merge(report: &ops::merge::MergeReport) {
    println!(
        "merged {}/{}",
        report.source_ref.remote.as_str(),
        report.source_ref.branch.as_str()
    );
}

pub fn print_rebase(report: &ops::rebase::RebaseReport) {
    println!(
        "rebased onto {}/{}",
        report.source_ref.remote.as_str(),
        report.source_ref.branch.as_str()
    );
}

pub fn print_push(report: &ops::push::PushReport) {
    println!(
        "outpost-to-source: {}",
        format_push_step(report.outpost_to_source)
    );
    println!(
        "source-to-origin: {}",
        format_push_step(report.source_to_origin)
    );
}

pub fn print_remove(report: &ops::remove::RemoveReport, gh_status: Option<&gh::GhStatus>) {
    println!("removed {}", report.path.display());
    if let Some(status) = gh_status.and_then(format_gh_status) {
        eprintln!("{status}");
    }
    for outcome in &report.branch_cleanup {
        eprintln!("{}", format_branch_cleanup_outcome(outcome));
    }
}

pub fn print_prune(report: &ops::prune::PruneReport, verbose: bool) {
    if report.dry_run {
        println!("dry-run: true");
    }
    println!("removed: {}", report.removed_entries.len());
    if verbose {
        for path in &report.removed_entries {
            println!("  {}", path.display());
        }
    }
    println!("source-missing: {}", report.orphaned_source_missing.len());
    println!("locked: {}", report.locked_entries.len());
}

fn list_state(state: ops::list::OutpostState) -> &'static str {
    match state {
        ops::list::OutpostState::Clean => "clean",
        ops::list::OutpostState::Dirty => "dirty",
        ops::list::OutpostState::Missing => "missing",
        ops::list::OutpostState::NotManaged => "not-managed",
    }
}

fn lock_suffix(locked: bool) -> &'static str {
    if locked { "\tlocked" } else { "" }
}

fn format_ahead_behind(value: Option<AheadBehind>) -> String {
    match value {
        Some(value) => format!("ahead {}, behind {}", value.ahead, value.behind),
        None => "-".to_owned(),
    }
}

fn format_problem(problem: &ConfigProblem) -> String {
    match problem {
        ConfigProblem::MissingSourceRepoConfig => "missing source repo config".to_owned(),
        ConfigProblem::SourceMissing(path) => format!("source missing: {}", path.display()),
        ConfigProblem::MissingRemoteNameConfig => "missing remote name config".to_owned(),
        ConfigProblem::LocalRemoteMismatch { configured, actual } => format!(
            "local remote mismatch: configured {}, actual {}",
            configured.display(),
            actual.display()
        ),
        ConfigProblem::NoUpstreamTracking { branch } => {
            format!("no upstream tracking for {}", branch.as_str())
        }
        ConfigProblem::NotInRegistry => "not in source registry".to_owned(),
        ConfigProblem::PushWouldFail { branch } => {
            format!("push would fail for {}", branch.as_str())
        }
    }
}

fn format_push_step(step: ops::push::StepResult) -> String {
    match step {
        ops::push::StepResult::Pushed { commits } => format!("pushed {commits} commit(s)"),
    }
}

fn format_gh_status(status: &gh::GhStatus) -> Option<String> {
    match status {
        gh::GhStatus::Available(_) => None,
        gh::GhStatus::NotInstalled => Some(
            "branch-cleanup: gh not found; merged-PR proof unavailable; trying local Git proof only"
                .to_owned(),
        ),
        gh::GhStatus::Unavailable { message } => Some(format!(
            "branch-cleanup: gh unavailable: {message}; merged-PR proof unavailable; trying local Git proof only"
        )),
    }
}

fn format_branch_cleanup_outcome(outcome: &ops::remove::BranchCleanupOutcome) -> String {
    match outcome {
        ops::remove::BranchCleanupOutcome::Skipped { branch, reason } => {
            format_branch_cleanup_skip(branch.as_ref(), *reason)
        }
        ops::remove::BranchCleanupOutcome::DeclinedSourceBranch { branch } => {
            format!("branch-cleanup: kept source branch {}", branch.as_str())
        }
        ops::remove::BranchCleanupOutcome::DeletedSourceBranch { branch } => {
            format!("cleanup: deleted source branch {}", branch.as_str())
        }
        ops::remove::BranchCleanupOutcome::DeclinedUpstreamBranch { branch } => {
            format!(
                "branch-cleanup: kept upstream branch origin/{}",
                branch.as_str()
            )
        }
        ops::remove::BranchCleanupOutcome::DeletedUpstreamBranch { branch } => {
            format!(
                "cleanup: deleted upstream branch origin/{}",
                branch.as_str()
            )
        }
        ops::remove::BranchCleanupOutcome::Warning { message, .. } => {
            format!("warning: {message}")
        }
    }
}

fn format_branch_cleanup_skip(
    branch: Option<&BranchName>,
    reason: ops::remove::BranchCleanupSkipReason,
) -> String {
    let prefix = match branch {
        Some(branch) => format!(
            "branch-cleanup: skipped source branch {}: ",
            branch.as_str()
        ),
        None => "branch-cleanup: skipped: ".to_owned(),
    };
    let reason = match reason {
        ops::remove::BranchCleanupSkipReason::CleanupDisabled => "cleanup disabled",
        ops::remove::BranchCleanupSkipReason::NonInteractive => {
            "non-interactive terminal; branch cleanup requires prompts"
        }
        ops::remove::BranchCleanupSkipReason::MissingOutpost => "outpost path was already missing",
        ops::remove::BranchCleanupSkipReason::DetachedHead => "outpost HEAD is detached",
        ops::remove::BranchCleanupSkipReason::NoUpstreamTracking => {
            "outpost has no upstream tracking branch"
        }
        ops::remove::BranchCleanupSkipReason::UpstreamRemoteMismatch => {
            "outpost upstream remote does not match the configured source remote"
        }
        ops::remove::BranchCleanupSkipReason::UpstreamNotBranch => {
            "outpost upstream is not a branch"
        }
        ops::remove::BranchCleanupSkipReason::SourceBranchMissing => "source branch is missing",
        ops::remove::BranchCleanupSkipReason::OutpostHeadMismatch => {
            "outpost HEAD does not match source branch tip"
        }
        ops::remove::BranchCleanupSkipReason::BranchCheckedOut => "branch is checked out",
        ops::remove::BranchCleanupSkipReason::DefaultBranch => {
            "branch is the upstream default branch"
        }
        ops::remove::BranchCleanupSkipReason::DefaultBranchUnknown => {
            "upstream default branch is unknown"
        }
        ops::remove::BranchCleanupSkipReason::NoProof => "no safe deletion proof found",
    };
    format!("{prefix}{reason}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ops::remove::BranchCleanupSkipReason;

    #[test]
    fn branch_cleanup_skip_reasons_have_useful_messages() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let cases = [
            (
                BranchCleanupSkipReason::CleanupDisabled,
                false,
                "cleanup disabled",
            ),
            (
                BranchCleanupSkipReason::NonInteractive,
                false,
                "non-interactive",
            ),
            (
                BranchCleanupSkipReason::MissingOutpost,
                false,
                "outpost path was already missing",
            ),
            (
                BranchCleanupSkipReason::DetachedHead,
                false,
                "outpost HEAD is detached",
            ),
            (
                BranchCleanupSkipReason::NoUpstreamTracking,
                false,
                "no upstream tracking branch",
            ),
            (
                BranchCleanupSkipReason::UpstreamRemoteMismatch,
                false,
                "upstream remote does not match",
            ),
            (
                BranchCleanupSkipReason::UpstreamNotBranch,
                false,
                "upstream is not a branch",
            ),
            (
                BranchCleanupSkipReason::SourceBranchMissing,
                true,
                "source branch is missing",
            ),
            (
                BranchCleanupSkipReason::OutpostHeadMismatch,
                true,
                "does not match source branch tip",
            ),
            (
                BranchCleanupSkipReason::BranchCheckedOut,
                true,
                "branch is checked out",
            ),
            (
                BranchCleanupSkipReason::DefaultBranch,
                true,
                "upstream default branch",
            ),
            (
                BranchCleanupSkipReason::DefaultBranchUnknown,
                true,
                "default branch is unknown",
            ),
            (
                BranchCleanupSkipReason::NoProof,
                true,
                "no safe deletion proof found",
            ),
        ];

        for (reason, include_branch, expected) in cases {
            let message = format_branch_cleanup_skip(include_branch.then_some(&branch), reason);
            assert!(
                message.starts_with("branch-cleanup: skipped"),
                "skip message should identify branch cleanup: {message}"
            );
            assert!(
                message.contains(expected),
                "skip message for {reason:?} should contain {expected:?}: {message}"
            );
            if include_branch {
                assert!(
                    message.contains("source branch feat"),
                    "branch-specific skip should include branch name: {message}"
                );
            }
        }
    }

    #[test]
    fn gh_status_diagnostics_explain_proof_fallback() {
        assert_eq!(
            format_gh_status(&gh::GhStatus::NotInstalled).as_deref(),
            Some(
                "branch-cleanup: gh not found; merged-PR proof unavailable; trying local Git proof only"
            )
        );

        let message = format_gh_status(&gh::GhStatus::Unavailable {
            message: "permission denied".to_owned(),
        })
        .expect("unavailable diagnostic");
        assert!(
            message.contains("permission denied")
                && message.contains("trying local Git proof only"),
            "unavailable gh diagnostic should preserve the cause and fallback: {message}"
        );
    }

    #[test]
    fn branch_cleanup_declines_are_reported_as_kept_branches() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");

        assert_eq!(
            format_branch_cleanup_outcome(
                &ops::remove::BranchCleanupOutcome::DeclinedSourceBranch {
                    branch: branch.clone(),
                }
            ),
            "branch-cleanup: kept source branch feat"
        );
        assert_eq!(
            format_branch_cleanup_outcome(
                &ops::remove::BranchCleanupOutcome::DeclinedUpstreamBranch { branch }
            ),
            "branch-cleanup: kept upstream branch origin/feat"
        );
    }
}
