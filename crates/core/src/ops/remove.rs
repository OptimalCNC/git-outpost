use std::path::{Path, PathBuf};

use crate::{safety, OutpostError, OutpostResult, SourceRepo};

pub struct RemoveOptions {
    pub path: PathBuf,
    pub force: bool,
}

pub fn run(source: &SourceRepo, opts: RemoveOptions) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    let entry = registry_entry(registry.entries(), &opts.path)?.clone();

    if entry.locked && !opts.force {
        return Err(OutpostError::OutpostLocked {
            path: entry.path,
            reason: lock_reason(&entry.lock_reason),
        });
    }

    if !entry.path.exists() {
        registry.remove_by_path(&entry.path)?;
        return registry.save();
    }

    let outpost = safety::check_path_is_managed_outpost_of(source, &entry.path)?;
    if !opts.force {
        safety::check_clean(outpost.work_tree(), outpost.git())?;
        safety::check_no_unpushed(&outpost, source)?;
    }

    registry.remove_by_path(&entry.path)?;
    registry.save()?;
    std::fs::remove_dir_all(&entry.path).map_err(|source| OutpostError::IoAt {
        path: entry.path,
        source,
    })
}

fn registry_entry<'a>(
    entries: &'a [crate::RegistryEntry],
    path: &Path,
) -> OutpostResult<&'a crate::RegistryEntry> {
    let lookup = canonicalize_existing_or_missing(path);
    entries
        .iter()
        .find(|entry| entry.path == lookup)
        .ok_or(OutpostError::RegistryEntryNotFound(lookup))
}

fn canonicalize_existing_or_missing(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => std::fs::canonicalize(parent)
                .map(|parent| parent.join(Path::new(name)))
                .unwrap_or_else(|_| path.to_path_buf()),
            _ => path.to_path_buf(),
        },
    }
}

fn lock_reason(reason: &Option<String>) -> String {
    reason
        .as_ref()
        .map(|reason| format!(": {reason}"))
        .unwrap_or_default()
}
