# QA Notes: lock-move-unlock

## Scope

- Phase: `phase-2`
- Chunk: `lock-move-unlock`
- Test file: `crates/core/tests/lock_move_unlock.rs`
- QA focus: core integration behavior for `ops::lock`, `ops::unlock`, and `ops::move`, not CLI E2E behavior.

## Covered IDs

- LMU-01: `lock_with_reason_marks_registry_entry_locked`
- LMU-02: `unlock_clears_registry_lock_state_and_reason`
- LMU-03: `move_updates_filesystem_and_registry_path`
- LMU-04: `move_refuses_locked_outpost_without_force`
- LMU-05: `move_force_moves_locked_outpost_and_preserves_lock`
- LMU-06: `move_refuses_dirty_outpost_but_force_succeeds`
- LMU-07: `move_refuses_non_empty_destination`
- LMU-08: `lock_move_unlock_reject_unregistered_paths`, `lock_move_unlock_reject_wrong_source_registered_path`

## Verification

- `cargo test -p outpost-core --test lock_move_unlock`: pass; 9 tests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test
