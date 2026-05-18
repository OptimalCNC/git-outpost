# Phase 4 Progress

## Phase

- `phase_id`: `phase-4`
- Roadmap scope: `ops::source`, `ops::pull` (UpstreamRef-driven), `ops::merge`, `ops::rebase`, `ops::push`, sync `Reporter` events
- Test IDs: SP-01..SP-05, P-01..P-09, MR-01..MR-06, Pu-01..Pu-10
- Progress log path: `.agents-artifacts/progress/phase-4.md`
- Review artifact root: `.agents-artifacts/reviews/phase-4/`
- QA artifact root: `.agents-artifacts/qa/phase-4/`
- Protected paths: none
- Protected exceptions: none
- Forbidden scope:
  - implementation outside Phase 4 unless required to make Phase 4 compile and explicitly justified here
  - Phase 5 CLI binary/global `-C`/E2E/cross-platform behavior
  - unrelated documentation cleanup
  - unrelated refactors
- Required verification:
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`

## Source Docs

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- Last observed repo revision before Phase 4: `564be55 phase-3: close phase`

## Current Snapshot

- Branch: `main`
- Initial Phase 4 `git status --short --branch`: `## main...origin/main [ahead 69]`, with no modified or untracked files before readiness artifact creation
- Workspace: one member, `outpost-core`
- Existing implementation: Phase 0/1/2/3 core foundation plus ops `add`, `list`, `lock`, `move`, `unlock`, `remove`, `prune`, and `status`
- Missing Phase 4 files at start: `crates/core/src/ops/source.rs`, `crates/core/src/ops/pull.rs`, `crates/core/src/ops/merge.rs`, `crates/core/src/ops/rebase.rs`, `crates/core/src/ops/push.rs`, and corresponding core integration tests
- Toolchain observed by readiness: `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
- Baseline verification before Phase 4 planning: required verification passed during Phase 4 readiness with 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests

## Readiness Log

- Verdict: `ready with cautions`
- Auditor: native subagent `019e38d5-6a65-75e3-9c6f-d430f833206c`
- Artifact: `.agents-artifacts/reviews/phase-4/readiness/readiness-audit.md`
- Phase reviewed: `phase-4`; roadmap scope `ops::source`, `ops::pull` (UpstreamRef-driven), `ops::merge`, `ops::rebase`, `ops::push`, sync `Reporter` events; test IDs SP-01..SP-05, P-01..P-09, MR-01..MR-06, Pu-01..Pu-10
- Source documents reviewed:
  - `docs/src/product.md`
  - `docs/src/architecture.md`
  - `docs/src/roadmap.md`
  - `docs/coordinator-prompt.md`
  - `.agents-artifacts/progress/phase-3.md`
- Repo state evidence:
  - cwd `/home/huwei/projects/git-outpost`
  - branch `main`
  - HEAD `564be55 phase-3: close phase`
  - `git status --short --branch`: `## main...origin/main [ahead 69]`; no modified/untracked files before artifact creation
  - Phase 4 production modules and tests absent before implementation, as expected
- Prerequisites checked:
  - Phase 3 closeout commit is HEAD
  - Phase 3 progress log records closeout passed
  - Existing foundations needed by sync ops are present: `Reporter`, `StepKind`, ref newtypes, `UpstreamRef::short_branch`, metadata-backed `Outpost`, `SourceRepo`, safety helpers, and A/B/C fixture
- Toolchain evidence:
  - `cargo --version`: `cargo 1.94.0`
  - `rustc --version`: `rustc 1.94.0`
  - `git --version`: `git version 2.43.0`
  - `cargo metadata --no-deps --format-version 1`: passed
  - `cargo test -p outpost-core`: passed; 46 unit tests, 22 add tests, 11 list tests, 9 lock/move/unlock tests, 9 prune tests, 11 remove tests, 15 status tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: passed with the same test binaries excluding doctests
  - `cargo test --workspace`: passed with the same workspace coverage
