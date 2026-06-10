use std::path::{Path, PathBuf};

use crate::{GitInvoker, OutpostError, OutpostResult, RemoteName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMetadata {
    pub managed: Option<bool>,
    pub source_repo: Option<PathBuf>,
    pub remote_name: Option<RemoteName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub source_repo: PathBuf,
    pub remote_name: RemoteName,
}

impl RawMetadata {
    pub fn read(git: &GitInvoker) -> OutpostResult<Self> {
        let managed = match read_optional_config(git, "outpost.managed")? {
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
        let source_repo = read_optional_config(git, "outpost.sourceRepo")?.map(PathBuf::from);
        let remote_name = read_optional_config(git, "outpost.remoteName")?
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
        let remote_name = raw.remote_name.ok_or_else(|| OutpostError::BadMetadata {
            outpost: outpost.to_path_buf(),
            reason: "missing outpost.remoteName".to_owned(),
        })?;

        Ok(Self {
            source_repo,
            remote_name,
        })
    }

    pub fn write(&self, git: &GitInvoker) -> OutpostResult<()> {
        let source_repo =
            std::fs::canonicalize(&self.source_repo).map_err(|source| OutpostError::IoAt {
                path: self.source_repo.clone(),
                source,
            })?;
        let source_repo = source_repo.to_string_lossy().into_owned();

        git.run_check(["config", "--local", "outpost.managed", "true"])?;
        git.run_check(["config", "--local", "outpost.sourceRepo", &source_repo])?;
        git.run_check([
            "config",
            "--local",
            "outpost.remoteName",
            self.remote_name.as_str(),
        ])?;
        Ok(())
    }
}

fn read_optional_config(git: &GitInvoker, key: &str) -> OutpostResult<Option<String>> {
    if git.run_status(["config", "--local", "--get", key])? {
        git.run_capture(["config", "--local", "--get", key])
            .map(Some)
    } else {
        Ok(None)
    }
}

fn parse_git_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn metadata_write_sets_local_outpost_config_keys() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outpost = temp.path().join("outpost");
        let source = temp.path().join("source");
        init_repo(&outpost);
        init_repo(&source);

        let metadata = Metadata {
            source_repo: source.clone(),
            remote_name: RemoteName::parse("local").expect("remote parses"),
        };
        let git = GitInvoker::at(&outpost);

        metadata.write(&git).expect("metadata writes");

        assert_eq!(
            git.run_capture(["config", "--local", "--get", "outpost.managed"])
                .expect("managed key"),
            "true"
        );
        assert_eq!(
            git.run_capture(["config", "--local", "--get", "outpost.sourceRepo"])
                .expect("source key"),
            fs::canonicalize(&source)
                .expect("canonical source")
                .to_string_lossy()
        );
        assert_eq!(
            git.run_capture(["config", "--local", "--get", "outpost.remoteName"])
                .expect("remote key"),
            "local"
        );
        assert!(
            !git.run_status(["config", "--local", "--get", "outpost.id"])
                .expect("id key absent")
        );
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

        let raw_false = RawMetadata {
            managed: Some(false),
            source_repo: None,
            remote_name: None,
        };
        assert!(matches!(
            Metadata::from_raw(temp.path(), raw_false),
            Err(OutpostError::NotAnOutpost(path)) if path == temp.path()
        ));
    }

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

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo dir");
        GitInvoker::at(path)
            .run_check(["init", "--initial-branch=main"])
            .expect("init repo");
    }
}
