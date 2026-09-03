mod common;

use std::path::Path;
use std::process::Output;

fn query(
    fixture: &common::CliFixture,
    shell: &str,
    cwd: &Path,
    words: &[&str],
    index: usize,
) -> Output {
    let mut command = fixture.gop();
    command
        .current_dir(cwd)
        .env("COMPLETE", shell)
        .env("_CLAP_COMPLETE_INDEX", index.to_string())
        .env("_CLAP_IFS", "\n")
        .arg("--")
        .args(words);
    common::run(&mut command)
}

fn has_candidate(output: &Output, candidate: &str) -> bool {
    common::stdout(output).lines().any(|line| line == candidate)
}

#[test]
fn complete_bash_registers_gop() {
    let output = common::run(common::gop_command().env("COMPLETE", "bash"));

    common::assert_success(&output, "bash completion registration");
    let stdout = common::stdout(&output);
    assert!(stdout.contains("_clap_complete_gop"), "{stdout}");
    assert!(stdout.contains("gop"), "{stdout}");
}

#[test]
fn complete_zsh_registers_gop() {
    let output = common::run(common::gop_command().env("COMPLETE", "zsh"));

    common::assert_success(&output, "zsh completion registration");
    let stdout = common::stdout(&output);
    assert!(stdout.contains("_clap_dynamic_completer_gop"), "{stdout}");
    assert!(stdout.contains("gop"), "{stdout}");
}

#[test]
fn complete_rejects_unsupported_shell() {
    let output = common::run(common::gop_command().env("COMPLETE", "fish"));

    common::assert_failure_code(&output, 2, "fish completion registration");
    assert!(
        common::stdout(&output).is_empty(),
        "{:?}",
        common::stdout(&output)
    );
    assert!(
        common::stderr(&output).contains("fish"),
        "{}",
        common::stderr(&output)
    );
}

#[test]
fn git_outpost_does_not_activate_completion() {
    let output = common::run(common::git_outpost_command().env("COMPLETE", "bash"));

    common::assert_failure_code(&output, 2, "git-outpost with COMPLETE");
    assert!(
        !common::stdout(&output).contains("_clap_complete_gop"),
        "{}",
        common::stdout(&output)
    );
    assert!(
        common::stderr(&output).contains("Usage: git-outpost"),
        "{}",
        common::stderr(&output)
    );
}

#[test]
fn completion_uses_source_context_for_remove_and_outpost_context_for_cd() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let id = outpost_core::OutpostId::derive(&fixture.source, &outpost);
    let id = &id.as_str()[..5];

    let remove_from_source = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);
    common::assert_success(&remove_from_source, "remove completion from source");
    assert!(has_candidate(&remove_from_source, id));

    let cd_from_outpost = query(&fixture, "bash", &outpost, &["gop", "cd", ""], 2);
    common::assert_success(&cd_from_outpost, "cd completion from outpost");
    assert!(has_candidate(&cd_from_outpost, id));

    let remove_from_outpost = query(&fixture, "bash", &outpost, &["gop", "remove", ""], 2);
    common::assert_success(&remove_from_outpost, "remove completion from outpost");
    assert!(!has_candidate(&remove_from_outpost, id));
}