- Spec/architecture/roadmap consistency: pass. Product, architecture, and roadmap agree that Phase 4 owns core sync operations and `Reporter` events; CLI/global `-C`/E2E/exit-code behavior remains Phase 5.
- Blocking issues: none
- Non-blocking cautions:
  - `SourceRepo::fast_forward_branch_from_origin` and `safety::check_no_divergence` are specified but not implemented yet; add narrow helpers with tests.
  - Detached `HEAD` behavior must surface `NoUpstreamTracking { branch: "HEAD" }` for `pull` and `push`; `merge`/`rebase` must fail before fetching.
  - `merge`/`rebase` must reject mismatched remotes like `origin/main` before fetching.
  - Source branch refresh must update a checked-out source worktree with `git merge --ff-only`; otherwise update refs with `git update-ref`.
  - Do not add Phase 5 CLI crate behavior, global `-C`, full E2E tests, or broad command formatting work.
- Recommended first chunk: minimal sync foundation in core, including `SourceRepo::fast_forward_branch_from_origin` and `ops::source::pull`, with SP-01..SP-05 coverage.
- Required human decisions: none

## QA/Test Map

| ID | Scope | Status | Notes |
| --- | --- | --- | --- |
| SP-01 | `source pull main` fast-forwards B's `main` from B's `origin/main` without switching B's checkout | completed | `crates/core/tests/source.rs` |
| SP-02 | `source pull main` updates B's working tree when `main` is checked out in B | completed | `crates/core/tests/source.rs` |
| SP-03 | `source pull main` returns `Divergence` when B's `main` and `origin/main` both have unique commits | completed | `crates/core/tests/source.rs` |
| SP-04 | `source pull missing` returns `BranchNotFound` | completed | `crates/core/tests/source.rs` |
| SP-05 | `source pull main` records a `SourceFetch` step event | completed | `crates/core/tests/source.rs` |
| P-01 | `pull` fast-forwards B from A, then C from B | completed | `crates/core/tests/pull.rs` |
| P-02 | `pull` fast-forwards C from B and leaves A unchanged after B-only commit | completed | `crates/core/tests/pull.rs` |
| P-03 | `pull` returns `Divergence` for B vs `origin/<branch>` divergence | completed | `crates/core/tests/pull.rs` |
| P-04 | `pull` returns `Divergence` for C vs B divergence | completed | `crates/core/tests/pull.rs` |
| P-05 | `pull` with B moved/deleted returns `SourceMissing` | completed | `crates/core/tests/pull.rs` |
| P-06 | `pull` on detached HEAD returns `NoUpstreamTracking { branch: "HEAD" }` | completed | `crates/core/tests/pull.rs` |
| P-07 | `pull` works with custom remote name | completed | `crates/core/tests/pull.rs` |
| P-08 | `pull` records `SourceFetch` and `OutpostFetch` step events | completed | `crates/core/tests/pull.rs` |
| P-09 | `pull` on attached C branch missing in B returns `BranchNotFound` before outpost fast-forward | completed | `crates/core/tests/pull.rs` |
| MR-01 | `merge local/main` fetches B's `main` and merges `local/main` into C | completed | `crates/core/tests/merge.rs` |
| MR-02 | `rebase local/main` fetches B's `main` and rebases C onto `local/main` | completed | `crates/core/tests/rebase.rs` |
| MR-03 | `merge custom/main` and `rebase custom/main` work with custom remote name | completed | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` |
| MR-04 | `merge origin/main` / `rebase origin/main` from `local` outpost returns `InvalidRefName` before fetching | completed | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` |
| MR-05 | `merge local/main` and `rebase local/main` record `OutpostFetch` step events | completed | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` |
| MR-06 | `merge local/main` and `rebase local/main` on detached HEAD return attached-branch error before fetching | completed | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` |
| Pu-01 | `push` sends C current branch to B and then B to `origin/<branch>` | planned | `crates/core/tests/push.rs` |
| Pu-02 | `push` records `OutpostPush` and `SourcePush` step events | planned | `crates/core/tests/push.rs` |
| Pu-03 | `push` from branch only in C returns `AmbiguousBranchCreation` | planned | `crates/core/tests/push.rs` |
| Pu-04 | `push` when B diverged from C returns `Divergence` | planned | `crates/core/tests/push.rs` |
| Pu-05 | `push` on dirty C succeeds | planned | `crates/core/tests/push.rs` |
| Pu-06 | `push` with B moved/deleted returns `SourceMissing` | planned | `crates/core/tests/push.rs` |
| Pu-07 | `push` uses custom remote for C->B and `origin` for B->A | planned | `crates/core/tests/push.rs` |
| Pu-08 | `push` into dirty checked-out source branch surfaces `GitFailed` with stderr | planned | `crates/core/tests/push.rs` |
| Pu-09 | `push` with `denyCurrentBranch=refuse` and checked-out B branch returns `PushIntoCheckedOutBranch` | planned | `crates/core/tests/push.rs` |
| Pu-10 | `push` on detached HEAD returns `NoUpstreamTracking { branch: "HEAD" }` before pushing | planned | `crates/core/tests/push.rs` |

