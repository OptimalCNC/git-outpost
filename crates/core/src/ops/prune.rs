use std::path::{Path, PathBuf};

use crate::{Outpost, OutpostError, OutpostResult, SourceRepo};

pub struct PruneOptions {
    pub dry_run: bool,
    pub verbose: bool,
}

pub struct PruneReport {
    pub removed_entries: Vec<PathBuf>,
    pub orphaned_source_missing: Vec<PathBuf>,
    pub locked_entries: Vec<PathBuf>,
    pub dry_run: bool,
}

pub fn run(source: &SourceRepo, opts: PruneOptions) -> OutpostResult<PruneReport> {
    let mut registry = source.registry_mut()?;
    let entries = registry.entries().to_vec();
    let mut report = PruneReport {
        removed_entries: Vec::new(),
        orphaned_source_missing: Vec::new(),
        locked_entries: Vec::new(),
        dry_run: opts.dry_run,
    };

    for entry in entries {
        if entry.locked {
            report.locked_entries.push(entry.path);
        } else if !entry.path.exists() {
            report.removed_entries.push(entry.path.clone());
            if !opts.dry_run {
                registry.remove_by_path(&entry.path)?;
            }
        } else if source_missing_outpost(&entry.path)? {
            report.orphaned_source_missing.push(entry.path);
        }
    }

    if !opts.dry_run && !report.removed_entries.is_empty() {
        registry.save()?;
    }

    let _ = opts.verbose;
    Ok(report)
}

fn source_missing_outpost(path: &Path) -> OutpostResult<bool> {
    if !path.is_dir() {
        return Ok(false);
    }

    match Outpost::at(path).and_then(|outpost| outpost.source_repo()) {
        Ok(_) => Ok(false),
        Err(OutpostError::SourceMissing(_)) => Ok(true),
        Err(OutpostError::NotARepo(_))
        | Err(OutpostError::NotAnOutpost(_))
        | Err(OutpostError::RegistryEntryNotManaged(_))
        | Err(OutpostError::GitFailed { .. }) => Ok(false),
        Err(err) => Err(err),
    }
}
