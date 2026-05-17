# Evidence Pack: remove-safety

## Phase And Chunk

- Phase: `phase-2`
- Chunk: `remove-safety`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.5 `Outpost::unpushed_commits`, 5.8 safety helpers, 5.9.12 `ops/remove.rs`, 11.10 remove tests
- Roadmap test IDs advanced: R-01, R-02, R-03, R-04, R-05, R-06, R-07, R-08, R-09, R-10, R-11

## Changed Files

- `.agents-artifacts/progress/phase-2.md`
- `.agents-artifacts/reviews/phase-2/remove-safety/evidence-pack.md`
- `.agents-artifacts/qa/phase-2/remove-safety.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/remove.rs`
- `crates/core/src/outpost.rs`
- `crates/core/src/safety.rs`
- `crates/core/tests/remove.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports the Phase 2 `remove` op module.
- `ops/remove.rs`
  - Adds `RemoveOptions` and `run(&SourceRepo, RemoveOptions)`.
  - Requires registry membership before any filesystem deletion.
  - Checks registry lock state before missing-path cleanup, matching architecture order.
  - Deregisters unlocked missing paths without filesystem work.
  - Requires current-source managed outpost validation before deletion.
  - Unless forced, applies dirty and unpushed guards with `"pass --force"` hints.
  - Removes the registry entry and saves before `remove_dir_all`.
- `outpost.rs`
  - Adds `Outpost::unpushed_commits(&SourceRepo)` as required support for remove R-03/R-05.
  - Fetches the configured source remote tracking ref and counts commits ahead of that ref.
  - Adds a focused unit test for local commits ahead of the source.
- `safety.rs`
  - Adds `check_no_unpushed`.
  - Adds a focused unit test for `UnpushedCommits { hint: "pass --force" }`.
- `tests/remove.rs`
  - Adds QA-owned core integration coverage for R-01..R-11.

## Tests Added / Updated

- `outpost::tests::unpushed_commits_reports_local_commits_ahead_of_source`
- `safety::tests::check_no_unpushed_reports_unpushed_commits`

## Integration Tests Added / Updated

- `remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry` covers R-01.
- `remove_dirty_outpost_returns_dirty_tree_with_force_hint` covers R-02.
- `remove_unpushed_outpost_returns_unpushed_commits` covers R-03.
- `remove_force_deletes_dirty_outpost` covers R-04.
- `remove_force_deletes_outpost_with_unpushed_commits` covers R-05.
- `remove_unregistered_path_returns_registry_entry_not_found` covers R-06.
- `remove_unlocked_missing_registered_path_deregisters_without_rmtree` covers R-07.
- `remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed` covers R-08.
- `remove_wrong_source_outpost_returns_not_managed` covers R-09.
- `remove_refuses_locked_outpost_unless_forced` covers R-10.
- `remove_locked_missing_path_requires_force_then_deregisters` covers R-11.

## Docs Added / Updated

- none
- Rationale: product and architecture already document stable remove behavior, unpushed safety support, safety ordering, and R-01..R-11 test inventory.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test remove`: pass; 11 remove integration tests
- `cargo test -p outpost-core`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed remove behavior.

## Residual Risks / Handoff Notes

- `ops::prune` remains unimplemented and is assigned to the remaining Phase 2 chunk.
- CLI dispatch and contextual path behavior remain Phase 5 scope.
- `Outpost::unpushed_commits` was added in this chunk because R-03/R-05 require it; no Phase 3+ status/sync behavior was added.
