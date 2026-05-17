# Scope Review: list-ahead-behind

- Verdict: approved
- Review scope/range: `a2ec631..HEAD`
- HEAD reviewed: `d5280ebac5e03965df011b89d997beb0d38649c1` (`phase-1: add list ahead behind`)
- Worktree check: no uncommitted changes observed.

## Path Matrix

| Path | Status | Scope assessment |
| --- | --- | --- |
| `.agents-artifacts/progress/phase-1.md` | modified | In scope: progress/evidence bookkeeping for active chunk. |
| `.agents-artifacts/qa/phase-1/list-ahead-behind.md` | added | In scope: QA note for L-05/L-06. |
| `.agents-artifacts/reviews/phase-1/list-ahead-behind/evidence-pack.md` | added | In scope: required review evidence. |
| `crates/core/src/outpost.rs` | modified | In scope: adds `Outpost::ahead_behind_source`, already Phase 1 architecture surface. |
| `crates/core/src/ops/list.rs` | modified | In scope: surfaces ahead/behind in `OutpostSummary`. |
| `crates/core/tests/common/fixture.rs` | modified | In scope: helper needed for L-05 integration coverage. |
| `crates/core/tests/list.rs` | modified | In scope: adds L-05/L-06 integration tests. |

## Scope Reasoning

- Product requires `list` to include ahead/behind state relative to the local source repository.
- Architecture explicitly includes `Outpost::ahead_behind_source` and `OutpostSummary.ahead_behind`.
- Roadmap Phase 1 includes `outpost.rs` and `ops::list`, with L-01..L-10 in scope.
- L-05/L-06 specifically require list ahead/behind counts after one commit in the outpost/source respectively.
- No CLI formatting, CLI dispatch/global `-C`, Phase 2+ commands, status behavior, or unrelated docs/refactors were changed.

## Findings

none

## Missing Evidence

none blocking. Evidence pack and QA artifact identify L-05/L-06 and recorded verification. Note: the top QA/Test Map in the progress log still labels `L-05..L-06` as `planned`, while later progress sections record implementation and verification; this is bookkeeping cleanup, not a scope blocker.

## Required Changes

none

## Notes

The reviewer did not perform deep implementation correctness review beyond scope impact and did not edit files.
