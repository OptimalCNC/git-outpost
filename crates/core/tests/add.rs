#[allow(dead_code)]
mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::add::{run, AddCheckout, AddOptions};
use outpost_core::{
    BranchName, Outpost, OutpostError, OutpostResult, RemoteName, Reporter, SourceRepo, StepKind,
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
fn add_new_branch_from_target_creates_source_branch_and_tracks_it() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let name = BranchName::parse("feature/new").expect("new branch");
    let target = BranchName::parse("main").expect("target branch");
    let target_oid = branch_oid(&fixture, &target);

    let outpost =
        add_new_branch(&source, &destination, name.clone(), Some(target)).expect("add outpost");

    assert_eq!(branch_oid(&fixture, &name), target_oid);
    assert_eq!(
        outpost
            .current_branch()
            .expect("outpost current branch")
            .as_str(),
        "feature/new"
    );
    let tracking = outpost
        .upstream_tracking()
        .expect("upstream tracking")
        .expect("new branch should track source remote");
    assert_eq!(tracking.remote.as_str(), "local");
    assert_eq!(tracking.merge_ref.as_str(), "refs/heads/feature/new");
}

#[test]
fn add_new_branch_without_target_uses_source_current_branch() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let name = BranchName::parse("feature/current").expect("new branch");
    let current = source.current_branch().expect("source current branch");
    let current_oid = branch_oid(&fixture, &current);

    add_new_branch(&source, &destination, name.clone(), None).expect("add outpost");

    assert_eq!(branch_oid(&fixture, &name), current_oid);
    assert_eq!(
        source
            .current_branch()
            .expect("source current branch")
            .as_str(),
        current.as_str()
    );
}

#[test]
fn add_rejects_existing_non_empty_directory() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    fs::create_dir(&destination).expect("destination dir");
    fs::write(destination.join("file.txt"), "content").expect("destination content");

    let err = expect_error(
        add_existing(&source, &destination, None),
        "non-empty dir should fail",
    );

    assert!(matches!(err, OutpostError::DestinationExists(path) if path == canonical_missing(&destination)));
    assert!(!destination.join(".git").exists());
}

#[test]
fn add_rejects_existing_file() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    fs::write(&destination, "content").expect("destination file");

    let err = expect_error(
        add_existing(&source, &destination, None),
        "file should fail",
    );

    assert!(matches!(err, OutpostError::DestinationExists(path) if path == canonical_missing(&destination)));
}

#[test]
fn add_outside_git_repo_cannot_discover_source() {
    let temp = tempfile::tempdir().expect("tempdir");

    let err = expect_error(
        SourceRepo::discover(temp.path()),
        "outside repo should fail",
    );

    assert!(matches!(err, OutpostError::NotARepo(path) if path == temp.path()));
}

#[test]
fn add_rejects_destination_inside_existing_repo() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.source.join("C");

    let err = expect_error(
        add_existing(&source, &destination, None),
        "repo-contained dest fails",
    );

    assert!(matches!(err, OutpostError::DestinationInsideRepo(path) if path == canonical_missing(&destination)));
    assert!(!destination.exists());
}

#[test]
fn add_rejects_relative_destination_inside_source_repo() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        add_existing(&source, Path::new("C"), None),
        "source-relative destination should fail inside repo",
    );

    assert!(matches!(
        err,
        OutpostError::DestinationInsideRepo(path) if path == canonical_missing(&source.work_tree().join("C"))
    ));
    assert!(!source.work_tree().join("C").exists());
}

#[test]
fn add_relative_sibling_destination_uses_same_resolved_path_for_all_steps() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");

    let outpost =
        add_existing(&source, Path::new("../C"), None).expect("add relative sibling outpost");

    assert_eq!(outpost.work_tree(), canonical(&destination));
    assert_eq!(
        outpost.metadata().source_repo,
        source.work_tree().to_path_buf()
    );
    assert_eq!(
        source.registry().expect("registry").entries()[0].path,
        canonical(&destination)
    );
    assert_eq!(
        remote_url_path(&fixture, &destination, "local"),
        source.work_tree()
    );
    let clone_argv = recorded_clone_argv(&source).expect("clone argv should be recorded");
    let expected_destination = git_path(&canonical(&destination));
    assert_eq!(
        clone_argv.last().expect("clone destination"),
        expected_destination.as_os_str()
    );
}

