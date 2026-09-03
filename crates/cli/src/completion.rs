use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use clap::CommandFactory as _;
use clap_complete::env::{Bash, EnvCompleter, Shells, Zsh};
use clap_complete::{ArgValueCandidates, CompleteEnv, CompletionCandidate};
use outpost_core::ops::list::{self, OutpostHead, OutpostState, OutpostSummary};
use outpost_core::{Outpost, OutpostError, SourceRepo};

use crate::cli::Cli;

const CLAP_OPTION_CANDIDATE_ID_PREFIX: &str = "arg::";

#[derive(Clone, Copy, PartialEq, Eq)]
enum CandidatePolicy {
    ExistingFromAssociatedSource,
    ExistingFromSource,
    RegisteredFromSource,
}

#[derive(Clone, Copy)]
struct GopBash;

#[derive(Clone, Copy)]
struct GopZsh;

pub(crate) fn try_complete(bin: &str, argv: &[OsString]) -> bool {
    if bin != "gop" {
        return false;
    }

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|base| completion_cwd_from_argv(&base, argv));
    let factory_cwd = cwd.clone();
    CompleteEnv::with_factory(move || command_for(factory_cwd.clone()))
        .bin("gop")
        .shells(Shells(&[&GopBash, &GopZsh]))
        .try_complete(argv.iter().cloned(), cwd.as_deref())
        .unwrap_or_else(|err| err.exit())
}

impl EnvCompleter for GopBash {
    fn name(&self) -> &'static str {
        Bash.name()
    }

    fn is(&self, name: &str) -> bool {
        Bash.is(name)
    }

    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        Bash.write_registration(var, name, bin, completer, buf)
    }

    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let completions = filtered_completions(cmd, args, current_dir, false)?;
        write_completion_records(buf, &completions, |candidate| {
            candidate.get_value().to_string_lossy().into_owned()
        })
    }
}

impl EnvCompleter for GopZsh {
    fn name(&self) -> &'static str {
        Zsh.name()
    }

    fn is(&self, name: &str) -> bool {
        Zsh.is(name)
    }

    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        Zsh.write_registration(var, name, bin, completer, buf)
    }

    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let completions = filtered_completions(cmd, args, current_dir, true)?;
        write_completion_records(buf, &completions, |candidate| {
            let mut record = escape_zsh(&candidate.get_value().to_string_lossy(), true);
            if let Some(help) = candidate.get_help() {
                record.push(':');
                record.push_str(&escape_zsh(
                    help.to_string().lines().next().unwrap_or_default(),
                    false,
                ));
            }
            record
        })
    }
}

fn filtered_completions(
    cmd: &mut clap::Command,
    mut args: Vec<OsString>,
    current_dir: Option<&Path>,
    append_missing_current_word: bool,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    let index = std::env::var("_CLAP_COMPLETE_INDEX")
        .ok()
        .and_then(|index| index.parse::<usize>().ok())
        .unwrap_or_default();
    if append_missing_current_word && index == args.len() {
        args.push(OsString::new());
    }

    let include_flags = args
        .get(index)
        .is_some_and(|arg| arg.as_encoded_bytes().starts_with(b"-"));
    let mut completions = clap_complete::engine::complete(cmd, args, index, current_dir)?;
    if !include_flags {
        completions.retain(|candidate| !is_flag_candidate(candidate));
    }
    Ok(completions)
}

fn is_flag_candidate(candidate: &CompletionCandidate) -> bool {
    candidate
        .get_id()
        .is_some_and(|id| id.starts_with(CLAP_OPTION_CANDIDATE_ID_PREFIX))
}

fn write_completion_records(
    buf: &mut dyn std::io::Write,
    completions: &[CompletionCandidate],
    format: impl Fn(&CompletionCandidate) -> String,
) -> Result<(), std::io::Error> {
    let separator = std::env::var("_CLAP_IFS").unwrap_or_else(|_| "\n".to_owned());
    for (index, candidate) in completions.iter().enumerate() {
        if index != 0 {
            write!(buf, "{separator}")?;
        }
        write!(buf, "{}", format(candidate))?;
    }
    Ok(())
}

