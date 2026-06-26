use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::Error as _;
use serde::{Deserialize, Serialize};

use crate::registry::ensure_local_ignore;
use crate::{OutpostError, OutpostResult, SourceRepo};

const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKey {
    OutpostContainer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    OutpostContainer(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: ConfigKey,
    pub value: ConfigValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigShowEntry {
    pub key: ConfigKey,
    pub value: Option<ConfigValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigShow {
    pub storage_path: PathBuf,
    pub entries: Vec<ConfigShowEntry>,
}

#[derive(Clone)]
pub struct ConfigStore<'src> {
    source: &'src SourceRepo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceConfig {
    outpost_container: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    version: u32,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_path",
        skip_serializing_if = "Option::is_none"
    )]
    outpost_container: Option<PathBuf>,
}

impl ConfigKey {
    pub const ALL: [Self; 1] = [Self::OutpostContainer];

    pub fn parse(value: &str) -> OutpostResult<Self> {
        match value {
            "outpost-container" => Ok(Self::OutpostContainer),
            other => Err(OutpostError::UnknownConfigKey {
                key: other.to_owned(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OutpostContainer => "outpost-container",
        }
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ConfigValue {
    pub fn key(&self) -> ConfigKey {
        match self {
            Self::OutpostContainer(_) => ConfigKey::OutpostContainer,
        }
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutpostContainer(path) => write!(f, "{}", path.display()),
        }
    }
}

impl<'src> ConfigStore<'src> {
    pub(crate) fn new(source: &'src SourceRepo) -> Self {
        Self { source }
    }

    pub fn storage_path(&self) -> PathBuf {
        self.source.config_path()
    }

    pub fn get(&self, key: ConfigKey) -> OutpostResult<Option<ConfigValue>> {
        Ok(self.load()?.get(key))
    }

    pub fn set(&self, key: ConfigKey, value: ConfigValue) -> OutpostResult<ConfigValue> {
        let mut config = self.load()?;
        let value = match (key, value) {
            (ConfigKey::OutpostContainer, ConfigValue::OutpostContainer(path)) => {
                ConfigValue::OutpostContainer(validated_container_for_set(
                    ConfigKey::OutpostContainer,
                    &path,
                )?)
            }
        };
        config.set(value.clone());
        self.save(&config)?;
        Ok(value)
    }

    pub fn unset(&self, key: ConfigKey) -> OutpostResult<()> {
        let mut config = self.load()?;
        config.unset(key);
        self.save(&config)
    }

    pub fn list(&self) -> OutpostResult<Vec<ConfigEntry>> {
        Ok(self.load()?.list())
    }

    pub fn show(&self) -> OutpostResult<ConfigShow> {
        let config = self.load()?;
        Ok(ConfigShow {
            storage_path: self.storage_path(),
            entries: ConfigKey::ALL
                .iter()
                .map(|key| ConfigShowEntry {
                    key: *key,
                    value: config.get(*key),
                })
                .collect(),
        })
    }

    fn load(&self) -> OutpostResult<SourceConfig> {
        let path = self.storage_path();
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SourceConfig::empty());
            }
            Err(source) => {
                return Err(OutpostError::IoAt { path, source });
            }
        };

        let file = serde_json::from_str::<ConfigFile>(&contents).map_err(|source| {
            OutpostError::BadConfig {
                path: path.clone(),
                reason: source.to_string(),
            }
        })?;
        if file.version != CONFIG_VERSION {
            return Err(OutpostError::BadConfig {
                path,
                reason: format!("unsupported config version {}", file.version),
            });
        }

        let outpost_container = file
            .outpost_container
            .map(|value| validated_container_for_storage(&path, ConfigKey::OutpostContainer, value))
            .transpose()?;

        Ok(SourceConfig { outpost_container })
    }

    fn save(&self, config: &SourceConfig) -> OutpostResult<()> {
        let path = self.storage_path();
        let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
            path: path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "config path has no parent",
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
            path: parent.to_path_buf(),
            source,
        })?;
        ensure_local_ignore(&self.source.local_exclude_path())?;

        let file = ConfigFile::from_config(config);
        let mut temp =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
                path: parent.to_path_buf(),
                source,
            })?;
        serde_json::to_writer_pretty(temp.as_file_mut(), &file).map_err(|source| {
            OutpostError::IoAt {
                path: path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::Other, source),
            }
        })?;
        use std::io::Write;
        writeln!(temp.as_file_mut()).map_err(|source| OutpostError::IoAt {
            path: path.clone(),
            source,
        })?;
        temp.persist(&path).map_err(|source| OutpostError::IoAt {
            path,
            source: source.error,
        })?;

        Ok(())
    }
}

impl SourceConfig {
    fn empty() -> Self {
        Self {
            outpost_container: None,
        }
    }

    fn get(&self, key: ConfigKey) -> Option<ConfigValue> {
        match key {
            ConfigKey::OutpostContainer => self
                .outpost_container
                .clone()
                .map(ConfigValue::OutpostContainer),
        }
    }

    fn set(&mut self, value: ConfigValue) {
        match value {
            ConfigValue::OutpostContainer(path) => self.outpost_container = Some(path),
        }
    }

    fn unset(&mut self, key: ConfigKey) {
        match key {
            ConfigKey::OutpostContainer => self.outpost_container = None,
        }
    }

    fn list(&self) -> Vec<ConfigEntry> {
        ConfigKey::ALL
            .iter()
            .filter_map(|key| self.get(*key).map(|value| ConfigEntry { key: *key, value }))
            .collect()
    }
}

impl ConfigFile {
    fn from_config(config: &SourceConfig) -> Self {
        Self {
            version: CONFIG_VERSION,
            outpost_container: config.outpost_container.clone(),
        }
    }
}

fn validated_container_for_set(key: ConfigKey, path: &Path) -> OutpostResult<PathBuf> {
    canonical_existing_directory(path).map_err(|reason| OutpostError::InvalidConfigValue {
        key: key.as_str().to_owned(),
        value: path.to_path_buf(),
        reason,
    })
}

fn validated_container_for_storage(
    config_path: &Path,
    key: ConfigKey,
    value: PathBuf,
) -> OutpostResult<PathBuf> {
    if !value.is_absolute() {
        return Err(OutpostError::BadConfig {
            path: config_path.to_path_buf(),
            reason: format!("{} must be an absolute path", key.as_str()),
        });
    }
    canonical_existing_directory(&value).map_err(|reason| OutpostError::BadConfig {
        path: config_path.to_path_buf(),
        reason: format!("invalid {}: {reason}", key.as_str()),
    })
}

fn canonical_existing_directory(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|source| source.to_string())?;
    let metadata = fs::metadata(&canonical).map_err(|source| source.to_string())?;
    if !metadata.is_dir() {
        return Err("path is not an existing directory".to_owned());
    }
    Ok(canonical)
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
