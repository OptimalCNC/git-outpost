#![allow(dead_code)]

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn command(bin_name: &str) -> Command {
    Command::new(binary_path(bin_name))
}

pub fn git_outpost_command() -> Command {
    command("git-outpost")
}

pub fn gop_command() -> Command {
    command("gop")
}

pub fn git_dispatch_command() -> Command {
    let mut command = Command::new("git");
    command.arg("outpost").env("PATH", path_with_binary_dir());
    command
}

pub fn binary_path(bin_name: &str) -> PathBuf {
    let key = format!("CARGO_BIN_EXE_{bin_name}");
    env::var_os(&key)
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback_binary_path(bin_name))
}

pub fn run(command: &mut Command) -> Output {
    command.output().expect("run command")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn assert_usage_error(output: &Output, flag: &str) {
    assert!(
        !output.status.success(),
        "expected usage error for {flag}, got success"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected clap usage exit for {flag}; stderr:\n{}",
        stderr(output)
    );

    let stderr = stderr(output);
    assert!(
        stderr.contains(flag),
        "expected stderr to mention {flag}; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Usage:"),
        "expected clap usage text for {flag}; stderr:\n{stderr}"
    );
}

fn fallback_binary_path(bin_name: &str) -> PathBuf {
    let mut path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("target")
        .join("debug")
        .join(bin_name);
    path.set_extension(env::consts::EXE_EXTENSION);
    path
}

fn path_with_binary_dir() -> OsString {
    let bin_dir = binary_path("git-outpost")
        .parent()
        .expect("binary directory")
        .to_path_buf();
    let existing = env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin_dir).chain(env::split_paths(&existing));

    env::join_paths(paths).expect("join PATH")
}
