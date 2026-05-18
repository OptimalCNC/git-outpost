# P5-C2 Dispatch E2E Evidence Pack

## Chunk

- Chunk: `P5-C2-dispatch-e2e`
- Scope: CLI dispatch to core ops, effective cwd/global `-C`, context classification, stdout/stderr rendering, `StderrReporter`, CLI E2E fixture.
- Test IDs advanced: E-02, E-04, E-05, E-06, E-10, E-11, E-12, E-14.
- Out of scope: E-07 copied-outpost degradation, E-08 full exit-code matrix, E-09 ANSI/color assertions.

## Implementation Summary

- Added dispatch in `crates/cli/src/main.rs` for every Phase 5 command, using `outpost-core` operations and the product Working Directory Matrix.
- Added `CliError` for CLI-only context errors while preserving `OutpostError` exit codes.
- Added `StderrReporter` for core progress events.
- Added human-readable output formatting for add/list/status/pull/source-pull/merge/rebase/push/prune.
- Added CLI-local A/B/C fixture using real Git repositories and hermetic Git env.
- Resolved user path arguments against the effective cwd after global `-C`.

## Files Changed

- `Cargo.lock`
- `crates/cli/Cargo.toml`
- `crates/cli/src/cli.rs`
- `crates/cli/src/main.rs`
- `crates/cli/src/exit.rs`
- `crates/cli/src/output.rs`
- `crates/cli/src/reporter_impls.rs`
- `crates/cli/tests/common/mod.rs`
- `crates/cli/tests/e2e.rs`
- `crates/cli/tests/flags.rs`

## Tests Added / Advanced

- `crates/cli/tests/e2e.rs::e_02_invocation_forms_produce_same_status_stdout`
- `crates/cli/tests/e2e.rs::e_04_basic_cli_lifecycle_round_trip_exits_zero`
- `crates/cli/tests/e2e.rs::e_05_push_makes_outpost_commit_visible_upstream`
- `crates/cli/tests/e2e.rs::e_06_two_outposts_round_trip_via_source`
- `crates/cli/tests/e2e.rs::e_10_story_flow_exits_zero`
- `crates/cli/tests/e2e.rs::e_11_merge_and_rebase_accept_story_source_ref`
- `crates/cli/tests/flags.rs::e_12_global_c_changes_effective_cwd`
- `crates/cli/tests/flags.rs::e_14_add_target_branch_starting_with_dash_returns_invalid_ref`

## Verification

- `cargo fmt --check`: pass
- `cargo build -p git-outpost`: pass; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture.
- `cargo test -p git-outpost --tests`: pass; 6 E2E tests, 5 flags tests, 4 help tests.
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests.
- `cargo test --workspace`: pass; 15 CLI integration tests plus existing core coverage, 0 doctests.
- `git diff --check`: pass.

## Notes

- The CLI fixture uses sibling outpost paths such as `../C` from the source repo. This preserves the core invariant that outposts are separate checkouts outside the source working tree; core tests intentionally reject source-contained destinations like `C`.
- Output formatting is deliberately human-readable and minimal. Full color/NO_COLOR hardening remains P5-C3.
