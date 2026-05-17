# Evidence Pack: Phase 0 / git-and-ref-boundary

## Scope

- Phase: `phase-0`
- Chunk: `git-and-ref-boundary`
- Roadmap scope: Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture
- Test IDs in this chunk: U-09, U-11, U-12
- Source docs reviewed: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Progress log: `.agents-artifacts/progress/phase-0.md`

## Changed Files

- `.agents-artifacts/progress/phase-0.md`
- `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md`
- `crates/core/Cargo.toml`
- `crates/core/src/lib.rs`
- `crates/core/src/git.rs`
- `crates/core/src/refname.rs`

## Moves / Renames

- none

## Diff Summary

- Added `test-helpers` feature to `outpost-core` so public test-only helpers can be enabled for integration tests.
- Exported `git` and `refname` modules and their Phase 0 public types.
- Implemented `GitInvoker` with pinned cwd, explicit env overrides, stdout/stderr capture, `run_capture`, `run_check`, `run_status`, signal handling, failed-argv display, and `argv_log` gated behind `#[cfg(any(test, feature = "test-helpers"))]`.
- Implemented validated newtypes `BranchName`, `RefName`, `RemoteName`, `SourceRemoteRef`, and `UpstreamRef::short_branch`.
- Added colocated unit tests covering U-09, U-11, and U-12 plus `run_status` and supporting ref parsing behavior.

## Tests Added / Updated

- U-09: `crates/core/src/git.rs::tests::run_check_bad_command_preserves_failed_argv`
- U-11: `crates/core/src/git.rs::tests::run_capture_keeps_leading_dash_value_positional_after_separator`
- U-12:
  - `crates/core/src/refname.rs::tests::branch_parse_rejects_leading_dash_and_accepts_feature_branch`
  - `crates/core/src/refname.rs::tests::remote_parse_rejects_shell_like_value`
- Additional support tests:
  - `crates/core/src/git.rs::tests::run_status_distinguishes_exit_one_from_real_failure`
  - `crates/core/src/refname.rs::tests::ref_parse_uses_full_ref_validation`
  - `crates/core/src/refname.rs::tests::source_remote_ref_parses_remote_and_branch`
  - `crates/core/src/refname.rs::tests::upstream_short_branch_returns_only_heads_refs`

## Integration Tests

- none; QA plan identified these Phase 0 test IDs as developer-owned colocated unit tests.

## Docs Added / Updated

- none. Product, architecture, and roadmap edits were out of scope; no stable implementation concept beyond the existing architecture needed extra docs.

## Verification

- `cargo fmt --check`: pass.
- `cargo test -p outpost-core`: pass; 10 unit tests passed, 0 doctests.
- `cargo test -p outpost-core --tests`: pass; 10 unit tests passed.
- `cargo test --workspace`: pass; 10 unit tests passed, 0 doctests.
- `cargo test -p outpost-core --features test-helpers`: pass; 10 unit tests passed, 0 doctests.

## Protected Path Exceptions

- none

## Architecture Deviations

- none.

## Residual Risks / Handoff Notes

- The Phase 0 fixture scaffold remains for the next chunk.
- No Phase 1+ command behavior was added.
