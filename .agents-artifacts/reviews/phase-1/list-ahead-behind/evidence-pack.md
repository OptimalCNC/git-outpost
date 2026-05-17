# Evidence Pack: list-ahead-behind

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `list-ahead-behind`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.5 `outpost.rs`, 5.9.2 `ops/list.rs`, 11.3 L-05/L-06
- Roadmap test IDs advanced: L-05, L-06

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/list-ahead-behind/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/list-ahead-behind.md`
- `crates/core/src/outpost.rs`
- `crates/core/src/ops/list.rs`
- `crates/core/tests/list.rs`
- `crates/core/tests/common/fixture.rs`

## Moves / Renames

- none

## Diff Summary

- `outpost.rs`
  - Adds `Outpost::ahead_behind_source()`.
  - Reads the current branch and upstream tracking config.
  - Requires the tracked remote to match the outpost metadata remote name.
  - Fetches the tracked source branch into the corresponding remote-tracking ref so source-side commits are visible.
  - Computes `git rev-list --left-right --count <local>...<remote-tracking>` and returns `AheadBehind { ahead, behind }`.
  - Returns existing typed errors for unavailable tracking information; list maps unavailable counts to `None`.
- `ops/list.rs`
  - Populates `OutpostSummary.ahead_behind` for managed outposts through `Outpost::ahead_behind_source()`.
- `tests/common/fixture.rs`
  - Adds `commit_in_outpost` helper for L-05.
- `tests/list.rs`
  - Adds L-05 and L-06 core integration coverage.

## Tests Added / Updated

- Unit tests added: none; behavior is covered through real-Git integration tests.

## Integration Tests Added / Updated

- `list_reports_outpost_ahead_of_source` covers L-05.
- `list_reports_outpost_behind_source` covers L-06.

## Docs Added / Updated

- none
- Rationale: product and architecture already document list ahead/behind summaries and `Outpost::ahead_behind_source`; this chunk implements those contracts without adding new stable concepts.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test list`: pass; 11 list integration tests
- `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed ahead/behind list behavior.

## Residual Risks / Handoff Notes

- CLI formatting and dispatch remain Phase 5 scope.
