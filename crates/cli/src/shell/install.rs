use std::env;
use std::fs;
use std::io::Write;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockRange {
    Absent,
    Present { start: usize, end: usize },
}

fn managed_block_range(path: &Path, contents: &str) -> OutpostResult<BlockRange> {
    let starts: Vec<_> = contents
        .match_indices(INSTALL_START)
        .map(|(idx, _)| idx)
        .collect();
    let ends: Vec<_> = contents
        .match_indices(INSTALL_END)
        .map(|(idx, _)| idx)
        .collect();

    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(BlockRange::Absent),
        ([_], []) => Err(invalid_marker(
            path,
            "missing git-outpost shell install end marker",
        )),
        ([], [_]) => Err(invalid_marker(
            path,
            "missing git-outpost shell install start marker",
        )),
        ([start], [end]) if start < end => {
            let mut end = end + INSTALL_END.len();
            if contents[end..].starts_with("\r\n") {
                end += 2;
            } else if contents[end..].starts_with('\n') {
                end += 1;
            }
            Ok(BlockRange::Present { start: *start, end })
        }
        ([_], [_]) => Err(invalid_marker(
            path,
            "git-outpost shell install end marker appears before start marker",
        )),
        _ => Err(invalid_marker(
            path,
            "multiple git-outpost shell install blocks found",
        )),
    }
}

fn invalid_marker(path: &Path, message: &'static str) -> OutpostError {
    OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    }
}

fn install_contents(path: &Path, contents: &str, block: &str) -> OutpostResult<String> {
    match managed_block_range(path, contents)? {
        BlockRange::Absent => {
            let mut next = String::from(contents);
            if !next.is_empty() && !next.ends_with('\n') {
                next.push('\n');
            }
            if !next.is_empty() {
                next.push('\n');
            }
            next.push_str(block);
            Ok(next)
        }
        BlockRange::Present { start, end } => {
            let mut next = String::new();
            next.push_str(&contents[..start]);
            next.push_str(block);
            next.push_str(&contents[end..]);
            Ok(next)
        }
    }
}

fn uninstall_contents(path: &Path, contents: &str) -> OutpostResult<(String, bool)> {
    match managed_block_range(path, contents)? {
        BlockRange::Absent => Ok((contents.to_owned(), false)),
        BlockRange::Present { start, end } => {
            let mut next = String::new();
            next.push_str(&contents[..start]);
            next.push_str(&contents[end..]);
            Ok((next, true))
        }
    }
}

fn read_optional(path: &Path) -> OutpostResult<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_text(path: &Path, contents: &str) -> OutpostResult<()> {
    let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
    })?;
    fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;
    temp.write_all(contents.as_bytes())
        .map_err(|source| OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        })?;
    temp.persist(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: source.error,
    })?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> OutpostResult<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn install(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    let rc_before = read_optional(&options.rc_file)?;
    let block = managed_source_block(options.shell, &options.script_file);
    let rc_after = install_contents(&options.rc_file, &rc_before, &block)?;
    let script = super::init_script(Some(options.shell));

    write_text(&options.script_file, script)?;
    write_text(&options.rc_file, &rc_after)?;

    Ok(ShellInstallReport {
        shell: options.shell,
        rc_file: options.rc_file,
        script_file: options.script_file,
        changed: true,
    })
}

pub fn uninstall(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    let rc_before = read_optional(&options.rc_file)?;
    let (rc_after, rc_changed) = uninstall_contents(&options.rc_file, &rc_before)?;
    if rc_changed {
        write_text(&options.rc_file, &rc_after)?;
    }
    let script_removed = remove_file_if_exists(&options.script_file)?;

    Ok(ShellInstallReport {
        shell: options.shell,
        rc_file: options.rc_file,
        script_file: options.script_file,
        changed: rc_changed || script_removed,
    })
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

#[cfg(test)]
mod operation_tests {
    use super::*;

    fn paths(root: &Path) -> InstallOptions {
        InstallOptions {
            shell: ShellKind::Bash,
            rc_file: root.join(".bashrc"),
            script_file: root.join("git-outpost").join("shell.bash"),
        }
    }

    #[test]
    fn install_appends_then_replaces_managed_block() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        std::fs::write(&options.rc_file, "# user content\n").expect("write rc");

        let first = install(options.clone()).expect("install");
        let first_rc = std::fs::read_to_string(&options.rc_file).expect("read rc");
        let first_script = std::fs::read_to_string(&options.script_file).expect("read script");

        let second = install(options.clone()).expect("install again");
        let second_rc = std::fs::read_to_string(&options.rc_file).expect("read rc");
        let second_script = std::fs::read_to_string(&options.script_file).expect("read script");

        assert!(first.changed);
        assert!(second.changed);
        assert_eq!(first.rc_file, options.rc_file);
        assert_eq!(first.script_file, options.script_file);
        assert_eq!(first_rc, second_rc);
        assert_eq!(first_script, second_script);
        assert_eq!(second_rc.matches(INSTALL_START).count(), 1, "{second_rc}");
        assert!(second_script.contains("gop()"), "{second_script}");
    }

    #[test]
    fn uninstall_removes_managed_block_and_script_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        std::fs::write(
            &options.rc_file,
            format!(
                "# manual init remains\n{}\n# after\n",
                super::super::init_script(Some(ShellKind::Bash))
            ),
        )
        .expect("write rc");
        install(options.clone()).expect("install");

        let report = uninstall(options.clone()).expect("uninstall");
        let rc = std::fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(report.changed);
        assert!(!options.script_file.exists());
        assert!(!rc.contains(INSTALL_START), "{rc}");
        assert!(rc.contains("# manual init remains"), "{rc}");
        assert!(rc.contains("# >>> git-outpost shell integration >>>"), "{rc}");
        assert!(rc.contains("# after"), "{rc}");
    }

    #[test]
    fn uninstall_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());

        let report = uninstall(options).expect("uninstall absent");

        assert!(!report.changed);
    }

    #[test]
    fn malformed_markers_fail_without_modifying_rc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        let original = format!("# before\n{INSTALL_START}\nmissing end\n");
        std::fs::write(&options.rc_file, &original).expect("write rc");

        let err = install(options.clone()).expect_err("malformed markers should fail");
        let after = std::fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(
            err.to_string()
                .contains("missing git-outpost shell install end marker")
        );
        assert_eq!(after, original);
        assert!(!options.script_file.exists());
    }
}
