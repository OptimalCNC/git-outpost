use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::metadata::Metadata;
use crate::registry::RegistryEntry;
use crate::safety;
use crate::{
    BranchName, Outpost, OutpostError, OutpostResult, RemoteName, Reporter, SourceRepo, StepKind,
};

pub enum AddCheckout {
    CheckoutExisting {
        target_branch: Option<BranchName>,
    },
    NewBranch {
        name: BranchName,
        target_branch: Option<BranchName>,
    },
}

pub struct AddOptions {
    pub destination: PathBuf,
    pub checkout: AddCheckout,
    pub remote_name: RemoteName,
}

pub fn run(
    source: &SourceRepo,
    opts: AddOptions,
    reporter: &mut dyn Reporter,
) -> OutpostResult<Outpost> {
    let AddOptions {
        destination,
        checkout,
        remote_name,
    } = opts;
    let (parent, name) = destination_parent_and_name(&destination)?;
    safety::check_destination_clean(&parent, &name)?;

    let branch = resolve_existing_branch(source, &checkout)?;

    source.git().run_check([
        OsString::from("-c"),
        OsString::from("protocol.file.allow=user"),
        OsString::from("clone"),
        OsString::from("--no-shared"),
        OsString::from("--"),
        source.work_tree().as_os_str().to_os_string(),
        destination.as_os_str().to_os_string(),
    ])?;

    let outpost_git = crate::source_repo::invoker_at(&destination, source.env());
    if remote_name.as_str() != "origin" {
        outpost_git.run_check(["remote", "rename", "origin", remote_name.as_str()])?;
    }
    apply_checkout(source, &outpost_git, &checkout, &branch, &remote_name)?;
    Metadata {
        source_repo: source.work_tree().to_path_buf(),
        remote_name: remote_name.clone(),
    }
    .write(&outpost_git)?;

    reporter.step(
        StepKind::ConfigChange,
        &format!(
            "configuring source {}: receive.denyCurrentBranch=updateInstead",
            source.work_tree().display()
        ),
    );
    source.git().run_check([
        "config",
        "--local",
        "receive.denyCurrentBranch",
        "updateInstead",
    ])?;

    let mut registry = source.registry_mut()?;
    registry.add(RegistryEntry::new(destination.clone(), remote_name)?)?;
    registry.save()?;

    source.outpost_at(&destination)
}

fn resolve_existing_branch(
    source: &SourceRepo,
    checkout: &AddCheckout,
) -> OutpostResult<BranchName> {
    match checkout {
        AddCheckout::CheckoutExisting { target_branch } => {
            let branch = match target_branch {
                Some(branch) => branch.clone(),
                None => source.current_branch()?,
            };
            require_branch_exists(source, &branch)?;
            Ok(branch)
        }
        AddCheckout::NewBranch { target_branch, .. } => {
            let branch = match target_branch {
                Some(branch) => branch.clone(),
                None => source.current_branch()?,
            };
            require_branch_exists(source, &branch)?;
            Ok(branch)
        }
    }
}

fn require_branch_exists(source: &SourceRepo, branch: &BranchName) -> OutpostResult<()> {
    if source.branch_exists(branch)? {
        Ok(())
    } else {
        Err(OutpostError::BranchNotFound {
            branch: branch.as_str().to_owned(),
            repo: source.work_tree().to_path_buf(),
        })
    }
}

fn destination_parent_and_name(destination: &Path) -> OutpostResult<(PathBuf, PathBuf)> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = destination.file_name().ok_or_else(|| OutpostError::IoAt {
        path: destination.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination path has no file name",
        ),
    })?;

    Ok((parent, PathBuf::from(name)))
}

fn apply_checkout(
    source: &SourceRepo,
    git: &crate::GitInvoker,
    checkout: &AddCheckout,
    target_branch: &BranchName,
    remote_name: &RemoteName,
) -> OutpostResult<()> {
    match checkout {
        AddCheckout::CheckoutExisting { .. } => git.run_check(["switch", target_branch.as_str()]),
        AddCheckout::NewBranch { name, .. } => {
            source
                .git()
                .run_check(["branch", name.as_str(), target_branch.as_str()])?;
            git.run_check(["fetch", remote_name.as_str(), name.as_str()])?;
            git.run_check(["switch", name.as_str()])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_parent_and_name_splits_bare_relative_path() {
        let (parent, name) =
            destination_parent_and_name(Path::new("outpost")).expect("split destination");

        assert_eq!(parent, PathBuf::from("."));
        assert_eq!(name, PathBuf::from("outpost"));
    }

    #[test]
    fn destination_parent_and_name_splits_nested_relative_path() {
        let (parent, name) =
            destination_parent_and_name(Path::new("nested/outpost")).expect("split destination");

        assert_eq!(parent, PathBuf::from("nested"));
        assert_eq!(name, PathBuf::from("outpost"));
    }
}
