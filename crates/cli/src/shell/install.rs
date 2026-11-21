use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
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
    let mut start = None;
    let mut end = None;
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let text = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix('\n'))
            .unwrap_or(line);
        let duplicate = match text {
            INSTALL_START => start.replace(offset),
            INSTALL_END => end.replace(offset + line.len()),
            _ => None,
        };
        if duplicate.is_some() {
            return Err(invalid_marker(
                path,
                "multiple git-outpost shell install blocks found",
            ));
        }
        offset += line.len();
    }

    match (start, end) {
        (None, None) => Ok(BlockRange::Absent),
        (Some(_), None) => Err(invalid_marker(
            path,
            "missing git-outpost shell install end marker",
        )),
        (None, Some(_)) => Err(invalid_marker(
            path,
            "missing git-outpost shell install start marker",
        )),
        (Some(start), Some(end)) if start < end => Ok(BlockRange::Present { start, end }),
        (Some(_), Some(_)) => Err(invalid_marker(
            path,
            "git-outpost shell install end marker appears before start marker",
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

fn read_optional(path: &Path) -> OutpostResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_text(path: &Path, contents: &str, replace: bool) -> OutpostResult<fs::File> {
    let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
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
    temp.write_all(contents.as_bytes())
        .map_err(|source| OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        })?;
    let file = if replace {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    };
    file.map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: source.error,
    })
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

fn existing_managed_script(path: &Path) -> OutpostResult<bool> {
    fn inspect(path: &Path) -> std::io::Result<bool> {
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let mut first = [0; super::INTEGRATION_START.len()];
        let mut last = [0; super::INTEGRATION_END.len()];
        if file.metadata()?.len() >= (first.len() + last.len()) as u64 {
            file.read_exact(&mut first)?;
            file.seek(SeekFrom::End(-(last.len() as i64)))?;
            file.read_exact(&mut last)?;
            if first == super::INTEGRATION_START.as_bytes()
                && last == super::INTEGRATION_END.as_bytes()
            {
                return Ok(true);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "existing script is not a Git Outpost generated integration file",
        ))
    }
    inspect(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

fn destination_path(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if missing.is_empty() && !metadata.is_file() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "shell setup requires a regular file and cannot replace a symlink or directory",
                    ));
                }
                let mut resolved = fs::canonicalize(ancestor)?;
                for name in missing.into_iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let Some((name, parent)) = ancestor.file_name().zip(ancestor.parent()) else {
                    return Err(err);
                };
                missing.push(name);
                ancestor = parent;
            }
            Err(err) => return Err(err),
        }
    }
}

fn overlapping_files(options: &InstallOptions) -> OutpostError {
    OutpostError::IoAt {
        path: options.script_file.clone(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "generated script must be separate from startup file {}",
                options.rc_file.display()
            ),
        ),
    }
}

fn check_destinations(options: &InstallOptions) -> OutpostResult<()> {
    let resolve = |path: &Path| {
        destination_path(path).map_err(|source| OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        })
    };
    let rc = resolve(&options.rc_file)?;
    let script = resolve(&options.script_file)?;
    let same_file = rc.is_file()
        && script.is_file()
        && same_file::is_same_file(&rc, &script).map_err(|source| OutpostError::IoAt {
            path: options.script_file.clone(),
            source,
        })?;
    if same_file || rc.starts_with(&script) || script.starts_with(&rc) {
        return Err(overlapping_files(options));
    }
    Ok(())
}

pub fn install(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    check_destinations(&options)?;
    let rc_before = read_optional(&options.rc_file)?;
    let block = managed_source_block(options.shell, &options.script_file);
    let rc_after = install_contents(
        &options.rc_file,
        rc_before.as_deref().unwrap_or_default(),
        &block,
    )?;
    let script = super::init_script(Some(options.shell));

    let script_existed = existing_managed_script(&options.script_file)?;
    let written = write_text(&options.script_file, &script, script_existed)?;
    if let Err(error) = write_text(&options.rc_file, &rc_after, rc_before.is_some()) {
        // Two originally absent names can alias on a case-insensitive filesystem.
        if rc_before.is_none() && !script_existed {
            let identity = same_file::Handle::from_file(written).ok();
            if identity.is_some()
                && same_file::Handle::from_path(&options.rc_file).ok() == identity
                && same_file::Handle::from_path(&options.script_file).ok() == identity
            {
                remove_file_if_exists(&options.script_file)?;
                return Err(overlapping_files(&options));
            }
        }
        return Err(error);
    }

    Ok(ShellInstallReport {
        shell: options.shell,
        rc_file: options.rc_file,
        script_file: options.script_file,
        changed: true,
    })
}