fn escape_zsh(value: &str, escape_colon: bool) -> String {
    let value = value.replace('\\', "\\\\");
    if escape_colon {
        value.replace(':', "\\:")
    } else {
        value
    }
}

fn completion_help_path(path: &Path) -> String {
    let mut rendered = String::new();
    for character in path.display().to_string().chars() {
        if character.is_control() {
            rendered.extend(character.escape_default());
        } else {
            rendered.push(character);
        }
    }
    rendered
}

fn command_for(cwd: Option<PathBuf>) -> clap::Command {
    let command = Cli::command().name("gop").bin_name("gop");
    let command = command.mut_subcommand("cd", |command| {
        command_with_candidates(
            command,
            "outpost",
            cwd.clone(),
            CandidatePolicy::ExistingFromAssociatedSource,
        )
    });
    let command = command.mut_subcommand("path", |command| {
        command_with_path_candidates(command, cwd.clone())
    });
    let command = command.mut_subcommand("lock", |command| {
        command_with_candidates(
            command,
            "outpost_path",
            cwd.clone(),
            CandidatePolicy::ExistingFromAssociatedSource,
        )
    });
    let command = command.mut_subcommand("unlock", |command| {
        command_with_candidates(
            command,
            "outpost_path",
            cwd.clone(),
            CandidatePolicy::ExistingFromAssociatedSource,
        )
    });
    let command = command.mut_subcommand("move", |command| {
        command_with_move_candidates(command, cwd.clone())
    });
    let command = command.mut_subcommand("remove", |command| {
        command_with_candidates(
            command,
            "outpost_path",
            cwd.clone(),
            CandidatePolicy::RegisteredFromSource,
        )
    });
    command.mut_subcommand("analyze", |command| {
        command_with_candidates(
            command,
            "outpost_path",
            cwd.clone(),
            CandidatePolicy::ExistingFromAssociatedSource,
        )
    })
}

fn command_with_candidates(
    command: clap::Command,
    arg_id: &str,
    cwd: Option<PathBuf>,
    policy: CandidatePolicy,
) -> clap::Command {
    command.mut_arg(arg_id, move |arg| {
        let cwd = cwd.clone();
        arg.add(ArgValueCandidates::new(move || {
            outpost_candidates(cwd.as_deref(), policy)
        }))
    })
}

fn command_with_move_candidates(command: clap::Command, cwd: Option<PathBuf>) -> clap::Command {
    command_with_candidates(
        command,
        "outpost_path",
        cwd,
        CandidatePolicy::ExistingFromSource,
    )
    .mut_arg("outpost_path", |arg| arg.index(1))
    .mut_arg("new_path", |arg| arg.index(2))
}

fn command_with_path_candidates(command: clap::Command, cwd: Option<PathBuf>) -> clap::Command {
    command.mut_arg("target", move |arg| {
        let cwd = cwd.clone();
        arg.add(ArgValueCandidates::new(move || {
            let mut candidates = vec![CompletionCandidate::new("src")];
            candidates.extend(outpost_candidates(
                cwd.as_deref(),
                CandidatePolicy::ExistingFromAssociatedSource,
            ));
            candidates
        }))
    })
}

fn completion_cwd_from_argv(base: &Path, argv: &[OsString]) -> Option<PathBuf> {
    let Some(separator) = argv.iter().position(|arg| arg == "--") else {
        return Some(base.to_path_buf());
    };
    let mut words = &argv[separator + 1..];
    if words.first().is_some_and(|word| word == "gop") {
        words = &words[1..];
    }

    let mut override_path = None;
    while let Some((word, rest)) = words.split_first() {
        if word == "--" {
            break;
        }
        if word == "-C" {
            let Some((value, after)) = rest.split_first() else {
                return None;
            };
            if value.is_empty() || value == "--" || override_path.is_some() {
                return None;
            }
            override_path = Some(value.clone());
            words = after;
            continue;
        }
        if let Some(value) = attached_cd_value(word) {
            if value.is_empty() || override_path.is_some() {
                return None;
            }
            override_path = Some(value);
        }
        words = rest;
    }

    override_path.map_or_else(
        || Some(base.to_path_buf()),
        |path| Some(base.join(PathBuf::from(path))),
    )
}

