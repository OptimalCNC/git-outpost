# Scope Review: list-basic-summaries

## Verdict

approved

## Evidence Reviewed

- Changed files in `548ca5d..HEAD`
- Per-path diffs
- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/list-basic-summaries/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/list-basic-summaries.md`
- Protected-path rules

## Path Matrix

- `.agents-artifacts/progress/phase-1.md`: allowed; records chunk scope, completed evidence, verification, and checkpoint status.
- `.agents-artifacts/reviews/phase-1/list-basic-summaries/evidence-pack.md`: allowed; evidence artifact for this chunk.
- `.agents-artifacts/qa/phase-1/list-basic-summaries.md`: allowed; QA note for L-01..L-04 and L-07..L-10.
- `crates/core/src/ops/list.rs`: allowed; Phase 1 `ops::list` basic summaries from `SourceRepo`.
- `crates/core/src/ops/mod.rs`: allowed; exports the Phase 1 list op module.
- `crates/core/tests/common/fixture.rs`: allowed; test support for list integration coverage.
- `crates/core/tests/list.rs`: allowed; core integration tests for claimed list IDs, no CLI/e2e behavior.

## Scope Reasoning

The implementation is confined to Phase 1 core behavior for `ops::list` and its core tests. It adds registry-backed summaries with path, current branch, clean/dirty/missing/not-managed state, and lock fields, matching the claimed `list-basic-summaries` scope.

`ahead_behind` is left as `None` and L-05/L-06 are explicitly deferred to `list-ahead-behind`, so the chunk does not implement the out-of-scope ahead/behind behavior. There are no CLI crate, binary dispatch, global `-C`, formatting, Phase 2 lock/unlock command, or unrelated docs changes. Direct registry locking in the L-10 test is setup for list summary coverage, not Phase 2 command behavior.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

L-05/L-06 remain deferred as documented; CLI formatting/dispatch remains Phase 5 scope.
