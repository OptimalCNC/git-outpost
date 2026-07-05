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
        "remove", "prune", "status", "analyze", "config", "path", "cd", "shell",
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
        "--no-branch-cleanup",
        "--dry-run",
    ] {
        assert!(help.contains(flag), "expected {flag} in help:\n{help}");
    }

    for (args, flags) in [
        (&["add", "--help"][..], &["--remote-name"][..]),
        (&["list", "--help"][..], &["--verbose"][..]),
        (&["lock", "--help"][..], &["--reason"][..]),
        (&["move", "--help"][..], &["--force"][..]),
        (
            &["remove", "--help"][..],
            &["--force", "--no-branch-cleanup"][..],
        ),
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

    let cd_help = help_for(&["cd", "--help"]);
    for token in [
        "shell integration",
        "gop shell install",
        "gop shell init",
        "OUTPOST",
    ] {
        assert!(
            cd_help.contains(token),
            "expected {token} in cd help:\n{cd_help}"
        );
    }

    let config_help = help_for(&["config", "--help"]);
    for token in ["set", "get", "unset", "list", "show", "outpost-container"] {
        assert!(
            config_help.contains(token),
            "expected {token} in config help:\n{config_help}"
        );
    }

    let shell_help = help_for(&["shell", "--help"]);
    for token in ["init", "install", "uninstall", "shell integration"] {
        assert!(
            shell_help.contains(token),
            "expected {token} in shell help:\n{shell_help}"
        );
    }

    let shell_init_help = help_for(&["shell", "init", "--help"]);
    for token in ["Print shell integration", "gop cd", "SHELL", "bash", "zsh"] {
        assert!(
            shell_init_help.contains(token),
            "expected {token} in shell init help:\n{shell_init_help}"
        );
    }

    let shell_install_help = help_for(&["shell", "install", "--help"]);
    for token in [
        "Install shell integration",
        "SHELL",
        "bash",
        "zsh",
        "--rc-file",
        "--script-file",
    ] {
        assert!(
            shell_install_help.contains(token),
            "expected {token} in shell install help:\n{shell_install_help}"
        );
    }

    let shell_uninstall_help = help_for(&["shell", "uninstall", "--help"]);
    for token in [
        "Uninstall shell integration",
        "SHELL",
        "bash",
        "zsh",
        "--rc-file",
        "--script-file",
    ] {
        assert!(
            shell_uninstall_help.contains(token),
            "expected {token} in shell uninstall help:\n{shell_uninstall_help}"
        );
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
