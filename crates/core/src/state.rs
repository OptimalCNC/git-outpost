use std::path::{Path, PathBuf};

use crate::OutpostResult;
use crate::config::SourceConfig;
use crate::metadata::{MetadataState, OutpostMetadata};
use crate::registry::Registry;

/// A document read that keeps absence distinct from a present empty value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored<T> {
    Absent,
    Present(T),
}

/// The complete typed state surface owned by a source repository.
pub trait SourceStateStore {
    fn read_config(&self) -> OutpostResult<Stored<SourceConfig>>;
    fn write_config(&self, config: &SourceConfig) -> OutpostResult<()>;
    fn read_registry(&self) -> OutpostResult<Stored<Registry>>;
    fn write_registry(&self, registry: &Registry) -> OutpostResult<()>;
}

/// The complete typed state surface owned by an outpost repository.
pub trait OutpostStateStore {
    fn read_metadata(&self) -> OutpostResult<MetadataState>;
    fn initialize_metadata(&self, metadata: &OutpostMetadata) -> OutpostResult<()>;
}

/// Canonical location facts returned by Git discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLocation {
    work_tree: PathBuf,
    git_dir: PathBuf,
}

impl RepositoryLocation {
    pub fn new(work_tree: PathBuf, git_dir: PathBuf) -> Self {
        Self { work_tree, git_dir }
    }

    pub fn work_tree(&self) -> &Path {
        &self.work_tree
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub(crate) fn state_dir(&self) -> PathBuf {
        self.git_dir.join("outpost")
    }

    pub(crate) fn state_path(&self, name: &str) -> PathBuf {
        self.state_dir().join(name)
    }
}