## QA/Test Plan Gate

- QA subagent: `019e38da-c58b-7e00-9d37-01ad628c60ae`
- Artifact: `.agents-artifacts/qa/phase-4/test-plan.md`
- Summary: QA will cover Phase 4 sync operations as core integration tests against real temporary Git repositories, primarily the A/B/C fixture, with captured `Reporter` events for SP-05, P-08, MR-05, and Pu-02.
- Planned test files:
  - `crates/core/tests/source.rs`: SP-01..SP-05
  - `crates/core/tests/pull.rs`: P-01..P-09
  - `crates/core/tests/merge.rs`: merge-side MR-01, MR-03, MR-04, MR-05, MR-06
  - `crates/core/tests/rebase.rs`: MR-02 plus rebase-side MR-03, MR-04, MR-05, MR-06
  - `crates/core/tests/push.rs`: Pu-01..Pu-10
- Fixture support planned:
  - custom-remote outpost helper
  - branch checkout/detach/delete helpers as needed
  - ref/OID inspection helpers
  - source `receive.denyCurrentBranch` config helper
  - `CapturingReporter` for `StepKind` events
- Blocked tests: none
- QA risks:
  - distinguish ref-only source updates from checked-out worktree updates
  - assert typed `Divergence`, not incidental `GitFailed`
  - prove custom remote is used for C->B while B->A remains `origin`
  - avoid depending on exact human-facing reporter message text
  - keep CLI E2E/global `-C` out of Phase 4

## Active Chunk

- `P4-C2-merge-rebase`
- Scope: `ops::merge`, `ops::rebase`, configured source-remote validation, detached-head preconditions, and `OutpostFetch` reporting.
- Test IDs: MR-01..MR-06
- Out of scope: `ops::push`, Phase 5 CLI/global `-C`/E2E, refreshing B from `origin` inside merge/rebase, unrelated docs cleanup, unrelated refactors.
- Status: implementation and QA evidence recorded; review pending.

## Remaining Chunks

Chunk Planning Gate:

- Planner subagent: `019e38e0-e9f9-7d91-b255-414976febf0f`
- Artifact: `.agents-artifacts/qa/phase-4/chunk-plan.md`
- Verdict: ready to plan Phase 4 implementation
- Recommended chunks:
  - `P4-C1-source-pull-foundation`: `SourceRepo::fast_forward_branch_from_origin`, narrow divergence/branch helpers, `ops::source`, `ops::pull`, and `SourceFetch`/`OutpostFetch` reporting; test IDs SP-01..SP-05 and P-01..P-09
  - `P4-C2-merge-rebase`: `ops::merge` and `ops::rebase`, configured remote validation, `OutpostFetch` reporting, detached-head preconditions; test IDs MR-01..MR-06
  - `P4-C3-push-publication`: `ops::push`, C->B then B->A publication, branch-creation refusal, divergence checks, checked-out source policy, `OutpostPush`/`SourcePush` reporting; test IDs Pu-01..Pu-10
- Dependencies:
  - `P4-C1-source-pull-foundation` first
  - `P4-C2-merge-rebase` can follow P4-C1; it does not depend on pull behavior but should reuse attached-branch and reporter conventions
  - `P4-C3-push-publication` after P4-C1 so it can reuse C/B divergence checks
