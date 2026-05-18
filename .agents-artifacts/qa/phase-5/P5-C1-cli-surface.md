# P5-C1 CLI Surface QA Note

## Scope

- Chunk: `P5-C1-cli-surface`
- Test IDs covered: E-01, E-03, E-13, E-15, H-01, H-02, H-03
- Test files:
  - `crates/cli/tests/flags.rs`
  - `crates/cli/tests/help.rs`
  - `crates/cli/tests/common/mod.rs`

## Implemented Tests

- `e_01_build_produces_both_binaries`
- `e_03_help_lists_commands_and_long_flags`
- `e_13_add_detach_is_rejected_by_clap`
- `e_15_deferred_and_removed_surfaces_are_rejected_by_clap`
- `h_01_git_outpost_help_uses_git_outpost_name`
- `h_02_gop_help_uses_gop_name`
- `h_03_git_dispatch_help_does_not_use_gop_name`

## Verification

- `cargo fmt --check`: pass
- `cargo build -p git-outpost`: pass
- `cargo test -p git-outpost --tests`: pass; 7 CLI integration tests
- `cargo test -p outpost-core`: pass
- `cargo test -p outpost-core --tests`: pass
- `cargo test --workspace`: pass
- `git diff --check`: pass

## Deferred To Later Phase 5 Chunks

- E-02, E-04, E-05, E-06, E-10, E-11, E-12, and E-14 remain planned for `P5-C2-dispatch-e2e`.
- E-07, E-08, and E-09 remain planned for `P5-C3-exit-color-platform-hardening`.
