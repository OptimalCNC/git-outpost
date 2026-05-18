# Independent Re-review: P4-C3-push-publication

## Verdict

Accepted. Current HEAD `594890f phase-4: fix push review findings` resolves the three prior independent-review findings, and focused verification did not reveal new push-publication side effects.

## Evidence Reviewed

- Source docs: `docs/src/product.md`; `docs/src/architecture.md` sections 5.8, 5.9.8, and 11.9; `docs/src/roadmap.md`.
- Prior review and evidence artifacts:
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/independent-review.md`
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
  - `.agents-artifacts/progress/phase-4.md`
- Implementation and tests:
  - `crates/core/src/ops/push.rs`
  - `crates/core/tests/push.rs`
  - `crates/core/tests/common/fixture.rs`

Verification run:

- `cargo test -p outpost-core --test push`: passed, 13/13 tests.
- `cargo test -p outpost-core`: passed, including 48 unit tests and all current core integration tests.

## Previous Finding Resolution

1. C-behind-B fell through to `GitFailed` after `OutpostPush`.

   Resolved. `ops::push::run` now calls `check_source_fast_forwardable` before reporter push steps. That helper fetches the current source branch into C's source-remote tracking ref and returns typed `Divergence { branch }` whenever B is ahead of C, including pure C-behind-B and both-sides-unique histories. The regression test `push_when_outpost_is_behind_source_returns_divergence_before_push` asserts `Divergence`, no reporter steps, unchanged B, and unchanged A.

2. Origin-ahead could mutate B before B->origin failed as `GitFailed`.

   Resolved. `ops::push::run` now reads the current `origin/<branch>` OID with `ls-remote` and runs `check_origin_fast_forwardable` before the C->B push. If the origin tip is not an ancestor of C `HEAD`, the operation returns typed `Divergence` before emitting `OutpostPush` or changing B. The regression test `push_when_origin_is_ahead_returns_divergence_before_source_mutation` asserts `Divergence`, no reporter steps, unchanged B, and unchanged A.

3. First push to absent origin branch overreported `source_to_origin` commit count.

   Resolved. The absent-origin count path now computes `rev-list --count <after> --not --remotes=origin`, so it counts commits newly introduced relative to known origin refs instead of the entire reachable history from the branch tip. The regression test `push_first_publication_to_absent_origin_branch_counts_only_new_commits` covers a source-existing branch absent from origin and asserts `source_to_origin: Pushed { commits: 1 }`.

## Findings

None.

## Missing Evidence

None for this re-review scope. The added tests directly exercise the three prior failure modes with error-variant, event-ordering, ref-movement, and commit-count assertions.

## Required Changes

None.

## Nits

- Architecture section 5.9.8 still describes the C<->B preflight as `safety::check_no_divergence`; the implementation now uses a push-specific fast-forwardability check. This is documentation drift, not a blocker for the code fix reviewed here.
