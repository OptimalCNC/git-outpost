mod common;

use outpost_core::OutpostError;

#[test]
fn e_01_build_produces_both_binaries() {
    assert!(
        common::binary_path("git-outpost").is_file(),
        "missing git-outpost binary at {}",
        common::binary_path("git-outpost").display()
    );
    assert!(
        common::binary_path("gop").is_file(),
        "missing gop binary at {}",
        common::binary_path("gop").display()
    );
}

#[test]
fn e_08_outpost_errors_map_to_documented_exit_codes() {
    let cases = [
        (OutpostError::NotARepo(path("/repo")), 2),
        (OutpostError::NotAnOutpost(path("/outpost")), 2),
        (OutpostError::SourceMissing(path("/source")), 2),
        (
            OutpostError::WrongContext {
                command: "pull",
                expected: "a managed outpost",
                cwd: path("/source"),
            },
            2,
        ),
        (
            OutpostError::MissingOutpostPath {
                command: "lock",
                cwd: path("/source"),
            },
            2,
        ),
        (
            OutpostError::OutpostContainerNotConfigured {
                name: "C".to_owned(),
                suggestion: Some(path("/outposts")),
            },
            2,
        ),
        (OutpostError::DestinationExists(path("/dest")), 3),
        (OutpostError::DestinationInsideRepo(path("/dest")), 3),
        (
            OutpostError::DirtyTree {
                repo: path("/repo"),
                hint: "pass --force",
            },
            3,
        ),
        (
            OutpostError::UnpushedCommits {
                repo: path("/repo"),
                branch: "main".to_owned(),
                hint: "push first",
            },
            3,
        ),
        (
            OutpostError::Divergence {
                branch: "main".to_owned(),
            },
            4,
        ),
        (
            OutpostError::BranchNotFound {
                branch: "feature".to_owned(),
                repo: path("/repo"),
            },
            5,
        ),
        (
            OutpostError::NoUpstreamTracking {
                branch: "feature".to_owned(),
            },
            5,
        ),
        (
            OutpostError::UpstreamNotABranch {
                merge_ref: "refs/tags/v1".to_owned(),
            },
            5,
        ),
        (
            OutpostError::InvalidRefName {
                name: "-evil".to_owned(),
            },
            5,
        ),
        (
            OutpostError::PushIntoCheckedOutBranch {
                source: path("/source"),
                branch: "main".to_owned(),
            },
            4,
        ),
        (
            OutpostError::AmbiguousBranchCreation {
                branch: "feature".to_owned(),
            },
            4,
        ),
        (
            OutpostError::OutpostLocked {
                path: path("/outpost"),
                reason: ": release".to_owned(),
            },
            3,
        ),
        (OutpostError::RegistryEntryNotManaged(path("/outpost")), 6),
        (OutpostError::RegistryEntryNotFound(path("/missing")), 6),
        (OutpostError::OutpostIdPrefixNotFound("abcde".to_owned()), 6),
        (
            OutpostError::OutpostIdPrefixAmbiguous("abcde".to_owned()),
            6,
        ),
        (
            OutpostError::OutpostSelectorAmbiguous("abcde".to_owned()),
            6,
        ),
        (
            OutpostError::BadRegistry {
                path: path("/repo/.outpost/registry.json"),
                reason: "invalid json".to_owned(),
            },
            6,
        ),
        (
            OutpostError::BadMetadata {
                outpost: path("/outpost"),
                reason: "missing source".to_owned(),
            },
            6,
        ),
        (
            OutpostError::GitFailed {
                args: "status".to_owned(),
                code: 42,
                stderr: "fatal".to_owned(),
            },
            42,
        ),
        (
            OutpostError::GitTerminatedBySignal {
                args: "fetch".to_owned(),
                signal_str: " (signal 9)".to_owned(),
            },
            137,
        ),
        (
            OutpostError::IoAt {
                path: path("/repo/.outpost/registry.json"),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
            },
            70,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.exit_code(), expected, "wrong exit for {error}");
    }

    assert_eq!(
        OutpostError::GitFailed {
            args: "status".to_owned(),
            code: -1,
            stderr: "fatal".to_owned(),
        }
        .exit_code(),
        1
    );
    assert_eq!(
        OutpostError::GitFailed {
            args: "status".to_owned(),
            code: 256,
            stderr: "fatal".to_owned(),
        }
        .exit_code(),
        125
    );
}

