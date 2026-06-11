use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::Command;

use outpost_core::ops::analyze::Probe;
use outpost_core::ops::branch_analysis::{BranchCleanupProvider, MergedPullRequest};
use outpost_core::{BranchName, OutpostError, OutpostResult, SourceRepo};
use serde::Deserialize;

const PR_FIELDS: &str = "number,url,headRefName,headRefOid,mergedAt";
const ANALYZE_PR_FIELDS: &str =
    "number,url,state,isDraft,baseRefName,headRefName,headRefOid,reviewDecision,statusCheckRollup";

pub struct GhProbe {
    program: OsString,
    cwd: PathBuf,
    env: BTreeMap<OsString, OsString>,
}

pub enum GhStatus {
    Available(GhProbe),
    NotInstalled,
    Unavailable { message: String },
}

pub struct GithubAnalysis {
    pub availability: GithubAvailability,
    pub pull_requests: Probe<Vec<PullRequestSummary>>,
}

pub enum GithubAvailability {
    Available,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSummary {
    pub id: String,
    pub state: String,
    pub draft: bool,
    pub base: String,
    pub head: String,
    pub review: String,
    pub checks: String,
}

impl GhStatus {
    pub fn detect(source: &SourceRepo) -> Self {
        Self::detect_program(source, OsString::from("gh"))
    }

    fn detect_program(source: &SourceRepo, program: OsString) -> Self {
        let probe = GhProbe::new(source, program);
        let output = match Command::new(&probe.program)
            .arg("--version")
            .current_dir(&probe.cwd)
            .envs(&probe.env)
            .output()
        {
            Ok(output) => output,
            Err(err) if err.kind() == ErrorKind::NotFound => return Self::NotInstalled,
            Err(err) => {
                return Self::Unavailable {
                    message: format!("gh --version failed: {err}"),
                };
            }
        };

        if output.status.success() {
            Self::Available(probe)
        } else {
            Self::Unavailable {
                message: format!(
                    "gh --version failed with status {:?}: {}",
                    output.status.code(),
                    command_stderr(&output.stderr)
                ),
            }
        }
    }

    pub fn provider(&self) -> Option<&dyn BranchCleanupProvider> {
        match self {
            Self::Available(probe) => Some(probe),
            Self::NotInstalled | Self::Unavailable { .. } => None,
        }
    }

    pub fn progress_message(&self) -> String {
        match self {
            Self::Available(_) => "available".to_owned(),
            Self::NotInstalled => "unavailable: gh not found".to_owned(),
            Self::Unavailable { message } => format!("unavailable: {message}"),
        }
    }

    pub fn analyze(&self, branch: Option<&BranchName>) -> GithubAnalysis {
        match self {
            Self::Available(probe) => {
                let pull_requests = match branch {
                    Some(branch) => match probe.pull_requests(branch) {
                        Ok(prs) => Probe::Known(prs),
                        Err(err) => Probe::Unavailable(err.to_string()),
                    },
                    None => Probe::Unknown("branch is unknown".to_owned()),
                };
                GithubAnalysis {
                    availability: GithubAvailability::Available,
                    pull_requests,
                }
            }
            Self::NotInstalled => GithubAnalysis {
                availability: GithubAvailability::Unavailable("gh not found".to_owned()),
                pull_requests: Probe::Unavailable("gh not found".to_owned()),
            },
            Self::Unavailable { message } => GithubAnalysis {
                availability: GithubAvailability::Unavailable(message.clone()),
                pull_requests: Probe::Unavailable(message.clone()),
            },
        }
    }
}

impl GithubAnalysis {
    pub fn progress_message(&self) -> String {
        match &self.pull_requests {
            Probe::Known(prs) => format!("{} pull request(s)", prs.len()),
            Probe::Unknown(reason) => format!("unknown: {reason}"),
            Probe::Unavailable(reason) => format!("unavailable: {reason}"),
        }
    }
}

impl GhProbe {
    fn new(source: &SourceRepo, program: OsString) -> Self {
        Self {
            program,
            cwd: source.work_tree().to_path_buf(),
            env: source.env().clone(),
        }
    }

    fn list_prs<I, S>(&self, args: I) -> OutpostResult<Vec<GhPullRequest>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new(&self.program)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .args(args)
            .output()
            .map_err(|source| OutpostError::IoAt {
                path: self.cwd.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(OutpostError::IoAt {
                path: self.cwd.clone(),
                source: io::Error::other(format!(
                    "gh pr list failed with status {:?}: {}",
                    output.status.code(),
                    command_stderr(&output.stderr)
                )),
            });
        }

        serde_json::from_slice(&output.stdout).map_err(|source| OutpostError::IoAt {
            path: self.cwd.clone(),
            source: io::Error::new(io::ErrorKind::InvalidData, source),
        })
    }

