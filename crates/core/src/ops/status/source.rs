use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::Error as _;

use crate::outpost_id::{OutpostId, shortest_unique_prefixes};
use crate::source_repo::{canonicalize_path, invoker_at, is_dirty};
use crate::{GitInvoker, OutpostError, OutpostIdPrefix, OutpostResult, RawMetadata, RemoteName};

use super::routes::{self, RouteDirection};
use super::{
    RegisteredOutpostHead, RegisteredOutpostStatus, RouteAvailability, SourceHead, SourceStatus,
    StaleRegistration, canonicalize_existing_or_missing, canonicalize_remote_path,
    current_branch_or_detached,
};

const STORAGE_VERSION: u32 = 1;

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
    let outpost_container = load_outpost_container(&source_path)?;
    let registry_path = source_path.join(".outpost/registry.json");
    let entries = load_registry(&registry_path)?;
    reject_duplicate_paths(&registry_path, &entries)?;

    let ids = entries
        .iter()
        .map(|entry| OutpostId::derive(&source_path, &entry.path))
        .collect::<Vec<_>>();
    let prefixes = shortest_unique_prefixes(ids.iter());
    let mut outposts = Vec::new();
    let mut stale_registrations = Vec::new();

    for (entry, prefix) in entries.iter().zip(prefixes) {
        let display_id = OutpostIdPrefix::parse(prefix).expect("derived prefix is valid");
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

#[derive(Deserialize)]
struct RegistryFile {
    version: u32,
    outposts: Vec<RegistryEntryFile>,
}

#[derive(Deserialize)]
struct RegistryEntryFile {
    path: PathBuf,
    #[serde(rename = "created_at")]
    _created_at: chrono::DateTime<chrono::Utc>,
    remote_name: String,
    locked: bool,
    #[serde(rename = "lock_reason")]
    _lock_reason: Option<String>,
    #[serde(rename = "locked_at")]
    _locked_at: Option<chrono::DateTime<chrono::Utc>>,
}

struct RegistryEntry {
    path: PathBuf,
    remote_name: RemoteName,
    locked: bool,
}

fn load_registry(path: &Path) -> OutpostResult<Vec<RegistryEntry>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let file = serde_json::from_str::<RegistryFile>(&contents).map_err(|source| {
        OutpostError::BadRegistry {
            path: path.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    if file.version != STORAGE_VERSION {
        return Err(OutpostError::BadRegistry {
            path: path.to_path_buf(),
            reason: format!("unsupported registry version {}", file.version),
        });
    }
    file.outposts
        .into_iter()
        .map(|entry| {
            let remote_name = RemoteName::parse(entry.remote_name.clone()).map_err(|source| {
                OutpostError::BadRegistry {
                    path: path.to_path_buf(),
                    reason: source.to_string(),
                }
            })?;
            Ok(RegistryEntry {
                path: entry.path,
                remote_name,
                locked: entry.locked,
            })
        })
        .collect()
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
    let git = invoker_at(&outpost_path, env);
    let raw = RawMetadata::read(&git)
        .map_err(|error| metadata_integrity_error(source_path, &outpost_path, error))?;
    if raw.managed != Some(true) {
        return Err(integrity_error(source_path, &outpost_path));
    }
    let Some(recorded_source) = raw.source_repo else {
        return Err(integrity_error(source_path, &outpost_path));
    };
    let recorded_source = canonicalize_existing_or_missing(&recorded_source)?;
    if recorded_source != source_path {
        return Err(integrity_error(source_path, &outpost_path));
    }
    if raw.remote_name.as_ref() != Some(&entry.remote_name) {
        return Err(integrity_error(source_path, &outpost_path));
    }
    validate_recorded_remote(&git, &outpost_path, source_path, &entry.remote_name)?;

    let head = match current_branch_or_detached(&git)? {
        Some(branch) => RegisteredOutpostHead::Attached(branch),
        None => RegisteredOutpostHead::Detached,
    };
    Ok(RegisteredOutpostStatus {
        display_id,
        path: outpost_path,
        head,
        dirty: is_dirty(&git)?,
        locked: entry.locked,
    })
}

fn metadata_integrity_error(
    source_path: &Path,
    outpost_path: &Path,
    error: OutpostError,
) -> OutpostError {
    match error {
        OutpostError::GitFailed { .. }
        | OutpostError::BadMetadata { .. }
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    version: u32,
    #[serde(default, deserialize_with = "deserialize_optional_path")]
    outpost_container: Option<PathBuf>,
}

fn load_outpost_container(source_path: &Path) -> OutpostResult<Option<PathBuf>> {
    let path = source_path.join(".outpost/config.json");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(OutpostError::IoAt { path, source }),
    };
    let file = serde_json::from_str::<ConfigFile>(&contents).map_err(|source| {
        OutpostError::BadConfig {
            path: path.clone(),
            reason: source.to_string(),
        }
    })?;
    if file.version != STORAGE_VERSION {
        return Err(OutpostError::BadConfig {
            path,
            reason: format!("unsupported config version {}", file.version),
        });
    }
    let Some(container) = file.outpost_container else {
        return Ok(None);
    };
    if !container.is_absolute() {
        return Err(OutpostError::BadConfig {
            path,
            reason: "outpost-container must be an absolute path".to_owned(),
        });
    }
    let canonical = fs::canonicalize(&container).map_err(|source| OutpostError::BadConfig {
        path: path.clone(),
        reason: format!("invalid outpost-container: {source}"),
    })?;
    let metadata = fs::metadata(&canonical).map_err(|source| OutpostError::BadConfig {
        path: path.clone(),
        reason: format!("invalid outpost-container: {source}"),
    })?;
    if !metadata.is_dir() {
        return Err(OutpostError::BadConfig {
            path,
            reason: "invalid outpost-container: path is not an existing directory".to_owned(),
        });
    }
    Ok(Some(canonical))
}

fn deserialize_optional_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(D::Error::custom("outpost_container must be a path string"));
    }
    PathBuf::deserialize(value)
        .map(Some)
        .map_err(D::Error::custom)
}
