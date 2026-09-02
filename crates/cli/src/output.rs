use std::path::Path;

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
use outpost_core::ops::status::{
    ConfigProblem, OutpostHeadStatus, OutpostStatus, RegisteredOutpostHead, RemoteRoutes,
    RouteAvailability, SourceHead, SourceLocation, SourceStatus, SourceUpstreamStatus,
    StatusReport, TrackedUpstream,
};

pub fn print_added(outpost: &outpost_core::Outpost) {
    println!("added {}", outpost.work_tree().display());
}

pub fn print_list(summaries: &[ops::list::OutpostSummary], verbose: bool) {
    for summary in summaries {
        match &summary.state {
            ops::list::OutpostState::Present { head_oid, head } => {
                let head = match head {
                    ops::list::OutpostHead::Attached(branch) => {
                        format!("[{}]", branch.as_str())
                    }
                    ops::list::OutpostHead::Detached => "(detached HEAD)".to_owned(),
                };
                println!(
                    "{}\t{}\t{}\t{}{}",
                    summary.display_id,
                    summary.path.display(),
                    short_list_oid(head_oid),
                    head,
                    lock_suffix(summary.locked)
                );
            }
            ops::list::OutpostState::Missing => println!(
                "{}\t{}\t-\t(missing){}",
                summary.display_id,
                summary.path.display(),
                lock_suffix(summary.locked)
            ),
            ops::list::OutpostState::NotManaged => println!(
                "{}\t{}\t-\t(not managed){}",
                summary.display_id,
                summary.path.display(),
                lock_suffix(summary.locked)
            ),
        }
        if verbose {
            if let Some(reason) = &summary.lock_reason {
                println!("  lock-reason: {reason}");
            }
        }
    }
}

pub fn print_path(path: &Path) {
    println!("{}", path.display());
}

pub fn print_status(report: &StatusReport) {
    match report {
        StatusReport::Source(report) => print_source_status(report),
        StatusReport::Outpost(report) => print_outpost_status(report),
    }
}

fn print_source_status(report: &SourceStatus) {
    println!("context: source");
    println!("source: {}", report.source_path.display());
    match &report.head {
        SourceHead::Attached { branch, .. } => println!("branch: {}", branch.as_str()),
        SourceHead::Detached => println!("branch: detached"),
    }
    println!(
        "source-state: {}",
        if report.source_dirty {
            "dirty"
        } else {
            "clean"
        }
    );
    match &report.head {
        SourceHead::Attached { upstream, .. } => match upstream {
            Some(upstream) => print_tracked_upstream("upstream", upstream),
            None => println!("upstream: <unset>"),
        },
        SourceHead::Detached => println!("upstream: <not-applicable>"),
    }
    match &report.outpost_container {
        Some(path) => println!("outpost-container: {}", path.display()),
        None => println!("outpost-container: <unset>"),
    }
    if report.outposts.is_empty() {
        println!("outposts: none");
    } else {
        println!("outposts:");
        for outpost in &report.outposts {
            let head = match &outpost.head {
                RegisteredOutpostHead::Attached(branch) => branch.as_str(),
                RegisteredOutpostHead::Detached => "detached",
            };
            let state = if outpost.dirty { "dirty" } else { "clean" };
            if outpost.locked {
                println!(
                    "  {}\t{}\t{}\t{}\tlocked",
                    outpost.display_id,
                    outpost.path.display(),
                    head,
                    state
                );
            } else {
                println!(
                    "  {}\t{}\t{}\t{}",
                    outpost.display_id,
                    outpost.path.display(),
                    head,
                    state
                );
            }
        }
    }
    if report.stale_registrations.is_empty() {
        println!("stale-registrations: none");
    } else {
        println!("stale-registrations:");
        for registration in &report.stale_registrations {
            println!(
                "  {}\t{}",
                registration.display_id,
                registration.path.display()
            );
        }
    }
}

