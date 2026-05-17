use std::path::PathBuf;

use crate::{safety, OutpostError, OutpostResult, SourceRepo};

pub struct UnlockOptions {
    pub path: PathBuf,
}

pub fn run(source: &SourceRepo, opts: UnlockOptions) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    let path = registered_path(registry.entries(), &opts.path)?;
    safety::check_path_is_managed_outpost_of(source, &path)?;
    registry.unlock(&path)?;
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
