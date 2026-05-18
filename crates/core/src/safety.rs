use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{
    BranchName, GitInvoker, Outpost, OutpostError, OutpostResult, SourceRepo, UpstreamRef,
};

const FORCE_HINT: &str = "pass --force";

pub fn check_clean(work_tree: &Path, git: &GitInvoker) -> OutpostResult<()> {
    if git
        .run_capture(["status", "--porcelain=v1", "--untracked-files=normal"])?
        .is_empty()
    {
        Ok(())
    } else {
        Err(OutpostError::DirtyTree {
            repo: work_tree.to_path_buf(),
            hint: FORCE_HINT,
        })
    }
}

pub fn check_no_unpushed(outpost: &Outpost, source: &SourceRepo) -> OutpostResult<()> {
    let count = outpost.unpushed_commits(source)?;
    if count == 0 {
        Ok(())
    } else {
        Err(OutpostError::UnpushedCommits {
            repo: outpost.work_tree().to_path_buf(),
            branch: outpost.current_branch()?.as_str().to_owned(),
            hint: FORCE_HINT,
        })
    }
}

pub fn check_no_divergence(
    outpost: &Outpost,
    local_branch: &BranchName,
    upstream: &UpstreamRef,
) -> OutpostResult<()> {
    check_no_divergence_with_fetch(outpost, local_branch, upstream, true)
}

pub fn check_no_divergence_after_fetch(
    outpost: &Outpost,
    local_branch: &BranchName,
    upstream: &UpstreamRef,
) -> OutpostResult<()> {
    check_no_divergence_with_fetch(outpost, local_branch, upstream, false)
}

pub fn check_path_is_managed_outpost_of(
    source: &SourceRepo,
    candidate: &Path,
) -> OutpostResult<Outpost> {
    let candidate = canonicalize_path(candidate)?;
    let outpost = source
        .outpost_at(&candidate)
        .map_err(|_| OutpostError::RegistryEntryNotManaged(candidate.clone()))?;
    let candidate_source = outpost
        .source_repo()
        .map_err(|_| OutpostError::RegistryEntryNotManaged(candidate.clone()))?;

    if candidate_source.work_tree() == source.work_tree() {
        Ok(outpost)
    } else {
        Err(OutpostError::RegistryEntryNotManaged(candidate))
    }
}

pub fn check_destination_clean(parent: &Path, dest: &Path) -> OutpostResult<()> {
    let dest_path = resolve_destination(parent, dest)?;
    if dest_path.exists() {
        let metadata = fs::metadata(&dest_path).map_err(|source| OutpostError::IoAt {
            path: dest_path.clone(),
            source,
        })?;
        if !metadata.is_dir() || has_entries(&dest_path)? {
            return Err(OutpostError::DestinationExists(dest.to_path_buf()));
        }
    }

    if let Some(repo) = containing_repo(parent)? {
        if dest_path.starts_with(&repo) && dest_path != repo {
            return Err(OutpostError::DestinationInsideRepo(dest.to_path_buf()));
        }
    }

    Ok(())
}

