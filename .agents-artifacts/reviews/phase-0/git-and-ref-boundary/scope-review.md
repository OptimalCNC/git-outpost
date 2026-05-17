# Scope Review: Phase 0 / git-and-ref-boundary

## Verdict

approved

## Evidence Reviewed

- Changed files and commit `c144b69` diff
- Current uncommitted progress-log hash update
- Evidence pack: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Progress log: `.agents-artifacts/progress/phase-0.md`
- Protected-path rules: none

## Path Matrix

- `.agents-artifacts/progress/phase-0.md` — allowed — Phase 0 tracking artifact; commit updates U-09/U-11/U-12 status, verification, and chunk log. Current uncommitted update only records commit hash.
- `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md` — allowed — Phase 0 review evidence artifact for this chunk.
- `crates/core/Cargo.toml` — allowed — Adds `test-helpers` feature documented for Phase 0 `GitInvoker` test access.
- `crates/core/src/lib.rs` — allowed — Exports Phase 0 `git`/`refname` modules and public boundary types.
- `crates/core/src/git.rs` — allowed — Implements Phase 0 `GitInvoker` boundary and U-09/U-11-supporting tests.
- `crates/core/src/refname.rs` — allowed — Implements Phase 0 validated ref boundary types and U-12-supporting tests.

## Scope Reasoning

Roadmap Phase 0 explicitly includes `git.rs`, `refname.rs`, `reporter.rs`, `error.rs`, workspace skeleton, and fixture. This chunk is limited to `git.rs`, `refname.rs`, exports, feature gating needed for documented test helpers, and colocated unit tests for U-09, U-11, and U-12. The implementation does not add CLI behavior, ops modules, command behavior, fixture behavior, unrelated docs cleanup, or unrelated refactors. `SourceRemoteRef` and `UpstreamRef` are future-facing types, but they are explicitly part of the Phase 0 `refname.rs` architecture boundary.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

none
