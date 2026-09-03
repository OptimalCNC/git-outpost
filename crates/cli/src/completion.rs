use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use clap::CommandFactory as _;
use clap_complete::env::{Bash, Shells, Zsh};
use clap_complete::{ArgValueCandidates, CompleteEnv, CompletionCandidate};
use outpost_core::outpost_id::{DuplicateOutpostIdError, OutpostId, shortest_unique_prefixes};
use outpost_core::{Outpost, OutpostError, SourceRepo};

use crate::cli::Cli;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CandidatePolicy {
    ExistingFromAssociatedSource,
    ExistingFromSource,
    RegisteredFromSource,
}

struct DerivedOutpostCandidate {
    path: PathBuf,
    id: OutpostId,
}

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
        .shells(Shells(&[&Bash, &Zsh]))
        .try_complete(argv.iter().cloned(), cwd.as_deref())
        .unwrap_or_else(|err| err.exit())
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
        command_with_candidates(
            command,
            "outpost_path",
            cwd.clone(),
            CandidatePolicy::ExistingFromSource,
        )
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
    let Ok(registry) = source.registry() else {
        return Vec::new();
    };
    let entries = registry
        .entries()
        .iter()
        .map(|entry| DerivedOutpostCandidate {
            path: entry.path.clone(),
            id: OutpostId::derive(source.work_tree(), &entry.path),
        })
        .collect::<Vec<_>>();

    candidates_from_entries(&entries, policy).unwrap_or_default()
}

fn candidates_from_entries(
    entries: &[DerivedOutpostCandidate],
    policy: CandidatePolicy,
) -> Result<Vec<CompletionCandidate>, DuplicateOutpostIdError> {
    let prefixes = shortest_unique_prefixes(entries.iter().map(|entry| &entry.id))?;
    let include_missing = policy == CandidatePolicy::RegisteredFromSource;
    Ok(entries
        .iter()
        .zip(prefixes)
        .filter(|(entry, _)| include_missing || entry.path.exists())
        .map(|(_, prefix)| CompletionCandidate::new(prefix.to_string()))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use clap_complete::{ArgValueCandidates, CompletionCandidate};
    use outpost_core::outpost_id::{DuplicateOutpostIdError, OutpostId};

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
    fn candidates_calculate_prefixes_before_excluding_missing_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let live_path = temp.path().join("live");
        fs::create_dir(&live_path).expect("live path");
        let stale_path = temp.path().join("stale");
        let live_id =
            OutpostId::parse("abcde00000000000000000000000000000000000000000000000000000000000")
                .expect("live ID");
        let stale_id =
            OutpostId::parse("abcde10000000000000000000000000000000000000000000000000000000000")
                .expect("stale ID");
        let entries = [
            DerivedOutpostCandidate {
                path: live_path,
                id: live_id,
            },
            DerivedOutpostCandidate {
                path: stale_path,
                id: stale_id,
            },
        ];

        assert_eq!(
            candidate_values(
                candidates_from_entries(&entries, CandidatePolicy::ExistingFromAssociatedSource)
                    .expect("distinct IDs"),
            ),
            ["abcde0"]
        );
        assert_eq!(
            candidate_values(
                candidates_from_entries(&entries, CandidatePolicy::ExistingFromSource)
                    .expect("distinct IDs"),
            ),
            ["abcde0"]
        );
        assert_eq!(
            candidate_values(
                candidates_from_entries(&entries, CandidatePolicy::RegisteredFromSource)
                    .expect("distinct IDs"),
            ),
            ["abcde0", "abcde1"]
        );

        let duplicates = [
            DerivedOutpostCandidate {
                path: temp.path().join("duplicate-one"),
                id: entries[0].id.clone(),
            },
            DerivedOutpostCandidate {
                path: temp.path().join("duplicate-two"),
                id: entries[0].id.clone(),
            },
        ];
        assert_eq!(
            candidates_from_entries(&duplicates, CandidatePolicy::RegisteredFromSource),
            Err(DuplicateOutpostIdError)
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

    fn candidate_values(candidates: Vec<CompletionCandidate>) -> Vec<String> {
        candidates
            .iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect()
    }
}
