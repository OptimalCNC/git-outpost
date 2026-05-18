# P4-C3 Push Publication Scope Review

## Verdict

approved

## Evidence Reviewed

- Commit under review: `03ee3f930b15d11ebb7e39027b66628d56ca882c` (`phase-4: add push publication`)
- Changed-file list from `git show --name-only` / `git diff-tree`:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/ops/push.rs`
  - `crates/core/tests/push.rs`
- Source of truth:
  - `docs/src/product.md` `push` command behavior and deferred/removed push flags
  - `docs/src/architecture.md` sections 5.9.0, 5.9.8, and 11.9
  - `docs/src/roadmap.md` Phase 4/5 boundaries
  - `docs/coordinator-prompt.md` review process and QA/test ownership
- Artifacts reviewed:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
- Implementation evidence inspected:
  - `crates/core/src/ops/push.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/tests/push.rs`

## Scope Findings

- `crates/core/src/ops/mod.rs` only exports the new `push` module, which is required for Phase 4 `ops::push` scope.
- `crates/core/src/ops/push.rs` stays within the assigned operation surface:
  - attached-branch handling maps detached `HEAD` to `NoUpstreamTracking { branch: "HEAD" }`
  - source repo resolution happens before push events
  - checked-out source branch policy is enforced before publication when `receive.denyCurrentBranch` is not `updateInstead`
  - source-branch auto-creation is refused with `AmbiguousBranchCreation`
  - divergence is checked before C-to-B publication
  - publication order is C-to-B, then B-to-A
  - reporter events are limited to `OutpostPush` and `SourcePush`
- `crates/core/tests/push.rs` maps directly to Pu-01..Pu-10 and remains in the core integration-test layer described by `docs/coordinator-prompt.md`.
- The QA and evidence artifacts are under the selected chunk path and describe only P4-C3 push publication behavior.
- The progress-log edits record P4-C3 implementation/evidence and do not expand the roadmap scope.

## Protected Path Findings

- No protected-path exception was required or used.
- No source-of-truth docs were modified.
- No CLI crate, binary entrypoint, global-option handling, or cross-platform E2E test path was modified.
- Production changes are limited to `crates/core/src/ops/mod.rs` and the new `crates/core/src/ops/push.rs`, which are within the assigned Phase 4 chunk.

## Forbidden Scope Findings

- No Phase 5 CLI/global `-C`/E2E/cross-platform behavior was added.
- No new push flags, routing options, or CLI surface were added.
- No source-branch auto-creation path was added; outpost-only branches are explicitly refused.
- No unrelated docs cleanup or broad refactor appears in the changed-file list or inspected diff.

## Missing Evidence

none

## Required Changes

none

## Nits

none