fn print_outpost_status(report: &OutpostStatus) {
    println!("context: outpost");
    println!("outpost: {}", report.outpost_path.display());
    match &report.source {
        SourceLocation::Unconfigured => {
            println!("source: -");
            println!("source-present: false");
        }
        SourceLocation::Missing(path) => {
            println!("source: {}", path.display());
            println!("source-present: false");
        }
        SourceLocation::Present(path) => {
            println!("source: {}", path.display());
            println!("source-present: true");
        }
    }
    match &report.remote_name {
        Some(remote) => println!("remote: {}", remote.as_str()),
        None => println!("remote: -"),
    }
    match &report.head {
        OutpostHeadStatus::Attached { branch, .. } => println!("branch: {}", branch.as_str()),
        OutpostHeadStatus::Detached => println!("branch: detached"),
    }
    println!(
        "outpost-state: {}",
        if report.outpost_dirty {
            "dirty"
        } else {
            "clean"
        }
    );
    match &report.head {
        OutpostHeadStatus::Attached {
            source_upstream, ..
        } => match source_upstream {
            SourceUpstreamStatus::Configured(upstream) => {
                print_tracked_upstream("source-upstream", upstream);
            }
            SourceUpstreamStatus::Unset => println!("source-upstream: <unset>"),
            SourceUpstreamStatus::Unavailable => println!("source-upstream: <unavailable>"),
        },
        OutpostHeadStatus::Detached => println!("source-upstream: <not-applicable>"),
    }
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

fn print_tracked_upstream(label: &str, upstream: &TrackedUpstream) {
    match upstream {
        TrackedUpstream::LocalRepository { branch } => {
            println!("{label}: ./{branch}  <local-repository>");
        }
        TrackedUpstream::Remote {
            remote,
            branch,
            routes,
        } => print_remote_routes(label, remote.as_str(), branch.as_str(), routes),
    }
}

fn print_remote_routes(label: &str, remote: &str, branch: &str, routes: &RemoteRoutes) {
    if routes.fetch == routes.push {
        print_route(label, remote, branch, &routes.fetch);
    } else {
        print_route(&format!("{label}-fetch"), remote, branch, &routes.fetch);
        print_route(&format!("{label}-push"), remote, branch, &routes.push);
    }
}

fn print_route(label: &str, remote: &str, branch: &str, route: &RouteAvailability) {
    match route {
        RouteAvailability::Known(urls) => {
            for url in urls.as_slice() {
                println!("{label}: {remote}/{branch}  {url}");
            }
        }
        RouteAvailability::Unavailable => {
            println!("{label}: {remote}/{branch}  <unavailable>");
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

fn short_list_oid(oid: &str) -> &str {
    oid.get(..12).unwrap_or(oid)
}

fn lock_suffix(locked: bool) -> &'static str {
    if locked { "\t(locked)" } else { "" }
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
        ConfigProblem::SourceBranchMissing { branch } => {
            format!("source branch missing: {}", branch.as_str())
        }
        ConfigProblem::OutpostSourceTrackingUnavailable { branch } => format!(
            "outpost-to-source tracking unavailable for {}",
            branch.as_str()
        ),
        ConfigProblem::SourceUpstreamTrackingUnset { branch } => {
            format!("source upstream tracking unset for {}", branch.as_str())
        }
        ConfigProblem::SourceUpstreamRouteUnavailable { remote } => {
            format!("source upstream route unavailable: {}", remote.as_str())
        }
        ConfigProblem::NotInRegistry => "not in source registry".to_owned(),
        ConfigProblem::PushWouldFail { branch } => {
            format!("push would fail for {}", branch.as_str())
        }
        ConfigProblem::InvalidMetadata { reason } => {
            format!("invalid outpost metadata: {reason}")
        }
    }
}

fn format_push_step(step: ops::push::StepResult) -> String {
    match step {
        ops::push::StepResult::Pushed { commits } => format!("pushed {commits} commit(s)"),
    }
}

fn format_gh_status(status: &gh::GhStatus) -> Option<String> {
    if status.is_not_installed() {
        Some(
            "branch-cleanup: gh not found; merged-PR proof unavailable; trying local Git proof only"
                .to_owned(),
        )
    } else {
        status.unavailable_message().map(|message| format!(
            "branch-cleanup: gh unavailable: {message}; merged-PR proof unavailable; trying local Git proof only"
        ))
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
            format_gh_status(&gh::GhStatus::not_installed_for_tests()).as_deref(),
            Some(
                "branch-cleanup: gh not found; merged-PR proof unavailable; trying local Git proof only"
            )
        );

        let message = format_gh_status(&gh::GhStatus::unavailable_for_tests("permission denied"))
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
