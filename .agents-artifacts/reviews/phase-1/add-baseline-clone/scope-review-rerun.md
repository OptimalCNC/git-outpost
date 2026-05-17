# Scope Review Rerun: add-baseline-clone

## Verdict

approved

## Evidence Reviewed

- `git diff --name-status e7d7a47..HEAD`
- Full diffs for all changed files
- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/scope-review.md`
- Protected-path rules
- Rerun commands: `git diff --check`, `cargo fmt --check`, `cargo test -p outpost-core --test add`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`; all passed

## Path Matrix

| Path | Status | Scope Assessment |
|---|---|---|
| `.agents-artifacts/progress/phase-1.md` | allowed | Phase 1 progress artifact; records expanded `add-baseline-clone` scope covering C-01..C-20 and absorption of `add-branch-modes`. |
| `.agents-artifacts/qa/phase-1/add-baseline-clone.md` | allowed | QA artifact for Phase 1 `ops::add`; maps C-01..C-20 to committed integration tests. |
| `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md` | allowed | Evidence artifact; revised claim now covers full Phase 1 add surface C-01..C-20. |
| `.agents-artifacts/reviews/phase-1/add-baseline-clone/scope-review.md` | allowed | Prior scope artifact; records original `needs changes` finding. |
| `crates/core/src/lib.rs` | in scope | Exposes Phase 1 `ops` module. |
| `crates/core/src/ops/mod.rs` | in scope | Adds Phase 1 `ops::add` module only. |
| `crates/core/src/ops/add.rs` | in scope | Implements Phase 1 `ops::add` behavior for C-01..C-20, including branch modes now explicitly claimed. |
| `crates/core/src/source_repo.rs` | in scope | Adds crate-private `git()` helper needed by Phase 1 add orchestration. |
| `crates/core/tests/add.rs` | in scope | Core integration coverage for C-01..C-20; no CLI/e2e/global CLI behavior. |
| `crates/core/tests/common/fixture.rs` | in scope | Narrow test helper for source branch setup in add integration tests. |

## Scope Reasoning

The revised source-of-truth artifacts now explicitly expand `add-baseline-clone` to cover all Phase 1 `ops::add` IDs C-01..C-20. That matches `docs/src/roadmap.md`, which places `ops::add` and C-01..C-20 in Phase 1, and matches `docs/src/product.md` plus `docs/src/architecture.md` sections 5.9.1 and 11.2, which define both existing-branch and `-b` branch creation behavior.

The previously out-of-scope `AddCheckout::NewBranch` execution is now claimed, evidenced, and tested in this chunk. No protected paths were touched, no `docs/src/*` files changed, and no Phase 2+ commands, CLI binary/e2e/global CLI behavior, unrelated docs cleanup, or unrelated refactors appear in the range.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

`HEAD` reviewed was `87f9df4`, with implementation checkpoint `8b894d9` and scope-fix implementation `1227687` present in the inspected range.
