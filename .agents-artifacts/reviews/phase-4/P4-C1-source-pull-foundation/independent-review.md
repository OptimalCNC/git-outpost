# Independent Review: P4-C1-source-pull-foundation

## Verdict

changes requested

## Evidence Reviewed

- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Chunk artifacts: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
- Commit reviewed: `9d491be8815b0276ba6d6e30d3a83820bdc40b47`
- Implementation files: `crates/core/src/ops/source.rs`, `crates/core/src/ops/pull.rs`, `crates/core/src/source_repo.rs`, `crates/core/src/safety.rs`, `crates/core/src/reporter.rs`, `crates/core/src/outpost.rs`, `crates/core/src/refname.rs`
- Test files: `crates/core/tests/source.rs`, `crates/core/tests/pull.rs`, `crates/core/tests/common/fixture.rs`
- Commands run:
  - `git status --short`
  - `git rev-parse HEAD`
  - `git show --stat --oneline --decorate --no-renames 9d491be`
  - `git show --name-status --no-renames --format=fuller 9d491be`
  - `git diff --no-renames 9d491be^ 9d491be -- crates/core/src/ops/mod.rs crates/core/src/ops/source.rs crates/core/src/ops/pull.rs crates/core/src/source_repo.rs crates/core/src/safety.rs`
  - `rg --files`
  - `rg -n "check_no_divergence|ops/source|ops/pull|SourceFetch|OutpostFetch|SP-0|P-0|source pull|gop pull|fast-forward|origin" docs/src/architecture.md docs/src/product.md`
  - `cargo fmt --check`
  - `cargo test -p outpost-core --test source`
  - `cargo test -p outpost-core --test pull`
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`
  - `git diff --check 9d491be^ 9d491be`
  - Manual disposable Git checks under `/tmp` for plain-fetch stale remote-tracking ref behavior

## Review Reasoning

The implementation stays within the P4-C1 core scope: it adds `ops::source`, `ops::pull`, source fast-forwarding, divergence checks, reporter events, and SP/P integration tests. I did not find merge, rebase, push, CLI, global `-C`, or Phase 5 behavior added in this commit.

The source refresh and pull paths mostly match the product and architecture: source branches are fast-forwarded from `origin`, checked-out source branches update through `merge --ff-only`, unchecked-out source branches update via `update-ref`, pull uses the metadata remote name for C->B, and reporter ordering is `SourceFetch` before source refresh and `OutpostFetch` before outpost pull. The implemented tests cover the requested SP-01..SP-05 and P-01..P-09 scenarios and all verification commands passed.

One stale-ref bug remains in `safety::check_no_divergence`. The architecture explicitly requires this helper to avoid stale `<remote>/<branch>` refs and to return `BranchNotFound` when the upstream branch is missing. The current helper runs a plain `git fetch <remote>` and then trusts any existing remote-tracking ref, which does not handle deleted upstream branches.

## Findings

1. `safety::check_no_divergence` can accept a deleted upstream branch when a stale remote-tracking ref exists. In `crates/core/src/safety.rs:136`, the helper runs `git fetch <remote>` without pruning or fetching/verifying the exact branch, then `crates/core/src/safety.rs:142` checks only whether `refs/remotes/<remote>/<branch>` exists locally. Plain `git fetch <remote>` leaves deleted remote branches behind in existing remote-tracking refs. I verified this with a disposable A/B/C Git topology: after deleting `feature` from B, `git fetch local` in C left `refs/remotes/local/feature` present and the same `rev-list --left-right --count refs/heads/feature...refs/remotes/local/feature` check returned `0 0`. That makes the helper return `Ok(())` instead of `BranchNotFound`. P-09 does not catch this because `ops::pull` checks `source.branch_exists(&branch)` before calling the helper; the bug is in the helper contract itself and can affect later Phase 4 callers.

## Missing Evidence

- No regression test proves `check_no_divergence` rejects a missing upstream/source branch when a stale `refs/remotes/<remote>/<branch>` ref already exists in the outpost.

## Required Changes

- Make `safety::check_no_divergence` stale-ref-safe before it computes ahead/behind. It should verify the upstream branch exists independently of any existing remote-tracking ref, or fetch/prune in a way that removes stale tracking refs, and return typed `BranchNotFound` when the upstream branch is gone.
- Add a focused regression test with a stale `refs/remotes/<remote>/<branch>` ref after the source branch is deleted.

## Nits

none
