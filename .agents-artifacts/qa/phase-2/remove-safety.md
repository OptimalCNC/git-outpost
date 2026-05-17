# QA Notes: remove-safety

## Scope

- Phase: `phase-2`
- Chunk: `remove-safety`
- Test file: `crates/core/tests/remove.rs`
- QA focus: core integration behavior for `ops::remove`, not CLI E2E behavior.

## Covered IDs

- R-01: `remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry`
- R-02: `remove_dirty_outpost_returns_dirty_tree_with_force_hint`
- R-03: `remove_unpushed_outpost_returns_unpushed_commits`
- R-04: `remove_force_deletes_dirty_outpost`
- R-05: `remove_force_deletes_outpost_with_unpushed_commits`
- R-06: `remove_unregistered_path_returns_registry_entry_not_found`
- R-07: `remove_unlocked_missing_registered_path_deregisters_without_rmtree`
- R-08: `remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed`
- R-09: `remove_wrong_source_outpost_returns_not_managed`
- R-10: `remove_refuses_locked_outpost_unless_forced`
- R-11: `remove_locked_missing_path_requires_force_then_deregisters`

## Verification

- QA worker `019e384b-ddb5-7fd2-96b5-a99e956f0a8c`: `cargo test -p outpost-core --test remove` passed, 11 tests.
- Coordinator rerun: `cargo test -p outpost-core --test remove` passed, 11 tests.
- Coordinator rerun: `cargo test -p outpost-core --tests` passed with the remove integration suite included.
