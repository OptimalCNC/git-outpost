# P4-C3 Push Publication QA

## Summary

Added core integration coverage for Phase 4 push publication behavior in `crates/core/tests/push.rs`. The tests exercise real A/B/C fixture repositories and call `outpost_core::ops::push::run` directly with `PushOptions` and `CapturingReporter`.

## Test IDs Addressed

- Pu-01: implemented
- Pu-02: implemented
- Pu-03: implemented
- Pu-04: implemented
- Pu-05: implemented
- Pu-06: implemented
- Pu-07: implemented
- Pu-08: implemented
- Pu-09: implemented
- Pu-10: implemented

## Test Coverage Map

| ID | Test | Coverage |
| --- | --- | --- |
| Pu-01 | `pu01_push_sends_outpost_branch_to_source_then_origin` | Commits in C, runs push, asserts the commit reaches B `main` and A `main`, and checks both report steps pushed one commit. |
| Pu-02 | `pu02_push_records_outpost_push_and_source_push_events` | Captures reporter steps and asserts ordered `OutpostPush`, then `SourcePush`, with no warnings. |
| Pu-03 | `pu03_push_from_outpost_only_branch_returns_ambiguous_branch_creation` | Creates a branch only in C and asserts `AmbiguousBranchCreation` before any push event or source branch creation. |
| Pu-04 | `pu04_push_when_source_diverged_from_outpost_returns_divergence` | Creates unique commits on C and B `main`, then asserts typed `Divergence` and unchanged B/A refs. |
| Pu-05 | `pu05_push_dirty_outpost_succeeds` | Leaves an untracked dirty file in C and asserts committed C history still pushes to B and A. |
| Pu-06 | `pu06_push_with_missing_source_returns_source_missing` | Moves B after C exists and asserts `SourceMissing` before any push event. |
| Pu-07 | `pu07_push_uses_custom_remote_for_outpost_to_source_and_origin_for_source_to_upstream` | Adds C with custom C-to-B remote, verifies no `local` remote exists, then asserts push updates B and A through the expected chain. |
| Pu-08 | `pu08_push_into_dirty_checked_out_source_branch_surfaces_update_instead_git_failed` | Dirties a tracked file on B's checked-out branch with `updateInstead`, asserts `GitFailed` preserves stderr, and confirms A is not pushed. |
| Pu-09 | `pu09_push_with_deny_current_branch_refuse_returns_push_into_checked_out_branch` | Sets B `receive.denyCurrentBranch=refuse` and asserts `PushIntoCheckedOutBranch` before any push event or B ref movement. |
| Pu-10 | `pu10_push_on_detached_head_returns_no_upstream_tracking_head_before_push` | Detaches C `HEAD` and asserts `NoUpstreamTracking { branch: "HEAD" }` before any push event or B/A ref movement. |

## Files Changed

- `crates/core/tests/push.rs`
- `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`

## Fixture Changes

None. Existing `AbcFixture` and `CapturingReporter` helpers were sufficient.

## Production Code Changes

None by QA. QA did not edit `crates/core/src/ops/push.rs` or `crates/core/src/ops/mod.rs`.

## Docs Added/Updated

- Added this QA artifact: `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`

## Verification Run

```bash
cargo test -p outpost-core --test push
```

Result: passed, 10 tests passed.

## Verification Not Run

- Full workspace tests were not run for this QA chunk.
- Other Phase 4 integration suites were not rerun.

## Blocked Tests

None.

## Risks/Handoff Notes

- Pu-08 intentionally dirties a tracked file in B. An unrelated untracked file does not trigger Git's `receive.denyCurrentBranch=updateInstead` dirty-worktree rejection in this fixture.
- The tests assert exact pushed commit counts for successful one-commit publication paths because `PushReport` exposes those counts.
- Reporter assertions check `StepKind` ordering only, not user-facing message text.
