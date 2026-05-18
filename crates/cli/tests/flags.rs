mod common;

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
