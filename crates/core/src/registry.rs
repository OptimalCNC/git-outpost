use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{OutpostError, OutpostResult, RemoteName, SourceRepo};

const REGISTRY_VERSION: u32 = 1;
const OUTPOST_IGNORE_LINE: &str = ".outpost/";

#[derive(Debug, Clone)]
pub struct Registry {
    path: PathBuf,
    exclude_path: PathBuf,
    version: u32,
    entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub remote_name: RemoteName,
    pub locked: bool,
    pub lock_reason: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
}

#[must_use = "RegistryMut changes are persisted only on save()"]
pub struct RegistryMut<'src> {
    _source: &'src SourceRepo,
    inner: Registry,
    dirty: bool,
    saved: bool,
}

impl Registry {
    pub fn load(source: &SourceRepo) -> OutpostResult<Self> {
        let path = source.registry_path();
        let exclude_path = source.local_exclude_path();
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    exclude_path,
                    version: REGISTRY_VERSION,
                    entries: Vec::new(),
                });
            }
            Err(source) => {
                return Err(OutpostError::IoAt { path, source });
            }
        };

        let file = serde_json::from_str::<RegistryFile>(&contents).map_err(|source| {
            OutpostError::BadRegistry {
                path: path.clone(),
                reason: source.to_string(),
            }
        })?;
        if file.version != REGISTRY_VERSION {
            return Err(OutpostError::BadRegistry {
                path,
                reason: format!("unsupported registry version {}", file.version),
            });
        }

        let entries = file
            .outposts
            .into_iter()
            .map(|entry| entry.try_into_entry(&path))
            .collect::<OutpostResult<Vec<_>>>()?;

        Ok(Self {
            path,
            exclude_path,
            version: REGISTRY_VERSION,
            entries,
        })
    }

    pub fn entries(&self) -> &[RegistryEntry] {
        &self.entries
    }

    pub fn save(&self) -> OutpostResult<()> {
        let parent = self.path.parent().ok_or_else(|| OutpostError::IoAt {
            path: self.path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "registry path has no parent",
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
            path: parent.to_path_buf(),
            source,
        })?;
        ensure_local_ignore(&self.exclude_path)?;

        let file = RegistryFile::from_registry(self);
        let mut temp =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
                path: parent.to_path_buf(),
                source,
            })?;
        serde_json::to_writer_pretty(temp.as_file_mut(), &file).map_err(|source| {
            OutpostError::IoAt {
                path: self.path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::Other, source),
            }
        })?;
        writeln!(temp.as_file_mut()).map_err(|source| OutpostError::IoAt {
            path: self.path.clone(),
            source,
        })?;
        temp.persist(&self.path)
            .map_err(|source| OutpostError::IoAt {
                path: self.path.clone(),
                source: source.error,
            })?;

        Ok(())
    }

    fn find(&self, path: &Path) -> Option<usize> {
        self.entries.iter().position(|entry| entry.path == path)
    }

    fn find_existing_or_recorded(&self, path: &Path) -> OutpostResult<(PathBuf, Option<usize>)> {
        match canonicalize_path(path) {
            Ok(canonical) => {
                let index = self.find(&canonical);
                Ok((canonical, index))
            }
            Err(canonicalize_err) => {
                if let Some(index) = self.entries.iter().position(|entry| entry.path == path) {
                    Ok((path.to_path_buf(), Some(index)))
                } else {
                    Err(canonicalize_err)
                }
            }
        }
    }
}

impl RegistryEntry {
    pub fn new(path: PathBuf, remote_name: RemoteName) -> OutpostResult<Self> {
        Ok(Self {
            path: canonicalize_path(&path)?,
            created_at: Utc::now(),
            remote_name,
            locked: false,
            lock_reason: None,
            locked_at: None,
        })
    }
}

impl<'src> RegistryMut<'src> {
    pub(crate) fn load(source: &'src SourceRepo) -> OutpostResult<Self> {
        Ok(Self {
            _source: source,
            inner: Registry::load(source)?,
            dirty: false,
            saved: false,
        })
    }

