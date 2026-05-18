# Scope Review: P4-C2-merge-rebase

## **Verdict**

Approved for scope.

Commit reviewed: `4a68f15 phase-4: add merge rebase`.

## **Scope Reviewed**

- Phase 4 scope: `ops::source`, `ops::pull` (UpstreamRef-driven), `ops::merge`, `ops::rebase`, `ops::push`, sync `Reporter` events.
- Chunk scope: `ops::merge`, `ops::rebase`, configured source-remote validation, detached-head preconditions, and `OutpostFetch` reporting.
- Test IDs reviewed for this chunk: MR-01..MR-06.
- Changed files in `4a68f15`:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/ops/merge.rs`
  - `crates/core/src/ops/rebase.rs`
  - `crates/core/tests/merge.rs`
  - `crates/core/tests/rebase.rs`

## **Evidence Reviewed**

- Source docs:
  - `docs/src/product.md`
  - `docs/src/architecture.md`
  - `docs/src/roadmap.md`
- Review artifacts:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
- Git evidence:
  - `git show --stat --oneline --decorate --name-status 4a68f15`
  - `git diff-tree --no-commit-id --name-status -r 4a68f15`
  - Direct review of changed production and test files.
- Claimed verification in evidence pack:
  - `cargo fmt --check`
  - `cargo check -p outpost-core`
  - `cargo test -p outpost-core --test merge`
  - `cargo test -p outpost-core --test rebase`
  - `cargo test -p outpost-core --test source`
  - `cargo test -p outpost-core --test pull`
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`

## **Scope Findings**

- In scope: `crates/core/src/ops/merge.rs` adds the expected `MergeOptions`, `MergeReport`, attached-branch precondition, configured source-remote validation, `OutpostFetch` event, source-remote fetch into `refs/remotes/<remote>/<branch>`, and `git merge <remote>/<branch>`.
- In scope: `crates/core/src/ops/rebase.rs` mirrors the same chunk-scoped behavior for rebase and runs `git rebase <remote>/<branch>`.
- In scope: `crates/core/src/ops/mod.rs` only exports the new `merge` and `rebase` modules.
- In scope: `crates/core/tests/merge.rs` and `crates/core/tests/rebase.rs` cover MR-01..MR-06 across baseline merge/rebase behavior, custom source remote names, wrong-remote rejection before fetch, `OutpostFetch` reporting, and detached-head rejection before fetch.
- The implementation does not silently broaden into other Phase 4 operations beyond the module export needed for merge/rebase.

## **Protected Path Findings**

- Protected paths: none.
- No protected-path exception is needed.

## **Forbidden Scope Findings**

- No `ops::push` implementation or tests were added.
- No Phase 5 CLI binaries, global `-C`, E2E, exit-code, `--no-color`, or cross-platform behavior was added.
- No unrelated docs cleanup or broad refactor was included.
- No refresh of source repo B from `origin` is implemented inside merge/rebase. The only `origin` usage in changed tests is the MR-04 decoy wrong-remote validation case.

## **Missing Evidence**

- None blocking for scope review.

## **Required Changes**

- None.

## **Nits**

- `.agents-artifacts/progress/phase-4.md` still lists the `P4-C2-merge-rebase` implementation/evidence commit as pending in the commit log, even though this review target is `4a68f15`. This is artifact bookkeeping only and does not change the scope verdict.