    fn pull_requests(&self, branch: &BranchName) -> OutpostResult<Vec<PullRequestSummary>> {
        let output = Command::new(&self.program)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .args([
                "pr",
                "list",
                "--state",
                "all",
                "--head",
                branch.as_str(),
                "--json",
                ANALYZE_PR_FIELDS,
                "--limit",
                "100",
            ])
            .output()
            .map_err(|source| OutpostError::IoAt {
                path: self.cwd.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(OutpostError::IoAt {
                path: self.cwd.clone(),
                source: io::Error::other(format!(
                    "gh pr list failed with status {:?}: {}",
                    output.status.code(),
                    command_stderr(&output.stderr)
                )),
            });
        }

        let prs: Vec<GhAnalyzePullRequest> =
            serde_json::from_slice(&output.stdout).map_err(|source| OutpostError::IoAt {
                path: self.cwd.clone(),
                source: io::Error::new(io::ErrorKind::InvalidData, source),
            })?;
        prs.into_iter().map(PullRequestSummary::try_from).collect()
    }
}

impl BranchCleanupProvider for GhProbe {
    fn merged_pull_request(
        &self,
        branch: &BranchName,
        source_oid: &str,
    ) -> OutpostResult<Option<MergedPullRequest>> {
        let by_head = self.list_prs([
            "pr",
            "list",
            "--state",
            "all",
            "--head",
            branch.as_str(),
            "--json",
            PR_FIELDS,
            "--limit",
            "100",
        ])?;
        if let Some(proof) = matching_merged_pr(by_head, branch, source_oid)? {
            return Ok(Some(proof));
        }

        let by_sha = self.list_prs([
            "pr", "list", "--state", "all", "--search", source_oid, "--json", PR_FIELDS, "--limit",
            "100",
        ])?;
        matching_merged_pr(by_sha, branch, source_oid)
    }
}

#[derive(Debug, Deserialize)]
struct GhPullRequest {
    number: Option<u64>,
    url: Option<String>,
    #[serde(rename = "headRefName")]
    head_ref_name: Option<String>,
    #[serde(rename = "headRefOid")]
    head_ref_oid: Option<String>,
    #[serde(rename = "mergedAt")]
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAnalyzePullRequest {
    number: Option<u64>,
    url: Option<String>,
    state: Option<String>,
    #[serde(rename = "isDraft")]
    is_draft: Option<bool>,
    #[serde(rename = "baseRefName")]
    base_ref_name: Option<String>,
    #[serde(rename = "headRefName")]
    head_ref_name: Option<String>,
    #[serde(rename = "headRefOid")]
    _head_ref_oid: Option<String>,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<serde_json::Value>,
}

impl TryFrom<GhAnalyzePullRequest> for PullRequestSummary {
    type Error = OutpostError;

