# P4-C3 Push Publication Scope Re-review

## Verdict

approved with nits

The current HEAD `594890f phase-4: fix push review findings` stays within the
`P4-C3-push-publication` chunk scope. I found no required scope, protected-path,
or forbidden-scope changes.

## Evidence Reviewed

- Commit under review: `594890f phase-4: fix push review findings`.
- Cumulative P4-C3 boundary checked from `5e42d0b phase-4: start push publication` through HEAD.
- Review-fix boundary checked from `03ee3f9 phase-4: add push publication` through HEAD.
- Source of truth:
  - `docs/src/product.md` `push`, Working Directory Matrix, and Deferred/Removed Surface.
  - `docs/src/architecture.md` sections 5.9.0, 5.9.8, and 11.9.
  - `docs/src/roadmap.md` Phase 4/5 boundaries.
  - `docs/coordinator-prompt.md` review process and QA/test ownership.
- Artifacts reviewed:
  - `.agents-artifacts/progress/phase-4.md`
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
  - prior P4-C3 `scope-review.md`, `normal-review.md`, and `independent-review.md`
- Implementation evidence inspected:
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/ops/push.rs`
  - `crates/core/tests/push.rs`

## Scope Findings

- The cumulative P4-C3 file set is limited to the progress/QA/review artifacts,
  the `push` module export, `crates/core/src/ops/push.rs`, and
  `crates/core/tests/push.rs`.
- The review-fix commit addresses the independent-review findings inside the
  assigned push publication surface:
  - C->B fast-forward preflight rejects C-behind-B and divergent histories as
    `Divergence` before `OutpostPush`.
  - existing `origin/<branch>` preflight rejects predictable B->A
    non-fast-forward publication before mutating B.
  - absent-origin first-publication reporting counts commits not already
    reachable from origin refs.
  - focused core integration regressions were added for those cases.
- `ops::push` remains a core operation with `PushOptions`, `PushReport`,
  attached-branch handling, source resolution, source-branch existence refusal,
  checked-out source policy, two-hop C->B then B->A publication, and
  `OutpostPush`/`SourcePush` reporting.
- `crates/core/tests/push.rs` remains in the core integration-test layer and
  maps to Pu-01..Pu-10 plus the accepted review regressions.
- Artifact updates describe P4-C3 push behavior and review evidence; they do
  not redefine the product surface.

## Protected Path Findings

- No protected-path exception was required or used.
- Source-of-truth docs were not modified by the current P4-C3 review-fix
  commit.
- No CLI crate, binary entrypoint, global-option handler, whole-binary E2E
  test, or cross-platform path was modified.
- Production code changes are confined to `crates/core/src/ops/push.rs` for
  the review fix, with the cumulative chunk adding only the `push` module export
  in `crates/core/src/ops/mod.rs`.

## Forbidden Scope Findings

- No Phase 5 CLI/global `-C`/E2E/cross-platform behavior was added.
- No new push flags, routing options, or CLI command surface were added.
- No source-branch auto-creation path was added; outpost-only branches still
  return `AmbiguousBranchCreation`.
- No unrelated docs cleanup, broad refactor, or unrelated production module
  change appears in the inspected diffs.

## Missing Evidence

none for scope

## Required Changes

none

## Nits

- `.agents-artifacts/progress/phase-4.md` still says the
  `P4-C3-push-publication` review-fix commit is pending and that the next
  recommended action is to commit review fixes. HEAD is already
  `594890f phase-4: fix push review findings`, so that progress-log text is
  stale.