#[test]
fn e_08_cli_errors_return_documented_exit_codes() {
    let fixture = common::CliFixture::new();
    let not_a_repo = common::run(fixture.gop().current_dir(&fixture.root).arg("status"));
    assert_failure_code_contains(&not_a_repo, 2, "NotARepo", "not inside a Git repository");

    let fixture = common::CliFixture::new();
    let not_an_outpost = common::run(fixture.gop().current_dir(&fixture.source).arg("status"));
    assert_failure_code_contains(
        &not_an_outpost,
        2,
        "NotAnOutpost",
        "not inside a managed outpost",
    );

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    std::fs::remove_dir_all(&fixture.source).expect("remove source");
    let source_missing = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    assert_failure_code_contains(
        &source_missing,
        2,
        "SourceMissing",
        "source repository not found",
    );

    let fixture = common::CliFixture::new();
    let wrong_context = common::run(fixture.gop().current_dir(&fixture.source).arg("pull"));
    assert_failure_code_contains(
        &wrong_context,
        2,
        "WrongContext",
        "pull must be run from a managed outpost",
    );

    let fixture = common::CliFixture::new();
    let missing_outpost_path = common::run(fixture.gop().current_dir(&fixture.source).arg("lock"));
    assert_failure_code_contains(
        &missing_outpost_path,
        2,
        "MissingOutpostPath",
        "lock requires <outpost>",
    );

    let fixture = common::CliFixture::new();
    fixture.add_outpost("C");
    let destination_exists = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "../C", "main"]),
    );
    assert_failure_code_contains(
        &destination_exists,
        3,
        "DestinationExists",
        "destination already exists",
    );

    let fixture = common::CliFixture::new();
    let destination_inside_repo = common::run(fixture.gop().current_dir(&fixture.source).args([
        "add",
        "./inside-source",
        "main",
    ]));
    assert_failure_code_contains(
        &destination_inside_repo,
        3,
        "DestinationInsideRepo",
        "inside an existing Git repository",
    );

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    std::fs::write(outpost.join("dirty.txt"), "dirty\n").expect("dirty outpost");
    let dirty_tree = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../C"]),
    );
    assert_failure_code_contains(&dirty_tree, 3, "DirtyTree", "working tree is dirty");

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    fixture.commit_file(&outpost, "unpushed", "unpushed.txt", "unpushed\n");
    let unpushed_commits = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../C"]),
    );
    assert_failure_code_contains(
        &unpushed_commits,
        3,
        "UnpushedCommits",
        "has unpushed commits",
    );

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    fixture.commit_file(&outpost, "outpost side", "outpost.txt", "from outpost\n");
    fixture.commit_file(
        &fixture.source,
        "source side",
        "source.txt",
        "from source\n",
    );
    let divergence = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    assert_failure_code_contains(&divergence, 4, "Divergence", "history diverges");

    let fixture = common::CliFixture::new();
    let branch_not_found = common::run(fixture.gop().current_dir(&fixture.source).args([
        "add",
        "../D",
        "missing-branch",
    ]));
    assert_failure_code_contains(&branch_not_found, 5, "BranchNotFound", "branch not found");

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let detach = common::run(fixture.git(&outpost).args(["checkout", "--detach"]));
    common::assert_success(&detach, "detach HEAD");
    let no_upstream = common::run(fixture.gop().current_dir(&outpost).arg("pull"));
    assert_failure_code_contains(
        &no_upstream,
        5,
        "NoUpstreamTracking",
        "no upstream tracking configured for branch HEAD",
    );

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let set_tag_upstream = common::run(fixture.git(&outpost).args([
        "config",
        "--local",
        "branch.main.merge",
        "refs/tags/v1",
    ]));
    common::assert_success(&set_tag_upstream, "set tag upstream");
    let upstream_not_branch = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../C"]),
    );
    assert_failure_code_contains(
        &upstream_not_branch,
        5,
        "UpstreamNotABranch",
        "upstream is not a branch ref",
    );

    let fixture = common::CliFixture::new();
    let invalid_ref = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "../D", "--", "-evil"]),
    );
    assert_failure_code_contains(&invalid_ref, 5, "InvalidRefName", "invalid ref name: -evil");

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    fixture.commit_file(&outpost, "outpost side", "outpost.txt", "from outpost\n");
    let checked_out_policy = common::run(fixture.git(&fixture.source).args([
        "config",
        "--local",
        "receive.denyCurrentBranch",
        "refuse",
    ]));
    common::assert_success(&checked_out_policy, "set checked-out branch policy");
    let push_checked_out = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    assert_failure_code_contains(
        &push_checked_out,
        4,
        "PushIntoCheckedOutBranch",
        "cannot push to a non-bare checked-out branch",
    );

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let outpost_only_branch =
        common::run(
            fixture
                .git(&outpost)
                .args(["switch", "-c", "feature/outpost-only"]),
        );
    common::assert_success(&outpost_only_branch, "create outpost-only branch");
    fixture.commit_file(
        &outpost,
        "outpost-only",
        "outpost-only.txt",
        "outpost-only\n",
    );
    let ambiguous_branch = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    assert_failure_code_contains(
        &ambiguous_branch,
        4,
        "AmbiguousBranchCreation",
        "does not exist on the source repository",
    );

    let fixture = common::CliFixture::new();
    fixture.add_outpost("C");
    let lock = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["lock", "../C"]),
    );
    common::assert_success(&lock, "lock outpost");
    let locked_remove = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../C"]),
    );
    assert_failure_code_contains(&locked_remove, 3, "OutpostLocked", "outpost is locked");

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    std::fs::remove_dir_all(&outpost).expect("remove managed outpost");
    std::fs::create_dir(&outpost).expect("create unrelated replacement");
    std::fs::write(outpost.join("keep.txt"), "keep\n").expect("write unrelated file");
    let not_managed = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "--force", "../C"]),
    );
    assert_failure_code_contains(
        &not_managed,
        6,
        "RegistryEntryNotManaged",
        "not a managed outpost of this source",
    );

    let fixture = common::CliFixture::new();
    let unregistered = fixture.outpost("unregistered");
    std::fs::create_dir(&unregistered).expect("create unregistered dir");
    let not_found = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../unregistered"]),
    );
    assert_failure_code_contains(
        &not_found,
        6,
        "RegistryEntryNotFound",
        "registry entry not found",
    );

    let fixture = common::CliFixture::new();
    fixture.add_outpost("C");
    std::fs::write(fixture.source.join(".outpost").join("registry.json"), "{\n")
        .expect("write invalid registry");
    let bad_registry = common::run(fixture.gop().current_dir(&fixture.source).arg("list"));
    assert_failure_code_contains(&bad_registry, 6, "BadRegistry", "invalid registry file");

    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let unset_remote = common::run(fixture.git(&outpost).args([
        "config",
        "--local",
        "--unset",
        "outpost.remoteName",
    ]));
    common::assert_success(&unset_remote, "unset outpost remote metadata");
    let bad_metadata = common::run(fixture.gop().current_dir(&outpost).arg("pull"));
    assert_failure_code_contains(&bad_metadata, 6, "BadMetadata", "invalid outpost metadata");

    let fixture = common::CliFixture::new();
    let baseline = fixture.commit_file(
        &fixture.source,
        "baseline tracked file",
        "tracked.txt",
        "base\n",
    );
    let push_baseline = common::run(
        fixture
            .git(&fixture.source)
            .args(["push", "origin", "main"]),
    );
    common::assert_success(&push_baseline, "push baseline source commit");
    let outpost = fixture.add_outpost("C");
    fixture.commit_file(&outpost, "outpost side", "outpost.txt", "from outpost\n");
    std::fs::write(fixture.source.join("tracked.txt"), "dirty source\n")
        .expect("dirty checked-out source file");
    assert_eq!(
        fixture.git_capture(&fixture.upstream, ["rev-parse", "main"]),
        baseline
    );
    let git_failed = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    assert_failure_code_contains(&git_failed, 1, "GitFailed", "git command failed");

    let fixture = common::CliFixture::new();
    let missing_cwd = fixture.root.join("missing-cwd");
    let io_at = common::run(fixture.gop().arg("-C").arg(&missing_cwd).arg("status"));
    assert_failure_code_contains(&io_at, 70, "IoAt", "io error at");
}

