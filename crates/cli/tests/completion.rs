mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Output;

fn query(
    fixture: &common::CliFixture,
    shell: &str,
    cwd: &Path,
    words: &[&str],
    index: usize,
) -> Output {
    query_with_index(fixture, shell, cwd, words, &index.to_string())
}

fn query_with_index(
    fixture: &common::CliFixture,
    shell: &str,
    cwd: &Path,
    words: &[&str],
    index: &str,
) -> Output {
    let mut command = fixture.gop();
    command
        .current_dir(cwd)
        .env("COMPLETE", shell)
        .env("_CLAP_COMPLETE_INDEX", index)
        .env("_CLAP_IFS", "\n")
        .arg("--")
        .args(words);
    common::run(&mut command)
}

fn has_candidate(output: &Output, candidate: &str) -> bool {
    common::stdout(output).lines().any(|line| {
        line == candidate
            || line
                .strip_prefix(candidate)
                .is_some_and(|suffix| suffix.starts_with(':'))
    })
}

fn candidate_help(output: &Output, candidate: &str) -> Option<String> {
    common::stdout(output).lines().find_map(|line| {
        line.strip_prefix(candidate)
            .and_then(|suffix| suffix.strip_prefix(':'))
            .map(str::to_owned)
    })
}

fn flag_candidates(output: &Output) -> Vec<String> {
    common::stdout(output)
        .lines()
        .filter(|line| line.starts_with('-'))
        .map(str::to_owned)
        .collect()
}

fn has_flag_candidate(output: &Output, candidate: &str) -> bool {
    flag_candidates(output).iter().any(|record| {
        record == candidate
            || record
                .strip_prefix(candidate)
                .is_some_and(|suffix| suffix.starts_with(':'))
    })
}

fn assert_no_flag_candidates(output: &Output, label: &str) {
    let flags = flag_candidates(output);
    assert!(
        flags.is_empty(),
        "{label} unexpectedly offered flags {flags:?}\nstdout:\n{}",
        common::stdout(output)
    );
}

fn dynamic_ids(output: &Output) -> BTreeSet<String> {
    common::stdout(output)
        .lines()
        .map(|line| line.split_once(':').map_or(line, |(value, _)| value))
        .filter(|value| {
            (outpost_core::outpost_id::MIN_PREFIX_LEN..=64).contains(&value.len())
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .map(str::to_owned)
        .collect()
}

fn expected_ids(fixture: &common::CliFixture, outposts: &[&Path]) -> Vec<String> {
    let ids = outposts
        .iter()
        .map(|outpost| outpost_core::OutpostId::derive(&fixture.source, outpost))
        .collect::<Vec<_>>();
    outpost_core::outpost_id::shortest_unique_prefixes(ids.iter())
        .expect("distinct fixture IDs")
        .into_iter()
        .map(|prefix| prefix.to_string())
        .collect()
}

fn assert_ids(output: &Output, expected: &[String], label: &str) {
    common::assert_success(output, label);
    assert_eq!(
        dynamic_ids(output),
        expected.iter().cloned().collect(),
        "{label} returned unexpected dynamic IDs\nstdout:\n{}",
        common::stdout(output)
    );
    assert_eq!(common::stderr(output), "", "{label} reported an error");
}

fn assert_no_dynamic_ids(output: &Output, label: &str) {
    common::assert_success(output, label);
    assert!(
        dynamic_ids(output).is_empty(),
        "{label} unexpectedly offered dynamic IDs {:?}",
        dynamic_ids(output)
    );
    assert_eq!(common::stderr(output), "", "{label} reported an error");
}

fn assert_no_candidates(output: &Output, label: &str) {
    common::assert_success(output, label);
    assert_eq!(
        common::stdout(output),
        "",
        "{label} unexpectedly offered candidates"
    );
    assert_eq!(common::stderr(output), "", "{label} reported an error");
}

fn assert_ids_absent(output: &Output, ids: &[String], label: &str) {
    for id in ids {
        assert!(
            !has_candidate(output, id),
            "{label} unexpectedly offered dynamic ID {id}"
        );
    }
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
fn completion_preserves_out_of_range_index_errors() {
    let fixture = common::CliFixture::new();

    for shell in ["bash", "zsh"] {
        let output = query_with_index(
            &fixture,
            shell,
            &fixture.source,
            &["gop", "remove", ""],
            "9",
        );
        assert!(
            !output.status.success(),
            "{shell} completion silently accepted an out-of-range index"
        );
        assert!(
            !common::stderr(&output).is_empty(),
            "{shell} completion omitted the protocol error"
        );
    }
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
fn dynamic_remove_offers_shortest_ids_from_the_source_registry() {
    let fixture = common::CliFixture::new();
    let first = fixture.add_outpost("C");
    let second = fixture.add_outpost("D");
    let expected = expected_ids(&fixture, &[&first, &second]);

    let output = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);

    assert_ids(&output, &expected, "remove completion from source");
}

#[test]
fn zsh_dynamic_outpost_hint_shows_path_and_current_branch() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let branch = "feature/completion-hint";
    let switch = common::run(fixture.git(&outpost).args(["switch", "-c", branch]));
    common::assert_success(&switch, "switch outpost branch");
    let expected_id = expected_ids(&fixture, &[&outpost]).remove(0);
    let expected_help = format!("{} [{branch}]", common::displayed_path(&outpost));

    let zsh = query(&fixture, "zsh", &fixture.source, &["gop", "remove", ""], 2);
    assert_ids(&zsh, std::slice::from_ref(&expected_id), "zsh branch hint");
    assert_eq!(
        candidate_help(&zsh, &expected_id).as_deref(),
        Some(expected_help.as_str())
    );

    let bash = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);
    assert_ids(
        &bash,
        std::slice::from_ref(&expected_id),
        "bash candidate value",
    );
    assert_eq!(candidate_help(&bash, &expected_id), None);
}

#[cfg(unix)]
#[test]
fn zsh_dynamic_outpost_hint_escapes_newlines_in_the_path() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("line\nbreak");
    let expected_id = expected_ids(&fixture, &[&outpost]).remove(0);
    let expected_help = common::displayed_path(&outpost).replace('\n', "\\\\n");

    let output = query(&fixture, "zsh", &fixture.source, &["gop", "remove", ""], 2);

    assert_ids(
        &output,
        std::slice::from_ref(&expected_id),
        "zsh escaped hint",
    );
    assert_eq!(
        candidate_help(&output, &expected_id).as_deref(),
        Some(expected_help.as_str())
    );
}

