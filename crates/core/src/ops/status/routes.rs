use crate::source_repo::read_optional_config;
use crate::{BranchName, GitInvoker, OutpostError, OutpostResult, RefName, RemoteName};

use super::{RemoteRoutes, RemoteUrlList, RouteAvailability, TrackedUpstream};

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
    let result = match direction {
        RouteDirection::Fetch => git.run_capture(["remote", "get-url", "--all", remote.as_str()]),
        RouteDirection::Push => {
            git.run_capture(["remote", "get-url", "--push", "--all", remote.as_str()])
        }
    };

    match result {
        Ok(output) => RemoteUrlList::from_output(git, &output).map(RouteAvailability::Known),
        Err(OutpostError::GitFailed { code: 2, .. }) => Ok(RouteAvailability::Unavailable),
        Err(error) => Err(error),
    }
}
