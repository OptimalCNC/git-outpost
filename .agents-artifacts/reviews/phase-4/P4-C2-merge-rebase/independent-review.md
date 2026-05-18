# Independent Review: P4-C2-merge-rebase

## Verdict

changes requested

## Evidence Reviewed

- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Chunk artifacts:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
- Commit reviewed: `4a68f151f20314ac04732edc8b1e5e0e44002a7e`
- Implementation files reviewed:
  - `crates/core/src/ops/merge.rs`
  - `crates/core/src/ops/rebase.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/refname.rs`
  - `crates/core/src/outpost.rs`
  - `crates/core/src/git.rs`
  - `crates/core/src/reporter.rs`
- Test files reviewed:
  - `crates/core/tests/merge.rs`
  - `crates/core/tests/rebase.rs`
  - `crates/core/tests/common/fixture.rs`
- Commands run:
  - `git status --short`
  - `git show --stat --oneline --decorate --no-renames 4a68f15`
  - `git show --no-ext-diff --no-renames --find-renames=0 --format=fuller 4a68f15 -- crates/core/src/ops/mod.rs crates/core/src/ops/merge.rs crates/core/src/ops/rebase.rs crates/core/tests/merge.rs crates/core/tests/rebase.rs`
  - focused `rg`, `sed`, and `nl` reads across the source docs, artifacts, implementation, and tests
  - `cargo fmt --check`
  - `cargo test -p outpost-core --test merge`
  - `cargo test -p outpost-core --test rebase`
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`
  - `git diff --check 4a68f15^ 4a68f15`
  - disposable raw-Git checks under `/tmp` for ambiguous `local/main` ref resolution
  - disposable `/tmp` scratch binary calling `outpost_core::ops::merge::run` and `outpost_core::ops::rebase::run` against an A/B/C fixture with a colliding local branch

## Review Reasoning

The chunk is mostly scoped correctly. I did not find source-origin refresh behavior inside `merge` or `rebase`, accidental push behavior, CLI/global `-C` work, or Phase 5 E2E behavior. Both ops validate `SourceRemoteRef.remote` against `outpost.metadata().remote_name` before reporting or fetching, use the configured custom remote name, emit one `OutpostFetch` before the user-visible fetch, and return the expected detached-HEAD `NoUpstreamTracking { branch: "HEAD" }` before fetching.

The fetch refspec itself matches the architecture shape: `<branch>:refs/remotes/<remote>/<branch>`. The bug is the next step. After fetching the exact remote-tracking ref, both ops pass the short name `<remote>/<branch>` to `git merge` or `git rebase`. Git does not guarantee that this short name resolves to `refs/remotes/<remote>/<branch>` when a local branch with the same spelling exists.

The committed MR tests cover MR-01..MR-06 happy paths, custom remotes, wrong-remote precondition ordering, reporter events, and detached-head precondition ordering. They do not cover a local branch collision with the remote-tracking ref name.

Verification was otherwise green: `cargo fmt --check`, focused merge/rebase tests, full `outpost-core` tests, `outpost-core --tests`, full workspace tests, and diff whitespace checks all passed.

## Findings

1. `merge` and `rebase` can operate on a local branch named like the source remote ref instead of the fetched source branch. In `crates/core/src/ops/merge.rs:34` and `crates/core/src/ops/rebase.rs:34`, the code builds `remote_branch` as `"{remote}/{branch}"`, then passes that short name to `git merge` / `git rebase` at `crates/core/src/ops/merge.rs:39` and `crates/core/src/ops/rebase.rs:39`. If C has a local branch such as `refs/heads/local/main`, Git warns that `local/main` is ambiguous and resolves it to the local branch, even though the immediately preceding fetch updated `refs/remotes/local/main`. I verified this first with raw Git and then with a `/tmp` scratch binary using the actual `outpost_core` APIs: after `ops::merge::run` and `ops::rebase::run`, `refs/remotes/local/main` matched B's new `main`, but B's source commit was not an ancestor of C's `HEAD` (`source_ancestor=false`) because the operation used the colliding local branch. This violates the product/architecture promise that `gop merge local/main` and `gop rebase local/main` merge/rebase the fetched source-remote ref.

## Missing Evidence

- No MR regression test creates a local branch in C named `local/main` or `custom/main` and proves `merge local/main` / `rebase local/main` still targets `refs/remotes/<remote>/<branch>`.
- No test asserts the final merge/rebase operand is unambiguous after the fetch. The existing tests prove the remote-tracking ref is updated, but not that Git is forced to use that exact ref when applying the operation.

## Required Changes

- Use an unambiguous target for the final operation, for example `refs/remotes/<remote>/<branch>` or the fetched remote-tracking OID, instead of the short `<remote>/<branch>` string.
- Add merge and rebase regression tests with a colliding local branch such as `refs/heads/local/main`, asserting that the fetched source commit becomes part of `HEAD`.

## Nits

none