#[test]
fn zsh_dynamic_outpost_hint_reports_detached_head() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let detach = common::run(fixture.git(&outpost).args(["switch", "--detach"]));
    common::assert_success(&detach, "detach outpost HEAD");
    let expected_id = expected_ids(&fixture, &[&outpost]).remove(0);
    let expected_help = format!("{} [detached]", common::displayed_path(&outpost));

    let output = query(&fixture, "zsh", &fixture.source, &["gop", "remove", ""], 2);

    assert_ids(
        &output,
        std::slice::from_ref(&expected_id),
        "zsh detached hint",
    );
    assert_eq!(
        candidate_help(&output, &expected_id).as_deref(),
        Some(expected_help.as_str())
    );
}

#[test]
fn zsh_stale_remove_candidate_hint_keeps_its_registered_path() {
    let fixture = common::CliFixture::new();
    let stale = fixture.add_outpost("C");
    let expected_id = expected_ids(&fixture, &[&stale]).remove(0);
    let expected_help = common::displayed_path(&stale);
    fs::remove_dir_all(&stale).expect("remove stale checkout");

    let output = query(&fixture, "zsh", &fixture.source, &["gop", "remove", ""], 2);

    assert_ids(
        &output,
        std::slice::from_ref(&expected_id),
        "zsh stale path hint",
    );
    assert_eq!(
        candidate_help(&output, &expected_id).as_deref(),
        Some(expected_help.as_str())
    );
}

#[test]
fn completion_flags_require_a_dash_prefix_across_shell_adapters() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let expected = expected_ids(&fixture, &[&outpost]);

    for shell in ["bash", "zsh"] {
        let selector = query(&fixture, shell, &fixture.source, &["gop", "remove", ""], 2);
        assert_ids(
            &selector,
            &expected,
            &format!("{shell} selector completion"),
        );
        assert_no_flag_candidates(&selector, &format!("{shell} selector completion"));

        for (word, expected_flag) in [("-", "-f"), ("--f", "--force")] {
            let flags = query(
                &fixture,
                shell,
                &fixture.source,
                &["gop", "remove", word],
                2,
            );
            common::assert_success(&flags, &format!("{shell} {word} flag completion"));
            assert!(
                has_flag_candidate(&flags, expected_flag),
                "{shell} {word} completion did not offer {expected_flag}\nstdout:\n{}",
                common::stdout(&flags)
            );
            assert_no_dynamic_ids(&flags, &format!("{shell} {word} flag completion"));
        }
    }
}

