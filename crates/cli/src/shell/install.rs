use std::env;
use std::path::{Path, PathBuf};

use outpost_core::{OutpostError, OutpostResult};

use crate::cli::ShellKind;

pub const INSTALL_START: &str = "# >>> git-outpost shell install >>>";
pub const INSTALL_END: &str = "# <<< git-outpost shell install <<<";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub shell: ShellKind,
    pub rc_file: PathBuf,
    pub script_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ShellInstallReport {
    pub shell: ShellKind,
    pub rc_file: PathBuf,
    pub script_file: PathBuf,
    pub changed: bool,
}

impl ShellKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
        }
    }

    fn default_rc_name(&self) -> &'static str {
        match self {
            ShellKind::Bash => ".bashrc",
            ShellKind::Zsh => ".zshrc",
        }
    }

    fn default_script_name(&self) -> &'static str {
        match self {
            ShellKind::Bash => "shell.bash",
            ShellKind::Zsh => "shell.zsh",
        }
    }
}

pub fn default_rc_file(shell: ShellKind) -> OutpostResult<PathBuf> {
    Ok(home_dir()?.join(shell.default_rc_name()))
}

pub fn default_script_file(shell: ShellKind) -> OutpostResult<PathBuf> {
    Ok(config_home()?
        .join("git-outpost")
        .join(shell.default_script_name()))
}

pub fn managed_source_block(shell: ShellKind, script_file: &Path) -> String {
    let quoted = shell_single_quote(script_file);
    format!(
        "{INSTALL_START}\n\
         # Managed by Git Outpost. Remove with: gop shell uninstall {}\n\
         # Sources the generated Git Outpost shell integration.\n\
         if [ -f {quoted} ]; then\n\
             . {quoted}\n\
         fi\n\
         {INSTALL_END}\n",
        shell.as_str()
    )
}

fn home_dir() -> OutpostResult<PathBuf> {
    non_empty_env_path("HOME").map_err(|source| OutpostError::IoAt {
        path: PathBuf::from("$HOME"),
        source,
    })
}

fn config_home() -> OutpostResult<PathBuf> {
    match env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        Some(value) => Ok(PathBuf::from(value)),
        None => Ok(home_dir()?.join(".config")),
    }
}

fn non_empty_env_path(name: &str) -> Result<PathBuf, std::io::Error> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("{name} is not set"))
        })
}

fn shell_single_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_source_block_quotes_script_path() {
        let block = managed_source_block(ShellKind::Bash, Path::new("/tmp/a dir/it's/gop.bash"));

        assert!(block.contains(INSTALL_START), "{block}");
        assert!(block.contains(INSTALL_END), "{block}");
        assert!(block.contains("gop shell uninstall bash"), "{block}");
        assert!(block.contains("'/tmp/a dir/it'\"'\"'s/gop.bash'"), "{block}");
    }
}
