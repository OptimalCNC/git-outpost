#![cfg(target_os = "linux")]

#[allow(dead_code)]
mod common;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::fixture::AbcFixture;
use outpost_core::{ConfigKey, ConfigValue, OutpostError, SourceRepo};

const CHILD_TEST: &str = "OUTPOST_CORE_STORAGE_LIMIT_CHILD_TEST";
const CHILD_SOURCE: &str = "OUTPOST_CORE_STORAGE_LIMIT_SOURCE";
const CHILD_CONTAINER: &str = "OUTPOST_CORE_STORAGE_LIMIT_CONTAINER";
const CHILD_OUTPOST: &str = "OUTPOST_CORE_STORAGE_LIMIT_OUTPOST";
const CHILD_COMPLETE: &str = "outpost-storage-write-limit-child-complete";
const FILE_SIZE_LIMIT: libc::rlim_t = 128;
const CHILD_WALL_LIMIT: Duration = Duration::from_secs(20);
const MAX_CHILD_LOG_BYTES: usize = 16 * 1024;

// A launcher may impose a file-size policy after configuration was saved.
// A later real write failure must preserve the previous usable configuration;
// invalid-value and obstructed-path tests fail before this write stage.
#[test]
fn config_write_limit_preserves_saved_value_and_user_work() {
    let name = "config_write_limit_preserves_saved_value_and_user_work";
    if is_child(name) {
        let source = child_source();
        let container = child_path(CHILD_CONTAINER);
        let result = {
            let _limit = FileSizeLimit::new().expect("apply child file-size policy");
            source.config().set(
                ConfigKey::OutpostContainer,
                ConfigValue::OutpostContainer(container),
            )
        };
        assert_write_failure(
            result.expect_err("limited write must fail"),
            &source.config_path(),
        );
        println!("{CHILD_COMPLETE}");
        return;
    }

    let fixture = AbcFixture::new();
    prepare_user_work(&fixture);
    let source = fixture.source_repo().expect("source repo");
    let short = fixture.root.join("containers");
    fs::create_dir(&short).expect("saved container directory");
    let saved = source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(short),
        )
        .expect("save initial configuration before the child policy");
    let mut long = fixture.root.clone();
    for _ in 0..5 {
        long.push("container_component_with_a_normal_finite_directory_name");
    }
    fs::create_dir_all(&long).expect("real longer container directory");
    assert!(
        fs::canonicalize(&long)
            .expect("canonical longer container")
            .as_os_str()
            .as_encoded_bytes()
            .len()
            > 2 * FILE_SIZE_LIMIT as usize
    );
    let bytes = fs::read(source.config_path()).expect("published config");
    let work = user_work(&fixture, &source);
    let state_dir = source.git_dir().join("outpost");
    let names = directory_names(&state_dir);

    run_child(&fixture, name, CHILD_CONTAINER, &long);

    assert_eq!(fs::read(source.config_path()).unwrap(), bytes);
    assert_eq!(
        source.config().get(ConfigKey::OutpostContainer).unwrap(),
        Some(saved)
    );
    assert_eq!(user_work(&fixture, &source), work);
    assert_eq!(
        directory_names(&state_dir),
        names,
        "no temporary write debris"
    );
}