    fn try_from(pr: GhAnalyzePullRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            id: pr_id_from_parts(pr.number, pr.url),
            state: non_empty_or("unknown", pr.state),
            draft: pr.is_draft.unwrap_or(false),
            base: non_empty_or("unknown", pr.base_ref_name),
            head: non_empty_or("unknown", pr.head_ref_name),
            review: non_empty_or("none", pr.review_decision),
            checks: check_summary(pr.status_check_rollup.as_ref()),
        })
    }
}

fn matching_merged_pr(
    prs: Vec<GhPullRequest>,
    branch: &BranchName,
    source_oid: &str,
) -> OutpostResult<Option<MergedPullRequest>> {
    for pr in prs {
        let Some(merged_at) = pr.merged_at.as_deref() else {
            continue;
        };
        if merged_at.is_empty() {
            continue;
        }
        if pr.head_ref_name.as_deref() != Some(branch.as_str()) {
            continue;
        }
        if pr.head_ref_oid.as_deref() != Some(source_oid) {
            continue;
        }

        return Ok(Some(MergedPullRequest {
            id: pr_id(&pr),
            head_ref_name: BranchName::parse(branch.as_str().to_owned())?,
            head_ref_oid: source_oid.to_owned(),
        }));
    }

    Ok(None)
}

fn pr_id(pr: &GhPullRequest) -> String {
    pr.url
        .clone()
        .or_else(|| pr.number.map(|number| format!("#{number}")))
        .unwrap_or_else(|| "merged pull request".to_owned())
}

fn pr_id_from_parts(number: Option<u64>, url: Option<String>) -> String {
    number
        .map(|number| format!("#{number}"))
        .or(url)
        .unwrap_or_else(|| "pull request".to_owned())
}

fn non_empty_or(fallback: &str, value: Option<String>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn check_summary(value: Option<&serde_json::Value>) -> String {
    let Some(serde_json::Value::Array(items)) = value else {
        return "unknown".to_owned();
    };
    if items.is_empty() {
        return "unknown".to_owned();
    }

    let mut has_pending = false;
    let mut has_success = false;
    for item in items {
        let status = item
            .get("conclusion")
            .or_else(|| item.get("status"))
            .or_else(|| item.get("state"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        match status {
            "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" => {
                return "failing".to_owned();
            }
            "PENDING" | "QUEUED" | "IN_PROGRESS" | "REQUESTED" | "WAITING" | "EXPECTED" => {
                has_pending = true;
            }
            "SUCCESS" | "SKIPPED" | "NEUTRAL" => {
                has_success = true;
            }
            _ => {}
        }
    }

    if has_pending {
        "pending".to_owned()
    } else if has_success {
        "passing".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn command_stderr(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
    if stderr.is_empty() {
        "<no stderr>".to_owned()
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use outpost_core::SourceRepo;

    #[test]
    fn matching_merged_pr_requires_branch_oid_and_merged_at() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let prs = vec![
            GhPullRequest {
                number: Some(1),
                url: None,
                head_ref_name: Some("feat".to_owned()),
                head_ref_oid: Some("wrong".to_owned()),
                merged_at: Some("2026-01-01T00:00:00Z".to_owned()),
            },
            GhPullRequest {
                number: Some(2),
                url: Some("https://example.test/pr/2".to_owned()),
                head_ref_name: Some("feat".to_owned()),
                head_ref_oid: Some("abc123".to_owned()),
                merged_at: Some("2026-01-01T00:00:00Z".to_owned()),
            },
        ];

        let proof = matching_merged_pr(prs, &branch, "abc123")
            .expect("match")
            .expect("proof");

        assert_eq!(proof.id, "https://example.test/pr/2");
        assert_eq!(proof.head_ref_name, branch);
        assert_eq!(proof.head_ref_oid, "abc123");
    }

    #[test]
    fn gh_status_reports_missing_program() {
        let (_temp, source) = test_source_repo();
        let missing = source.work_tree().join("missing-gh");

        let status = GhStatus::detect_program(&source, missing.into_os_string());

        assert!(matches!(status, GhStatus::NotInstalled));
    }

    #[test]
    #[cfg(unix)]
    fn gh_status_preserves_unavailable_version_failure() {
        let (_temp, source) = test_source_repo();
        let program = fake_executable(
            source.work_tree().join("gh-fails-version"),
            "#!/bin/sh\necho version unavailable >&2\nexit 7\n",
        );

        let status = GhStatus::detect_program(&source, program.into_os_string());

        match status {
            GhStatus::Unavailable { message } => {
                assert!(
                    message.contains("status Some(7)") && message.contains("version unavailable"),
                    "unexpected unavailable message: {message}"
                );
            }
            GhStatus::Available(_) | GhStatus::NotInstalled => {
                panic!("expected unavailable gh status")
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn provider_error_includes_pr_list_status_and_stderr() {
        let (_temp, source) = test_source_repo();
        let program = fake_executable(
            source.work_tree().join("gh-pr-list-fails"),
            "#!/bin/sh\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"list\" ]; then\n  echo auth required >&2\n  exit 3\nfi\nexit 0\n",
        );
        let probe = GhProbe::new(&source, program.into_os_string());
        let branch = BranchName::parse("feat".to_owned()).expect("branch");

        let err = probe
            .merged_pull_request(&branch, "abc123")
            .expect_err("provider should report gh failure");
        let message = err.to_string();

        assert!(
            message.contains("gh pr list failed with status Some(3)")
                && message.contains("auth required"),
            "provider failure should include status and stderr: {message}"
        );
    }

    #[test]
    fn analyze_pr_summary_normalizes_empty_review_decision() {
        let summary = PullRequestSummary::try_from(GhAnalyzePullRequest {
            number: Some(47),
            url: None,
            state: Some("OPEN".to_owned()),
            is_draft: Some(false),
            base_ref_name: Some("main".to_owned()),
            head_ref_name: Some("feat".to_owned()),
            _head_ref_oid: Some("abc123".to_owned()),
            review_decision: Some(String::new()),
            status_check_rollup: None,
        })
        .expect("summary");

        assert_eq!(summary.review, "none");
    }

    fn test_source_repo() -> (tempfile::TempDir, SourceRepo) {
        let temp = tempfile::tempdir().expect("tempdir");
        let init = Command::new("git")
            .arg("init")
            .arg("--initial-branch=main")
            .current_dir(temp.path())
            .output()
            .expect("git init");
        assert!(
            init.status.success(),
            "git init failed:\n{}",
            String::from_utf8_lossy(&init.stderr)
        );
        let source = SourceRepo::at(temp.path()).expect("source repo");
        (temp, source)
    }

    #[cfg(unix)]
    fn fake_executable(path: PathBuf, content: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        fs::write(&path, content).expect("write fake executable");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod fake executable");
        path
    }
}
