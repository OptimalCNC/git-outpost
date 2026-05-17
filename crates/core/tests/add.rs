#[allow(dead_code)]
mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::add::{run, AddCheckout, AddOptions};
use outpost_core::{
    BranchName, Outpost, OutpostResult, RemoteName, Reporter, SourceRepo, StepKind,
};

#[test]
fn add_without_branch_clones_current_branch_with_real_git_dir() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    let outpost = add_existing(&source, &destination, None).expect("add outpost");

    assert_eq!(outpost.work_tree(), canonical(&destination));
    assert_eq!(
        outpost
            .current_branch()
            .expect("outpost current branch")
            .as_str(),
        "main"
    );
    assert!(destination.join(".git").is_dir());
    assert!(!destination.join(".git").is_file());
}

#[test]
fn add_existing_branch_checks_out_branch_and_tracks_local_remote() {
    let fixture = AbcFixture::new();
    let branch = fixture
        .create_source_branch("feature/add")
        .expect("source branch");
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    let outpost = add_existing(&source, &destination, Some(branch)).expect("add outpost");

    assert_eq!(
        outpost
            .current_branch()
            .expect("outpost current branch")
            .as_str(),
        "feature/add"
    );
    let tracking = outpost
        .upstream_tracking()
        .expect("upstream tracking")
        .expect("outpost branch should track source remote");
    assert_eq!(tracking.remote.as_str(), "local");
    assert_eq!(tracking.merge_ref.as_str(), "refs/heads/feature/add");
}

#[test]
fn add_writes_outpost_metadata_keys() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    let outpost = add_existing(&source, &destination, None).expect("add outpost");
    let git = fixture.invoker(&destination);

    assert_eq!(
        git.run_capture(["config", "--local", "--get", "outpost.managed"])
            .expect("managed config"),
        "true"
    );
    assert_eq!(
        git.run_capture(["config", "--local", "--get", "outpost.sourceRepo"])
            .expect("source repo config"),
        source.work_tree().to_string_lossy()
    );
    assert_eq!(
        git.run_capture(["config", "--local", "--get", "outpost.remoteName"])
            .expect("remote name config"),
        "local"
    );
    assert_eq!(outpost.metadata().source_repo, source.work_tree());
    assert_eq!(outpost.metadata().remote_name.as_str(), "local");
}

#[test]
fn add_configures_local_remote_and_non_shared_clone() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    add_existing(&source, &destination, None).expect("add outpost");

    assert_eq!(
        fixture
            .invoker(&destination)
            .run_capture(["remote", "get-url", "local"])
            .expect("local remote url"),
        source.work_tree().to_string_lossy()
    );
    assert!(destination.join(".git").is_dir());
    assert!(!destination
        .join(".git")
        .join("objects/info/alternates")
        .exists());
    let clone_argv = recorded_clone_argv(&source).expect("clone argv should be recorded");
    assert!(
        clone_argv.iter().any(|arg| arg == "--no-shared"),
        "clone argv should include --no-shared: {clone_argv:?}"
    );
}

#[test]
fn add_registers_outpost_path_in_source_registry() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    add_existing(&source, &destination, None).expect("add outpost");

    let registry = source.registry().expect("registry");
    assert_eq!(registry.entries().len(), 1);
    assert_eq!(registry.entries()[0].path, canonical(&destination));
    assert_eq!(registry.entries()[0].remote_name.as_str(), "local");
}

#[test]
fn add_sets_source_receive_deny_current_branch_update_instead() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    add_existing(&source, &destination, None).expect("add outpost");

    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["config", "--local", "--get", "receive.denyCurrentBranch"])
            .expect("receive.denyCurrentBranch"),
        "updateInstead"
    );
}

#[test]
fn add_reports_source_config_change() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let mut reporter = RecordingReporter::default();

    add_existing_with_reporter(&source, &destination, None, &mut reporter).expect("add outpost");

    assert!(
        reporter.steps.iter().any(|(kind, message)| {
            *kind == StepKind::ConfigChange
                && message.contains("receive.denyCurrentBranch=updateInstead")
                && message.contains(&source.work_tree().to_string_lossy().into_owned())
        }),
        "expected ConfigChange reporter event, got {:?}",
        reporter.steps
    );
    assert!(
        reporter.warnings.is_empty(),
        "add should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

#[test]
fn add_clone_allows_user_file_protocol() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    add_existing(&source, &destination, None).expect("add outpost");

    let clone_argv = recorded_clone_argv(&source).expect("clone argv should be recorded");
    assert!(
        has_adjacent_args(&clone_argv, "-c", "protocol.file.allow=user"),
        "clone argv should include -c protocol.file.allow=user: {clone_argv:?}"
    );
}

#[test]
fn add_ignores_source_registry_directory_locally() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    add_existing(&source, &destination, None).expect("add outpost");

    let exclude =
        fs::read_to_string(fixture.source.join(".git/info/exclude")).expect("source local exclude");
    assert!(
        exclude.lines().any(|line| line.trim() == ".outpost/"),
        "source local exclude should ignore .outpost/: {exclude:?}"
    );
    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["status", "--porcelain"])
            .expect("source status"),
        ""
    );
}

fn add_existing(
    source: &SourceRepo,
    destination: &Path,
    target_branch: Option<BranchName>,
) -> OutpostResult<Outpost> {
    let mut reporter = RecordingReporter::default();
    add_existing_with_reporter(source, destination, target_branch, &mut reporter)
}

fn add_existing_with_reporter(
    source: &SourceRepo,
    destination: &Path,
    target_branch: Option<BranchName>,
    reporter: &mut RecordingReporter,
) -> OutpostResult<Outpost> {
    run(
        source,
        AddOptions {
            destination: destination.to_path_buf(),
            checkout: AddCheckout::CheckoutExisting { target_branch },
            remote_name: RemoteName::parse("local")?,
        },
        reporter,
    )
}

fn recorded_clone_argv(source: &SourceRepo) -> Option<Vec<OsString>> {
    source
        .test_invoker()
        .argv_log()
        .into_iter()
        .find(|argv| argv.iter().any(|arg| arg == "clone"))
}

fn has_adjacent_args(argv: &[OsString], first: &str, second: &str) -> bool {
    argv.windows(2)
        .any(|pair| pair[0] == first && pair[1] == second)
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

#[derive(Debug, Default)]
struct RecordingReporter {
    steps: Vec<(StepKind, String)>,
    warnings: Vec<String>,
}

impl Reporter for RecordingReporter {
    fn step(&mut self, kind: StepKind, message: &str) {
        self.steps.push((kind, message.to_owned()));
    }

    fn warn(&mut self, message: &str) {
        self.warnings.push(message.to_owned());
    }
}
