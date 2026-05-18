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
fn e_13_add_detach_is_rejected_by_clap() {
    let output = common::run(common::gop_command().args(["add", "--detach", "C", "main"]));
    common::assert_usage_error(&output, "--detach");
}

#[test]
fn e_15_deferred_and_removed_surfaces_are_rejected_by_clap() {
    let cases = [
        (vec!["--json", "status"], "--json"),
        (vec!["--quiet", "status"], "--quiet"),
        (vec!["list", "--all"], "--all"),
        (vec!["prune", "--expire", "now"], "--expire"),
        (vec!["pull", "--update-source"], "--update-source"),
        (vec!["pull", "--rebase"], "--rebase"),
        (vec!["pull", "--merge"], "--merge"),
        (vec!["pull", "--autostash"], "--autostash"),
    ];

    for (args, flag) in cases {
        let output = common::run(common::gop_command().args(args));
        common::assert_usage_error(&output, flag);
    }
}
