use std::path::{Path, PathBuf};

use crate::ops::branch_analysis::{
    self, BranchCleanupCandidate, BranchCleanupFinding, BranchCleanupProvider,
    BranchCleanupSkipReason,
};
use crate::selector::{OutpostSelector, resolve_entry};
use crate::source_repo::read_optional_config;
use crate::{
    AheadBehind, BranchName, GitInvoker, Outpost, OutpostError, OutpostResult, RemoteName,
    Reporter, SourceRepo, StepKind,
};

pub struct AnalyzeOptions {
    pub selector: OutpostSelector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeReport {
    pub outpost_path: PathBuf,
    pub source_path: PathBuf,
    pub locked: bool,
    pub lock_reason: Option<String>,
    pub branch: Option<BranchName>,
    pub outpost_dirty: bool,
    pub upstream_remote: Probe<UpstreamRemote>,
    pub outpost_vs_source: Probe<AheadBehind>,
    pub source_vs_upstream: Probe<AheadBehind>,
    pub source_vs_upstream_default: Probe<AheadBehind>,
    pub upstream_default_branch: Probe<RemoteBranchIdentity>,
    pub upstream_branch: Probe<RemoteBranchIdentity>,
    pub source_push_hazard: Probe<SourcePushHazard>,
    pub safe_delete: BranchDeleteSafety,
    pub safe_delete_findings: Vec<BranchCleanupFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe<T> {
    Known(T),
    Unknown(String),
    Unavailable(String),
}

impl<T> Probe<T> {
    pub fn as_ref(&self) -> Probe<&T> {
        match self {
            Self::Known(value) => Probe::Known(value),
            Self::Unknown(reason) => Probe::Unknown(reason.clone()),
            Self::Unavailable(reason) => Probe::Unavailable(reason.clone()),
        }
    }

    pub fn map<U, F>(self, f: F) -> Probe<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Known(value) => Probe::Known(f(value)),
            Self::Unknown(reason) => Probe::Unknown(reason),
            Self::Unavailable(reason) => Probe::Unavailable(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteBranchIdentity {
    pub remote: RemoteName,
    pub branch: BranchName,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamRemote {
    pub remote: RemoteName,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceUpstreamBranch {
    remote: RemoteName,
    branch: BranchName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePushHazard {
    pub checked_out: bool,
    pub push_would_fail: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchDeleteSafety {
    Yes(BranchCleanupCandidate),
    No {
        branch: Option<BranchName>,
        reason: BranchCleanupSkipReason,
    },
    Unknown(String),
}

pub fn run(
    source: &SourceRepo,
    opts: AnalyzeOptions,
    provider: Option<&dyn BranchCleanupProvider>,
) -> OutpostResult<AnalyzeReport> {
    let mut reporter = SilentReporter;
    run_with_reporter(source, opts, provider, &mut reporter)
}

pub fn run_with_reporter(
    source: &SourceRepo,
    opts: AnalyzeOptions,
    provider: Option<&dyn BranchCleanupProvider>,
    reporter: &mut dyn Reporter,
) -> OutpostResult<AnalyzeReport> {
    reporter.step(StepKind::Analysis, "resolving outpost");
    let resolved = resolve_entry(source, &opts.selector)?;
    let entry = resolved.entry;
    let outpost = crate::safety::check_entry_is_managed_outpost_of(source, &entry)?;
    reporter.step(
        StepKind::Analysis,
        &format!("resolved: {}", outpost.work_tree().display()),
    );

    reporter.step(StepKind::Analysis, "checking outpost state");
    let branch = current_branch_optional(&outpost)?;
    let outpost_dirty = outpost.is_dirty()?;
    reporter.step(
        StepKind::Analysis,
        &format!(
            "{}, branch {}, lock {}",
            if outpost_dirty { "dirty" } else { "clean" },
            branch
                .as_ref()
                .map(|branch| branch.as_str())
                .unwrap_or("detached"),
            if entry.locked { "locked" } else { "unlocked" }
        ),
    );

    reporter.step(StepKind::Analysis, "comparing outpost and source");
    let outpost_vs_source = probe_ahead_behind_outpost_source(&outpost);
    reporter.step(
        StepKind::Analysis,
        &format_probe_ahead_behind(&outpost_vs_source),
    );

    let source_upstream = match branch.as_ref() {
        Some(branch) => probe_source_upstream(source, branch),
        None => Probe::Unknown("outpost HEAD is detached".to_owned()),
    };

    reporter.step(StepKind::Analysis, "checking upstream remote");
    let upstream_remote = match source_upstream.as_ref() {
        Probe::Known(upstream) => probe_upstream_remote(source, &upstream.remote),
        Probe::Unknown(reason) => Probe::Unknown(reason),
        Probe::Unavailable(reason) => Probe::Unavailable(reason),
    };
    reporter.step(
        StepKind::Analysis,
        &format_probe_upstream_remote(&upstream_remote),
    );

    reporter.step(StepKind::Analysis, "checking upstream branch");
    let upstream_branch = match source_upstream.as_ref() {
        Probe::Known(upstream) => probe_remote_branch(source, &upstream.remote, &upstream.branch),
        Probe::Unknown(reason) => Probe::Unknown(reason),
        Probe::Unavailable(reason) => Probe::Unavailable(reason),
    };
    reporter.step(StepKind::Analysis, &format_probe_identity(&upstream_branch));

    reporter.step(StepKind::Analysis, "discovering upstream default branch");
    let upstream_default_branch = match source_upstream.as_ref() {
        Probe::Known(upstream) => probe_default_branch(source, &upstream.remote),
        Probe::Unknown(reason) => Probe::Unknown(reason),
        Probe::Unavailable(reason) => Probe::Unavailable(reason),
    };
    reporter.step(
        StepKind::Analysis,
        &format_probe_identity(&upstream_default_branch),
    );
    reporter.step(StepKind::Analysis, "comparing source and upstream");
    let source_vs_upstream = match (
        branch.as_ref(),
        source_upstream.as_ref(),
        upstream_branch.as_ref(),
    ) {
        (Some(branch), Probe::Known(upstream), Probe::Known(_)) => probe_source_vs_remote_ref(
            source,
            branch,
            &remote_branch_ref(&upstream.remote, &upstream.branch),
        ),
        (Some(_), Probe::Unknown(reason), _) | (Some(_), _, Probe::Unknown(reason)) => {
            Probe::Unknown(reason)
        }
        (Some(_), Probe::Unavailable(reason), _) | (Some(_), _, Probe::Unavailable(reason)) => {
            Probe::Unavailable(reason)
        }
        (None, _, _) => Probe::Unknown("outpost HEAD is detached".to_owned()),
    };
    reporter.step(
        StepKind::Analysis,
        &format_probe_ahead_behind(&source_vs_upstream),
    );
    reporter.step(StepKind::Analysis, "comparing source and upstream default");
    let source_vs_upstream_default = match (branch.as_ref(), upstream_default_branch.as_ref()) {
        (Some(branch), Probe::Known(default)) => probe_source_vs_remote_ref(
            source,
            branch,
            &remote_branch_ref(&default.remote, &default.branch),
        ),
        (Some(_), Probe::Unknown(reason)) => Probe::Unknown(reason),
        (Some(_), Probe::Unavailable(reason)) => Probe::Unavailable(reason),
        (None, _) => Probe::Unknown("outpost HEAD is detached".to_owned()),
    };
    reporter.step(
        StepKind::Analysis,
        &format_probe_ahead_behind(&source_vs_upstream_default),
    );
    reporter.step(StepKind::Analysis, "checking source push hazard");
    let source_push_hazard = match branch.as_ref() {
        Some(branch) => probe_source_push_hazard(source, branch),
        None => Probe::Unknown("outpost HEAD is detached".to_owned()),
    };
    reporter.step(
        StepKind::Analysis,
        &format_probe_push_hazard(&source_push_hazard),
    );
    reporter.step(StepKind::Analysis, "checking safe branch deletion proof");
    let delete_analysis = branch_analysis::analyze_branch_cleanup(source, &outpost, provider);
    let safe_delete = branch_delete_safety(&delete_analysis);
    reporter.step(StepKind::Analysis, &format_safe_delete(&safe_delete));

    Ok(AnalyzeReport {
        outpost_path: outpost.work_tree().to_path_buf(),
        source_path: source.work_tree().to_path_buf(),
        locked: entry.locked,
        lock_reason: entry.lock_reason,
        branch,
        outpost_dirty,
        upstream_remote,
        outpost_vs_source,
        source_vs_upstream,
        source_vs_upstream_default,
        upstream_default_branch,
        upstream_branch,
        source_push_hazard,
        safe_delete,
        safe_delete_findings: delete_analysis.findings,
    })
}

struct SilentReporter;

impl Reporter for SilentReporter {
    fn step(&mut self, _kind: StepKind, _message: &str) {}

    fn warn(&mut self, _message: &str) {}
}

fn current_branch_optional(outpost: &Outpost) -> OutpostResult<Option<BranchName>> {
    match outpost.current_branch() {
        Ok(branch) => Ok(Some(branch)),
        Err(OutpostError::BranchNotFound { branch, .. }) if branch == "HEAD" => Ok(None),
        Err(err) => Err(err),
    }
}

fn probe_ahead_behind_outpost_source(outpost: &Outpost) -> Probe<AheadBehind> {
    match outpost.ahead_behind_source() {
        Ok(value) => Probe::Known(value),
        Err(OutpostError::BranchNotFound { branch, .. }) if branch == "HEAD" => {
            Probe::Unknown("outpost HEAD is detached".to_owned())
        }
        Err(OutpostError::NoUpstreamTracking { .. }) => {
            Probe::Unknown("outpost has no upstream tracking branch".to_owned())
        }
        Err(OutpostError::UpstreamNotABranch { .. }) => {
            Probe::Unknown("outpost upstream is not a branch".to_owned())
        }
        Err(err) => Probe::Unavailable(err.to_string()),
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
                short_oid(&identity.oid)
            )
        }
        Probe::Unknown(reason) => format!("unknown: {reason}"),
        Probe::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

fn format_probe_upstream_remote(value: &Probe<UpstreamRemote>) -> String {
    match value {
        Probe::Known(upstream) => format!("{} {}", upstream.remote.as_str(), upstream.url),
        Probe::Unknown(reason) => format!("unknown: {reason}"),
        Probe::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

fn format_probe_push_hazard(value: &Probe<SourcePushHazard>) -> String {
    match value {
        Probe::Known(hazard) if hazard.push_would_fail => "yes".to_owned(),
        Probe::Known(_) => "no".to_owned(),
        Probe::Unknown(reason) => format!("unknown: {reason}"),
        Probe::Unavailable(reason) => format!("unavailable: {reason}"),
    }
}

fn format_safe_delete(value: &BranchDeleteSafety) -> String {
    match value {
        BranchDeleteSafety::Yes(candidate) => {
            format!("yes: {}", candidate.branch.as_str())
        }
        BranchDeleteSafety::No { branch, reason } => {
            let branch = branch
                .as_ref()
                .map(|branch| format!("{}: ", branch.as_str()))
                .unwrap_or_default();
            format!("no: {branch}{}", branch_cleanup_reason_text(*reason))
        }
        BranchDeleteSafety::Unknown(reason) => format!("unknown: {reason}"),
    }
}

fn branch_cleanup_reason_text(reason: BranchCleanupSkipReason) -> &'static str {
    match reason {
        BranchCleanupSkipReason::CleanupDisabled => "cleanup disabled",
        BranchCleanupSkipReason::NonInteractive => "non-interactive",
        BranchCleanupSkipReason::MissingOutpost => "outpost path was already missing",
        BranchCleanupSkipReason::DetachedHead => "outpost HEAD is detached",
        BranchCleanupSkipReason::NoUpstreamTracking => "outpost has no upstream tracking branch",
        BranchCleanupSkipReason::UpstreamRemoteMismatch => "outpost upstream remote mismatch",
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

fn short_oid(oid: &str) -> &str {
    oid.get(..12).unwrap_or(oid)
}

fn probe_source_upstream(source: &SourceRepo, branch: &BranchName) -> Probe<SourceUpstreamBranch> {
    match source.upstream_for(branch) {
        Ok(Some(upstream)) => {
            let Some(upstream_branch) = upstream.short_branch() else {
                return Probe::Unknown("source upstream is not a branch".to_owned());
            };
            match BranchName::parse(upstream_branch.to_owned()) {
                Ok(branch) => Probe::Known(SourceUpstreamBranch {
                    remote: upstream.remote,
                    branch,
                }),
                Err(err) => Probe::Unavailable(err.to_string()),
            }
        }
        Ok(None) => Probe::Known(SourceUpstreamBranch {
            remote: origin_remote(),
            branch: branch.clone(),
        }),
        Err(err) => Probe::Unavailable(err.to_string()),
    }
}

fn probe_upstream_remote(source: &SourceRepo, remote: &RemoteName) -> Probe<UpstreamRemote> {
    match source.remote_url(remote) {
        Ok(url) => Probe::Known(UpstreamRemote {
            remote: remote.clone(),
            url,
        }),
        Err(err) => Probe::Unavailable(err.to_string()),
    }
}

fn probe_default_branch(source: &SourceRepo, remote: &RemoteName) -> Probe<RemoteBranchIdentity> {
    match source.fetch_remote_default_branch(remote) {
        Ok(Some((branch, oid))) => Probe::Known(RemoteBranchIdentity {
            remote: remote.clone(),
            branch,
            oid,
        }),
        Ok(None) => Probe::Unknown(format!("{} default branch is unknown", remote.as_str())),
        Err(err) => Probe::Unavailable(err.to_string()),
    }
}

fn probe_remote_branch(
    source: &SourceRepo,
    remote: &RemoteName,
    branch: &BranchName,
) -> Probe<RemoteBranchIdentity> {
    match source.remote_branch_oid(remote, branch) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Probe::Unknown(format!(
                "{}/{} is missing",
                remote.as_str(),
                branch.as_str()
            ));
        }
        Err(err) => return Probe::Unavailable(err.to_string()),
    }

    let remote_tracking_ref = remote_branch_ref(remote, branch);
    let fetch_refspec = format!("+{}:{remote_tracking_ref}", source_branch_ref(branch));
    if let Err(err) = source
        .git()
        .run_check(["fetch", remote.as_str(), &fetch_refspec])
    {
        return Probe::Unavailable(err.to_string());
    }

    match rev_parse(source.git(), &remote_tracking_ref) {
        Ok(oid) => Probe::Known(RemoteBranchIdentity {
            remote: remote.clone(),
            branch: branch.clone(),
            oid,
        }),
        Err(err) => Probe::Unavailable(err.to_string()),
    }
}

fn probe_source_vs_remote_ref(
    source: &SourceRepo,
    branch: &BranchName,
    remote_ref: &str,
) -> Probe<AheadBehind> {
    match source.branch_exists(branch) {
        Ok(true) => {}
        Ok(false) => return Probe::Unknown("source branch is missing".to_owned()),
        Err(err) => return Probe::Unavailable(err.to_string()),
    }
    match ref_exists(source.git(), remote_ref) {
        Ok(true) => {}
        Ok(false) => return Probe::Unknown(format!("{remote_ref} is missing")),
        Err(err) => return Probe::Unavailable(err.to_string()),
    }

    let local_ref = source_branch_ref(branch);
    match ahead_behind_existing_refs(source.git(), &local_ref, remote_ref) {
        Ok(value) => Probe::Known(value),
        Err(err) => Probe::Unavailable(err.to_string()),
    }
}

fn probe_source_push_hazard(source: &SourceRepo, branch: &BranchName) -> Probe<SourcePushHazard> {
    match source.branch_exists(branch) {
        Ok(true) => {}
        Ok(false) => return Probe::Unknown("source branch is missing".to_owned()),
        Err(err) => return Probe::Unavailable(err.to_string()),
    }

    let checked_out = match source.is_branch_checked_out(branch) {
        Ok(value) => value,
        Err(err) => return Probe::Unavailable(err.to_string()),
    };
    let update_instead = match read_optional_config(source.git(), "receive.denyCurrentBranch") {
        Ok(value) => value.as_deref() == Some("updateInstead"),
        Err(err) => return Probe::Unavailable(err.to_string()),
    };

    Probe::Known(SourcePushHazard {
        checked_out,
        push_would_fail: checked_out && !update_instead,
    })
}

fn branch_delete_safety(analysis: &branch_analysis::BranchCleanupAnalysis) -> BranchDeleteSafety {
    if let Some(candidate) = &analysis.candidate {
        return BranchDeleteSafety::Yes(candidate.clone());
    }

    analysis
        .findings
        .iter()
        .rev()
        .find_map(|finding| match finding {
            BranchCleanupFinding::Skipped { branch, reason } => Some(BranchDeleteSafety::No {
                branch: branch.clone(),
                reason: *reason,
            }),
            BranchCleanupFinding::Warning { .. } => None,
        })
        .unwrap_or_else(|| {
            BranchDeleteSafety::Unknown(
                "branch cleanup analysis did not produce a proof or skip reason".to_owned(),
            )
        })
}

fn ahead_behind_existing_refs(
    git: &GitInvoker,
    local_ref: &str,
    remote_ref: &str,
) -> OutpostResult<AheadBehind> {
    let range = format!("{local_ref}...{remote_ref}");
    let output = git.run_capture(["rev-list", "--left-right", "--count", &range])?;
    parse_ahead_behind(git.cwd(), &output)
}

fn ref_exists(git: &GitInvoker, ref_name: &str) -> OutpostResult<bool> {
    git.run_status(["rev-parse", "--verify", "--quiet", ref_name])
}

fn rev_parse(git: &GitInvoker, reference: &str) -> OutpostResult<String> {
    git.run_capture(["rev-parse", reference])
}

fn parse_ahead_behind(repo: &Path, output: &str) -> OutpostResult<AheadBehind> {
    let mut parts = output.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_ahead_behind_output(repo, output))?;
    let behind = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_ahead_behind_output(repo, output))?;
    if parts.next().is_some() {
        return Err(invalid_ahead_behind_output(repo, output));
    }

    Ok(AheadBehind { ahead, behind })
}

fn invalid_ahead_behind_output(repo: &Path, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: repo.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected rev-list output: {output}"),
        ),
    }
}

fn source_branch_ref(branch: &BranchName) -> String {
    format!("refs/heads/{}", branch.as_str())
}

fn remote_branch_ref(remote: &RemoteName, branch: &BranchName) -> String {
    format!("refs/remotes/{}/{}", remote.as_str(), branch.as_str())
}

fn origin_remote() -> RemoteName {
    RemoteName::parse("origin").expect("origin is a valid remote name")
}
