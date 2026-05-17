# Scope Review: add-baseline-clone

## Verdict

needs changes

## Evidence Reviewed

- Changed files from `git diff --name-status e7d7a47..HEAD`
- Full implementation and artifact diffs
- `docs/src/product.md`
- `docs/src/architecture.md` sections 5.9.1 and 11.2
- `docs/src/roadmap.md` Phase 1 row
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- Protected-path rules: none

## Path Matrix

- `.agents-artifacts/progress/phase-1.md`: allowed artifact update; records active chunk, test IDs, evidence, verification, and review status.
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`: allowed QA evidence for claimed C-01, C-02, C-10..C-12, C-14..C-16, C-20 coverage.
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`: allowed evidence artifact.
- `crates/core/src/lib.rs`: in scope; exports Phase 1 `ops`.
- `crates/core/src/ops/mod.rs`: in scope; adds Phase 1 `add` module.
- `crates/core/src/ops/add.rs`: mostly in scope for baseline add, but includes `AddCheckout::NewBranch` branch-creation execution that belongs to the later `add-branch-modes` chunk.
- `crates/core/src/source_repo.rs`: in scope; crate-private `git()` supports Phase 1 add operation.
- `crates/core/tests/add.rs`: in scope; core integration tests only, no CLI/e2e behavior.
- `crates/core/tests/common/fixture.rs`: in scope; test helper for source branch setup.

## Scope Reasoning

The changed files are all Phase 1 core/library or Phase 1 artifact files. No protected paths were touched, no `docs/src/*` files changed, and there is no CLI, e2e, global `-C`, or Phase 2+ command implementation. The claimed chunk scope is baseline clone behavior for existing source branches plus metadata, registry, safety prechecks, reporter event, and selected C-test coverage. Most implementation matches that scope.

However, `crates/core/src/ops/add.rs` implements later branch-mode behavior for `AddCheckout::NewBranch`: it resolves the target for `NewBranch` and creates/fetches/switches the new branch. Progress and evidence both say branch-mode QA remains assigned to later add chunks, so this is outside the selected `add-baseline-clone` claim.

## Findings

- `crates/core/src/ops/add.rs:98` and `crates/core/src/ops/add.rs:146` implement `AddCheckout::NewBranch` behavior assigned to the later `add-branch-modes` chunk. This advances C-03/C-04 style branch creation behavior without claiming or evidencing that test scope in this chunk.

## Missing Evidence

- Evidence or scope authorization for implementing `AddCheckout::NewBranch` behavior in `add-baseline-clone`; current artifacts explicitly defer branch-mode QA to later chunks.

## Required Changes

- Either remove/defer the `NewBranch` execution path from this chunk, or escalate/update the chunk scope and evidence to explicitly include the advanced branch-mode behavior with corresponding QA coverage.

## Notes

No source docs changes were present, which is consistent with the evidence pack's claim that product and architecture already cover the stable add contract.
