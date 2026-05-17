use std::path::PathBuf;

use crate::{safety, OutpostError, OutpostResult, SourceRepo};

pub struct LockOptions {
    pub path: PathBuf,
    pub reason: Option<String>,
}

pub fn run(source: &SourceRepo, opts: LockOptions) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    let path = registered_path(registry.entries(), &opts.path)?;
    safety::check_path_is_managed_outpost_of(source, &path)?;
    registry.lock(&path, opts.reason)?;
    registry.save()
}

fn registered_path(
    entries: &[crate::RegistryEntry],
    path: &std::path::Path,
) -> OutpostResult<PathBuf> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| OutpostError::RegistryEntryNotFound(path.to_path_buf()))?;
    if entries.iter().any(|entry| entry.path == canonical) {
        Ok(canonical)
    } else {
        Err(OutpostError::RegistryEntryNotFound(canonical))
    }
}
