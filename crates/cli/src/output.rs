use crate::gh;
use outpost_core::AheadBehind;
use outpost_core::BranchName;
use outpost_core::ops;
use outpost_core::ops::analyze::{
    BranchDeleteSafety, Probe, RemoteBranchIdentity, SourcePushHazard, UpstreamRemote,
};
use outpost_core::ops::branch_analysis::{
    BranchCleanupFinding, BranchCleanupProof, BranchCleanupSkipReason,
};
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
            "{}\t{}\t{}\t{}\t{}{}",
            summary.display_id,
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

pub fn print_analyze(report: &ops::analyze::AnalyzeReport, github: &gh::GithubAnalysis) {
    println!("outpost: {}", report.outpost_path.display());
    println!("source: {}", report.source_path.display());
    print_upstream_remote(&report.upstream_remote);
    match &report.branch {
        Some(branch) => println!("branch: {}", branch.as_str()),
        None => println!("branch: detached"),
    }
    println!(
        "state: {}",
        if report.outpost_dirty {
            "dirty"
        } else {
            "clean"
        }
    );
    println!(
        "lock: {}",
        if report.locked { "locked" } else { "unlocked" }
    );
    println!(
        "lock-reason: {}",
        report.lock_reason.as_deref().unwrap_or("none")
    );
    println!();
    println!(
        "outpost-vs-source: {}",
        format_probe_ahead_behind(&report.outpost_vs_source)
    );
    println!(
        "source-vs-upstream: {}",
        format_probe_ahead_behind(&report.source_vs_upstream)
    );
    println!(
        "source-vs-upstream-default: {}",
        format_probe_ahead_behind(&report.source_vs_upstream_default)
    );
    println!(
        "upstream-default-branch: {}",
        format_probe_identity(&report.upstream_default_branch)
    );
    println!(
        "upstream-branch: {}",
        format_probe_identity(&report.upstream_branch)
    );
    print_source_push_hazard(&report.source_push_hazard);
    println!();
    print_github_analysis(github);
    println!();
    print_safe_delete(report);
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

fn format_probe_ahead_behind(value: &Probe<AheadBehind>) -> String {
    match value {
        Probe::Known(value) => format!("ahead {}, behind {}", value.ahead, value.behind),
        Probe::Unknown(reason) => format!("unknown: {reason}"),
        Probe::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

fn format_probe_identity(value: &Probe<RemoteBranchIdentity>) -> String {
    match value {
        Probe::Known(identity) => {
            format!(
                "{}/{} at {}",
                identity.remote.as_str(),
                identity.branch.as_str(),
                identity.oid
            )
        }
        Probe::Unknown(reason) => format!("unknown: {reason}"),
        Probe::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

fn print_source_push_hazard(value: &Probe<SourcePushHazard>) {
    match value {
        Probe::Known(hazard) => {
            println!("source-branch-checked-out: {}", yes_no(hazard.checked_out));
            println!("push-hazard: {}", yes_no(hazard.push_would_fail));
        }
        Probe::Unknown(reason) => {
            println!("source-branch-checked-out: unknown: {reason}");
            println!("push-hazard: unknown: {reason}");
        }
        Probe::Unavailable(reason) => {
            println!("source-branch-checked-out: unavailable: {reason}");
            println!("push-hazard: unavailable: {reason}");
        }
    }
}

fn print_upstream_remote(value: &Probe<UpstreamRemote>) {
    match value {
        Probe::Known(upstream) => {
            println!("upstream-remote: {}", upstream.remote.as_str());
            println!("upstream-url: {}", upstream.url);
        }
        Probe::Unknown(reason) => {
            println!("upstream-remote: unknown: {reason}");
            println!("upstream-url: unknown: {reason}");
        }
        Probe::Unavailable(reason) => {
            println!("upstream-remote: unavailable: {reason}");
            println!("upstream-url: unavailable: {reason}");
        }
    }
}

fn print_github_analysis(github: &gh::GithubAnalysis) {
    match &github.availability {
        gh::GithubAvailability::Available => println!("github: available"),
        gh::GithubAvailability::Unavailable(reason) => {
            println!("github: unavailable: {reason}");
        }
    }

    match &github.pull_requests {
        Probe::Known(prs) => {
            println!("pull-requests:");
            if prs.is_empty() {
                println!("  - none");
            } else {
                for pr in prs {
                    println!(
                        "  - {} {} draft={} base={} head={} review={} checks={}",
                        pr.id,
                        pr.state.to_ascii_lowercase(),
                        pr.draft,
                        pr.base,
                        pr.head,
                        pr.review,
                        pr.checks
                    );
                }
            }
        }
        Probe::Unknown(reason) => println!("pull-requests: unknown: {reason}"),
        Probe::Unavailable(reason) => println!("pull-requests: unavailable: {reason}"),
    }
}

fn print_safe_delete(report: &ops::analyze::AnalyzeReport) {
    match &report.safe_delete {
        BranchDeleteSafety::Yes(candidate) => {
            println!("safe-delete: yes");
            println!(
                "safe-delete-proof: {}",
                format_delete_proof(&candidate.proof)
            );
            println!("safe-delete-branch: {}", candidate.branch.as_str());
            println!("safe-delete-source-oid: {}", candidate.source_oid);
            println!(
                "safe-delete-upstream-oid: {}",
                candidate.upstream_oid.as_deref().unwrap_or("-")
            );
        }
        BranchDeleteSafety::No { branch: _, reason } => {
            println!("safe-delete: no");
            println!(
                "safe-delete-reason: {}",
                branch_cleanup_reason_text(*reason)
            );
        }
        BranchDeleteSafety::Unknown(reason) => {
            println!("safe-delete: unknown");
            println!("safe-delete-reason: {reason}");
        }
    }
    for finding in &report.safe_delete_findings {
        if let BranchCleanupFinding::Warning { message, .. } = finding {
            println!("safe-delete-warning: {message}");
        }
    }
}

fn format_delete_proof(proof: &BranchCleanupProof) -> String {
    match proof {
        BranchCleanupProof::MergedPullRequest(pr) => format!("merged-pull-request {}", pr.id),
        BranchCleanupProof::AncestorOfDefaultBranch {
            remote,
            default_branch,
            default_oid,
        } => {
            format!(
                "ancestor-of-upstream-default {}/{} at {}",
                remote.as_str(),
                default_branch.as_str(),
                default_oid
            )
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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
        ops::remove::BranchCleanupOutcome::DeclinedUpstreamBranch { remote, branch } => {
            format!(
                "branch-cleanup: kept upstream branch {}/{}",
                remote.as_str(),
                branch.as_str()
            )
        }
        ops::remove::BranchCleanupOutcome::DeletedUpstreamBranch { remote, branch } => {
            format!(
                "cleanup: deleted upstream branch {}/{}",
                remote.as_str(),
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
    reason: BranchCleanupSkipReason,
) -> String {
    let prefix = match branch {
        Some(branch) => format!(
            "branch-cleanup: skipped source branch {}: ",
            branch.as_str()
        ),
        None => "branch-cleanup: skipped: ".to_owned(),
    };
    format!("{prefix}{}", branch_cleanup_reason_text(reason))
}

fn branch_cleanup_reason_text(reason: BranchCleanupSkipReason) -> &'static str {
    match reason {
        BranchCleanupSkipReason::CleanupDisabled => "cleanup disabled",
        BranchCleanupSkipReason::NonInteractive => {
            "non-interactive terminal; branch cleanup requires prompts"
        }
        BranchCleanupSkipReason::MissingOutpost => "outpost path was already missing",
        BranchCleanupSkipReason::DetachedHead => "outpost HEAD is detached",
        BranchCleanupSkipReason::NoUpstreamTracking => "outpost has no upstream tracking branch",
        BranchCleanupSkipReason::UpstreamRemoteMismatch => {
            "outpost upstream remote does not match the configured source remote"
        }
        BranchCleanupSkipReason::UpstreamNotBranch => "outpost upstream is not a branch",
        BranchCleanupSkipReason::SourceBranchMissing => "source branch is missing",
        BranchCleanupSkipReason::OutpostHeadMismatch => {
            "outpost HEAD does not match source branch tip"
        }
        BranchCleanupSkipReason::BranchCheckedOut => "branch is checked out",
        BranchCleanupSkipReason::DefaultBranch => "branch is the upstream default branch",
        BranchCleanupSkipReason::DefaultBranchUnknown => "upstream default branch is unknown",
        BranchCleanupSkipReason::NoProof => "no safe deletion proof found",
    }
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
                "upstream default branch is unknown",
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
    fn default_ancestor_proof_names_upstream_default_branch() {
        let branch = BranchName::parse("main".to_owned()).expect("branch");
        let proof = BranchCleanupProof::AncestorOfDefaultBranch {
            remote: outpost_core::RemoteName::parse("upstream").expect("remote"),
            default_branch: branch,
            default_oid: "abc123".to_owned(),
        };

        assert!(
            format_delete_proof(&proof).contains("ancestor-of-upstream-default upstream/main"),
            "default branch proof should name the upstream default branch"
        );
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
                &ops::remove::BranchCleanupOutcome::DeclinedUpstreamBranch {
                    remote: outpost_core::RemoteName::parse("origin").expect("remote"),
                    branch
                }
            ),
            "branch-cleanup: kept upstream branch origin/feat"
        );
    }
}
