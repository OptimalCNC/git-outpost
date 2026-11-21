use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::source_repo::{canonicalize_path, read_optional_config};
use crate::{
    BranchName, GitInvoker, Outpost, OutpostError, OutpostResult, Reporter, SourceRepo, StepKind,
};

pub struct PushOptions;

pub struct PushReport {
    pub outpost_to_source: StepResult,
    pub source_to_origin: StepResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    Pushed { commits: u32 },
}

pub fn run(
    outpost: &Outpost,
    _opts: PushOptions,
    reporter: &mut dyn Reporter,
) -> OutpostResult<PushReport> {
    let branch = outpost.current_branch().map_err(|err| match err {
        OutpostError::BranchNotFound { .. } => OutpostError::NoUpstreamTracking {
            branch: "HEAD".to_owned(),
        },
        other => other,
    })?;
    let source = outpost.source_repo()?;
    let remote = &outpost.metadata().remote_name;

    check_checked_out_source_policy(&source, &branch)?;
    if !source.branch_exists(&branch)? {
        return Err(OutpostError::AmbiguousBranchCreation {
            branch: branch.as_str().to_owned(),
        });
    }

    verify_source_route(outpost, &source)?;
    let origin_fetch = verify_origin_destinations(&source)?;
    let tracking_ref = format!("refs/remotes/{}/{}", remote.as_str(), branch.as_str());
    check_source_fast_forwardable(outpost, &branch, &tracking_ref)?;
    let origin_before = source.origin_branch_oid(&branch)?;
    check_origin_fast_forwardable(outpost, &origin_fetch, &branch, origin_before.as_deref())?;

    let branch_ref = source_branch_ref(branch.as_str());
    let intended_tip = outpost.git().run_capture(["rev-parse", &branch_ref])?;
    let source_before = source.git().run_capture(["rev-parse", &branch_ref])?;
    reporter.step(
        StepKind::OutpostPush,
        &format!(
            "pushing outpost {} branch {} -> source {}",
            outpost.work_tree().display(),
            branch.as_str(),
            source.work_tree().display()
        ),
    );
    verify_source_route(outpost, &source)?;
    let refspec = branch_refspec(branch.as_str());
    outpost
        .git()
        .run_check(["push", "--", remote.as_str(), &refspec])?;
    let source_after = source.git().run_capture(["rev-parse", &branch_ref])?;
    if source_after != intended_tip {
        return Err(OutpostError::Divergence {
            branch: branch.as_str().to_owned(),
        });
    }

    let source_to_origin_commits =
        source_to_origin_commit_count(source.git(), origin_before.as_deref(), &source_after)?;
    reporter.step(
        StepKind::SourcePush,
        &format!(
            "pushing source {} branch {} -> origin/{}",
            source.work_tree().display(),
            branch.as_str(),
            branch.as_str()
        ),
    );
    verify_origin_destinations(&source)?;
    source
        .git()
        .run_check(["push", "--set-upstream", "origin", &refspec])?;
    let origin_after =
        source
            .origin_branch_oid(&branch)?
            .ok_or_else(|| OutpostError::BranchNotFound {
                branch: branch.as_str().to_owned(),
                repo: source.work_tree().to_path_buf(),
            })?;
    if origin_after != source_after {
        return Err(OutpostError::Divergence {
            branch: branch.as_str().to_owned(),
        });
    }

    Ok(PushReport {
        outpost_to_source: StepResult::Pushed {
            commits: pushed_commit_count(outpost, &source_before, &source_after)?,
        },
        source_to_origin: StepResult::Pushed {
            commits: source_to_origin_commits,
        },
    })
}

fn verify_source_route(outpost: &Outpost, source: &SourceRepo) -> OutpostResult<()> {
    let remote = &outpost.metadata().remote_name;
    if let Some(actual) =
        super::status::mismatched_source_route(outpost.git(), remote, source.work_tree())?
    {
        return Err(OutpostError::SourceRemoteMismatch {
            remote: remote.as_str().to_owned(),
            expected: source.work_tree().to_path_buf(),
            actual,
        });
    }
    Ok(())
}

