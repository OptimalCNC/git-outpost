use std::path::{Path, PathBuf};

use crate::registry::{Registry, RegistryMut};
use crate::OutpostResult;

pub struct SourceRepo {
    work_tree: PathBuf,
    git_dir: PathBuf,
}

impl SourceRepo {
    pub fn work_tree(&self) -> &Path {
        &self.work_tree
    }

    pub fn registry_path(&self) -> PathBuf {
        self.work_tree.join(".outpost").join("registry.json")
    }

    pub fn registry(&self) -> OutpostResult<Registry> {
        Registry::load(self)
    }

    pub fn registry_mut(&self) -> OutpostResult<RegistryMut<'_>> {
        RegistryMut::load(self)
    }

    pub(crate) fn local_exclude_path(&self) -> PathBuf {
        self.git_dir.join("info").join("exclude")
    }

    #[cfg(test)]
    pub(crate) fn from_storage_paths(work_tree: &Path, git_dir: &Path) -> OutpostResult<Self> {
        Ok(Self {
            work_tree: canonicalize_path(work_tree)?,
            git_dir: canonicalize_path(git_dir)?,
        })
    }
}

#[cfg(test)]
fn canonicalize_path(path: &Path) -> OutpostResult<PathBuf> {
    use crate::OutpostError;

    std::fs::canonicalize(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}
