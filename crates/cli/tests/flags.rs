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
        0
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
    let outpost = fixture.add_outpost("C");

    let not_a_repo = common::run(fixture.gop().current_dir(&fixture.root).arg("status"));
    common::assert_failure_code(&not_a_repo, 2, "status outside repo");

    let wrong_context = common::run(fixture.gop().current_dir(&fixture.source).arg("pull"));
    common::assert_failure_code(&wrong_context, 2, "pull from source");

    let destination_exists = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "../C", "main"]),
    );
    common::assert_failure_code(&destination_exists, 3, "add existing destination");

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
    common::assert_failure_code(&locked_remove, 3, "remove locked outpost");
    let unlock = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["unlock", "../C"]),
    );
    common::assert_success(&unlock, "unlock outpost");

    let checked_out_policy = common::run(fixture.git(&fixture.source).args([
        "config",
        "--local",
        "receive.denyCurrentBranch",
        "refuse",
    ]));
    common::assert_success(&checked_out_policy, "set checked-out branch policy");
    let push_checked_out = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    common::assert_failure_code(&push_checked_out, 4, "push into checked-out source branch");

    let missing_branch = common::run(fixture.gop().current_dir(&fixture.source).args([
        "add",
        "../D",
        "missing-branch",
    ]));
    common::assert_failure_code(&missing_branch, 5, "add missing branch");

    let bad_registry = common::run(fixture.git(&fixture.source).args([
        "config",
        "--local",
        "receive.denyCurrentBranch",
        "updateInstead",
    ]));
    common::assert_success(&bad_registry, "restore source push policy");
    std::fs::write(fixture.source.join(".outpost").join("registry.json"), "{\n")
        .expect("write invalid registry");
    let list = common::run(fixture.gop().current_dir(&fixture.source).arg("list"));
    common::assert_failure_code(&list, 6, "list with invalid registry");
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