#[test]
fn e_09_no_color_flag_and_env_do_not_emit_ansi_output() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let flag = common::run(
        fixture
            .gop()
            .current_dir(&outpost)
            .args(["--no-color", "status"]),
    );
    common::assert_success(&flag, "gop --no-color status");
    assert_no_ansi(&flag, "--no-color status");

    let env = common::run(
        fixture
            .gop()
            .current_dir(&outpost)
            .env("NO_COLOR", "1")
            .arg("status"),
    );
    common::assert_success(&env, "NO_COLOR=1 gop status");
    assert_no_ansi(&env, "NO_COLOR=1 status");
}

#[test]
fn e_12_global_c_changes_effective_cwd() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let direct = common::run(fixture.gop().current_dir(&outpost).arg("status"));
    common::assert_success(&direct, "direct status");

    let with_c = common::run(
        fixture
            .gop()
            .current_dir(&fixture.root)
            .arg("-C")
            .arg(&outpost)
            .arg("status"),
    );
    common::assert_success(&with_c, "status with -C");

    assert_eq!(common::stdout(&direct), common::stdout(&with_c));

    let remove = common::run(
        fixture
            .gop()
            .current_dir(&fixture.root)
            .arg("-C")
            .arg(&fixture.source)
            .args(["remove", "../C"]),
    );
    common::assert_success(&remove, "remove relative to -C source");
    assert!(
        !outpost.exists(),
        "relative path argument should be resolved against -C source"
    );

    let add = common::run(
        fixture
            .gop()
            .current_dir(&fixture.root)
            .arg("-C")
            .arg(&fixture.source)
            .args(["add", "../D", "main"]),
    );
    common::assert_success(&add, "add relative to -C source");
    assert!(
        fixture.outpost("D").exists(),
        "add destination should be resolved against -C source"
    );
}

