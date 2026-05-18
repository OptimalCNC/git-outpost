# Evidence Pack: P4-C1-source-pull-foundation

## Phase And Chunk

- Phase: `phase-4`
- Chunk: `P4-C1-source-pull-foundation`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `pull` and `source pull <source-branch>`
  - Architecture 5.8 `safety::check_no_divergence`
  - Architecture 5.9.0 `Reporter` event sink
  - Architecture 5.9.4 `ops/source.rs`
  - Architecture 5.9.5 `ops/pull.rs`
  - Architecture 11.6 and 11.7 integration tests
  - Roadmap Phase 4 scope
- Roadmap test IDs advanced: SP-01..SP-05, P-01..P-09

## Changed Files

- `.agents-artifacts/progress/phase-4.md`
- `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
- `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/source.rs`
- `crates/core/src/ops/pull.rs`
- `crates/core/src/source_repo.rs`
- `crates/core/src/safety.rs`
- `crates/core/tests/common/fixture.rs`
- `crates/core/tests/source.rs`
- `crates/core/tests/pull.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/source.rs`
  - Adds `SourceCommand`, `SourcePullOptions`, `SourcePullReport`, and `pull`.
  - Resolves B from outpost metadata, requires the source branch to exist, emits `SourceFetch`, delegates source fast-forward to `SourceRepo::fast_forward_branch_from_origin`, and reports `updated` by comparing source branch OIDs before and after the refresh.
- `ops/pull.rs`
  - Adds `PullOptions`, `PullReport`, and `run`.
  - Requires attached C branch, maps detached `HEAD` to `NoUpstreamTracking { branch: "HEAD" }`, resolves B, requires matching B branch, emits `SourceFetch`, refreshes B from `origin`, reports source update by comparing source branch OIDs, checks C/B divergence via metadata remote and `UpstreamRef`, emits `OutpostFetch`, and runs `git pull --ff-only <remote> <branch>` in C.
- `source_repo.rs`
  - Adds `fast_forward_branch_from_origin`.
  - Fetches `origin <branch>:refs/remotes/origin/<branch>`, classifies equal/source-ahead/source-behind/divergent histories, updates checked-out source worktrees with `git merge --ff-only`, and updates unchecked-out branch refs with `git update-ref`.
- `safety.rs`
  - Adds `check_no_divergence` and `check_no_divergence_after_fetch`.
  - Verifies the exact upstream branch with `git ls-remote`, fetches the configured source remote for fresh C/B remote-tracking refs, returns typed `BranchNotFound` for missing remote branch even when a stale remote-tracking ref exists, and returns typed `Divergence` when C and B both have unique commits.
- `tests/common/fixture.rs`
  - Adds narrow helpers for custom-remote outposts, branch-specific outposts, file-backed commits, ref reads, source branch deletion, source branch push, current branch reads, and captured reporter events.
- `tests/source.rs`
  - Adds SP-01..SP-05 integration coverage.
- `tests/pull.rs`
  - Adds P-01..P-09 integration coverage.

## Tests Added / Updated

- Unit tests added:
  - `safety::tests::check_no_divergence_reports_missing_remote_branch`
  - `safety::tests::check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref`

## Integration Tests Added / Updated

- `sp01_source_pull_fast_forwards_unchecked_out_source_branch_without_switching` covers SP-01.
- `sp02_source_pull_updates_checked_out_source_worktree` covers SP-02.
- `sp03_source_pull_returns_divergence_when_source_and_origin_diverge` covers SP-03.
- `sp04_source_pull_missing_branch_returns_branch_not_found` covers SP-04.
- `sp05_source_pull_records_source_fetch_event` covers SP-05.
- `p01_pull_fast_forwards_source_from_origin_then_outpost_from_source` covers P-01.
- `p02_pull_fast_forwards_outpost_from_source_without_touching_origin` covers P-02.
- `p03_pull_returns_divergence_when_source_and_origin_diverge` covers P-03.
- `p04_pull_returns_divergence_when_outpost_and_source_diverge` covers P-04.
- `p05_pull_with_missing_source_returns_source_missing` covers P-05.
- `p06_pull_on_detached_head_returns_no_upstream_tracking_head` covers P-06.
- `p07_pull_uses_custom_source_remote_name` covers P-07.
- `p08_pull_records_source_fetch_and_outpost_fetch_events` covers P-08.
- `p09_pull_missing_matching_source_branch_returns_branch_not_found_before_outpost_ff` covers P-09.

## Docs Added / Updated

- none
- Rationale: product and architecture already specify source refresh, pull sequencing, reporter events, and test scenarios. This chunk implements those stable concepts without changing public documentation.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --lib source_repo`: pass; 6 filtered source/outpost tests
- `cargo test -p outpost-core --lib safety`: pass; 16 filtered safety tests
- `cargo test -p outpost-core --lib safety::tests::check_no_divergence_reports_missing_remote_branch`: pass
- `cargo test -p outpost-core --lib safety::tests::check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref`: pass
- `cargo test -p outpost-core --test source`: pass; 5 source integration tests
- `cargo test -p outpost-core --test pull`: pass; 9 pull integration tests
- `cargo test -p outpost-core --test status`: pass; 15 status integration tests
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 9 pull integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; same test binaries excluding doctests
- `cargo test --workspace`: pass; same workspace coverage, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `P4-C1-source-pull-foundation` behavior.

## Residual Risks / Handoff Notes

- `check_no_divergence_after_fetch` exists for later Phase 4 operations that may have just fetched a remote-tracking ref. It still verifies the exact upstream branch with `ls-remote` before trusting local refs.
- Reporter tests assert `StepKind` ordering and intentionally do not depend on exact human-facing message text.
- `ops::merge`, `ops::rebase`, `ops::push`, CLI/global `-C`, and E2E behavior remain out of scope.
