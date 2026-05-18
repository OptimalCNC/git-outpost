# P5-C3 Exit Color Platform Hardening Evidence Pack

## Chunk

- Chunk: `P5-C3-exit-color-platform-hardening`
- Scope: copied-outpost degradation, CLI exit-code coverage, `--no-color`/`NO_COLOR`, status health output hardening, and cross-platform test hardening.
- Test IDs advanced: E-07, E-08, E-09.
- Out of scope: new command surfaces, global registry behavior, unrelated docs cleanup, unrelated refactors.

## Implementation Summary

- Hardened status output to print `health: ok` when no problems are present and `health: problems` before problem details when degraded.
- Added a Rust recursive copy helper for CLI tests, including platform-specific symlink handling for Unix and Windows.
- Added E-07 CLI integration coverage that copies an outpost with Rust filesystem APIs, deletes the source repository, proves ordinary Git still works in the copy, and verifies degraded `gop status` output.
- Added E-08 coverage with a complete `OutpostError` variant-to-exit-code table plus representative black-box CLI exit-code smoke cases for documented user-facing error buckets.
- Added E-09 coverage that verifies `gop --no-color status` and `NO_COLOR=1 gop status` emit no ANSI escape bytes on stdout or stderr.

## Files Changed

- `crates/cli/src/output.rs`
- `crates/cli/tests/common/mod.rs`
- `crates/cli/tests/e2e.rs`
- `crates/cli/tests/flags.rs`
- `.agents-artifacts/progress/phase-5.md`
- `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`
- `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`

## Tests Added / Advanced

- `crates/cli/tests/e2e.rs::e_07_copied_outpost_is_git_independent_when_source_is_missing`
- `crates/cli/tests/flags.rs::e_08_outpost_errors_map_to_documented_exit_codes`
- `crates/cli/tests/flags.rs::e_08_cli_errors_return_documented_exit_codes`
- `crates/cli/tests/flags.rs::e_09_no_color_flag_and_env_do_not_emit_ansi_output`

## Verification

- `cargo fmt --check`: pass
- `cargo build -p git-outpost`: pass; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture.
- `cargo test -p git-outpost --tests`: pass; 9 E2E tests, 8 flags tests, 4 help tests.
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests.
- `cargo test --workspace`: pass; 21 CLI integration tests plus existing core coverage, 0 doctests.
- `git diff --check`: pass.

## Notes

- The roadmap calls for ANSI matching via `strip-ansi-escapes`; the local test uses an equivalent stricter assertion that rejects any ESC byte and avoids adding a new dev dependency.
- Some `OutpostError` variants, such as `GitTerminatedBySignal`, are not practical to force through a stable black-box CLI fixture. E-08 therefore combines exhaustive variant mapping with representative actual CLI failures.
- Existing unrelated local changes were left unstaged: `crates/cli/Cargo.toml`, `.github/`, and `README.md`.
