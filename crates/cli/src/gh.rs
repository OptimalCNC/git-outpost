use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::Command;

use outpost_core::ops::analyze::Probe;
use outpost_core::ops::cleanup_evidence::{
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot, MergedPullRequest,
    ObservedRemoteBranch,
};
use outpost_core::{BranchName, OutpostError, OutpostResult, SourceRepo};
use serde::Deserialize;

const ANALYZE_PR_FIELDS: &str =
    "number,url,state,isDraft,baseRefName,headRefName,headRefOid,reviewDecision,statusCheckRollup";
const CLEANUP_GRAPHQL_QUERY: &str = r#"
query CleanupEvidence(
  $owner: String!
  $name: String!
  $candidateRef: String!
  $head: String!
  $search: String!
) {
  repository(owner: $owner, name: $name) {
    nameWithOwner
    defaultBranchRef { name target { oid } }
    candidate: ref(qualifiedName: $candidateRef) { target { oid } }
    byHead: pullRequests(
      first: 100
      states: [OPEN, CLOSED, MERGED]
      headRefName: $head
      orderBy: { field: UPDATED_AT, direction: DESC }
    ) {
      nodes { ...PullRequestEvidence }
    }
  }
  bySha: search(query: $search, type: ISSUE, first: 100) {
    nodes {
      __typename
      ... on PullRequest { ...PullRequestEvidence }
    }
  }
}

fragment PullRequestEvidence on PullRequest {
  __typename
  repository { nameWithOwner }
  number
  url
  state
  isDraft
  baseRefName
  headRefName
  headRefOid
  mergedAt
  reviewDecision
  statusCheckRollup: commits(last: 1) {
    nodes {
      commit {
        statusCheckRollup {
          contexts(first: 100) {
            nodes {
              __typename
              ... on CheckRun { status conclusion }
              ... on StatusContext { state }
            }
          }
        }
      }
    }
  }
}
"#;

pub struct GhProbe {
    program: OsString,
    cwd: PathBuf,
    env: BTreeMap<OsString, OsString>,
    state: RefCell<GhProbeState>,
}

