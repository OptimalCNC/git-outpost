use crate::{BranchName, Outpost, OutpostResult, Reporter, StepKind};

pub enum SourceCommand {
    Pull(SourcePullOptions),
}

pub struct SourcePullOptions {
    pub branch: BranchName,
}

pub struct SourcePullReport {
    pub branch: BranchName,
    pub updated: bool,
}

pub fn pull(
    outpost: &Outpost,
    opts: SourcePullOptions,
    reporter: &mut dyn Reporter,
) -> OutpostResult<SourcePullReport> {
    let source = outpost.source_repo()?;
    if !source.branch_exists(&opts.branch)? {
        return Err(crate::OutpostError::BranchNotFound {
            branch: opts.branch.as_str().to_owned(),
            repo: source.work_tree().to_path_buf(),
        });
    }

    reporter.step(
        StepKind::SourceFetch,
        &format!(
            "fast-forwarding source {} branch {} from origin/{}",
            source.work_tree().display(),
            opts.branch.as_str(),
            opts.branch.as_str()
        ),
    );
    let updated = source.fast_forward_branch_from_origin(&opts.branch)?;

    Ok(SourcePullReport {
        branch: opts.branch,
        updated,
    })
}