#[test]
fn completion_without_dash_never_falls_back_to_flags() {
    let fixture = common::CliFixture::new();

    for shell in ["bash", "zsh"] {
        for (words, index, label) in [
            (&["gop", ""][..], 1, "root completion"),
            (&["gop", "status", ""][..], 2, "non-selector completion"),
            (&["gop", "remove", ""][..], 2, "empty registry completion"),
            (
                &["gop", "remove", "unmatched"][..],
                2,
                "non-dash selector completion",
            ),
        ] {
            let output = query(&fixture, shell, &fixture.source, words, index);
            common::assert_success(&output, &format!("{shell} {label}"));
            assert_no_flag_candidates(&output, &format!("{shell} {label}"));
        }

        let non_git = query(&fixture, shell, &fixture.root, &["gop", "remove", ""], 2);
        common::assert_success(&non_git, &format!("{shell} non-Git completion"));
        assert_no_flag_candidates(&non_git, &format!("{shell} non-Git completion"));
    }
}

#[test]
fn completion_preserves_dash_prefixed_positional_candidates() {
    let fixture = common::CliFixture::new();
    fs::create_dir(fixture.source.join("-dash")).expect("dash-prefixed path");

    for shell in ["bash", "zsh"] {
        let output = query(
            &fixture,
            shell,
            &fixture.source,
            &["gop", "add", "--", ""],
            3,
        );
        common::assert_success(&output, &format!("{shell} dash-prefixed path completion"));
        assert!(
            has_candidate(&output, "-dash/"),
            "{shell} completion omitted the dash-prefixed positional path\nstdout:\n{}",
            common::stdout(&output)
        );
    }
}

#[test]
fn dynamic_selectors_offer_source_scoped_ids_and_path_src() {
    let fixture = common::CliFixture::new();
    let first = fixture.add_outpost("C");
    let second = fixture.add_outpost("D");
    let expected = expected_ids(&fixture, &[&first, &second]);

    for command in ["cd", "path", "lock", "unlock", "move", "remove", "analyze"] {
        let output = query(&fixture, "bash", &fixture.source, &["gop", command, ""], 2);
        assert_ids(&output, &expected, &format!("{command} first selector"));
        if command == "path" {
            assert!(has_candidate(&output, "src"), "path should offer src");
        }
    }

    let second_move = query(
        &fixture,
        "bash",
        &fixture.source,
        &["gop", "move", &expected[0], ""],
        3,
    );
    assert_no_dynamic_ids(&second_move, "move destination");

    let filtered = query(
        &fixture,
        "bash",
        &fixture.source,
        &["gop", "remove", &expected[0]],
        2,
    );
    assert_ids(&filtered, &expected[..1], "remove filtered prefix");
    assert!(
        !has_candidate(&filtered, &expected[1]),
        "remove should filter out non-matching IDs"
    );
}

#[test]
fn dynamic_ids_match_across_shell_adapters_and_sources() {
    let fixture = common::CliFixture::new();
    let first = fixture.add_outpost("C");
    let second = fixture.add_outpost("D");
    let expected = expected_ids(&fixture, &[&first, &second]);

    let bash = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);
    let zsh = query(&fixture, "zsh", &fixture.source, &["gop", "remove", ""], 2);
    assert_ids(&bash, &expected, "bash remove completion");
    assert_ids(&zsh, &expected, "zsh remove completion");
    assert_eq!(
        dynamic_ids(&bash),
        dynamic_ids(&zsh),
        "shell adapters returned different IDs"
    );

    let other = common::CliFixture::new();
    let other_outpost = other.add_outpost("C");
    let other_ids = expected_ids(&other, &[&other_outpost]);
    let first_source = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);
    assert_ids(&first_source, &expected, "first source isolation");
    assert_ids_absent(&first_source, &other_ids, "first source isolation");
}

