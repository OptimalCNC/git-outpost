use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::source_repo::{canonicalize_path, read_optional_config, source_branch_ref};
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

    let branch_ref = source_branch_ref(&branch);
    let intended_tip = outpost.git().ref_oid(&branch_ref)?;
    let source_before = source.git().ref_oid(&branch_ref)?;
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
    // Git can match the explicit name refs/heads/X to two branches: X and
    // a branch literally named refs/heads/X. Patterns use full ref names.
    // Every ref starts with "refs/", so the exclusion leaves only refs/heads/X.
    let pattern = format!("refs*/heads/{branch}");
    let refspec = format!("{pattern}:{pattern}");
    let exclude_nested = format!("^refs/*/heads/{branch}");
    outpost
        .git()
        .run_check(["push", "--", remote.as_str(), &refspec, &exclude_nested])?;
    let source_after = source.git().ref_oid(&branch_ref)?;
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
    source.git().run_check([
        "push",
        "--set-upstream",
        "origin",
        &refspec,
        &exclude_nested,
    ])?;
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
        let expanded = git.run_capture_path([
            "-c",
            &setting,
            "config",
            "--path",
            "--get",
            "outpost.localpath",
        ])?;
        return Ok(Some(git.cwd().join(expanded)));
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
        match git.run_capture_path([
            OsStr::new("--git-dir"),
            candidate.as_os_str(),
            OsStr::new("rev-parse"),
            OsStr::new("--path-format=absolute"),
            OsStr::new("--git-common-dir"),
        ]) {
            Ok(directory) => return canonicalize_path(&directory),
            // A directory can have an unrelated name like "foo" next to foo.git.
            // Git still selects an existing file as a gitfile, including an invalid one.
            Err(OutpostError::GitFailed { code: 128, .. }) if !file => continue,
            Err(error) => return Err(error),
        }
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
    let local_ref = source_branch_ref(branch);
    let fetch_refspec = format!("{local_ref}:{remote_tracking_ref}");
    outpost.git().run_check(["fetch", remote, &fetch_refspec])?;

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
        OsStr::new(&source_branch_ref(branch)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_accepts_network_routes_and_a_local_read_mirror() {
        let fixture = tempfile::tempdir().expect("temporary repositories");
        let git = GitInvoker::at(fixture.path());
        git.run_check(["init", "--quiet", "source"])
            .expect("initialize source");
        git.run_check(["init", "--quiet", "--bare", "mirror.git"])
            .expect("initialize local read mirror");
        let source = SourceRepo::at(fixture.path().join("source")).expect("open source");
        let mirror = crate::path::git_path_arg(&fixture.path().join("mirror.git"));
        let https = OsStr::new("https://example.invalid/team/repo.git");
        let ssh = OsStr::new("git@example.invalid:team/repo.git");
        for (fetch, push) in [(https, ssh), (ssh, https), (mirror.as_os_str(), ssh)] {
            source
                .git()
                .run_check(["config".as_ref(), "remote.origin.url".as_ref(), fetch])
                .expect("set fetch address");
            source
                .git()
                .run_check(["config".as_ref(), "remote.origin.pushurl".as_ref(), push])
                .expect("set push address");
            assert_eq!(
                verify_origin_destinations(&source).expect("compatible Git addresses"),
                fetch
            );
        }
    }
}