    pub fn add(&mut self, mut entry: RegistryEntry) -> OutpostResult<()> {
        entry.path = canonicalize_path(&entry.path)?;
        if let Some(index) = self.inner.find(&entry.path) {
            let old = &self.inner.entries[index];
            if old.locked && !entry.locked {
                entry.locked = true;
                entry.lock_reason = old.lock_reason.clone();
                entry.locked_at = old.locked_at;
            }
            self.inner.entries[index] = entry;
        } else {
            self.inner.entries.push(entry);
        }
        self.dirty = true;
        Ok(())
    }

    pub fn update_path(&mut self, old: &Path, new: PathBuf) -> OutpostResult<()> {
        let (old, index) = self.inner.find_existing_or_recorded(old)?;
        let new = canonicalize_path(&new)?;
        let index = index.ok_or_else(|| OutpostError::RegistryEntryNotFound(old.clone()))?;
        self.inner.entries[index].path = new;
        self.dirty = true;
        Ok(())
    }

    pub fn lock(&mut self, path: &Path, reason: Option<String>) -> OutpostResult<()> {
        let path = canonicalize_path(path)?;
        let index = self
            .inner
            .find(&path)
            .ok_or_else(|| OutpostError::RegistryEntryNotFound(path.clone()))?;
        let entry = &mut self.inner.entries[index];
        entry.locked = true;
        entry.lock_reason = reason;
        entry.locked_at = Some(Utc::now());
        self.dirty = true;
        Ok(())
    }

    pub fn unlock(&mut self, path: &Path) -> OutpostResult<()> {
        let path = canonicalize_path(path)?;
        let index = self
            .inner
            .find(&path)
            .ok_or_else(|| OutpostError::RegistryEntryNotFound(path.clone()))?;
        let entry = &mut self.inner.entries[index];
        entry.locked = false;
        entry.lock_reason = None;
        entry.locked_at = None;
        self.dirty = true;
        Ok(())
    }