#[test]
fn dynamic_context_uses_associated_source_or_explicit_cd() {
    let fixture = common::CliFixture::new();
    let first = fixture.add_outpost("C");
    let second = fixture.add_outpost("D");
    let expected = expected_ids(&fixture, &[&first, &second]);

    for command in ["cd", "path", "lock", "unlock", "analyze"] {
        let output = query(&fixture, "bash", &first, &["gop", command, ""], 2);
        assert_ids(&output, &expected, &format!("{command} from outpost"));
    }
    for command in ["move", "remove"] {
        let output = query(&fixture, "bash", &first, &["gop", command, ""], 2);
        assert_no_dynamic_ids(&output, &format!("{command} from outpost"));
    }

    let source = fixture.source.display().to_string();
    for (words, index) in [
        (vec!["gop", "-C", source.as_str(), "remove", ""], 4),
        (vec!["gop", &format!("-C{source}"), "remove", ""], 3),
        (vec!["gop", &format!("-C={source}"), "remove", ""], 3),
        (vec!["gop", "-C", "B", "remove", ""], 4),
    ] {
        let output = query(&fixture, "bash", &fixture.root, &words, index);
        assert_ids(&output, &expected, "remove with explicit -C");
    }
}

#[test]
fn dynamic_remove_keeps_stale_entries_while_other_selectors_hide_them() {
    let fixture = common::CliFixture::new();
    let existing = fixture.add_outpost("C");
    let stale = fixture.add_outpost("D");
    let expected = expected_ids(&fixture, &[&existing, &stale]);
    fs::remove_dir_all(&stale).expect("remove stale checkout");

    let remove = query(&fixture, "bash", &fixture.source, &["gop", "remove", ""], 2);
    assert_ids(&remove, &expected, "remove stale registration");

    for command in ["cd", "path", "lock", "unlock", "move", "analyze"] {
        let output = query(&fixture, "bash", &fixture.source, &["gop", command, ""], 2);
        assert_ids(
            &output,
            &expected[..1],
            &format!("{command} existing registration"),
        );
        assert_ids_absent(
            &output,
            &expected[1..],
            &format!("{command} stale registration"),
        );
    }
}

#[test]
fn dynamic_completion_fails_closed_for_invalid_context_and_registry() {
    let fixture = common::CliFixture::new();
    fixture.add_outpost("C");

    for shell in ["bash", "zsh"] {
        let non_git = query(&fixture, shell, &fixture.root, &["gop", "remove", ""], 2);
        assert_no_candidates(&non_git, &format!("{shell} non-Git completion"));

        for (words, index, label) in [
            (vec!["gop", "-C", "--", "remove", ""], 4, "missing -C"),
            (vec!["gop", "-C", "", "remove", ""], 4, "empty -C"),
            (
                vec!["gop", "-C", "B", "-C", "B", "remove", ""],
                6,
                "repeated -C",
            ),
        ] {
            let output = query(&fixture, shell, &fixture.root, &words, index);
            assert_no_candidates(&output, &format!("{shell} {label}"));
        }
    }

    let registry_path = fixture.source.join(".git/outpost/registry.json");
    let mut registry: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&registry_path).expect("read registry"))
            .expect("parse registry");
    let duplicate = registry["outposts"][0].clone();
    registry["outposts"]
        .as_array_mut()
        .expect("registry outposts")
        .push(duplicate);
    fs::write(
        &registry_path,
        serde_json::to_string(&registry).expect("serialize duplicate registry"),
    )
    .expect("write duplicate registry");
    for shell in ["bash", "zsh"] {
        let duplicate = query(&fixture, shell, &fixture.source, &["gop", "remove", ""], 2);
        assert_no_candidates(
            &duplicate,
            &format!("{shell} duplicate registry completion"),
        );
    }

    fs::write(&registry_path, "{invalid registry").expect("write malformed registry");
    for shell in ["bash", "zsh"] {
        let malformed = query(&fixture, shell, &fixture.source, &["gop", "remove", ""], 2);
        assert_no_candidates(
            &malformed,
            &format!("{shell} malformed registry completion"),
        );
    }
}

#[test]
fn normal_commands_remain_unchanged_without_completion_environment() {
    let fixture = common::CliFixture::new();
    let help = common::run(fixture.gop().arg("--help"));
    common::assert_success(&help, "normal gop help");
    assert!(common::stdout(&help).contains("Manage self-contained Git outposts"));
    assert_eq!(common::stderr(&help), "");

    let status = common::run(fixture.gop().current_dir(&fixture.source).arg("status"));
    common::assert_success(&status, "normal gop status");
    assert!(common::stdout(&status).contains("context: source\n"));
    assert_eq!(common::stderr(&status), "");
}