#[test]
fn add_rejects_missing_existing_branch_before_clone() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let missing = BranchName::parse("bogus-branch").expect("branch name");

    let err = expect_error(
        add_existing(&source, &destination, Some(missing)),
        "missing branch should fail",
    );

    assert!(
        matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == "bogus-branch" && repo == source.work_tree())
    );
    assert!(!destination.exists());
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
        remote_url_path(&fixture, &destination, "local"),
        source.work_tree()
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
fn add_custom_remote_name_replaces_origin_and_updates_metadata() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let mut reporter = RecordingReporter::default();

    let outpost = run(
        &source,
        AddOptions {
            destination: destination.clone(),
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse("custom").expect("remote name"),
        },
        &mut reporter,
    )
    .expect("add outpost");

    let git = fixture.invoker(&destination);
    assert_eq!(
        remote_url_path(&fixture, &destination, "custom"),
        source.work_tree()
    );
    assert!(matches!(
        git.run_capture(["remote", "get-url", "origin"]),
        Err(OutpostError::GitFailed { .. })
    ));
    assert_eq!(outpost.metadata().remote_name.as_str(), "custom");
    let tracking = outpost
        .upstream_tracking()
        .expect("upstream tracking")
        .expect("outpost branch should track custom source remote");
    assert_eq!(tracking.remote.as_str(), "custom");
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
fn add_rejects_unborn_source_head_before_clone() {
    let fixture = AbcFixture::new();
    let unborn = fixture.root.join("unborn");
    fixture
        .invoker(&fixture.root)
        .run_check([
            std::ffi::OsStr::new("init"),
            std::ffi::OsStr::new("--initial-branch=main"),
            unborn.as_os_str(),
        ])
        .expect("init unborn repo");
    let source = SourceRepo::at_with(&unborn, &fixture.git_env).expect("unborn source repo");
    let destination = fixture.root.join("C");

    let err = expect_error(
        add_existing(&source, &destination, None),
        "unborn source should fail",
    );

    assert!(
        matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == "HEAD" && repo == source.work_tree())
    );
    assert!(!destination.exists());
}

#[test]
fn add_new_branch_rejects_missing_target_before_clone() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let name = BranchName::parse("feature/new").expect("new branch");
    let missing = BranchName::parse("missing").expect("missing branch");

    let err = expect_error(
        add_new_branch(&source, &destination, name.clone(), Some(missing)),
        "missing target should fail",
    );

    assert!(
        matches!(err, OutpostError::BranchNotFound { branch, repo } if branch == "missing" && repo == source.work_tree())
    );
    assert!(!source.branch_exists(&name).expect("branch exists check"));
    assert!(!destination.exists());
}

#[test]
fn add_new_branch_does_not_switch_source_checkout() {
    let fixture = AbcFixture::new();
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "-c", "work"])
        .expect("switch source branch");
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join("C");
    let name = BranchName::parse("feature/review").expect("new branch");
    let target = BranchName::parse("main").expect("target branch");

    let outpost =
        add_new_branch(&source, &destination, name.clone(), Some(target)).expect("add outpost");

    assert!(source.branch_exists(&name).expect("branch exists check"));
    assert_eq!(
        source
            .current_branch()
            .expect("source current branch")
            .as_str(),
        "work"
    );
    assert_eq!(
        outpost
            .current_branch()
            .expect("outpost current branch")
            .as_str(),
        "feature/review"
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

fn add_new_branch(
    source: &SourceRepo,
    destination: &Path,
    name: BranchName,
    target_branch: Option<BranchName>,
) -> OutpostResult<Outpost> {
    let mut reporter = RecordingReporter::default();
    run(
        source,
        AddOptions {
            destination: destination.to_path_buf(),
            checkout: AddCheckout::NewBranch {
                name,
                target_branch,
            },
            remote_name: RemoteName::parse("local")?,
        },
        &mut reporter,
    )
}

fn branch_oid(fixture: &AbcFixture, branch: &BranchName) -> String {
    let branch_ref = format!("refs/heads/{}", branch.as_str());
    fixture
        .invoker(&fixture.source)
        .run_capture(["rev-parse", &branch_ref])
        .expect("branch oid")
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
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

fn canonical_missing(path: &Path) -> PathBuf {
    let parent = path.parent().expect("path parent");
    canonical(parent).join(path.file_name().expect("file name"))
}

fn git_path(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

fn remote_url_path(fixture: &AbcFixture, repo: &Path, remote: &str) -> PathBuf {
    let url = fixture
        .invoker(repo)
        .run_capture(["remote", "get-url", remote])
        .expect("remote url");
    canonical(Path::new(&url))
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