pub fn uninstall(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    check_destinations(&options)?;
    let rc_before = read_optional(&options.rc_file)?;
    let (rc_after, rc_changed) =
        uninstall_contents(&options.rc_file, rc_before.as_deref().unwrap_or_default())?;
    let script_exists = existing_managed_script(&options.script_file)?;
    if rc_changed {
        write_text(&options.rc_file, &rc_after, true)?;
    }
    let script_removed = script_exists && remove_file_if_exists(&options.script_file)?;

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
        assert!(
            block.contains("'/tmp/a dir/it'\"'\"'s/gop.bash'"),
            "{block}"
        );
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
        assert!(
            second_script.contains("COMPLETE=bash command gop"),
            "{second_script}"
        );
        assert!(
            !second_script.contains("_clap_complete_gop"),
            "{second_script}"
        );
    }

    #[test]
    fn uninstall_removes_managed_block_and_script_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        let original = format!(
            "alias show_gop_setup=\"sed -n '/{INSTALL_START}/,/{INSTALL_END}/p' ~/.bashrc\"\n{}\n# after\n",
            super::super::init_script(Some(ShellKind::Bash))
        );
        fs::write(&options.rc_file, &original).expect("write rc");
        install(options.clone()).expect("install");

        let report = uninstall(options.clone()).expect("uninstall");
        let rc = std::fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(report.changed);
        assert!(!options.script_file.exists());
        assert!(
            rc.strip_prefix(&original)
                .is_some_and(|suffix| suffix.trim().is_empty()),
            "{rc}"
        );
    }

    #[test]
    fn setup_preserves_a_user_script_at_the_generated_script_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        install(options.clone()).expect("install integration");
        let rc = fs::read(&options.rc_file).expect("installed startup file");
        let user_script = format!(
            "{}\n# my shell settings\n",
            super::super::init_script(Some(options.shell))
        );
        fs::write(&options.script_file, &user_script).expect("user adds shell settings");

        for operation in [install, uninstall] {
            let error = operation(options.clone()).expect_err("preserve a user-edited script");
            assert!(matches!(error, OutpostError::IoAt { path, source }
                if path == options.script_file && source.kind() == std::io::ErrorKind::InvalidInput));
            assert_eq!(fs::read(&options.rc_file).expect("startup file"), rc);
            assert_eq!(
                fs::read_to_string(&options.script_file).expect("user script"),
                user_script
            );
        }
    }

    #[test]
    fn overlapping_destinations_leave_the_startup_file_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut options = paths(tmp.path());
        let original = "export EDITOR=vim\n";
        fs::write(&options.rc_file, original).expect("write rc");
        fs::create_dir(tmp.path().join("config")).expect("create config directory");
        let linked = tmp.path().join("linked");
        fs::hard_link(&options.rc_file, &linked).expect("link to startup file");

        for alias in [tmp.path().join("config/../.bashrc"), linked] {
            options.script_file = alias.clone();
            for operation in [install, uninstall] {
                let err = operation(options.clone()).expect_err("overlap must be refused");
                assert!(
                    err.to_string().contains("separate from startup file"),
                    "{err}"
                );
                assert_eq!(
                    fs::read_to_string(&options.rc_file).expect("startup file"),
                    original
                );
                assert_eq!(fs::read_to_string(&alias).expect("aliased file"), original);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_startup_file_is_preserved_before_changing_the_script() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        let target = tmp.path().join("dotfiles-bashrc");
        let original = "export EDITOR=vim\n";
        fs::write(&target, original).expect("write backing dotfile");
        std::os::unix::fs::symlink(&target, &options.rc_file).expect("link startup file");
        fs::create_dir(options.script_file.parent().unwrap()).expect("script directory");
        fs::write(&options.script_file, "previous script\n").expect("write previous script");

        for operation in [install, uninstall] {
            let err = operation(options.clone()).expect_err("symlink must be refused");
            assert!(err.to_string().contains("regular file"), "{err}");
            assert_eq!(
                fs::read_link(&options.rc_file).expect("startup link"),
                target
            );
            assert_eq!(
                fs::read_to_string(&target).expect("backing dotfile"),
                original
            );
            assert_eq!(
                fs::read_to_string(&options.script_file).expect("previous script"),
                "previous script\n"
            );
        }
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

    #[test]
    fn duplicate_markers_fail_without_modifying_rc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        let original = format!(
            "# before\n{INSTALL_START}\none\n{INSTALL_END}\n{INSTALL_START}\ntwo\n{INSTALL_END}\n"
        );
        std::fs::write(&options.rc_file, &original).expect("write rc");

        let err = uninstall(options.clone()).expect_err("duplicate markers should fail");
        let after = std::fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(
            err.to_string()
                .contains("multiple git-outpost shell install blocks found")
        );
        assert_eq!(after, original);
    }
}