fn verify_origin_destinations(source: &SourceRepo) -> OutpostResult<OsString> {
    let origin = crate::RemoteName::parse("origin")?;
    let read_url = source.remote_url(&origin)?;
    let Some(fetch_path) = local_transport_path(source.git(), &read_url)? else {
        return Ok(OsString::from(read_url));
    };
    let push_urls = super::status::push_urls(source.git(), &origin)?;
    let mut fetch_repository = None;
    for url in push_urls.as_slice().iter().filter(|url| **url != read_url) {
        let Some(push_path) = local_transport_path(source.git(), url)? else {
            continue;
        };
        let fetch = match &mut fetch_repository {
            Some(path) => path,
            empty => empty.insert(local_repository(source.git(), &fetch_path)?),
        };
        let push = local_repository(source.git(), &push_path)?;
        if *fetch != push
            && !same_file::is_same_file(&*fetch, &push).map_err(|source| OutpostError::IoAt {
                path: push.clone(),
                source,
            })?
        {
            return Err(OutpostError::UpstreamRepositoryMismatch {
                remote: "origin".to_owned(),
                fetch: fetch.clone(),
                push,
            });
        }
    }
    if read_url.starts_with("file://") || Path::new(&read_url).is_absolute() {
        Ok(OsString::from(read_url))
    } else {
        Ok(crate::path::git_path_arg(&fetch_path))
    }
}

fn local_transport_path(git: &GitInvoker, url: &str) -> OutpostResult<Option<PathBuf>> {
    if let Some(location) = url.strip_prefix("file://") {
        let bytes: Vec<u8> = percent_encoding::percent_decode_str(location).collect();
        let missing = || invalid_local_route(git.cwd(), "file URL has no usable repository path");
        if bytes.contains(&0) {
            return Err(missing());
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let start = bytes
                .iter()
                .position(|byte| *byte == b'/')
                .ok_or_else(missing)?;
            return Ok(Some(PathBuf::from(OsStr::from_bytes(&bytes[start..]))));
        }
        #[cfg(windows)]
        {
            let location = std::str::from_utf8(&bytes).map_err(|_| missing())?;
            if let Some(drive_path) = location
                .strip_prefix('/')
                .filter(|path| Path::new(path).has_root() && !path.starts_with('/'))
            {
                return Ok(Some(PathBuf::from(drive_path)));
            }
            if Path::new(location).has_root() {
                return Ok(Some(PathBuf::from(location)));
            }
            if !location.contains('/') {
                return Err(missing());
            }
            return Ok(Some(PathBuf::from(format!("//{location}"))));
        }
    }
    // Git treats a colon before the first slash as SCP syntax, except for DOS drives.
    let path = Path::new(url);
    let first = url.split('/').next().unwrap_or_default();
    if !matches!(
        path.components().next(),
        Some(std::path::Component::Prefix(_))
    ) && first.contains(':')
    {
        return Ok(None);
    }
    if url.starts_with('~') {
        // Ask Git to expand ~ and ~user with the same home directory as the transport.
        let setting = format!("outpost.localpath={url}");
        let expanded = git.run_capture_lines([
            "-c",
            &setting,
            "config",
            "--path",
            "--get",
            "outpost.localpath",
        ])?;
        let [path] = expanded.as_slice() else {
            return Err(invalid_local_route(
                git.cwd(),
                "cannot expand local origin path",
            ));
        };
        return Ok(Some(git.cwd().join(path)));
    }
    Ok(Some(git.cwd().join(path)))
}

fn local_repository(git: &GitInvoker, path: &Path) -> OutpostResult<PathBuf> {
    // Git's file transport also accepts a worktree, a gitfile, or a bare
    // repository whose trailing .git was omitted. Never discover a parent repo.
    // A transport child does not inherit the calling checkout's common directory.
    let git = git.clone().without_env("GIT_COMMON_DIR");
    let mut suffixed = path.components().collect::<PathBuf>().into_os_string();
    suffixed.push(".git");
    let suffixed = PathBuf::from(suffixed);
    for candidate in [
        path.join(".git"),
        path.to_owned(),
        suffixed.join(".git"),
        suffixed,
    ] {
        let file = candidate.is_file();
        if !file && !candidate.is_dir() {
            continue;
        }
        let candidate = crate::path::git_path_arg(&candidate);
        let common = match git.run_capture_lines([
            OsStr::new("--git-dir"),
            candidate.as_os_str(),
            OsStr::new("rev-parse"),
            OsStr::new("--path-format=absolute"),
            OsStr::new("--git-common-dir"),
        ]) {
            Ok(lines) => lines,
            // A directory can have an unrelated name like "foo" next to foo.git.
            // Git still selects an existing file as a gitfile, including an invalid one.
            Err(OutpostError::GitFailed { code: 128, .. }) if !file => continue,
            Err(error) => return Err(error),
        };
        if let [directory] = common.as_slice() {
            return canonicalize_path(Path::new(directory));
        }
        break;
    }
    Err(invalid_local_route(
        path,
        "cannot identify the repository for this local origin URL",
    ))
}

