use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

use crate::{AheadBehind, OutpostError, OutpostResult};

// When passing a full ref to branch or switch, select its exact match even if
// another branch or tag is literally named after that ref.
pub(crate) const PREFER_EXACT_REF: &str = "core.warnAmbiguousRefs=false";

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[derive(Clone)]
pub struct GitInvoker {
    cwd: PathBuf,
    env: BTreeMap<OsString, Option<OsString>>,
    #[cfg(any(test, feature = "test-helpers"))]
    argv_log: std::sync::Arc<std::sync::Mutex<Vec<Vec<OsString>>>>,
}

impl GitInvoker {
    pub fn at(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            env: BTreeMap::new(),
            #[cfg(any(test, feature = "test-helpers"))]
            argv_log: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn with_env(mut self, key: impl Into<OsString>, val: impl Into<OsString>) -> Self {
        let key = key.into();
        #[cfg(windows)]
        self.env.retain(|name, _| !name.eq_ignore_ascii_case(&key));
        self.env.insert(key, Some(val.into()));
        self
    }

    pub(crate) fn without_env(mut self, key: &str) -> Self {
        self.env.insert(key.into(), None);
        self
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub(crate) fn ref_oid(&self, full_ref: &str) -> OutpostResult<String> {
        self.run_capture(["show-ref", "--verify", "--hash", full_ref])
    }

    pub(crate) fn ahead_behind(&self, local: &str, other: &str) -> OutpostResult<AheadBehind> {
        let range = format!("{local}...{other}");
        let output = self.run_capture(["rev-list", "--left-right", "--count", &range])?;
        let mut counts = output.split_whitespace();
        let ahead = counts.next().and_then(|value| value.parse().ok());
        let behind = counts.next().and_then(|value| value.parse().ok());
        match (ahead, behind, counts.next()) {
            (Some(ahead), Some(behind), None) => Ok(AheadBehind { ahead, behind }),
            _ => Err(OutpostError::IoAt {
                path: self.cwd.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected rev-list output: {output}"),
                ),
            }),
        }
    }

    pub fn run_capture<I, S>(&self, args: I) -> OutpostResult<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Ok(trimmed_lossy(&self.run_capture_bytes(args)?))
    }

    /// Read one path printed by Git. Only the final LF frames the result.
    pub(crate) fn run_capture_path<I, S>(&self, args: I) -> OutpostResult<PathBuf>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let bytes = self.run_capture_bytes(args)?;
        self.output_path(bytes.strip_suffix(b"\n").unwrap_or(&bytes))
            .map(Path::to_owned)
    }

    pub(crate) fn output_path<'a>(&self, bytes: &'a [u8]) -> OutpostResult<&'a Path> {
        if bytes.is_empty() {
            return Err(OutpostError::IoAt {
                path: self.cwd.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, "empty Git path"),
            });
        }
        #[cfg(unix)]
        let path = Path::new(OsStr::from_bytes(bytes));
        #[cfg(not(unix))]
        let path = Path::new(self.output_text(bytes)?);
        Ok(path)
    }

    pub(crate) fn output_text<'a>(&self, bytes: &'a [u8]) -> OutpostResult<&'a str> {
        std::str::from_utf8(bytes).map_err(|error| OutpostError::IoAt {
            path: self.cwd.clone(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        })
    }

    /// Preserve each line exactly, including surrounding spaces and empty records.
    pub(crate) fn run_capture_lines<I, S>(&self, args: I) -> OutpostResult<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let bytes = self.run_capture_bytes(args)?;
        let output = self.output_text(&bytes)?;
        Ok(output.split_terminator('\n').map(str::to_owned).collect())
    }

    pub(crate) fn run_capture_bytes<I, S>(&self, args: I) -> OutpostResult<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let argv = collect_argv(args);
        let output = self.output(&argv)?;
        match output.status.code() {
            Some(0) => Ok(output.stdout),
            Some(code) => Err(git_failed(&argv, code, &output.stderr)),
            None => Err(git_terminated(&argv, output.status)),
        }
    }

    pub fn run_check<I, S>(&self, args: I) -> OutpostResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let argv = collect_argv(args);
        let output = self.output(&argv)?;
        match output.status.code() {
            Some(0) => Ok(()),
            Some(code) => Err(git_failed(&argv, code, &output.stderr)),
            None => Err(git_terminated(&argv, output.status)),
        }
    }

    pub fn run_status<I, S>(&self, args: I) -> OutpostResult<bool>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let argv = collect_argv(args);
        let output = self.output(&argv)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            Some(code) => Err(git_failed(&argv, code, &output.stderr)),
            None => Err(git_terminated(&argv, output.status)),
        }
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn argv_log(&self) -> Vec<Vec<OsString>> {
        self.argv_log.lock().expect("argv log poisoned").clone()
    }

    fn output(&self, argv: &[OsString]) -> OutpostResult<Output> {
        #[cfg(any(test, feature = "test-helpers"))]
        self.argv_log
            .lock()
            .expect("argv log poisoned")
            .push(argv.to_vec());

        let mut command = Command::new("git");
        #[cfg(any(test, feature = "test-helpers"))]
        isolate_git_test_command(&mut command);
        command.envs(
            self.env
                .iter()
                .filter_map(|(key, value)| value.as_ref().map(|value| (key, value))),
        );
        // Remove overrides last because Windows environment names ignore case.
        for (key, value) in &self.env {
            if value.is_none() {
                command.env_remove(key);
            }
        }
        command
            .current_dir(crate::path::git_path(&self.cwd))
            // Keep argv as separate OS strings; no shell parses user input here.
            .args(argv)
            .output()
            .map_err(|source| OutpostError::IoAt {
                path: self.cwd.clone(),
                source,
            })
    }
}

