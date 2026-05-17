**Verdict**: approved with nits

**Evidence Reviewed**: `.agents-artifacts/reviews/phase-2/lock-move-unlock/evidence-pack.md`; `.agents-artifacts/qa/phase-2/lock-move-unlock.md`; `.agents-artifacts/reviews/phase-2/lock-move-unlock/scope-review.md`; `.agents-artifacts/progress/phase-2.md`; source docs `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`; diff `88e1b09..HEAD`; changed production/test files under `crates/core/src/ops/*`, `crates/core/src/outpost.rs`, and `crates/core/tests/lock_move_unlock.rs`; evidence-pack verification summaries for `cargo fmt --check`, core tests, workspace tests, feature tests, and `git diff --check`.

**Review Reasoning**: Phase scope matches the active chunk: only `ops::lock`, `ops::unlock`, `ops::move`, a narrow `Outpost::git()` accessor, LMU tests, and phase artifacts changed. No remove/prune, CLI, status, sync, registry-locking, or unrelated refactor scope was added. Product behavior is implemented: lock/unlock require registered managed outposts and mutate only registry lock state; move requires a registered managed outpost, refuses locked/dirty entries unless forced, validates destination safety, renames, and updates the registry. Architecture sections 5.9.9-5.9.11 are materially followed. Roadmap LMU-01..LMU-08 are mapped to integration tests and QA evidence.

**Verification And Risk Reasoning**: The evidence pack and QA note report passing LMU integration tests and broader `outpost-core`/workspace test commands. The test file covers the required LMU scenarios, including wrong-source and unregistered rejection. Code inspection supports claims that `move --force` bypasses only lock/dirty guards, not managed-outpost or destination validation. Residual risk is limited to edge cases not explicitly required by LMU IDs, such as move-specific coverage for empty existing destinations and `DestinationInsideRepo`; shared safety/helper tests and direct code paths reduce that risk.

**Docs Reasoning**: No docs changes are required for this chunk. The stable product behavior, operation algorithms, safety helper responsibilities, registry lock fields, and LMU test inventory are already documented in `product.md` and `architecture.md`. The evidence pack's "no docs changes" rationale is supported and does not introduce undocumented new concepts.

**Findings**: none

**Missing Evidence**: none

**Required Changes**: none

**Notes**: Scope-review bookkeeping nit was adopted in the progress log by recording checkpoint commit `786473d`. Minor residual test nit: `move` does not add dedicated LMU integration assertions for empty existing destinations or destination-inside-repo rejection, but those are covered by shared safety behavior rather than roadmap LMU IDs.