fn invalid_local_route(path: &Path, message: &str) -> OutpostError {
    OutpostError::IoAt {
        path: path.to_owned(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    }
}

fn check_checked_out_source_policy(source: &SourceRepo, branch: &BranchName) -> OutpostResult<()> {
    if read_optional_config(source.git(), "receive.denyCurrentBranch")?.as_deref()
        == Some("updateInstead")
    {
        return Ok(());
    }

    if source
        .checked_out_branches()?
        .iter()
        .any(|checked_out| checked_out == branch)
    {
        Err(OutpostError::PushIntoCheckedOutBranch {
            r#source: source.work_tree().to_path_buf(),
            branch: branch.as_str().to_owned(),
        })
    } else {
        Ok(())
    }
}

fn check_source_fast_forwardable(
    outpost: &Outpost,
    branch: &BranchName,
    remote_tracking_ref: &str,
) -> OutpostResult<()> {
    let remote = outpost.metadata().remote_name.as_str();
    let fetch_refspec = format!("{}:{remote_tracking_ref}", branch.as_str());
    outpost.git().run_check(["fetch", remote, &fetch_refspec])?;

    let local_ref = source_branch_ref(branch.as_str());
    if outpost
        .git()
        .ahead_behind(&local_ref, remote_tracking_ref)?
        .behind
        > 0
    {
        Err(OutpostError::Divergence {
            branch: branch.as_str().to_owned(),
        })
    } else {
        Ok(())
    }
}

fn check_origin_fast_forwardable(
    outpost: &Outpost,
    origin_fetch: &OsStr,
    branch: &BranchName,
    origin_oid: Option<&str>,
) -> OutpostResult<()> {
    let Some(origin_oid) = origin_oid else {
        return Ok(());
    };

    outpost.git().run_check([
        OsStr::new("fetch"),
        OsStr::new("--no-write-fetch-head"),
        OsStr::new("--"),
        origin_fetch,
        OsStr::new(&source_branch_ref(branch.as_str())),
    ])?;

    if outpost
        .git()
        .run_status(["merge-base", "--is-ancestor", origin_oid, "HEAD"])?
    {
        Ok(())
    } else {
        Err(OutpostError::Divergence {
            branch: branch.as_str().to_owned(),
        })
    }
}

fn pushed_commit_count(outpost: &Outpost, before: &str, after: &str) -> OutpostResult<u32> {
    if before == after {
        return Ok(0);
    }

    let range = format!("{before}..{after}");
    let output = outpost.git().run_capture(["rev-list", "--count", &range])?;
    parse_count(outpost.git(), &output)
}

fn source_to_origin_commit_count(
    git: &GitInvoker,
    before: Option<&str>,
    after: &str,
) -> OutpostResult<u32> {
    let Some(before) = before else {
        return parse_count(
            git,
            &git.run_capture(["rev-list", "--count", after, "--not", "--remotes=origin"])?,
        );
    };
    if before == after {
        return Ok(0);
    }

    let range = format!("{before}..{after}");
    parse_count(git, &git.run_capture(["rev-list", "--count", &range])?)
}

fn parse_count(git: &GitInvoker, output: &str) -> OutpostResult<u32> {
    let count = output
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_count_output(git, output))?;
    if output.split_whitespace().nth(1).is_some() {
        return Err(invalid_count_output(git, output));
    }
    Ok(count)
}

fn invalid_count_output(git: &GitInvoker, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: git.cwd().to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected rev-list output: {output}"),
        ),
    }
}

fn branch_refspec(branch: &str) -> String {
    format!("{branch}:{branch}")
}

fn source_branch_ref(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_fetch_and_ssh_push_addresses_remain_network_routes() {
        let repo = tempfile::tempdir().expect("temporary source repository");
        let git = GitInvoker::at(repo.path());
        git.run_check(["init", "--quiet"])
            .expect("initialize source");
        let source = SourceRepo::at(repo.path()).expect("open source");
        let https = "https://example.invalid/team/repo.git";
        let ssh = "git@example.invalid:team/repo.git";
        for (fetch, push) in [(https, ssh), (ssh, https)] {
            git.run_check(["config", "remote.origin.url", fetch])
                .expect("set fetch address");
            git.run_check(["config", "remote.origin.pushurl", push])
                .expect("set push address");
            assert_eq!(
                verify_origin_destinations(&source).expect("compatible network addresses"),
                OsStr::new(fetch)
            );
        }
    }
}
