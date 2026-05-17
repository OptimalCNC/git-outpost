use std::path::{Path, PathBuf};

use crate::{safety, OutpostError, OutpostResult, SourceRepo};

pub struct MoveOptions {
    pub path: PathBuf,
    pub new_path: PathBuf,
    pub force: bool,
}

pub fn run(source: &SourceRepo, opts: MoveOptions) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    let index = registry_entry_index(registry.entries(), &opts.path)?;
    let entry = registry.entries()[index].clone();

    if entry.locked && !opts.force {
        return Err(OutpostError::OutpostLocked {
            path: entry.path,
            reason: lock_reason(&entry.lock_reason),
        });
    }

    let outpost = safety::check_path_is_managed_outpost_of(source, &entry.path)?;
    if !opts.force {
        safety::check_clean(outpost.work_tree(), outpost.git())?;
    }
    check_destination_clean(&opts.new_path)?;

    std::fs::rename(&entry.path, &opts.new_path).map_err(|source| OutpostError::IoAt {
        path: entry.path.clone(),
        source,
    })?;
    registry.update_path(&entry.path, opts.new_path)?;
    registry.save()
}

fn registry_entry_index(entries: &[crate::RegistryEntry], path: &Path) -> OutpostResult<usize> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| OutpostError::RegistryEntryNotFound(path.to_path_buf()))?;
    entries
        .iter()
        .position(|entry| entry.path == canonical)
        .ok_or(OutpostError::RegistryEntryNotFound(canonical))
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
