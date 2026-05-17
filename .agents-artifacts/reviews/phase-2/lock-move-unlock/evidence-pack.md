# Evidence Pack: lock-move-unlock

## Phase And Chunk

- Phase: `phase-2`
- Chunk: `lock-move-unlock`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.8 safety helpers, 5.9.9 `ops/lock.rs`, 5.9.10 `ops/move.rs`, 5.9.11 `ops/unlock.rs`, 11.4 LMU tests
- Roadmap test IDs advanced: LMU-01, LMU-02, LMU-03, LMU-04, LMU-05, LMU-06, LMU-07, LMU-08

## Changed Files

- `.agents-artifacts/progress/phase-2.md`
- `.agents-artifacts/reviews/phase-2/lock-move-unlock/evidence-pack.md`
- `.agents-artifacts/qa/phase-2/lock-move-unlock.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/lock.rs`
- `crates/core/src/ops/move.rs`
- `crates/core/src/ops/unlock.rs`
- `crates/core/src/outpost.rs`
- `crates/core/tests/lock_move_unlock.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports `lock`, raw-identifier `r#move`, and `unlock` modules.
- `ops/lock.rs`
  - Adds `LockOptions` and `run(&SourceRepo, LockOptions)`.
  - Requires a registered canonical path before validating managed-source ownership.
  - Sets registry lock state and reason, then saves.
- `ops/unlock.rs`
  - Adds `UnlockOptions` and `run(&SourceRepo, UnlockOptions)`.
  - Requires a registered canonical path before validating managed-source ownership.
  - Clears registry lock state and reason, then saves.
- `ops/move.rs`
  - Adds `MoveOptions` and `run(&SourceRepo, MoveOptions)`.
  - Requires an existing registered path, refuses locked entries unless forced, validates managed-source ownership, refuses dirty outposts unless forced, validates destination safety, renames the directory, then updates and saves the registry path.
  - Preserves existing lock fields through registry `update_path`.
- `outpost.rs`
  - Adds a crate-private `git()` accessor so move can reuse the outpost's hermetic `GitInvoker` for clean-tree checks.
- `tests/lock_move_unlock.rs`
  - Adds QA-owned core integration coverage for LMU-01..LMU-08.
  - Uses direct registry locking only as setup for move/unlock cases; Phase 2 behavior is asserted through `ops::*::run`.

## Tests Added / Updated

- Unit tests added: none; LMU behavior is covered through core integration tests against real Git repositories.

## Integration Tests Added / Updated

- `lock_with_reason_marks_registry_entry_locked` covers LMU-01.
- `unlock_clears_registry_lock_state_and_reason` covers LMU-02.
- `move_updates_filesystem_and_registry_path` covers LMU-03.
- `move_refuses_locked_outpost_without_force` covers LMU-04.
- `move_force_moves_locked_outpost_and_preserves_lock` covers LMU-05.
- `move_refuses_dirty_outpost_but_force_succeeds` covers LMU-06.
- `move_refuses_non_empty_destination` covers LMU-07.
- `lock_move_unlock_reject_unregistered_paths` and `lock_move_unlock_reject_wrong_source_registered_path` cover LMU-08.

## Docs Added / Updated

- none
- Rationale: product and architecture already document stable lock, move, and unlock behavior; this chunk implements those contracts without adding new developer-facing concepts.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test lock_move_unlock`: pass; 9 lock/move/unlock integration tests
- `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed lock/move/unlock behavior.

## Residual Risks / Handoff Notes

- `ops::remove` and `ops::prune` remain unimplemented and are assigned to later Phase 2 chunks.
- CLI contextual path omission for lock/unlock remains Phase 5 scope.
