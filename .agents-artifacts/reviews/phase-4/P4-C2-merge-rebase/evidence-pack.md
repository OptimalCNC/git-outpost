# Evidence Pack: P4-C2-merge-rebase

## Phase And Chunk

- Phase: `phase-4`
- Chunk: `P4-C2-merge-rebase`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `merge <source-ref>` and `rebase <source-ref>`
  - Architecture 5.9.0 `Reporter` event sink
  - Architecture 5.9.6 `ops/merge.rs`
  - Architecture 5.9.7 `ops/rebase.rs`
  - Architecture 11.8 integration tests
  - Roadmap Phase 4 scope
- Roadmap test IDs advanced: MR-01..MR-06

## Changed Files

- `.agents-artifacts/progress/phase-4.md`
- `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
- `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/merge.rs`
- `crates/core/src/ops/rebase.rs`
- `crates/core/tests/merge.rs`
- `crates/core/tests/rebase.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports `merge` and `rebase` modules.
- `ops/merge.rs`
  - Adds `MergeOptions`, `MergeReport`, and `run`.
  - Requires an attached outpost branch before fetching.
  - Validates `SourceRemoteRef.remote` against outpost metadata remote before fetching.
  - Emits `OutpostFetch`, fetches `<branch>:refs/remotes/<remote>/<branch>` from the configured source remote, then runs `git merge <remote>/<branch>`.
- `ops/rebase.rs`
  - Adds `RebaseOptions`, `RebaseReport`, and `run`.
  - Mirrors merge preconditions and fetch behavior, then runs `git rebase <remote>/<branch>`.
- `tests/merge.rs`
  - Adds merge-side coverage for MR-01, MR-03, MR-04, MR-05, and MR-06.
- `tests/rebase.rs`
  - Adds rebase-side coverage for MR-02, MR-03, MR-04, MR-05, and MR-06.

## Tests Added / Updated

- Unit tests added/updated: none

## Integration Tests Added / Updated

- `mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref` covers MR-01.
- `mr02_rebase_fetches_source_branch_and_rebases_current_branch` covers MR-02.
- `mr03_merge_uses_custom_source_remote_name` covers merge-side MR-03.
- `mr03_rebase_uses_custom_source_remote_name` covers rebase-side MR-03.
- `mr04_merge_rejects_wrong_remote_before_fetching` covers merge-side MR-04.
- `mr04_rebase_rejects_wrong_remote_before_fetching` covers rebase-side MR-04.
- `mr05_merge_records_outpost_fetch_event` covers merge-side MR-05.
- `mr05_rebase_records_outpost_fetch_event` covers rebase-side MR-05.
- `mr06_merge_on_detached_head_returns_attached_branch_error_before_fetching` covers merge-side MR-06.
- `mr06_rebase_on_detached_head_returns_attached_branch_error_before_fetching` covers rebase-side MR-06.

## Docs Added / Updated

- none
- Rationale: product and architecture already document merge/rebase source-ref behavior, reporter event expectations, custom remote behavior, and test scenarios.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --test merge`: pass; 5 merge integration tests
- `cargo test -p outpost-core --test rebase`: pass; 5 rebase integration tests
- `cargo test -p outpost-core --test source`: pass; 5 source integration tests
- `cargo test -p outpost-core --test pull`: pass; 9 pull integration tests
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 5 merge integration tests, 9 prune integration tests, 9 pull integration tests, 5 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; same test binaries excluding doctests
- `cargo test --workspace`: pass; same workspace coverage, 0 doctests

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `P4-C2-merge-rebase` behavior.

## Residual Risks / Handoff Notes

- Merge and rebase intentionally do not refresh B from `origin`; users should run `source pull` first when needed.
- Wrong-remote and detached-head tests assert no reporter event and no remote-tracking ref update before failure.
- `ops::push`, CLI/global `-C`, and E2E behavior remain out of scope.