/// Clear Git's inherited repository selection before applying a fixture's explicit overrides.
#[cfg(any(test, feature = "test-helpers"))]
pub fn isolate_git_test_command(command: &mut Command) {
    // An empty global config path disables the user's file. Fixture-specific
    // configuration and identity can still be supplied by the caller.
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "");
    // Repository-local variables from git rev-parse --local-env-vars, plus
    // discovery and namespace settings which can redirect a fixture's refs.
    for name in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_DIR",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_GRAFT_FILE",
        "GIT_IMPLICIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PREFIX",
        "GIT_QUARANTINE_PATH",
        "GIT_REPLACE_REF_BASE",
        "GIT_SHALLOW_FILE",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(name);
    }
}

fn collect_argv<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect()
}

fn git_failed(argv: &[OsString], code: i32, stderr: &[u8]) -> OutpostError {
    OutpostError::GitFailed {
        args: display_argv(argv),
        code,
        stderr: trimmed_lossy(stderr),
    }
}

fn git_terminated(argv: &[OsString], status: ExitStatus) -> OutpostError {
    OutpostError::GitTerminatedBySignal {
        args: display_argv(argv),
        signal_str: signal_str(status),
    }
}

fn display_argv(argv: &[OsString]) -> String {
    let args = argv
        .iter()
        .map(|arg| display_arg(arg.as_os_str()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{args}]")
}

#[cfg(unix)]
fn display_arg(arg: &OsStr) -> String {
    let mut rendered = String::from("\"");
    for byte in arg.as_bytes() {
        for escaped in byte.escape_ascii() {
            rendered.push(escaped as char);
        }
    }
    rendered.push('"');
    rendered
}

#[cfg(windows)]
fn display_arg(arg: &OsStr) -> String {
    use std::fmt::Write;
    use std::os::windows::ffi::OsStrExt;

    let mut rendered = String::from("w\"");
    for unit in arg.encode_wide() {
        match char::from_u32(u32::from(unit)) {
            Some('\\') => rendered.push_str("\\\\"),
            Some('"') => rendered.push_str("\\\""),
            Some(c) if !c.is_control() => rendered.push(c),
            Some(c) => write!(rendered, "\\u{{{:x}}}", c as u32).expect("write to string"),
            None => write!(rendered, "\\u{{{:x}}}", unit).expect("write to string"),
        }
    }
    rendered.push('"');
    rendered
}

#[cfg(not(any(unix, windows)))]
fn display_arg(arg: &OsStr) -> String {
    format!("{arg:?}")
}

fn trimmed_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_matches(|ch: char| ch.is_ascii() && ch.is_whitespace())
        .to_owned()
}

#[cfg(unix)]
fn signal_str(status: ExitStatus) -> String {
    status
        .signal()
        .map(|signal| format!(" (signal {signal})"))
        .unwrap_or_default()
}

#[cfg(not(unix))]
fn signal_str(_status: ExitStatus) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_repo() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary repository");
        GitInvoker::at(directory.path())
            .run_check(["init", "--quiet"])
            .expect("initialize repository");
        directory
    }

    #[test]
    fn run_check_bad_command_preserves_failed_argv() {
        let directory = tempfile::tempdir().expect("command directory");
        let git = GitInvoker::at(directory.path());
        let err = git
            .run_check([
                "definitely-not-a-git-outpost-command",
                "--literal",
                "value with spaces",
            ])
            .expect_err("bad git command should fail");

        match err {
            OutpostError::GitFailed { args, code, stderr } => {
                assert_eq!(args, expected_bad_command_argv());
                assert_ne!(
                    args,
                    r#"["definitely-not-a-git-outpost-command", "--literal", "value", "with", "spaces"]"#
                );
                assert_ne!(code, 0);
                assert!(stderr.contains("git") || stderr.contains("not a git command"));
            }
            other => panic!("expected GitFailed, got {other:?}"),
        }
    }

    #[cfg(unix)]
    fn expected_bad_command_argv() -> &'static str {
        r#"["definitely-not-a-git-outpost-command", "--literal", "value with spaces"]"#
    }

    #[cfg(windows)]
    fn expected_bad_command_argv() -> &'static str {
        r#"[w"definitely-not-a-git-outpost-command", w"--literal", w"value with spaces"]"#
    }

    #[test]
    fn run_capture_keeps_leading_dash_value_positional_after_separator() {
        let directory = empty_repo();
        let git = GitInvoker::at(directory.path());
        let stdout = git
            .run_capture(["rev-parse", "--", "--not-a-flag"])
            .expect("rev-parse should echo positional value");

        assert_eq!(stdout, "--\n--not-a-flag");
        assert_eq!(
            git.argv_log(),
            vec![vec![
                OsString::from("rev-parse"),
                OsString::from("--"),
                OsString::from("--not-a-flag")
            ]]
        );
    }

    #[test]
    fn run_status_distinguishes_exit_one_from_real_failure() {
        let directory = empty_repo();
        let git = GitInvoker::at(directory.path());

        assert!(
            !git.run_status(["rev-parse", "--verify", "--quiet", "refs/heads/missing"])
                .expect("rev-parse reports missing ref as status false")
        );

        let err = git
            .run_status(["ls-tree", "--bad-option", "HEAD"])
            .expect_err("usage errors should be real failures");
        assert!(matches!(err, OutpostError::GitFailed { .. }));
    }
}
