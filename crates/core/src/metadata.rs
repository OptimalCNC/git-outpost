use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::source_repo::{canonicalize_path, read_optional_config};
use crate::state::OutpostStateStore;
use crate::{GitInvoker, OutpostError, OutpostResult, RemoteName};

const METADATA_VERSION: u32 = 1;
const LEGACY_MANAGED_KEY: &str = "outpost.managed";
const LEGACY_SOURCE_REPO_KEY: &str = "outpost.sourceRepo";
const LEGACY_REMOTE_NAME_KEY: &str = "outpost.remoteName";
const LEGACY_METADATA_KEYS: [&str; 3] = [
    LEGACY_MANAGED_KEY,
    LEGACY_SOURCE_REPO_KEY,
    LEGACY_REMOTE_NAME_KEY,
];

/// Legacy, diagnostic-only representation of the old local Git-config keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMetadata {
    pub managed: Option<bool>,
    pub source_repo: Option<PathBuf>,
    pub remote_name: Option<RemoteName>,
}

/// Validated reverse link from an outpost to its source repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub source_repo: PathBuf,
    pub remote_name: RemoteName,
}

/// Domain spelling used by the state Interface.
pub type OutpostMetadata = Metadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataProblems {
    pub path: PathBuf,
    pub reason: String,
    pub(crate) legacy: Option<RawMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataState {
    Absent,
    Valid(Metadata),
    Invalid(MetadataProblems),
}

impl MetadataProblems {
    pub(crate) fn as_error(&self) -> OutpostError {
        OutpostError::BadMetadata {
            outpost: self.path.clone(),
            reason: self.reason.clone(),
        }
    }
}

impl RawMetadata {
    pub fn read(git: &GitInvoker) -> OutpostResult<Self> {
        let managed = match read_optional_config(git, LEGACY_MANAGED_KEY)? {
            Some(value) => {
                Some(
                    parse_git_bool(&value).ok_or_else(|| OutpostError::BadMetadata {
                        outpost: git.cwd().to_path_buf(),
                        reason: format!("invalid outpost.managed value: {value}"),
                    })?,
                )
            }
            None => None,
        };
        let source_repo = read_optional_config(git, LEGACY_SOURCE_REPO_KEY)?.map(PathBuf::from);
        let remote_name = read_optional_config(git, LEGACY_REMOTE_NAME_KEY)?
            .map(RemoteName::parse)
            .transpose()?;

        Ok(Self {
            managed,
            source_repo,
            remote_name,
        })
    }
}

impl Metadata {
    pub fn from_raw(outpost: &Path, raw: RawMetadata) -> OutpostResult<Self> {
        if raw.managed != Some(true) {
            return Err(OutpostError::NotAnOutpost(outpost.to_path_buf()));
        }

        let source_repo = raw.source_repo.ok_or_else(|| OutpostError::BadMetadata {
            outpost: outpost.to_path_buf(),
            reason: "missing outpost.sourceRepo".to_owned(),
        })?;
        if !source_repo.is_absolute() {
            return Err(OutpostError::BadMetadata {
                outpost: outpost.to_path_buf(),
                reason: "source_repo must be an absolute path".to_owned(),
            });
        }
        let remote_name = raw.remote_name.ok_or_else(|| OutpostError::BadMetadata {
            outpost: outpost.to_path_buf(),
            reason: "missing outpost.remoteName".to_owned(),
        })?;

        Ok(Self {
            source_repo,
            remote_name,
        })
    }

    /// Compatibility façade for callers that already have a Git invoker.
    /// It atomically replaces the current Git-directory document and never
    /// writes legacy config keys.
    pub fn write(&self, git: &GitInvoker) -> OutpostResult<()> {
        current_store_at_git(git)?.replace_metadata(self)
    }
}

pub(crate) fn current_store_at_git(git: &GitInvoker) -> OutpostResult<GitDirOutpostStore> {
    let git_dir_raw = git.run_capture(["rev-parse", "--git-dir"])?;
    let git_dir = resolve_git_dir(git.cwd(), &git_dir_raw)?;
    Ok(GitDirOutpostStore::new(git.cwd().to_path_buf(), git_dir))
}

#[derive(Clone)]
pub(crate) struct GitDirOutpostStore {
    work_tree: PathBuf,
    git_dir: PathBuf,
}

#[derive(Clone)]
pub(crate) struct MigratingOutpostStore {
    current: GitDirOutpostStore,
    git: GitInvoker,
}

impl GitDirOutpostStore {
    pub(crate) fn new(work_tree: PathBuf, git_dir: PathBuf) -> Self {
        Self { work_tree, git_dir }
    }

    pub(crate) fn metadata_path(&self) -> PathBuf {
        self.git_dir.join("outpost").join("metadata.json")
    }

    fn read_current(&self) -> OutpostResult<MetadataState> {
        let document_path = self.metadata_path();
        let contents = match fs::read_to_string(&document_path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(MetadataState::Absent);
            }
            Err(source) => {
                return Err(OutpostError::IoAt {
                    path: document_path,
                    source,
                });
            }
        };

        let file = match serde_json::from_str::<MetadataFile>(&contents) {
            Ok(file) => file,
            Err(source) => {
                return Ok(MetadataState::Invalid(MetadataProblems {
                    path: self.work_tree.clone(),
                    reason: source.to_string(),
                    legacy: None,
                }));
            }
        };
        if file.version != METADATA_VERSION {
            return Ok(MetadataState::Invalid(MetadataProblems {
                path: self.work_tree.clone(),
                reason: format!("unsupported metadata version {}", file.version),
                legacy: None,
            }));
        }
        if !file.source_repo.is_absolute() {
            return Ok(MetadataState::Invalid(MetadataProblems {
                path: self.work_tree.clone(),
                reason: "source_repo must be an absolute path".to_owned(),
                legacy: None,
            }));
        }
        let remote_name = match RemoteName::parse(file.remote_name.clone()) {
            Ok(remote_name) => remote_name,
            Err(source) => {
                return Ok(MetadataState::Invalid(MetadataProblems {
                    path: self.work_tree.clone(),
                    reason: source.to_string(),
                    legacy: None,
                }));
            }
        };

        Ok(MetadataState::Valid(Metadata {
            source_repo: file.source_repo,
            remote_name,
        }))
    }

    fn write_current(&self, metadata: &Metadata) -> OutpostResult<()> {
        self.write_current_inner(metadata, true)
    }

    fn replace_metadata(&self, metadata: &Metadata) -> OutpostResult<()> {
        self.write_current_inner(metadata, false)
    }

    fn write_current_inner(&self, metadata: &Metadata, no_clobber: bool) -> OutpostResult<()> {
        let path = self.metadata_path();
        let source_repo = canonical_source_for_write(&metadata.source_repo)?;
        let file = MetadataFile {
            version: METADATA_VERSION,
            source_repo,
            remote_name: metadata.remote_name.as_str().to_owned(),
        };
        let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
            path: path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "metadata path has no parent",
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
            path: parent.to_path_buf(),
            source,
        })?;

        if no_clobber && path.exists() {
            return Err(OutpostError::IoAt {
                path,
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "metadata already exists",
                ),
            });
        }

        let mut temp =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
                path: parent.to_path_buf(),
                source,
            })?;
        serde_json::to_writer_pretty(temp.as_file_mut(), &file).map_err(|source| {
            OutpostError::IoAt {
                path: path.clone(),
                source: std::io::Error::other(source),
            }
        })?;
        writeln!(temp.as_file_mut()).map_err(|source| OutpostError::IoAt {
            path: path.clone(),
            source,
        })?;
        let result = if no_clobber {
            temp.persist_noclobber(&path)
        } else {
            temp.persist(&path)
        };
        result.map_err(|source| OutpostError::IoAt {
            path,
            source: source.error,
        })?;
        Ok(())
    }
}

