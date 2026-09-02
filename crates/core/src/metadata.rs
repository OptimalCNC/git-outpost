use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::source_repo::canonicalize_path;
use crate::{GitInvoker, OutpostError, OutpostResult, RemoteName};

const METADATA_VERSION: u32 = 1;

/// Validated reverse link from an outpost to its source repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub source_repo: PathBuf,
    pub remote_name: RemoteName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetadataState {
    Absent,
    Valid(Metadata),
    Invalid(String),
}

pub(crate) fn read(git_dir: &Path) -> OutpostResult<MetadataState> {
    let document_path = metadata_path(git_dir);
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
            return Ok(MetadataState::Invalid(source.to_string()));
        }
    };
    if file.version != METADATA_VERSION {
        return Ok(MetadataState::Invalid(format!(
            "unsupported metadata version {}",
            file.version
        )));
    }
    if !file.source_repo.is_absolute() {
        return Ok(MetadataState::Invalid(
            "source_repo must be an absolute path".to_owned(),
        ));
    }
    let remote_name = match RemoteName::parse(file.remote_name) {
        Ok(remote_name) => remote_name,
        Err(source) => return Ok(MetadataState::Invalid(source.to_string())),
    };

    Ok(MetadataState::Valid(Metadata {
        source_repo: file.source_repo,
        remote_name,
    }))
}

pub(crate) fn initialize(git: &GitInvoker, metadata: &Metadata) -> OutpostResult<()> {
    let git_dir_raw = git.run_capture(["rev-parse", "--git-dir"])?;
    let git_dir = resolve_git_dir(git.cwd(), &git_dir_raw)?;
    initialize_at_path(&metadata_path(&git_dir), metadata)
}

pub(crate) fn metadata_path(git_dir: &Path) -> PathBuf {
    git_dir.join("outpost").join("metadata.json")
}

fn initialize_at_path(path: &Path, metadata: &Metadata) -> OutpostResult<()> {
    let source_repo = canonical_source_for_write(&metadata.source_repo)?;
    let file = MetadataFile {
        version: METADATA_VERSION,
        source_repo,
        remote_name: metadata.remote_name.as_str().to_owned(),
    };
    let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "metadata path has no parent",
        ),
    })?;
    fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;

    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
            path: parent.to_path_buf(),
            source,
        })?;
    serde_json::to_writer_pretty(temp.as_file_mut(), &file).map_err(|source| {
        OutpostError::IoAt {
            path: path.to_path_buf(),
            source: std::io::Error::other(source),
        }
    })?;
    writeln!(temp.as_file_mut()).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    temp.persist_noclobber(path)
        .map_err(|source| OutpostError::IoAt {
            path: path.to_path_buf(),
            source: source.error,
        })?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataFile {
    version: u32,
    source_repo: PathBuf,
    remote_name: String,
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
    use super::*;

    #[test]
    fn metadata_file_rejects_unknown_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_repo(temp.path());
        let git_dir = fs::canonicalize(temp.path().join(".git")).expect("git dir");
        let path = metadata_path(&git_dir);
        fs::create_dir_all(path.parent().expect("state directory")).expect("state directory");
        fs::write(
            path,
            r#"{"version":1,"source_repo":"/source","remote_name":"local","extra":true}"#,
        )
        .expect("metadata");

        assert!(matches!(
            read(&git_dir).expect("read metadata"),
            MetadataState::Invalid(_)
        ));
    }

    #[test]
    fn metadata_initialization_is_no_clobber() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let outpost = temp.path().join("outpost");
        init_repo(&source);
        init_repo(&outpost);
        let git = GitInvoker::at(&outpost);
        let metadata = Metadata {
            source_repo: source,
            remote_name: RemoteName::parse("local").expect("remote parses"),
        };

        initialize(&git, &metadata).expect("initialize metadata");
        let error = initialize(&git, &metadata).expect_err("metadata must not be replaced");

        assert!(matches!(
            error,
            OutpostError::IoAt { path, source }
                if path == outpost.join(".git/outpost/metadata.json")
                    && source.kind() == std::io::ErrorKind::AlreadyExists
        ));
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo dir");
        GitInvoker::at(path)
            .run_check(["init", "--initial-branch=main"])
            .expect("init repo");
    }
}
