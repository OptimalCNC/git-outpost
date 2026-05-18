# Evidence Pack: prune

## Phase And Chunk

- Phase: `phase-2`
- Chunk: `prune`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.9.13 `ops/prune.rs`, 11.11 prune tests
- Roadmap test IDs advanced: Pr-01, Pr-02, Pr-03, Pr-04, Pr-05, Pr-06, Pr-07, Pr-08, Pr-09

## Changed Files

- `.agents-artifacts/progress/phase-2.md`
- `.agents-artifacts/reviews/phase-2/prune/evidence-pack.md`
- `.agents-artifacts/qa/phase-2/prune.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/prune.rs`
- `crates/core/tests/prune.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports the Phase 2 `prune` op module.
- `ops/prune.rs`
  - Adds `PruneOptions`, `PruneReport`, and `run(&SourceRepo, PruneOptions)`.
  - Classifies registry entries in the documented order: locked, missing, source-missing managed outpost, then keep existing paths.
  - Removes only stale registry entries, and only when `dry_run=false`.
  - Never deletes filesystem content or source-repo branches.
  - Keeps `verbose` as report-independent core input; CLI formatting remains out of scope.
- `tests/prune.rs`
  - Adds QA-owned core integration coverage for Pr-01..Pr-09.

## Tests Added / Updated

- Unit tests added: none; prune behavior is covered through core integration tests against real fixture repos.

## Integration Tests Added / Updated

- `prune_removes_missing_registry_entries` covers Pr-01.
- `prune_keeps_existing_valid_outposts` covers Pr-02.
- `prune_does_not_delete_real_dirs_or_source_branches` covers Pr-03.
- `prune_report_lists_removed_missing_entries` covers Pr-04.
- `prune_keeps_unrelated_dirs_and_wrong_source_outposts_registered` covers Pr-05.
- `prune_dry_run_reports_but_does_not_modify_registry` covers Pr-06.
- `prune_reports_existing_outpost_whose_source_repo_is_missing` covers Pr-07.
- `prune_keeps_locked_stale_entries_and_reports_locked` covers Pr-08.
- `prune_report_removed_entries_is_independent_of_verbose` covers Pr-09.

## Docs Added / Updated

- none
- Rationale: product and architecture already document stable prune behavior, classification ordering, report fields, and Pr-01..Pr-09 test inventory.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test prune`: pass; 9 prune integration tests
- `cargo test -p outpost-core`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed prune behavior.

## Residual Risks / Handoff Notes

- CLI dispatch and `-v` formatting remain Phase 5 scope.
- Registry file locking/concurrency remains post-MVP and out of scope.