- Out-of-scope guardrails:
  - no CLI crate, binary target, help text, stdout/stderr formatting, `--no-color`, global `-C`, E2E, or cross-platform whole-binary work
  - no new command flags such as pull strategy flags or push routing flags
  - no source-branch auto-creation in `push`
  - no `origin` refresh inside `merge` or `rebase`
  - no hardcoded `local` for C->B operations
  - no unrelated docs cleanup or broad Phase 0..3 refactors

Remaining chunk order:

- `P4-C3-push-publication`
- `phase-4-verification`

## Completed Chunks

- `P4-C1-source-pull-foundation` implementation evidence recorded:
  - Files changed: `crates/core/src/ops/mod.rs`, `crates/core/src/ops/source.rs`, `crates/core/src/ops/pull.rs`, `crates/core/src/source_repo.rs`, `crates/core/src/safety.rs`, `crates/core/tests/common/fixture.rs`, `crates/core/tests/source.rs`, `crates/core/tests/pull.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
  - Test IDs advanced: SP-01..SP-05, P-01..P-09
  - Evidence pack: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
  - Unit tests added: `safety::tests::check_no_divergence_reports_missing_remote_branch`, `safety::tests::check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref`
  - Integration tests added: `sp01_source_pull_fast_forwards_unchecked_out_source_branch_without_switching`, `sp02_source_pull_updates_checked_out_source_worktree`, `sp03_source_pull_returns_divergence_when_source_and_origin_diverge`, `sp04_source_pull_missing_branch_returns_branch_not_found`, `sp05_source_pull_records_source_fetch_event`, `p01_pull_fast_forwards_source_from_origin_then_outpost_from_source`, `p02_pull_fast_forwards_outpost_from_source_without_touching_origin`, `p03_pull_returns_divergence_when_source_and_origin_diverge`, `p04_pull_returns_divergence_when_outpost_and_source_diverge`, `p05_pull_with_missing_source_returns_source_missing`, `p06_pull_on_detached_head_returns_no_upstream_tracking_head`, `p07_pull_uses_custom_source_remote_name`, `p08_pull_records_source_fetch_and_outpost_fetch_events`, `p09_pull_missing_matching_source_branch_returns_branch_not_found_before_outpost_ff`
  - Docs updated: none; existing product and architecture document source refresh, pull sequencing, reporter events, and test scenarios
  - Architecture deviations: none for claimed `P4-C1-source-pull-foundation` behavior
  - Implementation/evidence commit: `9d491be phase-4: add source pull foundation`
  - Review fix: `check_no_divergence` now verifies the exact upstream branch with `ls-remote` before trusting local remote-tracking refs, covering stale tracking refs after upstream deletion
  - Review-fix commit: `96969ea phase-4: fix source pull review findings`
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/independent-review.md`
    - Scope Re-reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/scope-rereview.md`
    - Normal Re-reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/normal-rereview.md`
    - Independent Re-reviewer: `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/independent-rereview.md`
  - Review verdicts after fixes: scope `approved with nits`; normal `approved with nits`; independent `approved with nits`
  - Required review changes: none open
  - Adopted nits: progress log now records implementation/evidence commit `9d491be` and review-fix commit `96969ea`; helper return type now matches architecture API shape
  - Status: approved
- `P4-C2-merge-rebase` implementation evidence recorded:
  - Files changed: `crates/core/src/ops/mod.rs`, `crates/core/src/ops/merge.rs`, `crates/core/src/ops/rebase.rs`, `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
  - Test IDs advanced: MR-01..MR-06
  - Evidence pack: `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
  - Unit tests added: none
  - Integration tests added: `mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref`, `mr02_rebase_fetches_source_branch_and_rebases_current_branch`, `mr03_merge_uses_custom_source_remote_name`, `mr03_rebase_uses_custom_source_remote_name`, `mr04_merge_rejects_wrong_remote_before_fetching`, `mr04_rebase_rejects_wrong_remote_before_fetching`, `mr05_merge_records_outpost_fetch_event`, `mr05_rebase_records_outpost_fetch_event`, `mr06_merge_on_detached_head_returns_attached_branch_error_before_fetching`, `mr06_rebase_on_detached_head_returns_attached_branch_error_before_fetching`
  - Docs updated: none; existing product and architecture document merge/rebase source-ref behavior, reporter events, custom remote behavior, and test scenarios
  - Architecture deviations: none for claimed `P4-C2-merge-rebase` behavior
  - Status: review pending

