use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{self, SourceConfig};
use crate::registry::{self, Registry};
use crate::source_repo::SourceRepo;
use crate::state::{SourceStateStore, Stored};
use crate::{OutpostError, OutpostResult};

pub(crate) struct GitDirSourceStore<'src> {
    source: &'src SourceRepo,
}

pub(crate) struct MigratingSourceStore<'src> {
    current: GitDirSourceStore<'src>,
}

impl<'src> GitDirSourceStore<'src> {
    pub(crate) fn new(source: &'src SourceRepo) -> Self {
        Self { source }
    }

    fn read_config(&self) -> OutpostResult<Stored<SourceConfig>> {
        config::read_file(&self.source.config_path())
    }

    fn write_config(&self, config: &SourceConfig) -> OutpostResult<()> {
        config::write_file(&self.source.config_path(), config)
    }

    fn write_config_noclobber(&self, config: &SourceConfig) -> OutpostResult<()> {
        config::write_file_noclobber(&self.source.config_path(), config)
    }

    fn read_registry(&self) -> OutpostResult<Stored<Registry>> {
        registry::Registry::read_file(
            &self.source.registry_path(),
            self.source.local_exclude_path(),
        )
    }

    fn write_registry(&self, registry: &Registry) -> OutpostResult<()> {
        registry::Registry::write_file(&self.source.registry_path(), registry)
    }

    fn write_registry_noclobber(&self, registry: &Registry) -> OutpostResult<()> {
        registry::Registry::write_file_noclobber(&self.source.registry_path(), registry)
    }
}

impl<'src> MigratingSourceStore<'src> {
    pub(crate) fn new(source: &'src SourceRepo) -> Self {
        Self {
            current: GitDirSourceStore::new(source),
        }
    }

    fn legacy_config_path(&self) -> PathBuf {
        self.current
            .source
            .work_tree()
            .join(".outpost")
            .join("config.json")
    }

    fn legacy_registry_path(&self) -> PathBuf {
        self.current
            .source
            .work_tree()
            .join(".outpost")
            .join("registry.json")
    }

    fn migrate_config(&self) -> OutpostResult<Stored<SourceConfig>> {
        let legacy = config::read_file(&self.legacy_config_path())?;
        let Stored::Present(value) = legacy else {
            return Ok(Stored::Absent);
        };
        match self.current.write_config_noclobber(&value) {
            Ok(()) => {}
            Err(OutpostError::IoAt { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                match self.current.read_config()? {
                    Stored::Present(actual) if actual == value => {}
                    Stored::Present(_) => {
                        return Err(OutpostError::BadConfig {
                            path: self.current.source.config_path(),
                            reason: "conflicting concurrent config migration".to_owned(),
                        });
                    }
                    Stored::Absent => {
                        return Err(OutpostError::IoAt {
                            path: self.current.source.config_path(),
                            source: std::io::Error::other("config disappeared during migration"),
                        });
                    }
                }
            }
            Err(error) => return Err(error),
        }
        let actual = match self.current.read_config()? {
            Stored::Present(actual) if actual == value => actual,
            Stored::Present(_) => {
                return Err(OutpostError::BadConfig {
                    path: self.current.source.config_path(),
                    reason: "config changed during migration".to_owned(),
                });
            }
            Stored::Absent => {
                return Err(OutpostError::IoAt {
                    path: self.current.source.config_path(),
                    source: std::io::Error::other("config missing after migration"),
                });
            }
        };
        remove_legacy_file(&self.legacy_config_path())?;
        Ok(Stored::Present(actual))
    }

    fn migrate_registry(&self) -> OutpostResult<Stored<Registry>> {
        let legacy = registry::Registry::read_file(
            &self.legacy_registry_path(),
            self.current.source.local_exclude_path(),
        )?;
        let Stored::Present(value) = legacy else {
            return Ok(Stored::Absent);
        };
        match self.current.write_registry_noclobber(&value) {
            Ok(()) => {}
            Err(OutpostError::IoAt { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                match self.current.read_registry()? {
                    Stored::Present(actual) if actual.same_contents(&value) => {}
                    Stored::Present(_) => {
                        return Err(OutpostError::BadRegistry {
                            path: self.current.source.registry_path(),
                            reason: "conflicting concurrent registry migration".to_owned(),
                        });
                    }
                    Stored::Absent => {
                        return Err(OutpostError::IoAt {
                            path: self.current.source.registry_path(),
                            source: std::io::Error::other("registry disappeared during migration"),
                        });
                    }
                }
            }
            Err(error) => return Err(error),
        }
        let actual = match self.current.read_registry()? {
            Stored::Present(actual) if actual.same_contents(&value) => actual,
            Stored::Present(_) => {
                return Err(OutpostError::BadRegistry {
                    path: self.current.source.registry_path(),
                    reason: "registry changed during migration".to_owned(),
                });
            }
            Stored::Absent => {
                return Err(OutpostError::IoAt {
                    path: self.current.source.registry_path(),
                    source: std::io::Error::other("registry missing after migration"),
                });
            }
        };
        remove_legacy_file(&self.legacy_registry_path())?;
        Ok(Stored::Present(actual))
    }
}

impl<'src> SourceStateStore for GitDirSourceStore<'src> {
    fn read_config(&self) -> OutpostResult<Stored<SourceConfig>> {
        GitDirSourceStore::read_config(self)
    }

    fn write_config(&self, config: &SourceConfig) -> OutpostResult<()> {
        GitDirSourceStore::write_config(self, config)
    }

    fn read_registry(&self) -> OutpostResult<Stored<Registry>> {
        GitDirSourceStore::read_registry(self)
    }

    fn write_registry(&self, registry: &Registry) -> OutpostResult<()> {
        GitDirSourceStore::write_registry(self, registry)
    }
}

impl<'src> SourceStateStore for MigratingSourceStore<'src> {
    fn read_config(&self) -> OutpostResult<Stored<SourceConfig>> {
        match self.current.read_config()? {
            Stored::Absent => self.migrate_config(),
            state @ Stored::Present(_) => {
                remove_legacy_file(&self.legacy_config_path())?;
                Ok(state)
            }
        }
    }

    fn write_config(&self, config: &SourceConfig) -> OutpostResult<()> {
        self.current.write_config(config)
    }

    fn read_registry(&self) -> OutpostResult<Stored<Registry>> {
        match self.current.read_registry()? {
            Stored::Absent => self.migrate_registry(),
            state @ Stored::Present(_) => {
                remove_legacy_file(&self.legacy_registry_path())?;
                Ok(state)
            }
        }
    }

    fn write_registry(&self, registry: &Registry) -> OutpostResult<()> {
        self.current.write_registry(registry)
    }
}

fn remove_legacy_file(path: &Path) -> OutpostResult<()> {
    // Released Git Outpost created `.outpost` as a real directory containing
    // regular JSON files. Refuse any other static shape before unlinking. This
    // local migration does not defend against hostile concurrent path swaps.
    let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "legacy state path has no parent",
        ),
    })?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "legacy state parent is not a real directory",
                ),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source,
            });
        }
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "legacy state path is not a regular file",
                ),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(OutpostError::IoAt {
                path: path.to_path_buf(),
                source,
            });
        }
    }

    fs::remove_file(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}
