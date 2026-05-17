# QA Notes: list-basic-summaries

## Scope

- Phase: `phase-1`
- Chunk: `list-basic-summaries`
- Test file: `crates/core/tests/list.rs`
- QA focus: core integration behavior for `ops::list`, not CLI E2E behavior.

## Covered IDs

- L-01: `list_empty_source_returns_no_summaries`
- L-02: `list_reports_three_added_outpost_paths`
- L-03: `list_reports_current_branch_for_each_outpost`
- L-04: `list_reports_dirty_for_untracked_outpost_file`
- L-07: `list_reports_missing_registered_outpost`
- L-08: `list_reports_not_managed_registered_path`
- L-09: `list_outside_source_repo_returns_not_a_repo`
- L-10: `list_includes_lock_reason_from_registry`

## Deferred IDs

- L-05, L-06: ahead/behind counts, assigned to `list-ahead-behind`.

## Verification

- `cargo test -p outpost-core --test list`: pass; 8 tests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 8 list integration tests, 1 fixture smoke test
