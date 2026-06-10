use std::path::{Component, Path, PathBuf};

use crate::{
    OutpostError, OutpostId, OutpostIdPrefix, OutpostResult, RegistryEntry, SourceRepo, safety,
};

/// User-supplied `<outpost>` selector for source-scoped operations.
///
/// A selector may be a path or a Docker-style outpost ID prefix. ID prefixes
/// are derived from source path plus outpost path, scoped to one source
/// registry, and accepted only when unique. If a bare hex token resolves as
/// both a path and an ID for different entries, the selector is ambiguous and
/// must fail closed instead of picking precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutpostSelector {
    CliArg { cwd: PathBuf, value: PathBuf },
    Path(PathBuf),
}

/// Registry entry resolved from an `OutpostSelector`.
///
/// The resolved registry entry and canonical registry path for an
/// `OutpostSelector`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutpostEntry {
    pub entry: RegistryEntry,
    pub path: PathBuf,
}

impl OutpostSelector {
    pub fn from_cli_arg(cwd: &Path, value: PathBuf) -> Self {
        Self::CliArg {
            cwd: cwd.to_path_buf(),
            value,
        }
    }

    pub fn from_path(path: PathBuf) -> Self {
        Self::Path(path)
    }

    fn display_value(&self) -> String {
        match self {
            Self::CliArg { value, .. } | Self::Path(value) => value.to_string_lossy().into_owned(),
        }
    }
}

pub fn resolve_entry(
    source: &SourceRepo,
    selector: &OutpostSelector,
) -> OutpostResult<ResolvedOutpostEntry> {
    let registry = source.registry()?;
    resolve_entry_in_entries(source.work_tree(), registry.entries(), selector)
}

pub fn resolve_live_entry(
    source: &SourceRepo,
    selector: &OutpostSelector,
) -> OutpostResult<ResolvedOutpostEntry> {
    let resolved = resolve_entry(source, selector)?;
    safety::check_entry_is_managed_outpost_of(source, &resolved.entry)?;
    Ok(resolved)
}

pub(crate) fn resolve_entry_in_entries(
    source_path: &Path,
    entries: &[RegistryEntry],
    selector: &OutpostSelector,
) -> OutpostResult<ResolvedOutpostEntry> {
    let classified = classify(selector);
    match classified {
        ClassifiedSelector::PathOnly(path) => resolve_path(entries, &path),
        ClassifiedSelector::BarePath(path) => resolve_path(entries, &path),
        ClassifiedSelector::BareHex { path, prefix } => {
            let path_match = find_by_path(entries, &path)?;
            let id_match = find_by_prefix(source_path, entries, &prefix)?;
            match (path_match, id_match) {
                (Some(path_entry), Some(id_entry)) if path_entry.path == id_entry.path => {
                    Ok(resolved(path_entry))
                }
                (Some(_), Some(_)) => Err(OutpostError::OutpostSelectorAmbiguous(
                    selector.display_value(),
                )),
                (Some(path_entry), None) => Ok(resolved(path_entry)),
                (None, Some(id_entry)) => Ok(resolved(id_entry)),
                (None, None) => Err(OutpostError::OutpostIdPrefixNotFound(
                    prefix.as_str().to_owned(),
                )),
            }
        }
    }
}

enum ClassifiedSelector {
    PathOnly(PathBuf),
    BarePath(PathBuf),
    BareHex {
        path: PathBuf,
        prefix: OutpostIdPrefix,
    },
}

fn classify(selector: &OutpostSelector) -> ClassifiedSelector {
    match selector {
        OutpostSelector::Path(path) => ClassifiedSelector::PathOnly(path.clone()),
        OutpostSelector::CliArg { cwd, value } => {
            if explicit_path_syntax(value) {
                return ClassifiedSelector::PathOnly(absolutize(cwd, value));
            }
            let path = absolutize(cwd, value);
            let Some(value) = value.to_str() else {
                return ClassifiedSelector::PathOnly(path);
            };
            match OutpostIdPrefix::parse(value.to_owned()) {
                Ok(prefix) => ClassifiedSelector::BareHex { path, prefix },
                Err(_) => ClassifiedSelector::BarePath(path),
            }
        }
    }
}

fn explicit_path_syntax(path: &Path) -> bool {
    if path.is_absolute() || path.to_str().is_none() {
        return true;
    }
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::CurDir | Component::ParentDir), _) => true,
        (Some(_), Some(_)) => true,
        _ => false,
    }
}

fn absolutize(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn resolve_path(entries: &[RegistryEntry], path: &Path) -> OutpostResult<ResolvedOutpostEntry> {
    find_by_path(entries, path)?
        .map(resolved)
        .ok_or_else(|| OutpostError::RegistryEntryNotFound(canonicalize_existing_or_missing(path)))
}

fn find_by_path<'a>(
    entries: &'a [RegistryEntry],
    path: &Path,
) -> OutpostResult<Option<&'a RegistryEntry>> {
    let path = canonicalize_existing_or_missing(path);
    Ok(entries.iter().find(|entry| entry.path == path))
}

fn find_by_prefix<'a>(
    source_path: &Path,
    entries: &'a [RegistryEntry],
    prefix: &OutpostIdPrefix,
) -> OutpostResult<Option<&'a RegistryEntry>> {
    let mut matches = entries
        .iter()
        .filter(|entry| OutpostId::derive(source_path, &entry.path).starts_with(prefix));
    let first = matches.next();
    if matches.next().is_some() {
        return Err(OutpostError::OutpostIdPrefixAmbiguous(
            prefix.as_str().to_owned(),
        ));
    }
    Ok(first)
}

fn resolved(entry: &RegistryEntry) -> ResolvedOutpostEntry {
    ResolvedOutpostEntry {
        entry: entry.clone(),
        path: entry.path.clone(),
    }
}

fn canonicalize_existing_or_missing(path: &Path) -> PathBuf {
    if path.exists() {
        return std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    }

    let mut missing = Vec::new();
    let mut existing = path;
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            return normalize_existing_or_missing(path);
        };
        missing.push(name.to_os_string());
        let Some(parent) = existing.parent() else {
            return normalize_existing_or_missing(path);
        };
        existing = parent;
    }

    let mut canonical =
        std::fs::canonicalize(existing).unwrap_or_else(|_| normalize_existing_or_missing(existing));
    for component in missing.iter().rev() {
        canonical.push(component);
    }
    normalize_existing_or_missing(&canonical)
}

fn normalize_existing_or_missing(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