## Verification Log

- Phase 4 readiness baseline:
  - `cargo test -p outpost-core`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same integration/unit test set excluding doctests
  - `cargo test --workspace`: pass with the same workspace coverage
- `P4-C1-source-pull-foundation` local verification:
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
- `P4-C2-merge-rebase` local verification:
  - `cargo fmt --check`: pass
  - `cargo check -p outpost-core`: pass
  - `cargo test -p outpost-core --test merge`: pass; 5 merge integration tests
  - `cargo test -p outpost-core --test rebase`: pass; 5 rebase integration tests
  - `cargo test -p outpost-core --test source`: pass; 5 source integration tests
  - `cargo test -p outpost-core --test pull`: pass; 9 pull integration tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 5 merge integration tests, 9 prune integration tests, 9 pull integration tests, 5 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; same test binaries excluding doctests
  - `cargo test --workspace`: pass; same workspace coverage, 0 doctests

## Review Log

- `P4-C1-source-pull-foundation` Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/scope-review.md`; nit was stale progress commit-log text.
- `P4-C1-source-pull-foundation` Normal Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/normal-review.md`; nits were architecture/helper return type mismatch and stale progress commit-log text.
- `P4-C1-source-pull-foundation` Independent Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/independent-review.md`; required stale remote-tracking ref fix in `check_no_divergence` plus regression test.
- Adopted review fixes:
  - `check_no_divergence` verifies the exact upstream branch with `git ls-remote <remote> <merge_ref>` before using remote-tracking refs.
  - Added stale remote-tracking ref regression test after deleting the upstream branch.
  - `SourceRepo::fast_forward_branch_from_origin` now matches architecture API shape by returning `OutpostResult<()>`; `ops::source` and `ops::pull` compute `updated` from branch OIDs.
  - Progress log records implementation/evidence commit `9d491be`.
- `P4-C1-source-pull-foundation` Scope Re-reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/scope-rereview.md`; nit was stale progress commit-log text.
- `P4-C1-source-pull-foundation` Normal Re-reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/normal-rereview.md`; nit was stale progress commit-log text.
- `P4-C1-source-pull-foundation` Independent Re-reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/independent-rereview.md`; nit was stale progress commit-log text.
- Blocking review findings: none open for `P4-C1-source-pull-foundation`.

## Docs Log

- `P4-C1-source-pull-foundation`: no docs changes; stable concepts are already covered by product `pull`/`source pull` and architecture sections 5.8, 5.9.0, 5.9.4, 5.9.5, 11.6, and 11.7.
- `P4-C2-merge-rebase`: no docs changes; stable concepts are already covered by product `merge`/`rebase` and architecture sections 5.9.0, 5.9.6, 5.9.7, and 11.8.

## Commit Log

- `83e8778 phase-4: record readiness and plan`
- `a0a7f40 phase-4: start source pull foundation`
- `9d491be phase-4: add source pull foundation`
- `96969ea phase-4: fix source pull review findings`
- `1669ea2 phase-4: record source pull reviews`
- `6b4d8f5 phase-4: start merge rebase`
- pending `P4-C2-merge-rebase` implementation/evidence commit

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Keep Phase 4 core-only; CLI binary behavior and full E2E remain Phase 5.
- Preserve reporter event ordering: emit before each user-visible cross-repo action.
- Avoid silently creating source branches in `push`; outpost-only branches must return `AmbiguousBranchCreation`.

## Next Recommended Action

- Commit `P4-C2-merge-rebase` implementation/evidence, then run scope, normal, and independent reviews.
