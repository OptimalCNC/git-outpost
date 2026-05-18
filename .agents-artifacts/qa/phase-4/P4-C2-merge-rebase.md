# QA Note: P4-C2-merge-rebase

## Summary

QA completed core integration coverage for Phase 4 merge and rebase behavior. The tests call `outpost_core::ops::merge::run` and `outpost_core::ops::rebase::run` directly against real A/B/C fixture repositories and capture `OutpostFetch` events through `CapturingReporter`.

## Test IDs Addressed

- MR-01..MR-06

## Test Coverage Map

| ID | File | Test Name | Behavior | Status |
| --- | --- | --- | --- | --- |
| MR-01 | `crates/core/tests/merge.rs` | `mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref` | Merge fetches B `main` into `refs/remotes/local/main` and merges it into C | implemented passing |
| MR-02 | `crates/core/tests/rebase.rs` | `mr02_rebase_fetches_source_branch_and_rebases_current_branch` | Rebase fetches B `main` into `refs/remotes/local/main` and rebases C onto it | implemented passing |
| MR-03 | `crates/core/tests/merge.rs` | `mr03_merge_uses_custom_source_remote_name` | Merge uses custom metadata remote name | implemented passing |
| MR-03 | `crates/core/tests/rebase.rs` | `mr03_rebase_uses_custom_source_remote_name` | Rebase uses custom metadata remote name | implemented passing |
| MR-04 | `crates/core/tests/merge.rs` | `mr04_merge_rejects_wrong_remote_before_fetching` | Merge rejects `origin/main` from a `local` outpost before fetching | implemented passing |
| MR-04 | `crates/core/tests/rebase.rs` | `mr04_rebase_rejects_wrong_remote_before_fetching` | Rebase rejects `origin/main` from a `local` outpost before fetching | implemented passing |
| MR-05 | `crates/core/tests/merge.rs` | `mr05_merge_records_outpost_fetch_event` | Merge emits `OutpostFetch` | implemented passing |
| MR-05 | `crates/core/tests/rebase.rs` | `mr05_rebase_records_outpost_fetch_event` | Rebase emits `OutpostFetch` | implemented passing |
| MR-06 | `crates/core/tests/merge.rs` | `mr06_merge_on_detached_head_returns_attached_branch_error_before_fetching` | Merge on detached C returns `NoUpstreamTracking { branch: "HEAD" }` before fetch | implemented passing |
| MR-06 | `crates/core/tests/rebase.rs` | `mr06_rebase_on_detached_head_returns_attached_branch_error_before_fetching` | Rebase on detached C returns `NoUpstreamTracking { branch: "HEAD" }` before fetch | implemented passing |

## Files Changed

- `crates/core/tests/merge.rs`
- `crates/core/tests/rebase.rs`

## Fixture Changes

- none

## Production Code Changes

- none by QA

## Docs Added / Updated

- none; tests exercise behavior specified by product, architecture, and roadmap documents.

## Verification Run

- `cargo test -p outpost-core --test merge`: pass; 5 merge integration tests
- `cargo test -p outpost-core --test rebase`: pass; 5 rebase integration tests
- Coordinator also ran full chunk verification; see evidence pack.

## Verification Not Run

- none for the assigned QA slice

## Blocked Tests

- none

## Risks / Handoff Notes

- Merge/rebase tests use non-conflicting file-backed commits.
- Reporter assertions check `StepKind`, not exact message text.
- Push tests remain P4-C3.
