# Evidence Pack: list-basic-summaries

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `list-basic-summaries`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.9.2 `ops/list.rs`, 10.2 fixture design, 11.3 L-01..L-10
- Roadmap test IDs advanced: L-01, L-02, L-03, L-04, L-07, L-08, L-09, L-10
- Roadmap test IDs deferred to next chunk: L-05, L-06

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/list-basic-summaries/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/list-basic-summaries.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/list.rs`
- `crates/core/tests/list.rs`
- `crates/core/tests/common/fixture.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports the new `list` operation module.
- `ops/list.rs`
  - Adds `OutpostSummary`, `OutpostState`, and `run(&SourceRepo)`.
  - Reads the source registry and returns one summary per registered entry.
  - Classifies missing paths as `Missing`.
  - Opens existing managed outposts through the managed-outpost-of-source safety gate, preserving hermetic env threading and source ownership validation.
  - Maps non-repo, non-managed, or wrong-source existing paths to `NotManaged` instead of failing the whole list.
  - Reports current branch when available, clean/dirty state via real Git status, and registry lock fields.
  - Leaves `ahead_behind` as `None`; L-05/L-06 are assigned to `list-ahead-behind`.
- `tests/common/fixture.rs`
  - Adds `add_outpost` and `dirty_outpost` helpers for core list integration tests.
  - Uses a silent reporter helper so fixture-created outposts exercise the same `ops::add::run` path as production code.
- `tests/list.rs`
  - Adds QA-owned core integration coverage for L-01..L-04 and L-07..L-10.

## Tests Added / Updated

- Unit tests added: none; list behavior is covered through core integration tests against real Git repositories.

## Integration Tests Added / Updated

- `list_empty_source_returns_no_summaries` covers L-01.
- `list_reports_three_added_outpost_paths` covers L-02.
- `list_reports_current_branch_for_each_outpost` covers L-03.
- `list_reports_dirty_for_untracked_outpost_file` covers L-04.
- `list_reports_missing_registered_outpost` covers L-07.
- `list_reports_not_managed_registered_path` covers L-08.
- `list_reports_wrong_source_outpost_as_not_managed` covers the normal-review regression for registered paths that now contain outposts managed by another source.
- `list_outside_source_repo_returns_not_a_repo` covers L-09.
- `list_includes_lock_reason_from_registry` covers L-10.

## Docs Added / Updated

- none
- Rationale: product and architecture already document stable list behavior; this chunk implements those contracts without introducing new developer-facing concepts.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test list`: pass; 9 list integration tests
- `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 9 list integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 9 list integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 9 list integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 22 add integration tests, 9 list integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed list-basic behavior.
- `ahead_behind` is always `None` in this chunk because L-05 and L-06 are assigned to `list-ahead-behind`.

## Residual Risks / Handoff Notes

- Normal-review fix adopted: list now treats a registered path as `NotManaged` when the path contains a managed outpost whose `outpost.sourceRepo` resolves to a different source.
- The next chunk should implement `Outpost::ahead_behind_source`/list ahead-behind support for L-05/L-06.
- CLI formatting and dispatch remain Phase 5 scope.
