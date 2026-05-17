# QA/Test Plan: Phase 2

## Ownership

- QA owns Phase 2 integration tests under `crates/core/tests`, exercising `outpost_core::ops::{lock, unlock, move, remove, prune}` directly against real A/B/C Git fixtures.
- Developers own production code under `crates/core/src/**` plus narrow unit tests for pure helper behavior, especially `Outpost::unpushed_commits` and `safety::check_no_unpushed` required only for remove safety.
- Out of scope: CLI binary tests, global `-C`, contextual CLI dispatch, Phase 3+ `status`, Phase 4 sync commands, registry file locking, unrelated docs/refactors.

## Test Files

| File | IDs |
| --- | --- |
| `crates/core/tests/lock_move_unlock.rs` | LMU-01..LMU-08 |
| `crates/core/tests/remove.rs` | R-01..R-11 |
| `crates/core/tests/prune.rs` | Pr-01..Pr-09 |

## Planned Test Mapping

| ID | Test name |
| --- | --- |
| LMU-01 | `lock_with_reason_marks_registry_entry_locked` |
| LMU-02 | `unlock_clears_registry_lock_state_and_reason` |
| LMU-03 | `move_updates_filesystem_and_registry_path` |
| LMU-04 | `move_refuses_locked_outpost_without_force` |
| LMU-05 | `move_force_moves_locked_outpost_and_preserves_lock` |
| LMU-06 | `move_refuses_dirty_outpost_but_force_succeeds` |
| LMU-07 | `move_refuses_non_empty_destination` |
| LMU-08 | `lock_move_unlock_reject_unregistered_paths` |
| R-01 | `remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry` |
| R-02 | `remove_dirty_outpost_returns_dirty_tree_with_force_hint` |
| R-03 | `remove_unpushed_outpost_returns_unpushed_commits` |
| R-04 | `remove_force_deletes_dirty_outpost` |
| R-05 | `remove_force_deletes_outpost_with_unpushed_commits` |
| R-06 | `remove_unregistered_path_returns_registry_entry_not_found` |
| R-07 | `remove_unlocked_missing_registered_path_deregisters_without_rmtree` |
| R-08 | `remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed` |
| R-09 | `remove_wrong_source_outpost_returns_not_managed` |
| R-10 | `remove_refuses_locked_outpost_unless_forced` |
| R-11 | `remove_locked_missing_path_requires_force_then_deregisters` |
| Pr-01 | `prune_removes_missing_registry_entries` |
| Pr-02 | `prune_keeps_existing_valid_outposts` |
| Pr-03 | `prune_does_not_delete_real_dirs_or_source_branches` |
| Pr-04 | `prune_report_lists_removed_missing_entries` |
| Pr-05 | `prune_keeps_unrelated_dirs_and_wrong_source_outposts_registered` |
| Pr-06 | `prune_dry_run_reports_but_does_not_modify_registry` |
| Pr-07 | `prune_reports_existing_outpost_whose_source_repo_is_missing` |
| Pr-08 | `prune_keeps_locked_stale_entries_and_reports_locked` |
| Pr-09 | `prune_report_removed_entries_is_independent_of_verbose` |

## Fixture Helpers Needed

- Registry assertion helpers such as `single_registry_entry`.
- Direct registry lock helper for remove/prune setup, after LMU covers lock behavior through ops.
- Wrong-source setup using a second fixture or equivalent path replacement pattern.
- Unrelated registered directory setup for R-08 and Pr-05.
- Missing-path setup by removing outpost directories manually.
- `push_outpost_to_source` or equivalent for explicit fully-pushed remove setup if needed.
- Config rewrite helpers for source-missing and wrong-source metadata scenarios.

## Blockers And Risks

- R-03 and R-05 require `Outpost::unpushed_commits` and `safety::check_no_unpushed`; keep this support scoped to remove safety.
- Use direct registry/config mutation only to create corrupt or impossible states. Happy paths should use `ops::add` and Phase 2 `ops::*::run`.
- No CLI, global `-C`, contextual omitted outpost, or registry file locking tests in Phase 2.

## Recommended First Step

Implement `ops::lock`, `ops::unlock`, and `ops::move` with `crates/core/tests/lock_move_unlock.rs` covering LMU-01..LMU-08.