#[test]
fn e_13_add_detach_is_rejected_by_clap() {
    let output = common::run(common::gop_command().args(["add", "--detach", "C", "main"]));
    common::assert_usage_error(&output, "--detach");
}

#[test]
fn add_without_new_branch_still_requires_path_or_name() {
    let output = common::run(common::gop_command().arg("add"));
    common::assert_usage_error(&output, "PATH|NAME");
}

#[test]
fn e_14_add_target_branch_starting_with_dash_returns_invalid_ref() {
    let fixture = common::CliFixture::new();
    let output = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "C", "--", "-evil"]),
    );

    common::assert_failure_code(&output, 5, "gop add C -- -evil");
    let stderr = common::stderr(&output);
    assert!(
        stderr.contains("invalid ref name: -evil"),
        "stderr did not report InvalidRefName:\n{stderr}"
    );
    assert!(
        !stderr.contains("git command failed"),
        "stderr reported GitFailed instead of InvalidRefName:\n{stderr}"
    );
}

#[test]
fn e_15_deferred_and_removed_surfaces_are_rejected_by_clap() {
    let cases = [
        (vec!["--json", "status"], "--json"),
        (vec!["--quiet", "status"], "--quiet"),
        (vec!["add", "-B", "feature", "C", "main"], "-B"),
        (vec!["add", "-f", "C", "main"], "-f"),
        (vec!["add", "--checkout", "C", "main"], "--checkout"),
        (
            vec!["add", "--no-update-instead", "C", "main"],
            "--no-update-instead",
        ),
        (vec!["add", "--no-checkout", "C", "main"], "--no-checkout"),
        (vec!["add", "--orphan", "feature", "C"], "--orphan"),
        (vec!["add", "--lock", "C", "main"], "--lock"),
        (vec!["list", "--all"], "--all"),
        (vec!["list", "--porcelain"], "--porcelain"),
        (vec!["list", "-z"], "-z"),
        (vec!["lock", "--outpost", "abcde"], "--outpost"),
        (vec!["unlock", "--outpost", "abcde"], "--outpost"),
        (vec!["move", "--outpost", "abcde", "D"], "--outpost"),
        (vec!["remove", "--outpost", "abcde"], "--outpost"),
        (vec!["prune", "--expire", "now"], "--expire"),
        (vec!["pull", "--update-source"], "--update-source"),
        (vec!["pull", "--rebase"], "--rebase"),
        (vec!["pull", "--merge"], "--merge"),
        (vec!["pull", "--autostash"], "--autostash"),
        (vec!["push", "--to-upstream"], "--to-upstream"),
        (vec!["push", "--source-branch", "main"], "--source-branch"),
        (
            vec!["push", "--upstream-remote", "origin"],
            "--upstream-remote",
        ),
    ];

    for (args, flag) in cases {
        let output = common::run(common::gop_command().args(args));
        common::assert_usage_error(&output, flag);
    }
}

#[test]
fn config_unknown_keys_are_rejected_by_clap() {
    for args in [
        vec!["config", "get", "unknown-key"],
        vec!["config", "set", "unknown-key", "."],
        vec!["config", "unset", "unknown-key"],
    ] {
        let output = common::run(common::gop_command().args(args));
        common::assert_failure_code(&output, 2, "config unknown key");
        let stderr = common::stderr(&output);
        assert!(
            stderr.contains("unknown config key: unknown-key"),
            "stderr should explain unknown config key:\n{stderr}"
        );
    }
}

fn path(value: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(value)
}

fn assert_no_ansi(output: &std::process::Output, label: &str) {
    assert!(
        !contains_ansi_escape(&output.stdout),
        "{label} stdout contains ANSI escapes:\n{}",
        common::stdout(output)
    );
    assert!(
        !contains_ansi_escape(&output.stderr),
        "{label} stderr contains ANSI escapes:\n{}",
        common::stderr(output)
    );
}

fn contains_ansi_escape(bytes: &[u8]) -> bool {
    bytes.contains(&b'\x1b')
}

fn assert_failure_code_contains(
    output: &std::process::Output,
    code: i32,
    label: &str,
    expected_stderr: &str,
) {
    common::assert_failure_code(output, code, label);
    let stderr = common::stderr(output);
    assert!(
        stderr.contains(expected_stderr),
        "{label} stderr did not contain {expected_stderr:?}:\n{stderr}"
    );
}
