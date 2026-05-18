# Evidence Pack: P4-C3-push-publication

## Phase And Chunk

- Phase: `phase-4`
- Chunk: `P4-C3-push-publication`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `push`
  - Architecture 5.9.0 `Reporter` event sink
  - Architecture 5.9.8 `ops/push.rs`
  - Architecture 11.9 integration tests
  - Roadmap Phase 4 scope
- Roadmap test IDs advanced: Pu-01..Pu-10

## Changed Files

- `.agents-artifacts/progress/phase-4.md`
- `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
- `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/push.rs`
- `crates/core/tests/push.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports the new `push` module.
- `ops/push.rs`
  - Adds `PushOptions`, `PushReport`, `StepResult`, and `run`.
  - Requires an attached outpost branch and maps detached `HEAD` to `NoUpstreamTracking { branch: "HEAD" }`.
  - Resolves the source repo from outpost metadata and returns `SourceMissing` before emitting push events if B is unavailable.
  - Reads B's local `receive.denyCurrentBranch`; if it is not `updateInstead` and B has the target branch checked out, returns `PushIntoCheckedOutBranch` before pushing.
  - Refuses outpost-only branches with `AmbiguousBranchCreation` instead of creating a source branch.
  - Uses `safety::check_no_divergence` against the configured outpost remote before pushing C to B.
  - Emits `OutpostPush`, runs `git push <metadata.remote_name> <branch>:<branch>`, emits `SourcePush`, then runs `git push origin <branch>:<branch>` from B.
  - Computes `StepResult::Pushed { commits }` for each hop, using `ls-remote origin refs/heads/<branch>` for the origin-side before/after OIDs so the report does not depend on stale local remote-tracking refs.
- `tests/push.rs`
  - Adds core integration coverage for Pu-01..Pu-10 using real A/B/C fixture repositories and `CapturingReporter`.
- QA artifact
  - Records QA coverage, verification, and handoff notes for P4-C3.

## Tests Added / Updated

- Unit tests added/updated: none

## Integration Tests Added / Updated

- `pu01_push_sends_outpost_branch_to_source_then_origin` covers Pu-01.
- `pu02_push_records_outpost_push_and_source_push_events` covers Pu-02.
- `pu03_push_from_outpost_only_branch_returns_ambiguous_branch_creation` covers Pu-03.
- `pu04_push_when_source_diverged_from_outpost_returns_divergence` covers Pu-04.
- `pu05_push_dirty_outpost_succeeds` covers Pu-05.
- `pu06_push_with_missing_source_returns_source_missing` covers Pu-06.
- `pu07_push_uses_custom_remote_for_outpost_to_source_and_origin_for_source_to_upstream` covers Pu-07.
- `pu08_push_into_dirty_checked_out_source_branch_surfaces_update_instead_git_failed` covers Pu-08.
- `pu09_push_with_deny_current_branch_refuse_returns_push_into_checked_out_branch` covers Pu-09.
- `pu10_push_on_detached_head_returns_no_upstream_tracking_head_before_push` covers Pu-10.

## Docs Added / Updated

- none
- Rationale: product and architecture already document push sequencing, source branch creation refusal, checked-out source policy, reporter events, and test scenarios.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --test push`: pass; 10 push integration tests
- `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 10 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; same test binaries excluding doctests
- `cargo test --workspace`: pass; same workspace coverage, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `P4-C3-push-publication` behavior.

## Residual Risks / Handoff Notes

- `ops::push` intentionally does not create source branches; outpost-only branches return `AmbiguousBranchCreation`.
- Dirty outpost worktrees do not block push; dirty checked-out source worktrees are left to Git's `updateInstead` failure path and surface as `GitFailed`.
- C->B uses the outpost metadata remote name; B->A always uses `origin`.
- CLI/global `-C`, whole-binary E2E behavior, and user-facing command formatting remain Phase 5.