fn attached_cd_value(word: &OsStr) -> Option<OsString> {
    let value = word.as_encoded_bytes().strip_prefix(b"-C")?;
    let value = value.strip_prefix(b"=").unwrap_or(value);
    // The ASCII prefix ends at a valid encoded-byte boundary; the suffix came from this OsStr.
    Some(unsafe { OsString::from_encoded_bytes_unchecked(value.to_vec()) })
}

fn outpost_candidates(cwd: Option<&Path>, policy: CandidatePolicy) -> Vec<CompletionCandidate> {
    let Some(cwd) = cwd else {
        return Vec::new();
    };
    let Some(discovered) = SourceRepo::discover(cwd).ok() else {
        return Vec::new();
    };
    let source = match Outpost::at(discovered.work_tree()) {
        Ok(outpost) if policy == CandidatePolicy::ExistingFromAssociatedSource => {
            outpost.source_repo().ok()
        }
        Ok(_) => None,
        Err(OutpostError::NotAnOutpost(_)) => Some(discovered),
        Err(_) => None,
    };
    let Some(source) = source else {
        return Vec::new();
    };
    let Ok(outposts) = list::run(&source) else {
        return Vec::new();
    };

    candidates_from_summaries(&outposts, policy)
}

fn candidates_from_summaries(
    outposts: &[OutpostSummary],
    policy: CandidatePolicy,
) -> Vec<CompletionCandidate> {
    let include_missing = policy == CandidatePolicy::RegisteredFromSource;
    outposts
        .iter()
        .filter(|outpost| include_missing || outpost.path.exists())
        .map(|outpost| {
            let path = completion_help_path(&outpost.path);
            let help = match &outpost.state {
                OutpostState::Present {
                    head: OutpostHead::Attached(branch),
                    ..
                } => format!("{path} [{branch}]"),
                OutpostState::Present {
                    head: OutpostHead::Detached,
                    ..
                } => format!("{path} [detached]"),
                OutpostState::Missing | OutpostState::NotManaged => path.to_string(),
            };
            CompletionCandidate::new(outpost.display_id.clone()).help(Some(help.into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use clap_complete::{ArgValueCandidates, CompletionCandidate};
    use outpost_core::ops::list::{OutpostHead, OutpostState, OutpostSummary};

    use super::*;

    #[test]
    fn completion_cwd_uses_only_one_non_empty_command_cd_override() {
        let base = PathBuf::from("/work/base");

        let separated = os_args(["/bin/gop", "--", "gop", "-C", "one", "remove", ""]);
        assert_eq!(
            completion_cwd_from_argv(&base, &separated),
            Some(base.join("one"))
        );

        let attached = os_args(["/bin/gop", "--", "gop", "-Cone", "remove", ""]);
        assert_eq!(
            completion_cwd_from_argv(&base, &attached),
            Some(base.join("one"))
        );

        let equals = os_args(["/bin/gop", "--", "gop", "-C=one", "remove", ""]);
        assert_eq!(
            completion_cwd_from_argv(&base, &equals),
            Some(base.join("one"))
        );

        let registration = os_args(["/bin/gop", "-C", "ignored"]);
        assert_eq!(
            completion_cwd_from_argv(&base, &registration),
            Some(base.clone())
        );

        let ignored_before_outer =
            os_args(["/bin/gop", "-C", "ignored", "--", "gop", "remove", ""]);
        assert_eq!(
            completion_cwd_from_argv(&base, &ignored_before_outer),
            Some(base.clone())
        );

        let missing = os_args(["/bin/gop", "--", "gop", "remove", "-C"]);
        assert_eq!(completion_cwd_from_argv(&base, &missing), None);

        let separator_as_value = os_args(["/bin/gop", "--", "gop", "-C", "--", "remove", ""]);
        assert_eq!(completion_cwd_from_argv(&base, &separator_as_value), None);

        let empty_equals = os_args(["/bin/gop", "--", "gop", "remove", "-C="]);
        assert_eq!(completion_cwd_from_argv(&base, &empty_equals), None);

        let empty_separated = os_args(["/bin/gop", "--", "gop", "-C", "", "remove", ""]);
        assert_eq!(completion_cwd_from_argv(&base, &empty_separated), None);

        let repeated = os_args([
            "/bin/gop", "--", "gop", "-C", "one", "-C", "two", "remove", "",
        ]);
        assert_eq!(completion_cwd_from_argv(&base, &repeated), None);

        let terminated = os_args([
            "/bin/gop", "--", "gop", "-Cone", "remove", "--", "-C", "ignored",
        ]);
        assert_eq!(
            completion_cwd_from_argv(&base, &terminated),
            Some(base.join("one"))
        );
    }

    #[test]
    fn candidates_filter_missing_paths_without_losing_registered_hints() {
        let temp = tempfile::tempdir().expect("tempdir");
        let live_path = temp.path().join("live");
        fs::create_dir(&live_path).expect("live path");
        let stale_path = temp.path().join("stale");
        let outposts = [
            OutpostSummary {
                display_id: "abcde0".to_owned(),
                path: live_path.clone(),
                state: OutpostState::Present {
                    head_oid: "0".repeat(40),
                    head: OutpostHead::Detached,
                },
                locked: false,
                lock_reason: None,
            },
            OutpostSummary {
                display_id: "abcde1".to_owned(),
                path: stale_path.clone(),
                state: OutpostState::Missing,
                locked: false,
                lock_reason: None,
            },
        ];

        assert_eq!(
            candidate_values(candidates_from_summaries(
                &outposts,
                CandidatePolicy::ExistingFromAssociatedSource,
            ),),
            ["abcde0"]
        );
        assert_eq!(
            candidate_values(candidates_from_summaries(
                &outposts,
                CandidatePolicy::ExistingFromSource
            ),),
            ["abcde0"]
        );
        let registered =
            candidates_from_summaries(&outposts, CandidatePolicy::RegisteredFromSource);
        assert_eq!(candidate_values(&registered), ["abcde0", "abcde1"]);
        assert_eq!(
            candidate_helps(&registered),
            [
                format!("{} [detached]", live_path.display()),
                stale_path.display().to_string(),
            ]
        );
    }

    #[test]
    fn command_factory_attaches_candidates_to_only_supported_selector_arguments() {
        let command = command_for(None);
        assert_eq!(command.get_name(), "gop");
        assert_eq!(command.get_bin_name(), Some("gop"));

        for (subcommand, arg_id) in [
            ("cd", "outpost"),
            ("path", "target"),
            ("lock", "outpost_path"),
            ("unlock", "outpost_path"),
            ("move", "outpost_path"),
            ("remove", "outpost_path"),
            ("analyze", "outpost_path"),
        ] {
            let argument = command
                .find_subcommand(subcommand)
                .expect("subcommand")
                .get_arguments()
                .find(|argument| argument.get_id() == arg_id)
                .expect("argument");
            assert!(argument.get::<ArgValueCandidates>().is_some());
        }

        let move_new_path = command
            .find_subcommand("move")
            .expect("move subcommand")
            .get_arguments()
            .find(|argument| argument.get_id() == "new_path")
            .expect("new_path argument");
        assert!(move_new_path.get::<ArgValueCandidates>().is_none());
    }

    #[test]
    fn provider_errors_fail_closed_for_each_policy() {
        let temp = tempfile::tempdir().expect("tempdir");

        for policy in [
            CandidatePolicy::ExistingFromAssociatedSource,
            CandidatePolicy::ExistingFromSource,
            CandidatePolicy::RegisteredFromSource,
        ] {
            assert!(outpost_candidates(None, policy).is_empty());
            assert!(outpost_candidates(Some(temp.path()), policy).is_empty());
        }
    }

    #[test]
    fn runtime_entry_rejects_other_binary_names() {
        assert!(!try_complete("git-outpost", &os_args(["/bin/git-outpost"])));
    }

    fn os_args<const N: usize>(args: [&str; N]) -> Vec<OsString> {
        args.into_iter().map(OsString::from).collect()
    }

    fn candidate_values(candidates: impl AsRef<[CompletionCandidate]>) -> Vec<String> {
        candidates
            .as_ref()
            .iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect()
    }

    fn candidate_helps(candidates: &[CompletionCandidate]) -> Vec<String> {
        candidates
            .iter()
            .map(|candidate| candidate.get_help().expect("candidate help").to_string())
            .collect()
    }
}