    pub fn remove_by_path(&mut self, path: &Path) -> OutpostResult<bool> {
        let (_path, index) = self.inner.find_existing_or_recorded(path)?;
        if let Some(index) = index {
            self.inner.entries.remove(index);
            self.dirty = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn entries(&self) -> &[RegistryEntry] {
        self.inner.entries()
    }

    pub fn save(mut self) -> OutpostResult<()> {
        self.saved = true;
        let result = self.inner.save();
        if result.is_ok() {
            self.dirty = false;
        }
        result
    }
}

impl<'src> Drop for RegistryMut<'src> {
    fn drop(&mut self) {
        if self.dirty && !self.saved {
            debug_assert!(false, "RegistryMut dropped with unsaved changes");
            eprintln!("warning: registry changes dropped without save");
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    version: u32,
    outposts: Vec<RegistryEntryFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryEntryFile {
    path: PathBuf,
    created_at: DateTime<Utc>,
    remote_name: String,
    locked: bool,
    lock_reason: Option<String>,
    locked_at: Option<DateTime<Utc>>,
}

impl RegistryFile {
    fn from_registry(registry: &Registry) -> Self {
        Self {
            version: registry.version,
            outposts: registry
                .entries
                .iter()
                .map(RegistryEntryFile::from_entry)
                .collect(),
        }
    }
}

impl RegistryEntryFile {
    fn from_entry(entry: &RegistryEntry) -> Self {
        Self {
            path: entry.path.clone(),
            created_at: entry.created_at,
            remote_name: entry.remote_name.as_str().to_owned(),
            locked: entry.locked,
            lock_reason: entry.lock_reason.clone(),
            locked_at: entry.locked_at,
        }
    }

    fn try_into_entry(self, registry_path: &Path) -> OutpostResult<RegistryEntry> {
        let remote_name = RemoteName::parse(self.remote_name.clone()).map_err(|source| {
            OutpostError::BadRegistry {
                path: registry_path.to_path_buf(),
                reason: source.to_string(),
            }
        })?;
        Ok(RegistryEntry {
            path: self.path,
            created_at: self.created_at,
            remote_name,
            locked: self.locked,
            lock_reason: self.lock_reason,
            locked_at: self.locked_at,
        })
    }
}

fn ensure_local_ignore(exclude_path: &Path) -> OutpostResult<()> {
    let parent = exclude_path.parent().ok_or_else(|| OutpostError::IoAt {
        path: exclude_path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "exclude path has no parent",
        ),
    })?;
    fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;

    let mut contents = match fs::read_to_string(exclude_path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(OutpostError::IoAt {
                path: exclude_path.to_path_buf(),
                source,
            });
        }
    };

    if contents
        .lines()
        .any(|line| line.trim() == OUTPOST_IGNORE_LINE)
    {
        return Ok(());
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(OUTPOST_IGNORE_LINE);
    contents.push('\n');
    fs::write(exclude_path, contents).map_err(|source| OutpostError::IoAt {
        path: exclude_path.to_path_buf(),
        source,
    })
}

fn canonicalize_path(path: &Path) -> OutpostResult<PathBuf> {
    fs::canonicalize(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use chrono::TimeZone;
    use serde_json::json;

    use super::*;
    use crate::GitInvoker;

    #[test]
    fn empty_registry_serializes_to_expected_json_and_round_trips() {
        let (_temp, source) = init_source_repo();
        let registry = Registry::load(&source).expect("missing registry loads");

        registry.save().expect("save empty registry");

        let value: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(source.registry_path()).expect("registry file"),
        )
        .expect("registry json parses");
        assert_eq!(value, json!({ "version": 1, "outposts": [] }));
        assert!(fs::read_to_string(source.local_exclude_path())
            .expect("local exclude")
            .lines()
            .any(|line| line == OUTPOST_IGNORE_LINE));

        let loaded = Registry::load(&source).expect("registry reloads");
        assert!(loaded.entries().is_empty());
    }

    #[test]
    fn add_readd_remove_and_add_round_trips_by_canonical_path() {
        let (temp, source) = init_source_repo();
        let outpost = temp.path().join("C");
        let other = temp.path().join("D");
        fs::create_dir_all(&outpost).expect("outpost dir");
        fs::create_dir_all(&other).expect("other dir");

        let mut registry = source.registry_mut().expect("registry mut");
        registry
            .add(entry_at(&outpost, "local", 1))
            .expect("add local");
        registry
            .lock(&outpost, Some("keep".to_owned()))
            .expect("lock");
        registry
            .add(entry_at(&outpost, "custom", 2))
            .expect("re-add same path");
        assert_eq!(registry.entries().len(), 1);
        assert_eq!(registry.entries()[0].remote_name.as_str(), "custom");
        assert!(registry.entries()[0].locked);
        assert_eq!(registry.entries()[0].lock_reason.as_deref(), Some("keep"));

        assert!(registry.remove_by_path(&outpost).expect("remove existing"));
        assert!(!registry.remove_by_path(&outpost).expect("remove absent"));
        registry
            .add(entry_at(&other, "local", 3))
            .expect("add other");
        registry.save().expect("save");

        let loaded = Registry::load(&source).expect("reload");
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].path, fs::canonicalize(&other).unwrap());
        assert_eq!(loaded.entries()[0].remote_name.as_str(), "local");
    }

    #[test]
    fn load_missing_registry_returns_empty_registry() {
        let (_temp, source) = init_source_repo();
        let registry = Registry::load(&source).expect("missing registry loads");

        assert!(registry.entries().is_empty());
        assert!(!source.registry_path().exists());
    }

    #[test]
    fn update_path_handles_registered_old_path_after_rename() {
        let (temp, source) = init_source_repo();
        let old = temp.path().join("C");
        let new = temp.path().join("D");
        fs::create_dir_all(&old).expect("old outpost dir");
        let canonical_old = fs::canonicalize(&old).expect("canonical old path");

        let mut registry = source.registry_mut().expect("registry mut");
        registry
            .add(entry_at(&old, "local", 1))
            .expect("add old path");
        fs::rename(&old, &new).expect("rename outpost");

        registry
            .update_path(&canonical_old, new.clone())
            .expect("update renamed path");
        registry.save().expect("save");

        let loaded = Registry::load(&source).expect("reload");
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].path, fs::canonicalize(&new).unwrap());
    }

