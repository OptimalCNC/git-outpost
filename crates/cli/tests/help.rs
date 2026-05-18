mod common;

fn count_command_line(help: &str, command: &str) -> usize {
    let prefix = format!("  {command}");
    help.lines()
        .filter(|line| {
            line.strip_prefix(&prefix)
                .is_some_and(|rest| rest.starts_with(' ') || rest.is_empty())
        })
        .count()
}

fn help_for(args: &[&str]) -> String {
    let output = common::run(common::gop_command().args(args));
    assert!(
        output.status.success(),
        "help for {args:?} failed:\n{}",
        common::stderr(&output)
    );
    common::stdout(&output)
}

#[test]
fn e_03_help_lists_commands_and_long_flags() {
    let output = common::run(common::gop_command().arg("--help"));
    assert!(
        output.status.success(),
        "help failed:\n{}",
        common::stderr(&output)
    );

    let help = common::stdout(&output);
    for command in [
        "add", "pull", "source", "merge", "rebase", "push", "list", "lock", "unlock", "move",
        "remove", "prune", "status",
    ] {
        assert_eq!(
            count_command_line(&help, command),
            1,
            "expected {command} exactly once in help:\n{help}"
        );
    }

    for flag in [
        "--no-color",
        "--remote-name",
        "--reason",
        "--verbose",
        "--force",
        "--dry-run",
    ] {
        assert!(help.contains(flag), "expected {flag} in help:\n{help}");
    }

    for (args, flags) in [
        (&["add", "--help"][..], &["--remote-name"][..]),
        (&["list", "--help"][..], &["--verbose"][..]),
        (&["lock", "--help"][..], &["--reason"][..]),
        (&["move", "--help"][..], &["--force"][..]),
        (&["remove", "--help"][..], &["--force"][..]),
        (&["prune", "--help"][..], &["--dry-run", "--verbose"][..]),
    ] {
        let subcommand_help = help_for(args);
        for flag in flags {
            assert!(
                subcommand_help.contains(flag),
                "expected {flag} in help for {args:?}:\n{subcommand_help}"
            );
        }
    }
}

#[test]
fn h_01_git_outpost_help_uses_git_outpost_name() {
    let output = common::run(common::git_outpost_command().arg("--help"));
    assert!(
        output.status.success(),
        "help failed:\n{}",
        common::stderr(&output)
    );

    let help = common::stdout(&output);
    assert!(help.contains("Usage: git-outpost"), "{help}");
    assert!(!help.contains("Usage: gop"), "{help}");
}

#[test]
fn h_02_gop_help_uses_gop_name() {
    let output = common::run(common::gop_command().arg("--help"));
    assert!(
        output.status.success(),
        "help failed:\n{}",
        common::stderr(&output)
    );

    let help = common::stdout(&output);
    assert!(help.contains("Usage: gop"), "{help}");
}

#[test]
fn h_03_git_dispatch_help_does_not_use_gop_name() {
    // Git intercepts `git outpost --help` as a manpage request before running
    // external commands, while `-h` is forwarded to `git-outpost`.
    let output = common::run(common::git_dispatch_command().arg("-h"));
    assert!(
        output.status.success(),
        "git dispatch help failed:\n{}",
        common::stderr(&output)
    );

    let help = common::stdout(&output);
    assert!(
        help.contains("Usage: git-outpost") || help.contains("Usage: git outpost"),
        "{help}"
    );
    assert!(!help.contains("Usage: gop"), "{help}");
}