// A registry update may be admitted in memory and then fail under the writer's
// process policy. Failed save must preserve the published entries and return
// normally; early directory obstructions do not establish this late failure.
#[test]
fn registry_write_limit_preserves_saved_entries_without_drop_panic() {
    let name = "registry_write_limit_preserves_saved_entries_without_drop_panic";
    if is_child(name) {
        let source = child_source();
        let outpost = fs::canonicalize(child_path(CHILD_OUTPOST)).expect("outpost path");
        let mut registry = source.registry_mut().expect("mutable registry");
        registry
            .lock(&outpost, Some("keep user work".to_owned()))
            .expect("admit pending lock");
        let entry = registry
            .entries()
            .iter()
            .find(|entry| entry.path == outpost)
            .expect("pending entry");
        assert!(entry.locked);
        assert_eq!(entry.lock_reason.as_deref(), Some("keep user work"));
        let result = {
            let _limit = FileSizeLimit::new().expect("apply child file-size policy");
            registry.save()
        };
        assert_write_failure(
            result.expect_err("limited save must fail without a drop panic"),
            &source.registry_path(),
        );
        println!("{CHILD_COMPLETE}");
        return;
    }

    let fixture = AbcFixture::new();
    prepare_user_work(&fixture);
    let outpost = fixture.add_outpost("C").expect("managed outpost");
    let outpost_file = outpost.join("work.txt");
    fs::write(&outpost_file, "outpost draft\n").expect("uncommitted outpost work");
    let source = fixture.source_repo().expect("source repo");
    let entries = source
        .registry()
        .expect("saved registry")
        .entries()
        .to_vec();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].locked);
    let bytes = fs::read(source.registry_path()).expect("published registry");
    assert!(bytes.len() > FILE_SIZE_LIMIT as usize);
    let work = user_work(&fixture, &source);
    let state_dir = source.git_dir().join("outpost");
    let names = directory_names(&state_dir);

    run_child(&fixture, name, CHILD_OUTPOST, &outpost);

    assert_eq!(fs::read(source.registry_path()).unwrap(), bytes);
    assert_eq!(source.registry().unwrap().entries(), entries);
    assert_eq!(user_work(&fixture, &source), work);
    assert_eq!(fs::read(outpost_file).unwrap(), b"outpost draft\n");
    assert_eq!(
        directory_names(&state_dir),
        names,
        "no temporary write debris"
    );
}

fn is_child(name: &str) -> bool {
    let Some(actual) = std::env::var_os(CHILD_TEST) else {
        return false;
    };
    assert_eq!(actual, name);
    true
}

fn child_path(key: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .expect("child path")
}

fn child_source() -> SourceRepo {
    let git_env = std::env::vars_os()
        .filter(|(key, _)| key.to_str().is_some_and(|key| key.starts_with("GIT_")))
        .collect::<BTreeMap<_, _>>();
    SourceRepo::at_with(child_path(CHILD_SOURCE), &git_env).expect("child source repo")
}

fn assert_write_failure(error: OutpostError, expected_path: &Path) {
    let OutpostError::IoAt { path, .. } = error else {
        panic!("expected a path-qualified write failure, got {error:?}");
    };
    assert_eq!(path, expected_path);
}

