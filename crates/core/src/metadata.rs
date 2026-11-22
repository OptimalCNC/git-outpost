use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::source_repo::canonicalize_path;
use crate::state_file::{Publish, publish_json};
use crate::{GitInvoker, OutpostError, OutpostResult, RemoteName, SourceRepo};

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

pub(crate) fn metadata_path(git_dir: &Path) -> PathBuf {
    git_dir.join("outpost").join("metadata.json")
}

pub(crate) fn initialize(
    git: &GitInvoker,
    source: &SourceRepo,
    remote_name: &RemoteName,
) -> OutpostResult<()> {
    let git_dir = git.run_capture_path(["rev-parse", "--git-dir"])?;
    let state_dir = canonicalize_path(&git.cwd().join(git_dir))?.join("outpost");
    let path = state_dir.join("metadata.json");
    // SourceRepo resolved its canonical work tree when it was opened.
    let file = MetadataFile {
        version: METADATA_VERSION,
        source_repo: source.work_tree().to_owned(),
        remote_name: remote_name.as_str().to_owned(),
    };
    publish_json(&state_dir, &path, &file, Publish::NoClobber)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataFile {
    version: u32,
    source_repo: PathBuf,
    remote_name: String,
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
        let source = SourceRepo::at(&source).expect("source repo");
        let expected_path = fs::canonicalize(outpost.join(".git"))
            .expect("canonical git dir")
            .join("outpost/metadata.json");
        let remote = RemoteName::parse("local").expect("remote parses");
        initialize(&git, &source, &remote).expect("initialize metadata");
        let before = fs::read(&expected_path).expect("original metadata");

        let other_remote = RemoteName::parse("origin").expect("remote parses");
        let error =
            initialize(&git, &source, &other_remote).expect_err("metadata must not be replaced");

        let OutpostError::IoAt { path, source } = error else {
            panic!("expected path-qualified I/O error");
        };
        assert_eq!(path, expected_path);
        assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).expect("metadata after refusal"), before);
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo dir");
        GitInvoker::at(path)
            .run_check(["init", "--initial-branch=main"])
            .expect("init repo");
    }
}
