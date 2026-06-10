use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::Command;

use outpost_core::ops::remove::{BranchCleanupProvider, MergedPullRequest};
use outpost_core::{BranchName, OutpostError, OutpostResult, SourceRepo};
use serde::Deserialize;

const PR_FIELDS: &str = "number,url,headRefName,headRefOid,mergedAt";

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
