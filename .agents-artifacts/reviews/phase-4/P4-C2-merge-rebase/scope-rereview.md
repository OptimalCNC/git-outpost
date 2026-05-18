# Scope Re-review: P4-C2-merge-rebase

## Verdict

approved with nits

Current HEAD reviewed: `6d5ee16 phase-4: fix merge rebase review findings`.

## Evidence Reviewed

- Source docs:
  - `docs/src/product.md` lines 291-310 (`merge <source-ref>` and `rebase <source-ref>`)
  - `docs/src/architecture.md` lines 1125-1161 (`ops/merge.rs`, `ops/rebase.rs`)
  - `docs/src/architecture.md` lines 1966-1975 (MR-01..MR-06)
  - `docs/src/roadmap.md` lines 35-41 (Phase 4 and Phase 5 boundaries)
  - `docs/coordinator-prompt.md` review process sections for scope review evidence and missing-evidence handling
- Chunk artifacts:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
  - Prior reviews in `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/`
- Current implementation and tests:
  - `crates/core/src/ops/merge.rs`
  - `crates/core/src/ops/rebase.rs`
  - `crates/core/tests/merge.rs`
  - `crates/core/tests/rebase.rs`
  - Supporting reads of `crates/core/src/outpost.rs` and `crates/core/src/refname.rs`
- Git and command evidence:
  - `git rev-parse --short HEAD` -> `6d5ee16`
  - `git status --short` -> clean before artifact creation
  - `git diff --name-status 4a68f15..HEAD`
  - `git diff --unified=80 4a68f15..HEAD -- crates/core/src/ops/merge.rs crates/core/src/ops/rebase.rs crates/core/tests/merge.rs crates/core/tests/rebase.rs`
  - `cargo fmt --check` -> pass
  - `cargo test -p outpost-core --test merge` -> pass; 6 tests
  - `cargo test -p outpost-core --test rebase` -> pass; 6 tests

## Scope Findings

- In scope: `crates/core/src/ops/merge.rs` still limits behavior to `ops::merge`. It requires an attached outpost branch before reporting/fetching, validates `SourceRemoteRef.remote` against `outpost.metadata().remote_name`, emits one `OutpostFetch`, fetches `<branch>:refs/remotes/<remote>/<branch>`, and then runs `git merge refs/remotes/<remote>/<branch>` (`merge.rs` lines 16-34 and 55-65).
- In scope: `crates/core/src/ops/rebase.rs` mirrors the same scoped behavior for `ops::rebase`, ending with `git rebase refs/remotes/<remote>/<branch>` (`rebase.rs` lines 16-34 and 55-65).
- In scope: the adopted review fix is present. The final merge/rebase operand is now the full fetched remote-tracking ref returned by `fetch_source_ref`, not the ambiguous short `<remote>/<branch>` form.
- In scope: `crates/core/tests/merge.rs` and `crates/core/tests/rebase.rs` cover MR-01..MR-06 and add the required collision regressions:
  - `merge_uses_full_remote_tracking_ref_when_local_branch_name_collides` creates `refs/heads/local/main` and asserts the fetched source commit is an ancestor of `HEAD` (`merge.rs` lines 187-228).
  - `rebase_uses_full_remote_tracking_ref_when_local_branch_name_collides` creates `refs/heads/local/main` and asserts the rebased parent is the fetched source commit (`rebase.rs` lines 202-248).
- The scoped tests also continue to prove custom source remote support, wrong-remote rejection before fetch, `OutpostFetch` reporting, and detached-head rejection before fetch.

## Protected Path Findings

- No protected-path exceptions were used or needed.
- The only file changed by this re-review is this review artifact.

## Forbidden Scope Findings

- No `ops::push` implementation or tests were added in the reviewed HEAD diff.
- No Phase 5 CLI, global `-C`, E2E, exit-code, `--no-color`, or cross-platform behavior was added.
- No source-origin refresh is implemented inside `ops::merge` or `ops::rebase`; neither module calls `SourceRepo::fast_forward_branch_from_origin` or otherwise fetches from `origin`.
- The only `origin` references in the scoped merge/rebase tests are wrong-remote decoys used to prove pre-fetch validation.
- No unrelated production refactor or docs cleanup was included in the current fix. The code still duplicates small helpers between merge and rebase, but that duplication is local to the scoped ops.

## Missing Evidence

- none

## Required Changes

- none

## Nits

- `.agents-artifacts/progress/phase-4.md` lines 280-289 still lists the `P4-C2-merge-rebase` review-fix commit as pending even though current HEAD is `6d5ee16 phase-4: fix merge rebase review findings`. This is artifact bookkeeping only and does not affect the scope approval.
