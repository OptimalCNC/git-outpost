# Evidence Pack: Phase 0 / core-foundation

## Scope

- Phase: `phase-0`
- Chunk: `core-foundation`
- Roadmap scope: Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture
- Test IDs in this chunk: U-07, U-08
- Source docs reviewed: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Progress log: `.agents-artifacts/progress/phase-0.md`

## Changed Files

- `.gitignore`
- `Cargo.toml`
- `Cargo.lock`
- `crates/core/Cargo.toml`
- `crates/core/src/lib.rs`
- `crates/core/src/error.rs`
- `crates/core/src/reporter.rs`

## Moves / Renames

- none

## Diff Summary

- Added a root Cargo workspace with `crates/core` as the only member, workspace edition/rust-version, and workspace `thiserror` dependency.
- Added `outpost-core` crate manifest.
- Added public module exports for `error` and `reporter`.
- Implemented `OutpostError`, `OutpostResult`, and `OutpostError::exit_code` per architecture sections 5.1 and 9.
- Implemented `Reporter` and `StepKind` per architecture section 5.9.0.
- Added `/target/` to `.gitignore` because Phase 0 introduces Cargo build output.
- Generated `Cargo.lock`.

## Tests Added / Updated

- U-07: `crates/core/src/error.rs::tests::display_strings_match_snapshot`
- U-08: `crates/core/src/error.rs::tests::exit_code_maps_each_variant`

## Integration Tests

- none; QA plan identified these Phase 0 test IDs as developer-owned colocated unit tests.

## Docs Added / Updated

- none. Product, architecture, and roadmap edits were out of scope; no stable implementation concept beyond the existing architecture needed extra docs.

## Verification

- `cargo test -p outpost-core`: pass; 2 unit tests passed, 0 doctests.
- `cargo test -p outpost-core --tests`: pass; 2 unit tests passed.
- `cargo test --workspace`: pass; 2 unit tests passed, 0 doctests.

## Protected Path Exceptions

- none

## Architecture Deviations

- none.
- Implementation note: `PushIntoCheckedOutBranch` uses raw field identifier `r#source` internally so `thiserror` does not treat the field as an error source; construction still uses `source: ...`.

## Residual Risks / Handoff Notes

- `git.rs`, `refname.rs`, and fixture work remain for later Phase 0 chunks.
- No Phase 1+ command behavior was added.
