# QA Notes: prune

## Scope

- Phase: `phase-2`
- Chunk: `prune`
- Test file: `crates/core/tests/prune.rs`
- QA focus: core integration behavior for `ops::prune`, not CLI E2E behavior.

## Covered IDs

- Pr-01: `prune_removes_missing_registry_entries`
- Pr-02: `prune_keeps_existing_valid_outposts`
- Pr-03: `prune_does_not_delete_real_dirs_or_source_branches`
- Pr-04: `prune_report_lists_removed_missing_entries`
- Pr-05: `prune_keeps_unrelated_dirs_and_wrong_source_outposts_registered`
- Pr-06: `prune_dry_run_reports_but_does_not_modify_registry`
- Pr-07: `prune_reports_existing_outpost_whose_source_repo_is_missing`
- Pr-08: `prune_keeps_locked_stale_entries_and_reports_locked`
- Pr-09: `prune_report_removed_entries_is_independent_of_verbose`

## Verification

- QA worker `019e3864-f449-7a90-90dc-eb0ac78df901`: `cargo fmt --check` passed.
- QA worker `019e3864-f449-7a90-90dc-eb0ac78df901`: `cargo test -p outpost-core --test prune` passed, 9 tests.
- Coordinator rerun: `cargo test -p outpost-core --test prune` passed, 9 tests.
- Coordinator rerun: `cargo test -p outpost-core --tests` passed with the prune integration suite included.
