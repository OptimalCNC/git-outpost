use std::path::{Path, PathBuf};

use crate::source_repo::read_optional_config;
use crate::{BranchName, GitInvoker, OutpostError, OutpostResult, RefName, RemoteName};

use super::{RemoteRoutes, RemoteUrlList, RouteAvailability, TrackedUpstream};

pub(crate) fn mismatched_source_route(
    git: &GitInvoker,
    remote: &RemoteName,
    source: &Path,
) -> OutpostResult<Option<PathBuf>> {
    for direction in [RouteDirection::Fetch, RouteDirection::Push] {
        for url in remote_urls(git, remote, direction)?.as_slice() {
            let path = super::canonicalize_remote_path(git.cwd(), url)?;
            if path != source {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

pub(super) fn read_upstream(
    git: &GitInvoker,
    branch: &BranchName,
) -> OutpostResult<Option<TrackedUpstream>> {
    let remote_key = format!("branch.{}.remote", branch.as_str());
    let merge_key = format!("branch.{}.merge", branch.as_str());
    let Some(remote) = read_optional_config(git, &remote_key)? else {
        return Ok(None);
    };
    let Some(merge_ref) = read_optional_config(git, &merge_key)? else {
        return Ok(None);
    };
    let merge_ref = RefName::parse(merge_ref)?;
    let Some(target_branch) = merge_ref.as_str().strip_prefix("refs/heads/") else {
        return Ok(None);
    };
    let target_branch = BranchName::parse(target_branch)?;
    let remote = RemoteName::parse(remote)?;

    if remote.as_str() == "." {
        return Ok(Some(TrackedUpstream::LocalRepository {
            branch: target_branch,
        }));
    }

    Ok(Some(TrackedUpstream::Remote {
        routes: RemoteRoutes {
            fetch: probe_urls(git, &remote, RouteDirection::Fetch)?,
            push: probe_urls(git, &remote, RouteDirection::Push)?,
        },
        remote,
        branch: target_branch,
    }))
}

#[derive(Clone, Copy)]
pub(super) enum RouteDirection {
    Fetch,
    Push,
}

pub(super) fn probe_urls(
    git: &GitInvoker,
    remote: &RemoteName,
    direction: RouteDirection,
) -> OutpostResult<RouteAvailability> {
    match remote_urls(git, remote, direction) {
        Ok(urls) => Ok(RouteAvailability::Known(urls)),
        Err(OutpostError::GitFailed { code: 2, .. }) => Ok(RouteAvailability::Unavailable),
        Err(error) => Err(error),
    }
}

pub(in crate::ops) fn push_urls(
    git: &GitInvoker,
    remote: &RemoteName,
) -> OutpostResult<RemoteUrlList> {
    remote_urls(git, remote, RouteDirection::Push)
}

fn remote_urls(
    git: &GitInvoker,
    remote: &RemoteName,
    direction: RouteDirection,
) -> OutpostResult<RemoteUrlList> {
    let output = match direction {
        RouteDirection::Fetch => {
            git.run_capture_lines(["remote", "get-url", "--all", remote.as_str()])
        }
        RouteDirection::Push => {
            git.run_capture_lines(["remote", "get-url", "--push", "--all", remote.as_str()])
        }
    }?;
    // get-url separates addresses with LF, which may also occur inside a config value.
    // Only a multiline result needs this check. Git applies pushurl before the url fallback.
    if output.len() > 1 {
        let push_key = format!("remote.{}.pushurl", remote.as_str());
        let key = if matches!(direction, RouteDirection::Push)
            && git.run_status(["config", "--get-all", &push_key])?
        {
            push_key
        } else {
            format!("remote.{}.url", remote.as_str())
        };
        if git.run_status(["config", "--get-all", &key, "\n"])? {
            return Err(OutpostError::IoAt {
                path: git.cwd().to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{key} contains an address with an embedded newline"),
                ),
            });
        }
    }
    RemoteUrlList::from_output(git, output)
}
