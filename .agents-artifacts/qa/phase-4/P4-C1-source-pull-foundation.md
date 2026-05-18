# QA Note: P4-C1-source-pull-foundation

## Summary

QA completed core integration coverage for Phase 4 source refresh and pull behavior. The tests call `outpost_core::ops::source::pull` and `outpost_core::ops::pull::run` directly against real A/B/C fixture repositories and capture `Reporter` events through a test reporter.

## Test IDs Addressed

- SP-01..SP-05
- P-01..P-09

## Test Coverage Map

| ID | File | Test Name | Behavior | Status |
| --- | --- | --- | --- | --- |
| SP-01 | `crates/core/tests/source.rs` | `sp01_source_pull_fast_forwards_unchecked_out_source_branch_without_switching` | Source pull fast-forwards an unchecked-out B branch from `origin` and leaves B on its original branch | implemented passing |
| SP-02 | `crates/core/tests/source.rs` | `sp02_source_pull_updates_checked_out_source_worktree` | Source pull updates checked-out B `main` and its working tree | implemented passing |
| SP-03 | `crates/core/tests/source.rs` | `sp03_source_pull_returns_divergence_when_source_and_origin_diverge` | Source pull returns typed `Divergence` for B/origin divergence | implemented passing |
| SP-04 | `crates/core/tests/source.rs` | `sp04_source_pull_missing_branch_returns_branch_not_found` | Source pull returns `BranchNotFound` before source update when B branch is missing | implemented passing |
| SP-05 | `crates/core/tests/source.rs` | `sp05_source_pull_records_source_fetch_event` | Source pull emits `SourceFetch` | implemented passing |
| P-01 | `crates/core/tests/pull.rs` | `p01_pull_fast_forwards_source_from_origin_then_outpost_from_source` | Pull fast-forwards B from A, then C from B | implemented passing |
| P-02 | `crates/core/tests/pull.rs` | `p02_pull_fast_forwards_outpost_from_source_without_touching_origin` | Pull fast-forwards C from B and leaves A unchanged after B-only commit | implemented passing |
| P-03 | `crates/core/tests/pull.rs` | `p03_pull_returns_divergence_when_source_and_origin_diverge` | Pull returns typed `Divergence` for B/origin divergence | implemented passing |
| P-04 | `crates/core/tests/pull.rs` | `p04_pull_returns_divergence_when_outpost_and_source_diverge` | Pull returns typed `Divergence` for C/B divergence before outpost fast-forward | implemented passing |
| P-05 | `crates/core/tests/pull.rs` | `p05_pull_with_missing_source_returns_source_missing` | Pull returns `SourceMissing` when B is moved/deleted | implemented passing |
| P-06 | `crates/core/tests/pull.rs` | `p06_pull_on_detached_head_returns_no_upstream_tracking_head` | Pull on detached C returns `NoUpstreamTracking { branch: "HEAD" }` | implemented passing |
| P-07 | `crates/core/tests/pull.rs` | `p07_pull_uses_custom_source_remote_name` | Pull uses metadata remote name `custom`, not hardcoded `local` | implemented passing |
| P-08 | `crates/core/tests/pull.rs` | `p08_pull_records_source_fetch_and_outpost_fetch_events` | Pull emits ordered `SourceFetch`, then `OutpostFetch` | implemented passing |
| P-09 | `crates/core/tests/pull.rs` | `p09_pull_missing_matching_source_branch_returns_branch_not_found_before_outpost_ff` | Pull returns `BranchNotFound` for missing B branch before outpost update | implemented passing |

## Files Changed

- `crates/core/tests/common/fixture.rs`
- `crates/core/tests/source.rs`
- `crates/core/tests/pull.rs`

## Fixture Changes

- Added custom-remote outpost helpers.
- Added branch-specific outpost helper.
- Added file-backed commit helpers for source, upstream, and outpost repos.
- Added source branch push/delete helpers.
- Added ref and current-branch read helpers.
- Added `CapturingReporter` for `StepKind` and warning assertions.

## Production Code Changes

- none by QA

## Docs Added / Updated

- none; tests exercise behavior specified by product, architecture, and roadmap documents.

## Verification Run

- `cargo fmt -p outpost-core`: pass
- `cargo test -p outpost-core --test source`: pass; 5 source integration tests
- `cargo test -p outpost-core --test pull`: pass; 9 pull integration tests
- `git diff --check -- crates/core/tests/common/fixture.rs crates/core/tests/source.rs crates/core/tests/pull.rs`: pass
- Coordinator also ran full chunk verification; see evidence pack.

## Verification Not Run

- none for the assigned QA slice

## Blocked Tests

- none

## Risks / Handoff Notes

- Reporter assertions intentionally check `StepKind` order, not exact user-facing messages.
- Push, merge, and rebase tests remain later Phase 4 chunks.
