use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

use crate::{OutpostError, OutpostResult};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[derive(Clone)]
pub struct GitInvoker {
    cwd: PathBuf,
    env: BTreeMap<OsString, OsString>,
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
        self.env.insert(key.into(), val.into());
        self
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn run_capture<I, S>(&self, args: I) -> OutpostResult<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let argv = collect_argv(args);
        let output = self.output(&argv)?;
        match output.status.code() {
            Some(0) => Ok(trimmed_lossy(&output.stdout)),
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

        Command::new("git")
            .current_dir(&self.cwd)
            .envs(&self.env)
            // Keep argv as separate OS strings; no shell parses user input here.
            .args(argv)
            .output()
            .map_err(|source| OutpostError::IoAt {
                path: self.cwd.clone(),
                source,
            })
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
    String::from_utf8_lossy(bytes).trim().to_owned()
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

    #[test]
    fn run_check_bad_command_preserves_failed_argv() {
        let git = GitInvoker::at(env!("CARGO_MANIFEST_DIR"));
        let err = git
            .run_check([
                "definitely-not-a-git-outpost-command",
                "--literal",
                "value with spaces",
            ])
            .expect_err("bad git command should fail");

        match err {
            OutpostError::GitFailed { args, code, stderr } => {
                assert_eq!(
                    args,
                    r#"["definitely-not-a-git-outpost-command", "--literal", "value with spaces"]"#
                );
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

    #[test]
    fn run_capture_keeps_leading_dash_value_positional_after_separator() {
        let git = GitInvoker::at(env!("CARGO_MANIFEST_DIR"));
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
        let git = GitInvoker::at(env!("CARGO_MANIFEST_DIR"));

        assert!(!git
            .run_status(["rev-parse", "--verify", "--quiet", "refs/heads/missing"])
            .expect("rev-parse reports missing ref as status false"));

        let err = git
            .run_status(["ls-tree", "--bad-option", "HEAD"])
            .expect_err("usage errors should be real failures");
        assert!(matches!(err, OutpostError::GitFailed { .. }));
    }
}
