use crate::safety;
use crate::source_repo::read_optional_config;
use crate::{
    BranchName, GitInvoker, Outpost, OutpostError, OutpostResult, RefName, Reporter, SourceRepo,
    StepKind, UpstreamRef,
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

    check_checked_out_source_policy(&source, &branch)?;
    if !source.branch_exists(&branch)? {
        return Err(OutpostError::AmbiguousBranchCreation {
            branch: branch.as_str().to_owned(),
        });
    }

    let upstream = UpstreamRef {
        remote: outpost.metadata().remote_name.clone(),
        merge_ref: RefName::parse(format!("refs/heads/{}", branch.as_str()))?,
    };
    safety::check_no_divergence(outpost, &branch, &upstream)?;

    let outpost_before = source
        .git()
        .run_capture(["rev-parse", &source_branch_ref(branch.as_str())])?;
    reporter.step(
        StepKind::OutpostPush,
        &format!(
            "pushing outpost {} branch {} -> source {}",
            outpost.work_tree().display(),
            branch.as_str(),
            source.work_tree().display()
        ),
    );
    let outpost_refspec = branch_refspec(branch.as_str());
    outpost.git().run_check([
        "push",
        outpost.metadata().remote_name.as_str(),
        &outpost_refspec,
    ])?;
    let outpost_after = source
        .git()
        .run_capture(["rev-parse", &source_branch_ref(branch.as_str())])?;

    let origin_before = remote_origin_oid(&source, &branch)?;
    reporter.step(
        StepKind::SourcePush,
        &format!(
            "pushing source {} branch {} -> origin/{}",
            source.work_tree().display(),
            branch.as_str(),
            branch.as_str()
        ),
    );
    let source_refspec = branch_refspec(branch.as_str());
    source
        .git()
        .run_check(["push", "origin", &source_refspec])?;
    let origin_after =
        remote_origin_oid(&source, &branch)?.ok_or_else(|| OutpostError::BranchNotFound {
            branch: branch.as_str().to_owned(),
            repo: source.work_tree().to_path_buf(),
        })?;

    Ok(PushReport {
        outpost_to_source: StepResult::Pushed {
            commits: pushed_commit_count(outpost, &outpost_before, &outpost_after)?,
        },
        source_to_origin: StepResult::Pushed {
            commits: pushed_remote_commit_count(
                source.git(),
                origin_before.as_deref(),
                &origin_after,
            )?,
        },
    })
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

fn pushed_commit_count(outpost: &Outpost, before: &str, after: &str) -> OutpostResult<u32> {
    if before == after {
        return Ok(0);
    }

    let range = format!("{before}..{after}");
    let output = outpost.git().run_capture(["rev-list", "--count", &range])?;
    let count = output
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| invalid_rev_list_output(outpost, &output))?;
    if output.split_whitespace().nth(1).is_some() {
        return Err(invalid_rev_list_output(outpost, &output));
    }
    Ok(count)
}

fn pushed_remote_commit_count(
    git: &GitInvoker,
    before: Option<&str>,
    after: &str,
) -> OutpostResult<u32> {
    let Some(before) = before else {
        return parse_count(git, &git.run_capture(["rev-list", "--count", after])?);
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

fn invalid_rev_list_output(outpost: &Outpost, output: &str) -> OutpostError {
    OutpostError::IoAt {
        path: outpost.work_tree().to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected rev-list output: {output}"),
        ),
    }
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

fn remote_origin_oid(source: &SourceRepo, branch: &BranchName) -> OutpostResult<Option<String>> {
    let remote_ref = source_branch_ref(branch.as_str());
    let output = source
        .git()
        .run_capture(["ls-remote", "origin", &remote_ref])?;
    if output.is_empty() {
        return Ok(None);
    }

    let mut fields = output.split_whitespace();
    let oid = fields
        .next()
        .ok_or_else(|| invalid_count_output(source.git(), &output))?;
    Ok(Some(oid.to_owned()))
}

fn branch_refspec(branch: &str) -> String {
    format!("{branch}:{branch}")
}

fn source_branch_ref(branch: &str) -> String {
    format!("refs/heads/{branch}")
}
