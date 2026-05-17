# Normal Review Rerun - storage-foundations

## Verdict: `approved`

## Evidence Reviewed

- Commits: `e80bd1e` and review-fix `98591c6`
- Reviewed cited docs/artifacts, including prior normal and independent reviews
- Reviewed `crates/core/src/registry.rs` diff from `e80bd1e..98591c6`
- Ran:
  - `cargo fmt --check`
  - `cargo test -p outpost-core registry::tests::`
  - `cargo test --workspace`
  - `cargo test -p outpost-core --features test-helpers`

## Previous Findings Status

- Resolved: `RegistryMut::save()` now marks an attempted save before calling the fallible save path, so failed saves return the typed `OutpostError` instead of triggering the dirty Drop guard. Covered by `failed_save_returns_error_without_drop_guard_panic` at `crates/core/src/registry.rs:523`.
- Resolved: `RegistryMut::update_path()` now supports the move flow where the old path was already renamed away, via recorded canonical-path lookup. Covered by `update_path_handles_registered_old_path_after_rename` at `crates/core/src/registry.rs:444`.

## Findings (severity, file/line, issue, required change)

None.

## Test/Verification Gaps

- No blocking gaps found for this rerun.
- The evidence pack's all-target dependency materialization gap remains documented and non-blocking for this storage-fix review.

## Required Changes

None.

## Notes

The review-fix also addresses the independent stale-path removal issue by reusing the same recorded-path lookup in `remove_by_path()`, covered by `remove_by_path_handles_registered_missing_path`. Worktree remained clean; no files were modified.
