# Normal Review Rerun: Phase 0 / git-and-ref-boundary

## Verdict

approved

## Evidence Reviewed

- `c144b69..83a1f74` diff for `crates/core/src/git.rs`
- `crates/core/src/error.rs`
- Evidence pack: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md`
- Previous normal review: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/normal-review.md`
- Scope review: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/scope-review.md`
- Progress log hash/update context
- `docs/src/architecture.md` section 5.2 and section 11.1 U-09
- Supplied verification commands

## Requirement Reasoning

U-09 requires failed argv preserved verbatim. The fix replaces lossy space-joining with bracketed per-argument rendering using escaped contents, so argv element boundaries are now represented unambiguously. `GitFailed.args` and `GitTerminatedBySignal.args` both use the corrected renderer. Architecture section 5.2's `args: String` shape is retained.

## Test Reasoning

Updated U-09 now proves a single argv element containing spaces renders differently from multiple argv elements. Existing tests also continue to cover U-11, U-12, and `run_status`. Tests do not exhaustively prove all escaping cases, but local code inspection shows Unix byte escaping and Windows wide-unit escaping avoid the prior boundary loss.

## Docs Reasoning

No docs change was required. Existing architecture already documents the `GitInvoker` contract and `GitFailed { args: String, ... }`; the implementation now matches that contract more closely.

## Verification Reasoning

Supplied post-fix evidence shows `cargo fmt --check`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, and `cargo test -p outpost-core --features test-helpers` all passing. No verification gap blocks approval.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

none
