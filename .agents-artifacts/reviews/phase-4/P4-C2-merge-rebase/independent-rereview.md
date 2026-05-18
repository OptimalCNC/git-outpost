# Independent Re-review: P4-C2-merge-rebase

## Verdict

Approved. The prior independent finding is resolved in current HEAD
`6d5ee16 phase-4: fix merge rebase review findings`.

`ops::merge` and `ops::rebase` now fetch into and then operate on the full
`refs/remotes/<remote>/<branch>` target instead of the ambiguous short
`<remote>/<branch>` spelling. Focused collision regressions exist for both
operations and pass.

## Evidence Reviewed

- Source docs:
  - `docs/src/product.md` merge/rebase source-ref behavior and no implicit
    source-origin refresh.
  - `docs/src/architecture.md` sections 5.9.6, 5.9.7, and 11.8.
  - `docs/src/roadmap.md` Phase 4 scope for `ops::merge` and `ops::rebase`.
- Prior review artifacts:
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/independent-review.md`
  - `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`
  - `.agents-artifacts/progress/phase-4.md`
- Current implementation:
  - `crates/core/src/ops/merge.rs:33` fetches and returns the full
    remote-tracking ref; `crates/core/src/ops/merge.rs:34` passes that full ref
    to `git merge`.
  - `crates/core/src/ops/rebase.rs:33` fetches and returns the full
    remote-tracking ref; `crates/core/src/ops/rebase.rs:34` passes that full ref
    to `git rebase`.
  - `crates/core/src/ops/merge.rs:55-65` and
    `crates/core/src/ops/rebase.rs:55-65` construct
    `refs/remotes/<remote>/<branch>` and fetch
    `<branch>:refs/remotes/<remote>/<branch>`.
- Current regression tests:
  - `crates/core/tests/merge.rs:187-228`
    `merge_uses_full_remote_tracking_ref_when_local_branch_name_collides`
    creates `refs/heads/local/main`, runs merge with `local/main`, verifies
    `refs/remotes/local/main` was fetched to the source OID, and proves that
    source OID is an ancestor of `HEAD`.
  - `crates/core/tests/rebase.rs:202-248`
    `rebase_uses_full_remote_tracking_ref_when_local_branch_name_collides`
    creates `refs/heads/local/main`, runs rebase with `local/main`, verifies
    `refs/remotes/local/main` was fetched to the source OID, and proves
    `HEAD^` is the source OID.
- Verification run:
  - `git rev-parse --short HEAD`: `6d5ee16`
  - `git status --short`: clean
  - `cargo test -p outpost-core --test merge merge_uses_full_remote_tracking_ref_when_local_branch_name_collides -- --exact`: pass
  - `cargo test -p outpost-core --test rebase rebase_uses_full_remote_tracking_ref_when_local_branch_name_collides -- --exact`: pass
  - `cargo test -p outpost-core --test merge`: pass, 6 tests
  - `cargo test -p outpost-core --test rebase`: pass, 6 tests
  - `cargo fmt --check`: pass

## Previous Finding Resolution

Resolved. The prior finding was that both operations fetched the exact
remote-tracking ref but then invoked Git with the short `<remote>/<branch>`
operand, which can resolve to a colliding local branch such as
`refs/heads/local/main`.

Current HEAD no longer builds or passes the short final operand for the Git
operation. Both `fetch_source_ref` helpers return the full
`refs/remotes/<remote>/<branch>` string after the fetch, and `run` passes that
returned string directly to `git merge` or `git rebase`.

The new regression tests cover the exact failure mode from the prior review:
they create a local branch named `local/main` before running the operation and
then assert the fetched source commit, not the colliding local branch, is the
one integrated into the outpost branch.

## Findings

None.

## Missing Evidence

None for the prior finding. The focused merge and rebase collision
regressions are present and passing.

## Required Changes

None.

## Nits

- `docs/src/architecture.md` sections 5.9.6 and 5.9.7 still describe the final
  internal Git command as `git merge <remote_name>/<branch>` and
  `git rebase <remote_name>/<branch>`. The implementation now correctly uses
  the full `refs/remotes/<remote_name>/<branch>` operand. This is a docs wording
  cleanup nit, not a blocker for the prior production-code finding.
