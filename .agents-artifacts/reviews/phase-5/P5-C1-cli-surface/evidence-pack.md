# P5-C1 CLI Surface Evidence Pack

## Chunk

- Chunk ID: `P5-C1-cli-surface`
- Phase: `phase-5`
- Scope: add `git-outpost` CLI crate, workspace membership, both binaries, clap command tree, help rendering, deferred/removed flag rejection, and initial CLI test harness.
- Test IDs: E-01, E-03, E-13, E-15, H-01, H-02, H-03
- Out of scope: real core command dispatch, command output formatting, `--no-color` behavior beyond clap parsing, exit-code matrix beyond clap usage errors, E2E Story behavior, global registry behavior, unrelated docs cleanup, unrelated refactors.

## Files Changed

- `Cargo.toml`
- `Cargo.lock`
- `crates/cli/Cargo.toml`
- `crates/cli/src/main.rs`
- `crates/cli/src/cli.rs`
- `crates/cli/tests/common/mod.rs`
- `crates/cli/tests/flags.rs`
- `crates/cli/tests/help.rs`
- `.agents-artifacts/progress/phase-5.md`
- `.agents-artifacts/qa/phase-5/P5-C1-cli-surface.md`
- `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/evidence-pack.md`

## Implementation Summary

- Added workspace member `crates/cli` and package `git-outpost`.
- Added two binary targets, `git-outpost` and `gop`, both using `src/main.rs`.
- Added `clap` workspace dependency constrained to the `4.5` line and CLI package dependency on `outpost-core`.
- Added top-level CLI parser with global `-C <path>` and `--no-color`.
- Added command tree for `add`, `pull`, `source pull`, `merge`, `rebase`, `push`, `list`, `lock`, `unlock`, `move`, `remove`, `prune`, and `status`.
- Added CLI boundary validation for branch names, remote names, and source remote refs using existing core newtypes.
- Added placeholder parse/validate-only execution. Real command dispatch remains P5-C2 scope.
- Added CLI test helpers for binary path lookup, Git dispatch PATH setup, command execution, and clap usage assertions.

## Test Coverage Added

- E-01: `crates/cli/tests/flags.rs::e_01_build_produces_both_binaries`
- E-03: `crates/cli/tests/help.rs::e_03_help_lists_commands_and_long_flags`
- E-13: `crates/cli/tests/flags.rs::e_13_add_detach_is_rejected_by_clap`
- E-15: `crates/cli/tests/flags.rs::e_15_deferred_and_removed_surfaces_are_rejected_by_clap`
- H-01: `crates/cli/tests/help.rs::h_01_git_outpost_help_uses_git_outpost_name`
- H-02: `crates/cli/tests/help.rs::h_02_gop_help_uses_gop_name`
- H-03: `crates/cli/tests/help.rs::h_03_git_dispatch_help_does_not_use_gop_name`

## Verification

- `cargo fmt --check`: pass
- `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is used by both bin targets, matching the Phase 5 architecture.
- `cargo test -p git-outpost --tests`: pass; 3 `flags` tests and 4 `help` tests.
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests.
- `cargo test --workspace`: pass; 7 CLI integration tests plus existing core coverage, 0 doctests.
- `git diff --check`: pass
- Review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass
  - `cargo test -p git-outpost --tests`: pass; 3 `flags` tests and 4 `help` tests with hardened E-03/E-15 coverage
  - `cargo test -p outpost-core`: pass
  - `cargo test -p outpost-core --tests`: pass
  - `cargo test --workspace`: pass; 7 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass

## Notes For Reviewers

- `git outpost --help` is intercepted by Git 2.43 as a manpage request before external command dispatch. H-03 therefore uses `git outpost -h`, which Git forwards to `git-outpost`; `docs/src/architecture.md` now records this acceptance reality.
- Root help includes a concise after-help line listing command-specific long flags so E-03 can assert the complete documented surface before command dispatch exists.
- E-03 also checks subcommand help for actual command-owned long flags so the test cannot pass only because of the root after-help summary.
- E-15 includes representative removed/deferred global, add, list, prune, pull, and push surfaces.
- No real command dispatch is implemented in this chunk; successful non-help invocations only parse and validate refs. P5-C2 owns dispatch.

## Review Fixes

- Constrained `clap` to `>=4.5, <4.6`; the resolved `clap 4.5.61` line declares `rust-version = "1.74"` in the crates.io index and is compatible with the project Rust 1.75 MSRV.
- Updated H-03 source acceptance docs from `git outpost --help` to `git outpost -h`, because Git intercepts the literal `--help` form before dispatching external commands.
- Strengthened E-03 with subcommand help assertions for real command-owned long flags.
- Expanded E-15 representative coverage to include removed/deferred add, list, and push flags.
