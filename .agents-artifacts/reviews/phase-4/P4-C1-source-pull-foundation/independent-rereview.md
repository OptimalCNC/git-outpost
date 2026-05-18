# Independent Re-review: P4-C1-source-pull-foundation

## Verdict

approved with nits

## Evidence Reviewed

- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Chunk artifacts:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
- Prior reviews in `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/`:
  - `scope-review.md`
  - `normal-review.md`
  - `independent-review.md`
  - `scope-rereview.md`
  - `normal-rereview.md`
- Commits reviewed:
  - `9d491be phase-4: add source pull foundation`
  - `96969ea phase-4: fix source pull review findings`
- Implementation files inspected:
  - `crates/core/src/ops/source.rs`
  - `crates/core/src/ops/pull.rs`
  - `crates/core/src/source_repo.rs`
  - `crates/core/src/safety.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/reporter.rs`
- Test files inspected:
  - `crates/core/tests/source.rs`
  - `crates/core/tests/pull.rs`
  - `crates/core/tests/common/fixture.rs`
  - `crates/core/src/safety.rs` unit tests
- Commands run or inspected:
  - `git status --short --branch`
  - `git rev-parse HEAD`
  - `git show --stat --oneline --decorate --no-renames 96969ea`
  - `git diff --stat --no-renames 9d491be 96969ea`
  - `git diff --no-renames 9d491be 96969ea -- crates/core/src/safety.rs crates/core/src/source_repo.rs crates/core/src/ops/source.rs crates/core/src/ops/pull.rs`
  - focused `rg`, `sed`, and `nl` reads across source docs, artifacts, implementation, and tests
  - `cargo fmt --check`
  - `cargo test -p outpost-core --lib safety::tests::check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref`
  - `cargo test -p outpost-core --test source`
  - `cargo test -p outpost-core --test pull`
  - `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-rereview-target cargo test -p outpost-core`
  - `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-rereview-target cargo test -p outpost-core --tests`
  - `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-rereview-target cargo test --workspace`
  - `git diff --check HEAD`
  - `git diff --check 9d491be 96969ea`

## Review Reasoning

The required stale remote-tracking ref fix from the prior independent review is correctly adopted. `safety::check_no_divergence` now verifies the exact upstream branch with `git ls-remote <remote> <merge_ref>` before trusting the local `refs/remotes/<remote>/<branch>` ref. If the upstream branch has been deleted but a stale remote-tracking ref remains, the helper returns typed `BranchNotFound` instead of incorrectly accepting the stale ref.

The regression test `safety::tests::check_no_divergence_rejects_deleted_upstream_branch_despite_stale_tracking_ref` directly covers the requested case: it creates `refs/remotes/local/feature`, deletes the source `feature` branch, confirms the stale tracking ref still exists, and verifies `check_no_divergence` returns `BranchNotFound`.

`SourceRepo::fast_forward_branch_from_origin` remains aligned with the architecture behavior: it requires the source branch to exist, fetches `origin <branch>:refs/remotes/origin/<branch>`, treats equal/source-ahead as no-op, fast-forwards source-behind branches with either checked-out worktree `merge --ff-only` or unchecked-out `update-ref`, and rejects true divergence. Its return type now matches the architecture API shape, while `ops::source` and `ops::pull` compute update booleans by comparing branch OIDs before and after source refresh.

`ops::source::pull` and `ops::pull::run` still match the product and architecture sequencing. Source pull emits `SourceFetch` and refreshes the named source branch from `origin`. Pull runs from an attached outpost branch, refreshes the matching source branch first, uses the metadata remote name for C->B divergence and fast-forward, emits ordered `SourceFetch` then `OutpostFetch`, and uses `git pull --ff-only <remote> <branch>` for the outpost update.

I did not find merge/rebase/push implementation, CLI/global `-C`, Phase 5 E2E behavior, or strategy flag behavior added in this checkpoint. The only merge command in the reviewed implementation is the required `git merge --ff-only` used to update a checked-out source branch during source fast-forward.

## Findings

none

## Missing Evidence

none blocking

One low-risk residual gap remains from the earlier normal review: there is no dedicated equal B/origin no-op integration test. The equal path is straightforward (`local_oid == remote_oid`) and adjacent no-op/source-ahead behavior is exercised by P-02, so this does not require a change for P4-C1.

## Required Changes

none

## Nits

- `.agents-artifacts/progress/phase-4.md` still has stale bookkeeping text: the commit log says `pending P4-C1-source-pull-foundation review-fix commit`, and the next action still recommends committing review fixes. This re-review is after fixed commit `96969ea`, so the implementation is not blocked; the progress artifact should be refreshed when artifacts are next updated.