#[derive(Clone)]
enum GhProbeState {
    Unchecked,
    Success {
        request: CleanupEvidenceRequest,
        bundle: GithubEvidenceBundle,
    },
    Unsupported {
        request: CleanupEvidenceRequest,
    },
    Failed {
        request: CleanupEvidenceRequest,
        kind: GhFailureKind,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GhFailureKind {
    NotInstalled,
    Unavailable,
}

struct GhFailure {
    kind: GhFailureKind,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubRepository {
    host: String,
    owner: String,
    name: String,
}

#[derive(Clone)]
struct GithubEvidenceBundle {
    snapshot: CleanupEvidenceSnapshot,
    pull_requests: Vec<PullRequestSummary>,
}

pub struct GhStatus {
    probe: GhProbe,
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
        Self {
            probe: GhProbe::new(source, program),
        }
    }

    pub fn provider(&self) -> Option<&dyn CleanupEvidenceProvider> {
        Some(&self.probe)
    }

    pub fn analyze(&self, branch: Option<&BranchName>) -> GithubAnalysis {
        self.probe.analyze(branch)
    }

    pub fn is_not_installed(&self) -> bool {
        self.probe.failure().map(|failure| failure.kind) == Some(GhFailureKind::NotInstalled)
    }

    pub fn unavailable_message(&self) -> Option<String> {
        self.probe
            .failure()
            .filter(|failure| failure.kind == GhFailureKind::Unavailable)
            .map(|failure| failure.message)
    }

    #[cfg(test)]
    pub fn not_installed_for_tests() -> Self {
        Self::failure_for_tests(GhFailureKind::NotInstalled, "gh not found")
    }

    #[cfg(test)]
    pub fn unavailable_for_tests(message: &str) -> Self {
        Self::failure_for_tests(GhFailureKind::Unavailable, message)
    }

    #[cfg(test)]
    fn failure_for_tests(kind: GhFailureKind, message: &str) -> Self {
        let request = CleanupEvidenceRequest {
            upstream_remote: outpost_core::RemoteName::parse("origin").expect("test remote"),
            upstream_url: "git@github.com:example/repository.git".to_owned(),
            branch: BranchName::parse("test").expect("test branch"),
            source_oid: "0000000000000000000000000000000000000000".to_owned(),
        };
        Self {
            probe: GhProbe {
                program: OsString::from("gh"),
                cwd: PathBuf::new(),
                env: BTreeMap::new(),
                state: RefCell::new(GhProbeState::Failed {
                    request,
                    kind,
                    message: message.to_owned(),
                }),
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
            state: RefCell::new(GhProbeState::Unchecked),
        }
    }

    fn failure(&self) -> Option<GhFailure> {
        match &*self.state.borrow() {
            GhProbeState::Failed { kind, message, .. } => Some(GhFailure {
                kind: *kind,
                message: message.clone(),
            }),
            GhProbeState::Unchecked
            | GhProbeState::Success { .. }
            | GhProbeState::Unsupported { .. } => None,
        }
    }

    fn analyze(&self, branch: Option<&BranchName>) -> GithubAnalysis {
        let Some(branch) = branch else {
            return GithubAnalysis {
                availability: GithubAvailability::Available,
                pull_requests: Probe::Unknown("branch is unknown".to_owned()),
            };
        };

        match self.state.borrow().clone() {
            GhProbeState::Success { request, bundle } if request.branch == *branch => {
                return GithubAnalysis {
                    availability: GithubAvailability::Available,
                    pull_requests: Probe::Known(bundle.pull_requests),
                };
            }
            GhProbeState::Failed { message, .. } => {
                return GithubAnalysis {
                    availability: GithubAvailability::Unavailable(message.clone()),
                    pull_requests: Probe::Unavailable(message),
                };
            }
            GhProbeState::Unsupported { .. } => {
                let message = "upstream remote is not GitHub".to_owned();
                return GithubAnalysis {
                    availability: GithubAvailability::Unavailable(message.clone()),
                    pull_requests: Probe::Unavailable(message),
                };
            }
            GhProbeState::Unchecked | GhProbeState::Success { .. } => {}
        }

        match self.pull_requests(branch) {
            Ok(pull_requests) => GithubAnalysis {
                availability: GithubAvailability::Available,
                pull_requests: Probe::Known(pull_requests),
            },
            Err(err) => GithubAnalysis {
                availability: GithubAvailability::Unavailable(err.to_string()),
                pull_requests: Probe::Unavailable(err.to_string()),
            },
        }
    }

    fn graphql_snapshot(
        &self,
        request: &CleanupEvidenceRequest,
        repository: &GithubRepository,
    ) -> Result<GithubEvidenceBundle, GhFailure> {
        let candidate_ref = format!("refs/heads/{}", request.branch.as_str());
        let search = format!(
            "repo:{}/{} is:pr {}",
            repository.owner, repository.name, request.source_oid
        );
        let output = Command::new(&self.program)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .args(["api", "graphql", "--hostname", &repository.host])
            .args(["-f", &format!("query={CLEANUP_GRAPHQL_QUERY}")])
            .args(["-F", &format!("owner={}", repository.owner)])
            .args(["-F", &format!("name={}", repository.name)])
            .args(["-F", &format!("candidateRef={candidate_ref}")])
            .args(["-F", &format!("head={}", request.branch.as_str())])
            .args(["-F", &format!("search={search}")])
            .output()
            .map_err(|err| GhFailure {
                kind: if err.kind() == ErrorKind::NotFound {
                    GhFailureKind::NotInstalled
                } else {
                    GhFailureKind::Unavailable
                },
                message: if err.kind() == ErrorKind::NotFound {
                    "gh not found".to_owned()
                } else {
                    format!("gh api graphql failed: {err}")
                },
            })?;

        if !output.status.success() {
            return Err(GhFailure {
                kind: GhFailureKind::Unavailable,
                message: format!(
                    "gh api graphql failed with status {:?}: {}",
                    output.status.code(),
                    command_stderr(&output.stderr)
                ),
            });
        }

        parse_graphql_response(&output.stdout, request, repository).map_err(|err| GhFailure {
            kind: GhFailureKind::Unavailable,
            message: err.to_string(),
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

impl CleanupEvidenceProvider for GhProbe {
    fn snapshot(
        &self,
        request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        match self.state.borrow().clone() {
            GhProbeState::Success {
                request: cached_request,
                bundle,
            } if cached_request == *request => return Ok(Some(bundle.snapshot)),
            GhProbeState::Unsupported {
                request: cached_request,
            } if cached_request == *request => return Ok(None),
            GhProbeState::Failed {
                request: cached_request,
                message,
                ..
            } if cached_request == *request => {
                return Err(cached_gh_error(&self.cwd, &message));
            }
            GhProbeState::Unchecked
            | GhProbeState::Success { .. }
            | GhProbeState::Unsupported { .. }
            | GhProbeState::Failed { .. } => {}
        }

        let Some(repository) = GithubRepository::parse(&request.upstream_url) else {
            self.state.replace(GhProbeState::Unsupported {
                request: request.clone(),
            });
            return Ok(None);
        };

        match self.graphql_snapshot(request, &repository) {
            Ok(bundle) => {
                let snapshot = bundle.snapshot.clone();
                self.state.replace(GhProbeState::Success {
                    request: request.clone(),
                    bundle,
                });
                Ok(Some(snapshot))
            }
            Err(failure) => {
                let err = cached_gh_error(&self.cwd, &failure.message);
                self.state.replace(GhProbeState::Failed {
                    request: request.clone(),
                    kind: failure.kind,
                    message: failure.message,
                });
                Err(err)
            }
        }
    }
}

impl GithubRepository {
    fn parse(url: &str) -> Option<Self> {
        let (raw_host, raw_path) = if let Some((_, rest)) = url.split_once("://") {
            let (authority, path) = rest.split_once('/')?;
            let host = authority.rsplit('@').next()?.split(':').next()?;
            (host, path)
        } else {
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') {
                return None;
            }
            (authority.rsplit('@').next()?, path)
        };

        let raw_host = raw_host.to_ascii_lowercase();
        let host = match raw_host.as_str() {
            "github.com" | "www.github.com" | "ssh.github.com" => "github.com".to_owned(),
            other if std::env::var("GH_HOST").ok().as_deref() == Some(other) => other.to_owned(),
            _ => return None,
        };
        let path = raw_path
            .split(['?', '#'])
            .next()?
            .trim_matches('/')
            .strip_suffix(".git")
            .unwrap_or_else(|| raw_path.split(['?', '#']).next().unwrap().trim_matches('/'));
        let mut segments = path.split('/');
        let owner = segments.next()?.trim();
        let name = segments.next()?.trim();
        if owner.is_empty() || name.is_empty() || segments.next().is_some() {
            return None;
        }

        Some(Self {
            host,
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    fn name_with_owner(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlEnvelope {
    data: Option<GraphqlData>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlData {
    repository: Option<GraphqlRepositoryData>,
    #[serde(rename = "bySha", default)]
    by_sha: GraphqlPullRequestConnection,
}

#[derive(Debug, Deserialize)]
struct GraphqlRepositoryData {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: Option<String>,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<GraphqlNamedRef>,
    candidate: Option<GraphqlRef>,
    #[serde(rename = "byHead", default)]
    by_head: GraphqlPullRequestConnection,
}

#[derive(Debug, Deserialize)]
struct GraphqlNamedRef {
    name: Option<String>,
    target: Option<GraphqlTarget>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRef {
    target: Option<GraphqlTarget>,
}

#[derive(Debug, Deserialize)]
struct GraphqlTarget {
    oid: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct GraphqlPullRequestConnection {
    #[serde(default)]
    nodes: Vec<GraphqlPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphqlPullRequest {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    repository: Option<GraphqlRepositoryIdentity>,
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
    head_ref_oid: Option<String>,
    #[serde(rename = "mergedAt")]
    merged_at: Option<String>,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRepositoryIdentity {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: Option<String>,
}

fn parse_graphql_response(
    bytes: &[u8],
    request: &CleanupEvidenceRequest,
    expected_repository: &GithubRepository,
) -> OutpostResult<GithubEvidenceBundle> {
    let envelope: GraphqlEnvelope =
        serde_json::from_slice(bytes).map_err(|source| OutpostError::IoAt {
            path: PathBuf::from("gh api graphql"),
            source: io::Error::new(io::ErrorKind::InvalidData, source),
        })?;
    let Some(data) = envelope.data else {
        let message = envelope
            .errors
            .into_iter()
            .filter_map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(invalid_graphql_response(if message.is_empty() {
            "GraphQL response has no data".to_owned()
        } else {
            format!("GraphQL response has no data: {message}")
        }));
    };
    let repository = data
        .repository
        .ok_or_else(|| invalid_graphql_response("GitHub repository was not found".to_owned()))?;
    let name_with_owner = expected_repository.name_with_owner();
    if repository.name_with_owner.as_deref() != Some(name_with_owner.as_str()) {
        return Err(invalid_graphql_response(
            "GraphQL repository identity did not match the requested remote".to_owned(),
        ));
    }

    let default_branch = match repository.default_branch_ref {
        Some(default) => {
            let name = default.name.ok_or_else(|| {
                invalid_graphql_response("default branch name is missing".to_owned())
            })?;
            let oid = graphql_oid(default.target, "default branch OID")?;
            Some(ObservedRemoteBranch {
                branch: BranchName::parse(name)?,
                oid,
            })
        }
        None => None,
    };
    let upstream_oid = match repository.candidate {
        Some(candidate) => Some(graphql_oid(candidate.target, "candidate branch OID")?),
        None => None,
    };

    let merged_pull_request = repository
        .by_head
        .nodes
        .iter()
        .chain(data.by_sha.nodes.iter())
        .find(|pr| exact_merged_pr(pr, request, &name_with_owner))
        .map(|pr| MergedPullRequest {
            id: pr
                .url
                .clone()
                .or_else(|| pr.number.map(|number| format!("#{number}")))
                .unwrap_or_else(|| "merged pull request".to_owned()),
            head_ref_name: request.branch.clone(),
            head_ref_oid: request.source_oid.clone(),
        });
    let pull_requests = repository
        .by_head
        .nodes
        .iter()
        .filter(|pr| graphql_pull_request(pr, &name_with_owner))
        .map(PullRequestSummary::try_from)
        .collect::<OutpostResult<Vec<_>>>()?;

    Ok(GithubEvidenceBundle {
        snapshot: CleanupEvidenceSnapshot {
            default_branch,
            upstream_oid,
            merged_pull_request,
        },
        pull_requests,
    })
}

fn graphql_oid(target: Option<GraphqlTarget>, field: &str) -> OutpostResult<String> {
    let oid = target
        .and_then(|target| target.oid)
        .ok_or_else(|| invalid_graphql_response(format!("{field} is missing")))?;
    if !valid_oid(&oid) {
        return Err(invalid_graphql_response(format!("{field} is invalid")));
    }
    Ok(oid)
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn graphql_pull_request(pr: &GraphqlPullRequest, expected_repository: &str) -> bool {
    pr.typename.as_deref() == Some("PullRequest")
        && pr
            .repository
            .as_ref()
            .and_then(|repository| repository.name_with_owner.as_deref())
            == Some(expected_repository)
}

fn exact_merged_pr(
    pr: &GraphqlPullRequest,
    request: &CleanupEvidenceRequest,
    expected_repository: &str,
) -> bool {
    graphql_pull_request(pr, expected_repository)
        && pr
            .merged_at
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        && pr.head_ref_name.as_deref() == Some(request.branch.as_str())
        && pr.head_ref_oid.as_deref() == Some(request.source_oid.as_str())
}

impl TryFrom<&GraphqlPullRequest> for PullRequestSummary {
    type Error = OutpostError;

    fn try_from(pr: &GraphqlPullRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            id: pr_id_from_parts(pr.number, pr.url.clone()),
            state: non_empty_or("unknown", pr.state.clone()),
            draft: pr.is_draft.unwrap_or(false),
            base: non_empty_or("unknown", pr.base_ref_name.clone()),
            head: non_empty_or("unknown", pr.head_ref_name.clone()),
            review: non_empty_or("none", pr.review_decision.clone()),
            checks: graphql_check_summary(pr.status_check_rollup.as_ref()),
        })
    }
}

fn graphql_check_summary(value: Option<&serde_json::Value>) -> String {
    let contexts = value
        .and_then(|value| value.get("nodes"))
        .and_then(|value| value.as_array())
        .and_then(|nodes| nodes.first())
        .and_then(|value| value.get("commit"))
        .and_then(|value| value.get("statusCheckRollup"))
        .and_then(|value| value.get("contexts"))
        .and_then(|value| value.get("nodes"));
    check_summary(contexts)
}

fn invalid_graphql_response(message: String) -> OutpostError {
    OutpostError::IoAt {
        path: PathBuf::from("gh api graphql"),
        source: io::Error::new(io::ErrorKind::InvalidData, message),
    }
}

fn cached_gh_error(cwd: &std::path::Path, message: &str) -> OutpostError {
    OutpostError::IoAt {
        path: cwd.to_path_buf(),
        source: io::Error::other(message.to_owned()),
    }
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
    use outpost_core::ops::cleanup_evidence::CleanupEvidenceRequest;

    #[test]
    fn graphql_snapshot_parses_remote_identities_exact_proof_and_analyze_summary() {
        let request = graphql_request();
        let repository = GithubRepository {
            host: "github.com".to_owned(),
            owner: "acme".to_owned(),
            name: "widgets".to_owned(),
        };

        let bundle = parse_graphql_response(graphql_fixture().as_bytes(), &request, &repository)
            .expect("GraphQL snapshot");

        assert_eq!(
            bundle
                .snapshot
                .default_branch
                .as_ref()
                .map(|default| (default.branch.as_str(), default.oid.as_str())),
            Some(("main", "1111111111111111111111111111111111111111"))
        );
        assert_eq!(
            bundle.snapshot.upstream_oid.as_deref(),
            Some("2222222222222222222222222222222222222222")
        );
        assert_eq!(
            bundle.snapshot.merged_pull_request.as_ref().map(|proof| (
                proof.id.as_str(),
                proof.head_ref_name.as_str(),
                proof.head_ref_oid.as_str()
            )),
            Some((
                "https://github.com/acme/widgets/pull/9",
                "feat",
                "2222222222222222222222222222222222222222",
            ))
        );
        assert_eq!(
            bundle.pull_requests,
            vec![PullRequestSummary {
                id: "#7".to_owned(),
                state: "OPEN".to_owned(),
                draft: false,
                base: "main".to_owned(),
                head: "feat".to_owned(),
                review: "none".to_owned(),
                checks: "passing".to_owned(),
            }]
        );
    }

    #[test]
    #[cfg(unix)]
    fn snapshot_then_analyze_executes_one_graphql_process_without_version_probe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let response = temp.path().join("response.json");
        let log = temp.path().join("calls.log");
        fs::write(&response, graphql_fixture()).expect("write GraphQL response");
        let mut env = BTreeMap::new();
        env.insert(
            OsString::from("GH_TEST_RESPONSE"),
            response.as_os_str().to_os_string(),
        );
        env.insert(
            OsString::from("GH_TEST_LOG"),
            log.as_os_str().to_os_string(),
        );
        let source_path = temp.path().join("source");
        fs::create_dir(&source_path).expect("source dir");
        let init = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(&source_path)
            .output()
            .expect("git init");
        assert!(init.status.success());
        let source = SourceRepo::at_with(&source_path, &env).expect("source repo");
        let program = fake_executable(
            temp.path().join("gh-one-call"),
            "#!/bin/sh\nprintf 'call\\n' >> \"$GH_TEST_LOG\"\nif [ \"$1\" != api ] || [ \"$2\" != graphql ]; then\n  echo unexpected gh invocation >&2\n  exit 9\nfi\n/bin/cat \"$GH_TEST_RESPONSE\"\n",
        );
        let status = GhStatus::detect_program(&source, program.into_os_string());
        let request = graphql_request();

        let snapshot = status
            .provider()
            .expect("provider")
            .snapshot(&request)
            .expect("snapshot")
            .expect("GitHub adapter handles remote");
        let analysis = status.analyze(Some(&request.branch));

        assert!(snapshot.merged_pull_request.is_some());
        assert_eq!(
            analysis.pull_requests,
            Probe::Known(vec![PullRequestSummary {
                id: "#7".to_owned(),
                state: "OPEN".to_owned(),
                draft: false,
                base: "main".to_owned(),
                head: "feat".to_owned(),
                review: "none".to_owned(),
                checks: "passing".to_owned(),
            }])
        );
        assert_eq!(fs::read_to_string(log).expect("call log"), "call\n");
    }

    #[test]
    fn github_remote_parser_rejects_local_and_accepts_https_ssh_and_scp_forms() {
        assert_eq!(
            GithubRepository::parse("https://github.com/acme/widgets.git"),
            Some(GithubRepository {
                host: "github.com".to_owned(),
                owner: "acme".to_owned(),
                name: "widgets".to_owned(),
            })
        );
        assert_eq!(
            GithubRepository::parse("ssh://git@github.com/acme/widgets.git"),
            GithubRepository::parse("git@github.com:acme/widgets.git")
        );
        assert_eq!(GithubRepository::parse("/tmp/upstream.git"), None);
    }

    #[test]
    fn provider_declines_non_github_remote_without_spawning_gh() {
        let (_temp, source) = test_source_repo();
        let missing = source.work_tree().join("must-not-run-gh");
        let status = GhStatus::detect_program(&source, missing.into_os_string());
        let mut request = graphql_request();
        request.upstream_url = "/tmp/upstream.git".to_owned();

        let snapshot = status
            .provider()
            .expect("lazy provider")
            .snapshot(&request)
            .expect("unsupported remotes are not provider failures");

        assert_eq!(snapshot, None);
        assert!(!status.is_not_installed());
        assert_eq!(status.unavailable_message(), None);
    }

    fn graphql_request() -> CleanupEvidenceRequest {
        CleanupEvidenceRequest {
            upstream_remote: outpost_core::RemoteName::parse("origin").expect("remote"),
            upstream_url: "git@github.com:acme/widgets.git".to_owned(),
            branch: BranchName::parse("feat".to_owned()).expect("branch"),
            source_oid: "2222222222222222222222222222222222222222".to_owned(),
        }
    }

    fn graphql_fixture() -> String {
        r#"{
  "data": {
    "repository": {
      "nameWithOwner": "acme/widgets",
      "defaultBranchRef": {
        "name": "main",
        "target": { "oid": "1111111111111111111111111111111111111111" }
      },
      "candidate": {
        "target": { "oid": "2222222222222222222222222222222222222222" }
      },
      "byHead": {
        "nodes": [
          {
            "__typename": "PullRequest",
            "repository": { "nameWithOwner": "acme/widgets" },
            "number": 7,
            "url": "https://github.com/acme/widgets/pull/7",
            "state": "OPEN",
            "isDraft": false,
            "baseRefName": "main",
            "headRefName": "feat",
            "headRefOid": "3333333333333333333333333333333333333333",
            "mergedAt": null,
            "reviewDecision": "",
            "statusCheckRollup": {
              "nodes": [
                {
                  "commit": {
                    "statusCheckRollup": {
                      "contexts": {
                        "nodes": [
                          {
                            "__typename": "CheckRun",
                            "status": "COMPLETED",
                            "conclusion": "SUCCESS"
                          }
                        ]
                      }
                    }
                  }
                }
              ]
            }
          }
        ]
      }
    },
    "bySha": {
      "nodes": [
        {
          "__typename": "PullRequest",
          "repository": { "nameWithOwner": "acme/widgets" },
          "number": 9,
          "url": "https://github.com/acme/widgets/pull/9",
          "state": "MERGED",
          "isDraft": false,
          "baseRefName": "main",
          "headRefName": "feat",
          "headRefOid": "2222222222222222222222222222222222222222",
          "mergedAt": "2026-08-14T00:00:00Z",
          "reviewDecision": "APPROVED",
          "statusCheckRollup": { "nodes": [] }
        },
        {
          "__typename": "PullRequest",
          "repository": { "nameWithOwner": "other/widgets" },
          "number": 10,
          "url": "https://github.com/other/widgets/pull/10",
          "state": "MERGED",
          "isDraft": false,
          "baseRefName": "main",
          "headRefName": "feat",
          "headRefOid": "2222222222222222222222222222222222222222",
          "mergedAt": "2026-08-14T00:00:00Z",
          "reviewDecision": "APPROVED",
          "statusCheckRollup": { "nodes": [] }
        }
      ]
    }
  }
}"#
        .to_owned()
    }

    #[test]
    fn gh_status_reports_missing_program() {
        let (_temp, source) = test_source_repo();
        let missing = source.work_tree().join("missing-gh");

        let status = GhStatus::detect_program(&source, missing.into_os_string());
        let err = status
            .provider()
            .expect("lazy provider")
            .snapshot(&graphql_request())
            .expect_err("useful GraphQL call should report missing gh");

        assert!(status.is_not_installed());
        assert!(err.to_string().contains("gh not found"));
    }

    #[test]
    #[cfg(unix)]
    fn gh_status_preserves_unavailable_graphql_failure() {
        let (_temp, source) = test_source_repo();
        let program = fake_executable(
            source.work_tree().join("gh-fails-graphql"),
            "#!/bin/sh\necho auth required >&2\nexit 7\n",
        );

        let status = GhStatus::detect_program(&source, program.into_os_string());
        status
            .provider()
            .expect("lazy provider")
            .snapshot(&graphql_request())
            .expect_err("GraphQL failure");
        let message = status
            .unavailable_message()
            .expect("unavailable diagnostic");

        assert!(
            message.contains("status Some(7)") && message.contains("auth required"),
            "unexpected unavailable message: {message}"
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
