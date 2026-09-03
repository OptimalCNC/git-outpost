use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::outpost_id::{OutpostId, shortest_unique_prefixes};
use crate::source_repo::{SourceRepo, canonicalize_path, is_dirty};
use crate::{GitInvoker, Outpost, OutpostError, OutpostIdPrefix, OutpostResult, RemoteName};

use super::routes::{self, RouteDirection};
use super::{
    RegisteredOutpostHead, RegisteredOutpostStatus, RouteAvailability, SourceHead, SourceStatus,
    StaleRegistration, canonicalize_existing_or_missing, canonicalize_remote_path,
    current_branch_or_detached,
};

pub(super) fn build(
    source_path: PathBuf,
    git: &GitInvoker,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<SourceStatus> {
    let head = match current_branch_or_detached(git)? {
        Some(branch) => SourceHead::Attached {
            upstream: routes::read_upstream(git, &branch)?,
            branch,
        },
        None => SourceHead::Detached,
    };
    let source_dirty = is_dirty(git)?;
    let source = SourceRepo::at_with(&source_path, env)?;
    let outpost_container = source.outpost_container()?;
    let registry_path = source.registry_path();
    let entries = source
        .registry()?
        .entries()
        .iter()
        .map(|entry| RegistryEntry {
            path: entry.path.clone(),
            remote_name: entry.remote_name.clone(),
            locked: entry.locked,
        })
        .collect::<Vec<_>>();
    reject_duplicate_paths(&registry_path, &entries)?;

    let ids = entries
        .iter()
        .map(|entry| OutpostId::derive(&source_path, &entry.path))
        .collect::<Vec<_>>();
    let prefixes =
        shortest_unique_prefixes(ids.iter()).map_err(|error| OutpostError::BadRegistry {
            path: source.registry_path(),
            reason: error.to_string(),
        })?;
    let mut outposts = Vec::new();
    let mut stale_registrations = Vec::new();

    for (entry, prefix) in entries.iter().zip(prefixes) {
        let display_id = prefix;
        match fs::metadata(&entry.path) {
            Ok(metadata) if metadata.is_dir() => {
                outposts.push(build_live_row(&source_path, entry, display_id, env)?)
            }
            Ok(_) => return Err(integrity_error(&source_path, &entry.path)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                stale_registrations.push(StaleRegistration {
                    display_id,
                    path: entry.path.clone(),
                });
            }
            Err(source) => {
                return Err(OutpostError::IoAt {
                    path: entry.path.clone(),
                    source,
                });
            }
        }
    }

    Ok(SourceStatus {
        source_path,
        head,
        source_dirty,
        outpost_container,
        outposts,
        stale_registrations,
    })
}

struct RegistryEntry {
    path: PathBuf,
    remote_name: RemoteName,
    locked: bool,
}

fn reject_duplicate_paths(path: &Path, entries: &[RegistryEntry]) -> OutpostResult<()> {
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(&entry.path) {
            return Err(OutpostError::BadRegistry {
                path: path.to_path_buf(),
                reason: format!("duplicate registered path: {}", entry.path.display()),
            });
        }
    }
    Ok(())
}

fn build_live_row(
    source_path: &Path,
    entry: &RegistryEntry,
    display_id: OutpostIdPrefix,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<RegisteredOutpostStatus> {
    let outpost_path = canonicalize_path(&entry.path)?;
    if outpost_path != entry.path {
        return Err(integrity_error(source_path, &entry.path));
    }
    let outpost = Outpost::at_with(&outpost_path, env)
        .map_err(|error| metadata_integrity_error(source_path, &outpost_path, error))?;
    let git = outpost.git();
    let recorded_source = outpost.metadata().source_repo.clone();
    let recorded_source = canonicalize_existing_or_missing(&recorded_source)?;
    if recorded_source != source_path {
        return Err(integrity_error(source_path, &outpost_path));
    }
    if outpost.metadata().remote_name != entry.remote_name {
        return Err(integrity_error(source_path, &outpost_path));
    }
    validate_recorded_remote(git, &outpost_path, source_path, &entry.remote_name)?;

    let head = match current_branch_or_detached(git)? {
        Some(branch) => RegisteredOutpostHead::Attached(branch),
        None => RegisteredOutpostHead::Detached,
    };
    Ok(RegisteredOutpostStatus {
        display_id,
        path: outpost_path,
        head,
        dirty: is_dirty(git)?,
        locked: entry.locked,
    })
}

fn metadata_integrity_error(
    source_path: &Path,
    outpost_path: &Path,
    error: OutpostError,
) -> OutpostError {
    match error {
        OutpostError::BadMetadata { .. }
        | OutpostError::NotAnOutpost(_)
        | OutpostError::InvalidRefName { .. } => integrity_error(source_path, outpost_path),
        other => other,
    }
}

fn validate_recorded_remote(
    git: &GitInvoker,
    outpost_path: &Path,
    source_path: &Path,
    remote: &RemoteName,
) -> OutpostResult<()> {
    for direction in [RouteDirection::Fetch, RouteDirection::Push] {
        match routes::probe_urls(git, remote, direction)? {
            RouteAvailability::Known(urls) => {
                for url in urls.as_slice() {
                    if canonicalize_remote_path(outpost_path, url)? != source_path {
                        return Err(integrity_error(source_path, outpost_path));
                    }
                }
            }
            RouteAvailability::Unavailable => {
                return Err(integrity_error(source_path, outpost_path));
            }
        }
    }
    Ok(())
}

fn integrity_error(source: &Path, outpost: &Path) -> OutpostError {
    OutpostError::RegisteredOutpostIntegrity {
        source: source.to_path_buf(),
        outpost: outpost.to_path_buf(),
    }
}