fn run_child(fixture: &AbcFixture, name: &str, path_key: &str, path: &Path) {
    let stdout_path = fixture.root.join("policy-child.stdout");
    let stderr_path = fixture.root.join("policy-child.stderr");
    let stdout = fs::File::create(&stdout_path).expect("child stdout log");
    let stderr = fs::File::create(&stderr_path).expect("child stderr log");
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_TEST, name)
        .env(CHILD_SOURCE, &fixture.source)
        .env(path_key, path)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .process_group(0);
    for (key, _) in std::env::vars_os() {
        if key.to_str().is_some_and(|key| key.starts_with("GIT_")) {
            command.env_remove(key);
        }
    }
    command
        .envs(&fixture.git_env)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    // The child owns its resource policy. Parent death also stops it; the
    // canonical runner's cgroup owns the full tree on external cancellation.
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(125);
            }
            Ok(())
        });
    }
    let mut owned = OwnedChild(Some(command.spawn().expect("spawn policy child")));
    let deadline = Instant::now() + CHILD_WALL_LIMIT;
    loop {
        let child = owned.0.as_mut().expect("owned child");
        if child.try_wait().expect("supervise policy child").is_some() {
            // try_wait has reaped the child. Disarm group cleanup immediately
            // so no assertion or log-read failure can signal a reused PID.
            let status = owned
                .0
                .take()
                .expect("completed child")
                .wait()
                .expect("cached child exit status");
            // Child Git queries wait synchronously for real Git. The writer
            // starts no processes and emits no output under the policy; its
            // guard restores the limit before assertions, marker, and exit.
            // No asynchronous descendant can retain output after that marker.
            let stdout = bounded_log(&stdout_path);
            let stderr = bounded_log(&stderr_path);
            let stdout = String::from_utf8_lossy(&stdout);
            let stderr = String::from_utf8_lossy(&stderr);
            assert!(status.success(), "{stdout}\n{stderr}");
            assert!(
                stdout.contains(CHILD_COMPLETE),
                "child test did not complete: {stdout}"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "policy child exceeded {CHILD_WALL_LIMIT:?}"
        );
        // Polling supervises completion; it does not create the failure state.
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn bounded_log(path: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .expect("child log")
        .take((MAX_CHILD_LOG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .expect("bounded child log read");
    assert!(
        bytes.len() <= MAX_CHILD_LOG_BYTES,
        "child log exceeded {MAX_CHILD_LOG_BYTES} bytes"
    );
    bytes
}

struct OwnedChild(Option<Child>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            // A deadline or parent assertion must not leave Git descendants.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct FileSizeLimit {
    previous: libc::rlimit,
    previous_signal: libc::sigaction,
}

impl FileSizeLimit {
    fn new() -> io::Result<Self> {
        let mut previous = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut previous) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut ignored: libc::sigaction = unsafe { std::mem::zeroed() };
        ignored.sa_sigaction = libc::SIG_IGN;
        if unsafe { libc::sigemptyset(&mut ignored.sa_mask) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut previous_signal: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(libc::SIGXFSZ, &ignored, &mut previous_signal) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let limited = libc::rlimit {
            rlim_cur: FILE_SIZE_LIMIT,
            rlim_max: previous.rlim_max,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) } != 0 {
            let error = io::Error::last_os_error();
            let restored =
                unsafe { libc::sigaction(libc::SIGXFSZ, &previous_signal, std::ptr::null_mut()) };
            assert_eq!(restored, 0, "restore disposition after policy setup fails");
            return Err(error);
        }
        let guard = Self {
            previous,
            previous_signal,
        };
        let mut actual = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut actual) } != 0 {
            return Err(io::Error::last_os_error());
        }
        assert_eq!(actual.rlim_cur, FILE_SIZE_LIMIT, "active child policy");
        assert_eq!(
            actual.rlim_max, guard.previous.rlim_max,
            "hard limit preserved"
        );
        Ok(guard)
    }
}

impl Drop for FileSizeLimit {
    fn drop(&mut self) {
        let limit = unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &self.previous) };
        let signal =
            unsafe { libc::sigaction(libc::SIGXFSZ, &self.previous_signal, std::ptr::null_mut()) };
        assert_eq!(limit, 0, "restore limit before assertions and profile exit");
        assert_eq!(signal, 0, "restore complete signal disposition");
    }
}

fn prepare_user_work(fixture: &AbcFixture) {
    fixture
        .commit_file_in_source("user work", "work.txt", "committed work\n")
        .expect("tracked user work");
    fs::write(fixture.source.join("work.txt"), "source draft\n").expect("source edits");
    fs::write(fixture.source.join("notes.txt"), "source notes\n").expect("source notes");
}

#[derive(Debug, PartialEq, Eq)]
struct UserWork {
    refs: String,
    head: String,
    status: String,
    git_config: Vec<u8>,
    tracked: Vec<u8>,
    notes: Vec<u8>,
}

fn user_work(fixture: &AbcFixture, source: &SourceRepo) -> UserWork {
    let git = fixture.invoker(&fixture.source);
    UserWork {
        refs: git
            .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
            .expect("source refs"),
        head: git
            .run_capture(["symbolic-ref", "HEAD"])
            .expect("source HEAD"),
        status: git
            .run_capture(["status", "--porcelain=v1", "--untracked-files=all"])
            .expect("index and worktree state"),
        git_config: fs::read(source.git_dir().join("config")).expect("Git user configuration"),
        tracked: fs::read(fixture.source.join("work.txt")).expect("tracked user work"),
        notes: fs::read(fixture.source.join("notes.txt")).expect("untracked user work"),
    }
}

fn directory_names(path: &Path) -> Vec<OsString> {
    let mut names = fs::read_dir(path)
        .expect("state directory")
        .map(|entry| entry.expect("state file").file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}
