# Normal Review: list-basic-summaries

- Verdict: needs_changes
- Review scope/range: `548ca5d..22c13ee`

## Findings

1. Medium - `crates/core/src/ops/list.rs:49` reports wrong-source outposts as healthy.
   - Problem: `summarize_entry` opens `source.outpost_at(&entry.path)` but never verifies that the outpost metadata points back to the `SourceRepo` being listed. `SourceRepo::outpost_at` only proves the path is a managed outpost, not that it is managed by this source. The existing safety helper shows the intended source-ownership check in `crates/core/src/safety.rs`.
   - Impact: A registered path that now contains a managed outpost for another source is summarized as `Clean` or `Dirty` with a branch, instead of `NotManaged`. That makes `list` misleading for stale or tampered registry entries.
   - Required change: Treat existing paths that are not managed outposts of the current source as `OutpostState::NotManaged`, either by using the established ownership check or by explicitly comparing the outpost's resolved source repo to `source.work_tree()`. Add a core integration test for a registered path whose `outpost.sourceRepo` points to another source.

## Test / Verification Notes

- `cargo test -p outpost-core --test list`: passed, 8 tests.
- `cargo test -p outpost-core --tests`: passed.
- `cargo test --workspace`: passed.
- `cargo fmt --check`: passed.
- `git diff --check 548ca5d..HEAD`: passed.

## Scope Notes

- No CLI formatting, dispatch, or global `-C` behavior was changed.
- L-05/L-06 ahead/behind remain deferred.
- Direct registry locking in the L-10 test stays within Phase 1 list-summary coverage.

## Approval Conditions

- Fix the source-ownership classification issue.
- Add regression coverage for wrong-source registered paths returning `NotManaged`.
