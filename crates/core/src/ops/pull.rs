use crate::safety;
use crate::{Outpost, OutpostError, OutpostResult, RefName, Reporter, StepKind, UpstreamRef};

pub struct PullOptions;

pub struct PullReport {
    pub source_updated: bool,
    pub outpost_updated: bool,
}

pub fn run(
    outpost: &Outpost,
    _opts: PullOptions,
    reporter: &mut dyn Reporter,
) -> OutpostResult<PullReport> {
    let branch = outpost.current_branch().map_err(|err| match err {
        OutpostError::BranchNotFound { .. } => OutpostError::NoUpstreamTracking {
            branch: "HEAD".to_owned(),
        },
        other => other,
    })?;
    let source = outpost.source_repo()?;
    if !source.branch_exists(&branch)? {
        return Err(OutpostError::BranchNotFound {
            branch: branch.as_str().to_owned(),
            repo: source.work_tree().to_path_buf(),
        });
    }

    reporter.step(
        StepKind::SourceFetch,
        &format!(
            "fast-forwarding source {} branch {} from origin/{}",
            source.work_tree().display(),
            branch.as_str(),
            branch.as_str()
        ),
    );
    let source_updated = source.fast_forward_branch_from_origin(&branch)?;

    let upstream = UpstreamRef {
        remote: outpost.metadata().remote_name.clone(),
        merge_ref: RefName::parse(format!("refs/heads/{}", branch.as_str()))?,
    };
    safety::check_no_divergence(outpost, &branch, &upstream)?;

    reporter.step(
        StepKind::OutpostFetch,
        &format!(
            "fast-forwarding outpost {} branch {} from {}/{}",
            outpost.work_tree().display(),
            branch.as_str(),
            upstream.remote.as_str(),
            branch.as_str()
        ),
    );
    let before = outpost.git().run_capture(["rev-parse", "HEAD"])?;
    outpost.git().run_check([
        "pull",
        "--ff-only",
        upstream.remote.as_str(),
        branch.as_str(),
    ])?;
    let after = outpost.git().run_capture(["rev-parse", "HEAD"])?;

    Ok(PullReport {
        source_updated,
        outpost_updated: before != after,
    })
}
