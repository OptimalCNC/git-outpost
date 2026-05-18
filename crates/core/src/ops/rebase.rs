use crate::{Outpost, OutpostError, OutpostResult, Reporter, SourceRemoteRef, StepKind};

pub struct RebaseOptions {
    pub source_ref: SourceRemoteRef,
}

pub struct RebaseReport {
    pub source_ref: SourceRemoteRef,
}

pub fn run(
    outpost: &Outpost,
    opts: RebaseOptions,
    reporter: &mut dyn Reporter,
) -> OutpostResult<RebaseReport> {
    outpost.current_branch().map_err(|err| match err {
        OutpostError::BranchNotFound { .. } => OutpostError::NoUpstreamTracking {
            branch: "HEAD".to_owned(),
        },
        other => other,
    })?;
    validate_source_remote(outpost, &opts.source_ref)?;

    reporter.step(
        StepKind::OutpostFetch,
        &format!(
            "fetching source {} branch {} into outpost {}",
            outpost.metadata().source_repo.display(),
            opts.source_ref.branch.as_str(),
            outpost.work_tree().display()
        ),
    );
    let remote_tracking_ref = fetch_source_ref(outpost, &opts.source_ref)?;
    outpost.git().run_check(["rebase", &remote_tracking_ref])?;

    Ok(RebaseReport {
        source_ref: opts.source_ref,
    })
}

fn validate_source_remote(outpost: &Outpost, source_ref: &SourceRemoteRef) -> OutpostResult<()> {
    if source_ref.remote == outpost.metadata().remote_name {
        Ok(())
    } else {
        Err(OutpostError::InvalidRefName {
            name: format!(
                "{}/{}",
                source_ref.remote.as_str(),
                source_ref.branch.as_str()
            ),
        })
    }
}

fn fetch_source_ref(outpost: &Outpost, source_ref: &SourceRemoteRef) -> OutpostResult<String> {
    let remote_tracking_ref = format!(
        "refs/remotes/{}/{}",
        source_ref.remote.as_str(),
        source_ref.branch.as_str()
    );
    let fetch_refspec = format!("{}:{remote_tracking_ref}", source_ref.branch.as_str());
    outpost
        .git()
        .run_check(["fetch", source_ref.remote.as_str(), &fetch_refspec])?;
    Ok(remote_tracking_ref)
}
