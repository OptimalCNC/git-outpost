use std::path::{Path, PathBuf};

use crate::selector::{OutpostSelector, resolve_entry};
use crate::{OutpostError, OutpostResult, SourceRepo, safety};

pub struct MoveOptions {
    pub selector: OutpostSelector,
    pub new_path: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveReport {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
}

pub fn run(source: &SourceRepo, opts: MoveOptions) -> OutpostResult<MoveReport> {
    let entry = resolve_entry(source, &opts.selector)?.entry;
    if entry.locked && !opts.force {
        return Err(OutpostError::OutpostLocked {
            path: entry.path,
            reason: lock_reason(&entry.lock_reason),
        });
    }

    let outpost = safety::check_entry_is_managed_outpost_of(source, &entry)?;
    if !opts.force {
        safety::check_clean(outpost.work_tree(), outpost.git())?;
    }
    check_destination_clean(&opts.new_path)?;

    std::fs::rename(&entry.path, &opts.new_path).map_err(|source| OutpostError::IoAt {
        path: entry.path.clone(),
        source,
    })?;
    let old_path = entry.path;
    let mut registry = source.registry_mut()?;
    registry.update_path(&old_path, opts.new_path.clone())?;
    registry.save()?;
    Ok(MoveReport {
        old_path,
        new_path: std::fs::canonicalize(&opts.new_path).map_err(|source| OutpostError::IoAt {
            path: opts.new_path,
            source,
        })?,
    })
}

fn check_destination_clean(destination: &Path) -> OutpostResult<()> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = destination.file_name().ok_or_else(|| OutpostError::IoAt {
        path: destination.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination path has no file name",
        ),
    })?;

    safety::check_destination_clean(&parent, Path::new(name)).map_err(|err| match err {
        OutpostError::DestinationExists(_) => {
            OutpostError::DestinationExists(destination.to_path_buf())
        }
        OutpostError::DestinationInsideRepo(_) => {
            OutpostError::DestinationInsideRepo(destination.to_path_buf())
        }
        other => other,
    })
}

fn lock_reason(reason: &Option<String>) -> String {
    reason
        .as_ref()
        .map(|reason| format!(": {reason}"))
        .unwrap_or_default()
}
