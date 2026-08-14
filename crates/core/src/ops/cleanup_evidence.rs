use std::io;
use std::path::PathBuf;

use crate::{BranchName, OutpostError, OutpostResult, RemoteName, SourceRepo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvidenceRequest {
    pub upstream_remote: RemoteName,
    pub upstream_url: String,
    pub branch: BranchName,
    pub source_oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedRemoteBranch {
    pub branch: BranchName,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedPullRequest {
    pub id: String,
    pub head_ref_name: BranchName,
    pub head_ref_oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvidenceSnapshot {
    pub default_branch: Option<ObservedRemoteBranch>,
    pub upstream_oid: Option<String>,
    pub merged_pull_request: Option<MergedPullRequest>,
}

pub trait CleanupEvidenceProvider {
    fn snapshot(
        &self,
        request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>>;
}

pub(crate) struct GitCleanupEvidenceProvider<'a> {
    source: &'a SourceRepo,
}

impl<'a> GitCleanupEvidenceProvider<'a> {
    pub(crate) fn new(source: &'a SourceRepo) -> Self {
        Self { source }
    }
}

impl CleanupEvidenceProvider for GitCleanupEvidenceProvider<'_> {
    fn snapshot(
        &self,
        request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        let candidate_ref = format!("refs/heads/{}", request.branch.as_str());
        let output = self.source.git().run_capture([
            "ls-remote",
            "--symref",
            request.upstream_remote.as_str(),
            "HEAD",
            &candidate_ref,
        ])?;
        parse_ls_remote_snapshot(&output, &request.branch).map(Some)
    }
}

fn parse_ls_remote_snapshot(
    output: &str,
    branch: &BranchName,
) -> OutpostResult<CleanupEvidenceSnapshot> {
    let candidate_ref = format!("refs/heads/{}", branch.as_str());
    let mut default_ref = None;
    let mut head_oid = None;
    let mut upstream_oid = None;

    for line in output.lines() {
        if let Some(symref) = line.strip_prefix("ref: ") {
            let mut fields = symref.split_whitespace();
            let reference = fields
                .next()
                .ok_or_else(|| invalid_ls_remote_output(output, "malformed symbolic ref"))?;
            let target = fields
                .next()
                .ok_or_else(|| invalid_ls_remote_output(output, "malformed symbolic ref"))?;
            if fields.next().is_some() {
                return Err(invalid_ls_remote_output(output, "malformed symbolic ref"));
            }
            if target == "HEAD" {
                let name = reference.strip_prefix("refs/heads/").ok_or_else(|| {
                    invalid_ls_remote_output(output, "default branch is not under refs/heads")
                })?;
                set_once(
                    &mut default_ref,
                    BranchName::parse(name.to_owned())?,
                    output,
                    "duplicate default branch",
                )?;
            }
            continue;
        }

        let mut fields = line.split_whitespace();
        let oid = fields
            .next()
            .ok_or_else(|| invalid_ls_remote_output(output, "malformed object line"))?;
        let reference = fields
            .next()
            .ok_or_else(|| invalid_ls_remote_output(output, "malformed object line"))?;
        if fields.next().is_some() || !valid_oid(oid) {
            return Err(invalid_ls_remote_output(output, "malformed object line"));
        }
        if reference == "HEAD" {
            set_once(
                &mut head_oid,
                oid.to_owned(),
                output,
                "duplicate default branch OID",
            )?;
        } else if reference == candidate_ref {
            set_once(
                &mut upstream_oid,
                oid.to_owned(),
                output,
                "duplicate candidate branch OID",
            )?;
        }
    }

    let default_branch = match (default_ref, head_oid) {
        (Some(branch), Some(oid)) => Some(ObservedRemoteBranch { branch, oid }),
        (None, None) => None,
        _ => {
            return Err(invalid_ls_remote_output(
                output,
                "incomplete default branch identity",
            ));
        }
    };

    Ok(CleanupEvidenceSnapshot {
        default_branch,
        upstream_oid,
        merged_pull_request: None,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, output: &str, message: &str) -> OutpostResult<()> {
    if slot.is_some() {
        return Err(invalid_ls_remote_output(output, message));
    }
    *slot = Some(value);
    Ok(())
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn invalid_ls_remote_output(output: &str, message: &str) -> OutpostError {
    OutpostError::IoAt {
        path: PathBuf::from("git ls-remote"),
        source: io::Error::new(io::ErrorKind::InvalidData, format!("{message}: {output}")),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;
    use crate::{BranchName, RemoteName, SourceRepo};

    #[test]
    fn parses_default_and_candidate_from_one_ls_remote_response() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let output = concat!(
            "ref: refs/heads/main\tHEAD\n",
            "1111111111111111111111111111111111111111\tHEAD\n",
            "2222222222222222222222222222222222222222\trefs/heads/feat\n",
        );

        let snapshot = parse_ls_remote_snapshot(output, &branch).expect("snapshot");

        assert_eq!(
            snapshot.default_branch,
            Some(ObservedRemoteBranch {
                branch: BranchName::parse("main".to_owned()).expect("default branch"),
                oid: "1111111111111111111111111111111111111111".to_owned(),
            })
        );
        assert_eq!(
            snapshot.upstream_oid.as_deref(),
            Some("2222222222222222222222222222222222222222")
        );
        assert_eq!(snapshot.merged_pull_request, None);
    }

    #[test]
    fn parses_missing_candidate_without_losing_default_identity() {
        let branch = BranchName::parse("absent".to_owned()).expect("branch");
        let output = concat!(
            "ref: refs/heads/main\tHEAD\n",
            "1111111111111111111111111111111111111111\tHEAD\n",
        );

        let snapshot = parse_ls_remote_snapshot(output, &branch).expect("snapshot");

        assert_eq!(
            snapshot
                .default_branch
                .as_ref()
                .map(|default| (default.branch.as_str(), default.oid.as_str())),
            Some(("main", "1111111111111111111111111111111111111111"))
        );
        assert_eq!(snapshot.upstream_oid, None);
    }

    #[test]
    fn rejects_head_oid_without_default_symref() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let output = concat!(
            "1111111111111111111111111111111111111111\tHEAD\n",
            "2222222222222222222222222222222222222222\trefs/heads/feat\n",
        );

        let err = parse_ls_remote_snapshot(output, &branch)
            .expect_err("HEAD without its symbolic target is malformed");

        assert!(err.to_string().contains("default branch"));
    }

    #[test]
    fn rejects_default_symref_without_head_oid() {
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let output = concat!(
            "ref: refs/heads/main\tHEAD\n",
            "2222222222222222222222222222222222222222\trefs/heads/feat\n",
        );

        let err = parse_ls_remote_snapshot(output, &branch)
            .expect_err("default symbolic target without its OID is malformed");

        assert!(err.to_string().contains("default branch"));
    }

    #[test]
    fn generic_adapter_observes_default_and_candidate_in_one_command() {
        let (_temp, source, upstream) = test_repository();
        let branch = BranchName::parse("feat".to_owned()).expect("branch");
        let request = CleanupEvidenceRequest {
            upstream_remote: RemoteName::parse("origin").expect("remote"),
            upstream_url: upstream.to_string_lossy().into_owned(),
            branch: branch.clone(),
            source_oid: source
                .branch_oid(&branch)
                .expect("source branch query")
                .expect("source branch"),
        };
        let adapter = GitCleanupEvidenceProvider::new(&source);

        let snapshot = adapter
            .snapshot(&request)
            .expect("generic evidence")
            .expect("generic adapter handles Git remotes");

        assert_eq!(
            snapshot
                .default_branch
                .as_ref()
                .map(|default| default.branch.as_str()),
            Some("main")
        );
        assert_eq!(
            snapshot.upstream_oid.as_deref(),
            Some(request.source_oid.as_str())
        );
        let remote_calls = source
            .git()
            .argv_log()
            .into_iter()
            .filter(|argv| argv.first() == Some(&OsString::from("ls-remote")))
            .collect::<Vec<_>>();
        assert_eq!(
            remote_calls,
            vec![vec![
                OsString::from("ls-remote"),
                OsString::from("--symref"),
                OsString::from("origin"),
                OsString::from("HEAD"),
                OsString::from("refs/heads/feat"),
            ]]
        );
    }

    #[test]
    fn source_repo_detects_commit_objects_without_network_access() {
        let (_temp, source, _upstream) = test_repository();
        let branch = BranchName::parse("main".to_owned()).expect("branch");
        let oid = source
            .branch_oid(&branch)
            .expect("branch query")
            .expect("branch");

        assert!(source.has_commit_oid(&oid).expect("existing commit query"));
        assert!(
            !source
                .has_commit_oid("0000000000000000000000000000000000000000")
                .expect("missing commit query")
        );
        assert!(
            source
                .git()
                .argv_log()
                .iter()
                .all(|argv| argv.first() != Some(&OsString::from("fetch")))
        );
    }

    #[test]
    fn source_repo_fetches_multiple_remote_branches_in_one_deduplicated_command() {
        let (_temp, source, _upstream) = test_repository();
        let remote = RemoteName::parse("origin").expect("remote");
        let feat = BranchName::parse("feat".to_owned()).expect("feat");
        let main = BranchName::parse("main".to_owned()).expect("main");

        source
            .fetch_remote_branches(&remote, &[feat.clone(), main.clone(), feat])
            .expect("fetch branches");

        let fetch_calls = source
            .git()
            .argv_log()
            .into_iter()
            .filter(|argv| argv.first() == Some(&OsString::from("fetch")))
            .collect::<Vec<_>>();
        assert_eq!(
            fetch_calls,
            vec![vec![
                OsString::from("fetch"),
                OsString::from("origin"),
                OsString::from("+refs/heads/feat:refs/remotes/origin/feat"),
                OsString::from("+refs/heads/main:refs/remotes/origin/main"),
            ]]
        );
    }

    fn test_repository() -> (tempfile::TempDir, SourceRepo, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let upstream = temp.path().join("upstream.git");
        let source_path = temp.path().join("source");
        run_git(
            temp.path(),
            ["init", "--bare", "--initial-branch=main", path(&upstream)],
        );
        run_git(temp.path(), ["clone", path(&upstream), path(&source_path)]);
        run_git(&source_path, ["config", "user.name", "Test User"]);
        run_git(
            &source_path,
            ["config", "user.email", "test@example.invalid"],
        );
        run_git(&source_path, ["commit", "--allow-empty", "-m", "initial"]);
        run_git(&source_path, ["push", "origin", "main"]);
        run_git(&source_path, ["branch", "feat"]);
        run_git(&source_path, ["push", "origin", "feat"]);
        let source = SourceRepo::at(&source_path).expect("source repo");
        (temp, source, upstream)
    }

    fn path(path: &Path) -> &str {
        path.to_str().expect("utf-8 test path")
    }

    fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