impl OutpostStateStore for GitDirOutpostStore {
    fn read_metadata(&self) -> OutpostResult<MetadataState> {
        self.read_current()
    }

    fn initialize_metadata(&self, metadata: &Metadata) -> OutpostResult<()> {
        self.write_current(metadata)
    }
}

impl MigratingOutpostStore {
    pub(crate) fn new(work_tree: PathBuf, git_dir: PathBuf, git: GitInvoker) -> Self {
        Self {
            current: GitDirOutpostStore::new(work_tree, git_dir),
            git,
        }
    }

    pub(crate) fn metadata_path(&self) -> PathBuf {
        self.current.metadata_path()
    }

    fn read_legacy(&self) -> OutpostResult<MetadataState> {
        let path = self.current.work_tree.clone();
        let managed_value = read_optional_config(&self.git, LEGACY_MANAGED_KEY)?;
        let managed = match managed_value.as_deref() {
            Some(value) => match parse_git_bool(value) {
                Some(value) => Some(value),
                None => {
                    return Ok(MetadataState::Invalid(MetadataProblems {
                        path,
                        reason: format!("invalid outpost.managed value: {value}"),
                        legacy: None,
                    }));
                }
            },
            None => None,
        };
        if managed != Some(true) {
            return Ok(MetadataState::Absent);
        }

        let source_value = read_optional_config(&self.git, LEGACY_SOURCE_REPO_KEY)?;
        let remote_value = read_optional_config(&self.git, LEGACY_REMOTE_NAME_KEY)?;
        let raw_remote = match remote_value {
            Some(value) => match RemoteName::parse(value) {
                Ok(value) => Some(value),
                Err(error) => {
                    return Ok(MetadataState::Invalid(MetadataProblems {
                        path,
                        reason: error.to_string(),
                        legacy: None,
                    }));
                }
            },
            None => None,
        };
        let raw = RawMetadata {
            managed,
            source_repo: source_value.map(PathBuf::from),
            remote_name: raw_remote,
        };

        if raw
            .source_repo
            .as_ref()
            .is_some_and(|source_repo| !source_repo.is_absolute())
        {
            return Ok(MetadataState::Invalid(MetadataProblems {
                path: path.clone(),
                reason: "source_repo must be an absolute path".to_owned(),
                legacy: None,
            }));
        }

        let metadata = match Metadata::from_raw(&path, raw.clone()) {
            Ok(metadata) => metadata,
            Err(error) => {
                return Ok(MetadataState::Invalid(MetadataProblems {
                    path,
                    reason: error.to_string(),
                    legacy: Some(raw),
                }));
            }
        };
        let metadata = Metadata {
            source_repo: canonical_recorded_source(&metadata.source_repo)?,
            remote_name: metadata.remote_name,
        };
        match self.current.initialize_metadata(&metadata) {
            Ok(()) => {}
            Err(OutpostError::IoAt { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                match self.current.read_metadata()? {
                    MetadataState::Valid(existing) if existing == metadata => {}
                    MetadataState::Valid(_) => {
                        return Err(OutpostError::BadMetadata {
                            outpost: self.current.work_tree.clone(),
                            reason: "conflicting concurrent metadata migration".to_owned(),
                        });
                    }
                    MetadataState::Invalid(problem) => return Err(problem.as_error()),
                    MetadataState::Absent => {
                        return Err(OutpostError::IoAt {
                            path: self.metadata_path(),
                            source: std::io::Error::other("metadata disappeared during migration"),
                        });
                    }
                }
            }
            Err(error) => return Err(error),
        }
        let value = match self.current.read_metadata()? {
            MetadataState::Valid(value) if value == metadata => value,
            MetadataState::Valid(_) => {
                return Err(OutpostError::BadMetadata {
                    outpost: self.current.work_tree.clone(),
                    reason: "metadata changed during migration".to_owned(),
                });
            }
            MetadataState::Invalid(problem) => return Err(problem.as_error()),
            MetadataState::Absent => {
                return Err(OutpostError::IoAt {
                    path: self.metadata_path(),
                    source: std::io::Error::other("metadata missing after migration"),
                });
            }
        };
        self.remove_legacy()?;
        Ok(MetadataState::Valid(value))
    }

    fn remove_legacy(&self) -> OutpostResult<()> {
        for key in LEGACY_METADATA_KEYS {
            match self
                .git
                .run_check(["config", "--local", "--unset-all", key])
            {
                Ok(()) | Err(OutpostError::GitFailed { code: 5, .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl OutpostStateStore for MigratingOutpostStore {
    fn read_metadata(&self) -> OutpostResult<MetadataState> {
        match self.current.read_metadata()? {
            MetadataState::Absent => self.read_legacy(),
            state @ MetadataState::Valid(_) => {
                self.remove_legacy()?;
                Ok(state)
            }
            state @ MetadataState::Invalid(_) => Ok(state),
        }
    }

    fn initialize_metadata(&self, metadata: &Metadata) -> OutpostResult<()> {
        self.current.initialize_metadata(metadata)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataFile {
    version: u32,
    source_repo: PathBuf,
    remote_name: String,
}

fn parse_git_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn canonical_source_for_write(path: &Path) -> OutpostResult<PathBuf> {
    if path.is_absolute() {
        match fs::canonicalize(path) {
            Ok(path) => Ok(path),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
            Err(source) => Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source,
            }),
        }
    } else {
        Err(OutpostError::BadMetadata {
            outpost: path.to_path_buf(),
            reason: "source_repo must be an absolute path".to_owned(),
        })
    }
}

fn canonical_recorded_source(path: &Path) -> OutpostResult<PathBuf> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn resolve_git_dir(start: &Path, value: &str) -> OutpostResult<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        canonicalize_path(&path)
    } else {
        canonicalize_path(&start.join(path))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs;

    use super::*;

    #[test]
    fn raw_metadata_read_ignores_global_outpost_managed_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        let global = temp.path().join("global.gitconfig");
        init_repo(&repo);
        fs::write(&global, "[outpost]\n\tmanaged = true\n").expect("write global config");

        let env = BTreeMap::from([(
            OsString::from("GIT_CONFIG_GLOBAL"),
            global.as_os_str().to_os_string(),
        )]);
        let git = env.iter().fold(GitInvoker::at(&repo), |git, (key, val)| {
            git.with_env(key.clone(), val.clone())
        });

        let raw = RawMetadata::read(&git).expect("read raw metadata");
        assert_eq!(raw.managed, None);
    }

    #[test]
    fn raw_metadata_on_non_managed_repo_promotes_to_not_an_outpost() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let raw = RawMetadata::read(&GitInvoker::at(temp.path())).expect("read raw metadata");

        assert_eq!(raw.managed, None);
        assert!(matches!(
            Metadata::from_raw(temp.path(), raw),
            Err(OutpostError::NotAnOutpost(path)) if path == temp.path()
        ));
    }

    #[test]
    fn metadata_file_rejects_unknown_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let git_dir = fs::canonicalize(temp.path().join(".git")).expect("git dir");
        let store = GitDirOutpostStore::new(temp.path().to_path_buf(), git_dir);
        fs::create_dir_all(store.metadata_path().parent().unwrap()).expect("state dir");
        fs::write(
            store.metadata_path(),
            r#"{"version":1,"source_repo":"/source","remote_name":"local","extra":true}"#,
        )
        .expect("metadata");

        assert!(matches!(
            store.read_metadata().expect("read metadata"),
            MetadataState::Invalid(_)
        ));
    }

    #[test]
    fn compatibility_write_replaces_current_metadata_document() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_a = temp.path().join("source-a");
        let source_b = temp.path().join("source-b");
        let outpost = temp.path().join("outpost");
        init_repo(&source_a);
        init_repo(&source_b);
        init_repo(&outpost);
        let git = GitInvoker::at(&outpost);

        Metadata {
            source_repo: source_a,
            remote_name: RemoteName::parse("local").expect("remote parses"),
        }
        .write(&git)
        .expect("first metadata write");
        Metadata {
            source_repo: source_b.clone(),
            remote_name: RemoteName::parse("upstream").expect("remote parses"),
        }
        .write(&git)
        .expect("replacement metadata write");

        let git_dir = fs::canonicalize(outpost.join(".git")).expect("git dir");
        let store = GitDirOutpostStore::new(outpost, git_dir);
        assert_eq!(
            store.read_metadata().expect("read replacement"),
            MetadataState::Valid(Metadata {
                source_repo: fs::canonicalize(source_b).expect("canonical source"),
                remote_name: RemoteName::parse("upstream").expect("remote parses"),
            })
        );
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo dir");
        GitInvoker::at(path)
            .run_check(["init", "--initial-branch=main"])
            .expect("init repo");
    }
}