fn containing_repo(parent: &Path) -> OutpostResult<Option<PathBuf>> {
    let git = GitInvoker::at(parent);
    match git.run_capture(["rev-parse", "--show-toplevel"]) {
        Ok(repo) => canonicalize_path(Path::new(&repo)).map(Some),
        Err(OutpostError::GitFailed { .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

fn has_entries(path: &Path) -> OutpostResult<bool> {
    let mut entries = fs::read_dir(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    entries
        .next()
        .transpose()
        .map(|entry| entry.is_some())
        .map_err(|source| OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        })
}

fn canonicalize_path(path: &Path) -> OutpostResult<PathBuf> {
    fs::canonicalize(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

fn check_no_divergence_with_fetch(
    outpost: &Outpost,
    local_branch: &BranchName,
    upstream: &UpstreamRef,
    fetch: bool,
) -> OutpostResult<()> {
    let remote_branch =
        upstream
            .short_branch()
            .ok_or_else(|| OutpostError::UpstreamNotABranch {
                merge_ref: upstream.merge_ref.as_str().to_owned(),
            })?;
    if !upstream_branch_exists(outpost, upstream)? {
        return Err(OutpostError::BranchNotFound {
            branch: remote_branch.to_owned(),
            repo: outpost.work_tree().to_path_buf(),
        });
    }
    if fetch {
        outpost
            .git()
            .run_check(["fetch", upstream.remote.as_str()])?;
    }

    let remote_tracking_ref = format!(
        "refs/remotes/{}/{}",
        upstream.remote.as_str(),
        remote_branch
    );
    if !outpost
        .git()
        .run_status(["rev-parse", "--verify", "--quiet", &remote_tracking_ref])?
    {
        return Err(OutpostError::BranchNotFound {
            branch: remote_branch.to_owned(),
            repo: outpost.work_tree().to_path_buf(),
        });
    }

    let local_ref = format!("refs/heads/{}", local_branch.as_str());
    let range = format!("{local_ref}...{remote_tracking_ref}");
    let output = outpost
        .git()
        .run_capture(["rev-list", "--left-right", "--count", &range])?;
    let mut parts = output.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_rev_list_output(outpost.work_tree(), &output))?;
    let behind = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_rev_list_output(outpost.work_tree(), &output))?;
    if parts.next().is_some() {
        return Err(invalid_rev_list_output(outpost.work_tree(), &output));
    }

    if ahead > 0 && behind > 0 {
        Err(OutpostError::Divergence {
            branch: local_branch.as_str().to_owned(),
        })
    } else {
        Ok(())
    }
}

fn upstream_branch_exists(outpost: &Outpost, upstream: &UpstreamRef) -> OutpostResult<bool> {
    Ok(!outpost
        .git()
        .run_capture([
            "ls-remote",
            upstream.remote.as_str(),
            upstream.merge_ref.as_str(),
        ])?
        .is_empty())
}

fn invalid_rev_list_output(repo: &Path, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: repo.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected rev-list output: {output}"),
        ),
    }
}

fn resolve_destination(parent: &Path, dest: &Path) -> OutpostResult<PathBuf> {
    let anchored = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        let parent = canonicalize_path(parent)?;
        parent.join(dest)
    };

    if anchored.exists() {
        canonicalize_path(&anchored)
    } else {
        Ok(normalize_existing_or_missing(&anchored))
    }
}

fn normalize_existing_or_missing(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::{Metadata, RemoteName};

    #[test]
    fn check_clean_reports_staged_changes_as_dirty() {
        let temp = init_repo();
        let git = GitInvoker::at(temp.path());
        fs::write(temp.path().join("file.txt"), "changed").expect("write file");
        git.run_check(["add", "file.txt"]).expect("stage file");

        assert_dirty(check_clean(temp.path(), &git), temp.path());
    }

    #[test]
    fn check_clean_reports_unstaged_changes_as_dirty() {
        let temp = init_repo();
        let git = GitInvoker::at(temp.path());
        fs::write(temp.path().join("file.txt"), "changed").expect("write file");

        assert_dirty(check_clean(temp.path(), &git), temp.path());
    }

    #[test]
    fn check_clean_reports_untracked_changes_as_dirty() {
        let temp = init_repo();
        let git = GitInvoker::at(temp.path());
        fs::write(temp.path().join("untracked.txt"), "new").expect("write untracked");

        assert_dirty(check_clean(temp.path(), &git), temp.path());
    }

    #[test]
    fn check_clean_allows_clean_work_tree() {
        let temp = init_repo();
        let git = GitInvoker::at(temp.path());

        check_clean(temp.path(), &git).expect("clean repo");
    }

    #[test]
    fn managed_outpost_gate_rejects_path_with_no_git_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("source");
        let candidate = temp.path().join("candidate");
        init_repo_at(&source_path);
        fs::create_dir_all(&candidate).expect("candidate dir");
        let source = SourceRepo::at(&source_path).expect("source repo");

        let Err(err) = check_path_is_managed_outpost_of(&source, &candidate) else {
            panic!("unmanaged path should fail");
        };

        assert!(
            matches!(err, OutpostError::RegistryEntryNotManaged(path) if path == fs::canonicalize(&candidate).unwrap())
        );
    }

    #[test]
    fn managed_outpost_gate_rejects_managed_false() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("source");
        let candidate = temp.path().join("candidate");
        init_repo_at(&source_path);
        init_repo_at(&candidate);
        GitInvoker::at(&candidate)
            .run_check(["config", "--local", "outpost.managed", "false"])
            .expect("write managed false");
        let source = SourceRepo::at(&source_path).expect("source repo");

        let Err(err) = check_path_is_managed_outpost_of(&source, &candidate) else {
            panic!("managed false should fail");
        };

        assert!(
            matches!(err, OutpostError::RegistryEntryNotManaged(path) if path == fs::canonicalize(&candidate).unwrap())
        );
    }

    #[test]
    fn managed_outpost_gate_rejects_different_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("source");
        let other_source_path = temp.path().join("other-source");
        let candidate = temp.path().join("candidate");
        init_repo_at(&source_path);
        init_repo_at(&other_source_path);
        init_repo_at(&candidate);
        Metadata {
            source_repo: other_source_path.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        }
        .write(&GitInvoker::at(&candidate))
        .expect("metadata write");
        let source = SourceRepo::at(&source_path).expect("source repo");

        let Err(err) = check_path_is_managed_outpost_of(&source, &candidate) else {
            panic!("different source should fail");
        };

        assert!(
            matches!(err, OutpostError::RegistryEntryNotManaged(path) if path == fs::canonicalize(&candidate).unwrap())
        );
    }

    #[test]
    fn managed_outpost_gate_accepts_matching_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("source");
        let candidate = temp.path().join("candidate");
        init_repo_at(&source_path);
        init_repo_at(&candidate);
        Metadata {
            source_repo: source_path.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        }
        .write(&GitInvoker::at(&candidate))
        .expect("metadata write");
        let source = SourceRepo::at(&source_path).expect("source repo");

        let outpost =
            check_path_is_managed_outpost_of(&source, &candidate).expect("matching source outpost");

        assert_eq!(outpost.work_tree(), fs::canonicalize(&candidate).unwrap());
    }

    #[test]
    fn destination_clean_rejects_existing_file_and_non_empty_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("file");
        let dir = temp.path().join("dir");
        fs::write(&file, "file").expect("write file");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("child"), "child").expect("write child");

        assert!(matches!(
            check_destination_clean(temp.path(), &file),
            Err(OutpostError::DestinationExists(path)) if path == file
        ));
        assert!(matches!(
            check_destination_clean(temp.path(), &dir),
            Err(OutpostError::DestinationExists(path)) if path == dir
        ));
    }

    #[test]
    fn destination_clean_allows_missing_and_empty_dir_outside_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");
        let empty = temp.path().join("empty");
        fs::create_dir_all(&empty).expect("empty dir");

        check_destination_clean(temp.path(), &missing).expect("missing destination");
        check_destination_clean(temp.path(), &empty).expect("empty destination");
    }

    #[test]
    fn destination_clean_rejects_target_inside_existing_repo() {
        let temp = init_repo();
        let dest = temp.path().join("nested").join("outpost");

        assert!(matches!(
            check_destination_clean(temp.path(), &dest),
            Err(OutpostError::DestinationInsideRepo(path)) if path == dest
        ));
    }

    #[test]
    fn destination_clean_allows_relative_sibling_outside_repo() {
        let temp = init_repo();

        check_destination_clean(temp.path(), Path::new("../outpost"))
            .expect("sibling destination outside repo");
    }

    #[test]
    fn destination_clean_resolves_relative_path_under_parent_before_exists_check() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd_dest = tempfile::tempdir_in(".").expect("cwd dest dir");
        let dest = PathBuf::from(cwd_dest.path().file_name().expect("cwd dest file name"));
        let parent = temp.path().join("parent");
        fs::create_dir_all(parent.join(&dest)).expect("dest dir");
        fs::write(parent.join(&dest).join("child"), "child").expect("child");

        let result = check_destination_clean(&parent, &dest);

        assert!(cwd_dest.path().exists());
        assert!(matches!(
            result,
            Err(OutpostError::DestinationExists(path)) if path == dest
        ));
    }

    #[test]
    fn check_no_unpushed_reports_unpushed_commits() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo_at(&source);
        init_repo_at(&outpost);
        let source_git = GitInvoker::at(&source);
        source_git
            .run_check(["commit", "--allow-empty", "-m", "source"])
            .expect("source commit");
        let outpost_git = GitInvoker::at(&outpost);
        outpost_git
            .run_check(["pull", &source.to_string_lossy(), "main"])
            .expect("pull source into outpost");
        outpost_git
            .run_check(["remote", "add", "local", &source.to_string_lossy()])
            .expect("add source remote");
        outpost_git
            .run_check(["fetch", "local", "main"])
            .expect("fetch source remote");
        outpost_git
            .run_check(["branch", "--set-upstream-to", "local/main", "main"])
            .expect("set upstream");
        Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        }
        .write(&outpost_git)
        .expect("metadata write");
        outpost_git
            .run_check(["commit", "--allow-empty", "-m", "outpost"])
            .expect("outpost commit");
        let source = SourceRepo::at(&source).expect("source repo");
        let outpost = Outpost::at(&outpost).expect("outpost");

        let err = check_no_unpushed(&outpost, &source).expect_err("unpushed should fail");

        assert!(matches!(
            err,
            OutpostError::UnpushedCommits { repo, branch, hint }
                if repo == outpost.work_tree() && branch == "main" && hint == FORCE_HINT
        ));
    }

    #[test]
    fn check_no_divergence_reports_missing_remote_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo_at(&source);
        init_repo_at(&outpost);
        GitInvoker::at(&source)
            .run_check(["commit", "--allow-empty", "-m", "source"])
            .expect("source commit");
        let outpost_git = GitInvoker::at(&outpost);
        outpost_git
            .run_check(["pull", &source.to_string_lossy(), "main"])
            .expect("pull source into outpost");
        outpost_git
            .run_check(["remote", "add", "local", &source.to_string_lossy()])
            .expect("add source remote");
        Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        }
        .write(&outpost_git)
        .expect("metadata write");
        let outpost = Outpost::at(&outpost).expect("outpost");
        let branch = BranchName::parse("main").unwrap();
        let upstream = UpstreamRef {
            remote: RemoteName::parse("local").unwrap(),
            merge_ref: crate::RefName::parse("refs/heads/missing").unwrap(),
        };

        let err = check_no_divergence_after_fetch(&outpost, &branch, &upstream)
            .expect_err("missing remote branch should fail");

        assert!(
            matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == "missing" && repo == outpost.work_tree())
        );
    }

    #[test]
    fn check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo_at(&source);
        init_repo_at(&outpost);
        let source_git = GitInvoker::at(&source);
        source_git
            .run_check(["commit", "--allow-empty", "-m", "source"])
            .expect("source commit");
        source_git
            .run_check(["branch", "feature"])
            .expect("feature branch");
        let outpost_git = GitInvoker::at(&outpost);
        outpost_git
            .run_check(["remote", "add", "local", &source.to_string_lossy()])
            .expect("add source remote");
        outpost_git
            .run_check(["fetch", "local", "feature:refs/remotes/local/feature"])
            .expect("fetch feature branch");
        outpost_git
            .run_check([
                "switch",
                "--create",
                "feature",
                "refs/remotes/local/feature",
            ])
            .expect("switch feature");
        Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").unwrap(),
        }
        .write(&outpost_git)
        .expect("metadata write");
        source_git
            .run_check(["branch", "-D", "feature"])
            .expect("delete source feature");
        assert!(outpost_git
            .run_status([
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/remotes/local/feature"
            ])
            .expect("stale remote-tracking ref check"));
        let outpost = Outpost::at(&outpost).expect("outpost");
        let branch = BranchName::parse("feature").unwrap();
        let upstream = UpstreamRef {
            remote: RemoteName::parse("local").unwrap(),
            merge_ref: crate::RefName::parse("refs/heads/feature").unwrap(),
        };

        let err = check_no_divergence(&outpost, &branch, &upstream)
            .expect_err("deleted branch should not pass because of stale tracking ref");

        assert!(
            matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == "feature" && repo == outpost.work_tree())
        );
    }

    fn assert_dirty(result: OutpostResult<()>, repo: &Path) {
        assert!(matches!(
            result,
            Err(OutpostError::DirtyTree { repo: dirty_repo, hint })
                if dirty_repo == repo && hint == FORCE_HINT
        ));
    }

    fn init_repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo_at(temp.path());
        temp
    }

    fn init_repo_at(path: &Path) {
        fs::create_dir_all(path).expect("repo dir");
        let git = GitInvoker::at(path);
        git.run_check(["init", "--initial-branch=main"])
            .expect("init repo");
        git.run_check(["config", "user.name", "Test Author"])
            .expect("set user.name");
        git.run_check(["config", "user.email", "test@example.com"])
            .expect("set user.email");
    }
}