    #[test]
    fn remove_by_path_handles_registered_missing_path() {
        let (temp, source) = init_source_repo();
        let outpost = temp.path().join("C");
        fs::create_dir_all(&outpost).expect("outpost dir");
        let canonical_outpost = fs::canonicalize(&outpost).expect("canonical outpost");

        let mut registry = source.registry_mut().expect("registry mut");
        registry
            .add(entry_at(&outpost, "local", 1))
            .expect("add path");
        fs::remove_dir(&outpost).expect("remove outpost dir");

        assert!(registry
            .remove_by_path(&canonical_outpost)
            .expect("remove missing registered path"));
        registry.save().expect("save");

        assert!(Registry::load(&source)
            .expect("reload")
            .entries()
            .is_empty());
    }

    #[test]
    fn load_malformed_json_returns_bad_registry() {
        let (_temp, source) = init_source_repo();
        fs::create_dir_all(source.registry_path().parent().unwrap()).expect("registry dir");
        fs::write(source.registry_path(), "{not json").expect("write bad json");

        let err = Registry::load(&source).expect_err("bad json should fail");
        assert!(matches!(
            err,
            OutpostError::BadRegistry { path, .. } if path == source.registry_path()
        ));
    }

    #[test]
    #[cfg(debug_assertions)]
    fn dropping_dirty_registry_mut_trips_debug_drop_guard() {
        let (temp, source) = init_source_repo();
        let outpost = temp.path().join("C");
        fs::create_dir_all(&outpost).expect("outpost dir");

        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut registry = source.registry_mut().expect("registry mut");
            registry
                .add(entry_at(&outpost, "local", 1))
                .expect("add entry");
        }));

        assert!(result.is_err());
    }

    #[test]
    #[cfg(debug_assertions)]
    fn failed_save_returns_error_without_drop_guard_panic() {
        let temp = tempfile::tempdir().expect("tempdir");
        let work_tree = temp.path().join("source");
        let git_dir = temp.path().join("git-file");
        let outpost = temp.path().join("C");
        fs::create_dir_all(&work_tree).expect("source dir");
        fs::write(&git_dir, "not a dir").expect("git file");
        fs::create_dir_all(&outpost).expect("outpost dir");
        let source =
            SourceRepo::from_storage_paths(&work_tree, &git_dir).expect("source repo storage");

        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut registry = source.registry_mut().expect("registry mut");
            registry
                .add(entry_at(&outpost, "local", 1))
                .expect("add entry");
            registry.save()
        }));

        let save_result = result.expect("save error should not panic");
        assert!(matches!(save_result, Err(OutpostError::IoAt { .. })));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn dropping_dirty_registry_mut_does_not_panic_in_release_builds() {
        let (temp, source) = init_source_repo();
        let outpost = temp.path().join("C");
        fs::create_dir_all(&outpost).expect("outpost dir");

        let mut registry = source.registry_mut().expect("registry mut");
        registry
            .add(entry_at(&outpost, "local", 1))
            .expect("add entry");
    }

    fn init_source_repo() -> (tempfile::TempDir, SourceRepo) {
        let temp = tempfile::tempdir().expect("tempdir");
        GitInvoker::at(temp.path())
            .run_check(["init", "--initial-branch=main"])
            .expect("init source");
        let source = SourceRepo::from_storage_paths(temp.path(), &temp.path().join(".git"))
            .expect("source repo");
        (temp, source)
    }

    fn entry_at(path: &Path, remote: &str, seconds: i64) -> RegistryEntry {
        RegistryEntry {
            path: path.to_path_buf(),
            created_at: Utc.timestamp_opt(seconds, 0).single().unwrap(),
            remote_name: RemoteName::parse(remote).expect("remote parses"),
            locked: false,
            lock_reason: None,
            locked_at: None,
        }
    }
}
