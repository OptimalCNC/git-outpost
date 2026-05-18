# Phase 2 Progress

## Phase

- `phase_id`: `phase-2`
- Roadmap scope: `ops::lock`, `ops::move`, `ops::unlock`, `ops::remove`, `ops::prune`
- Test IDs: LMU-01..LMU-08, R-01..R-11, Pr-01..Pr-09
- Progress log path: `.agents-artifacts/progress/phase-2.md`
- Review artifact root: `.agents-artifacts/reviews/phase-2/`
- QA artifact root: `.agents-artifacts/qa/phase-2/`
- Protected paths: none
- Protected exceptions: none
- Forbidden scope:
  - implementation outside Phase 2 unless required to make Phase 2 compile and explicitly justified here
  - Phase 3+ status/sync command behavior
  - CLI binary/e2e/global CLI behavior from Phase 5
  - unrelated documentation cleanup
  - unrelated refactors
- Required verification:
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`
- Invocation note: Phase 2 was started after Phase 1 closeout from the user's phase-by-phase instruction, using roadmap scope and coordinator defaults rather than a separate pasted Phase 2 invocation block.

## Source Docs

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- Last observed repo revision before Phase 2: `3c3f7fe phase-1: record closeout commit`

## Current Snapshot

- Branch: `main`
- Initial Phase 2 `git status --short --untracked-files=all`: clean
- Workspace: one member, `outpost-core`
- Existing implementation: Phase 0/1 core foundation plus `source_repo.rs`, `outpost.rs`, `metadata.rs`, `registry.rs`, `safety.rs`, `ops::add`, and `ops::list`
- Missing Phase 2 files at start: `ops/lock.rs`, `ops/move.rs`, `ops/unlock.rs`, `ops/remove.rs`, `ops/prune.rs`, `crates/core/tests/lock_move_unlock.rs`, `crates/core/tests/remove.rs`, `crates/core/tests/prune.rs`
- Toolchain observed: `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
- Baseline verification before Phase 2 planning: required verification passed during Phase 2 readiness with 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests

## Readiness Log

- Verdict: `ready with cautions`
- Auditor: native subagent `019e3825-f262-7b02-bec6-f4374c614314`
- Phase reviewed: `phase-2`; roadmap scope `ops::lock`, `ops::move`, `ops::unlock`, `ops::remove`, `ops::prune`; test IDs LMU-01..LMU-08, R-01..R-11, Pr-01..Pr-09
- Source documents reviewed:
  - `docs/src/product.md` Core Model, Artifact Ownership And Deletion, Working Directory Matrix, `lock`/`unlock`/`move`/`remove`/`prune`, Options, Deferred Or Removed Surface
  - `docs/src/architecture.md` sections 5.1, 5.4, 5.5, 5.7, 5.8, 5.9.0, 5.9.9..5.9.13, 7, 11.4, 11.10, 11.11, 13, 14
  - `docs/src/roadmap.md` Phase 2 row
  - `docs/coordinator-prompt.md` Phase Invocation Interface, Phase Readiness Gate, QA/Test Ownership
- Repo state evidence:
  - cwd `/home/huwei/projects/git-outpost`
  - branch `main`
  - HEAD `3c3f7fe`
  - `git status --short --untracked-files=all` produced no output
  - workspace metadata reports one member, `outpost-core`
  - `crates/core/src/ops/mod.rs` exports only `add` and `list`; Phase 2 op modules/tests are not present yet, as expected
  - existing support includes registry lock fields/mutators, `update_path`, `remove_by_path`, `safety::check_clean`, `safety::check_path_is_managed_outpost_of`, and `safety::check_destination_clean`
- Prerequisites checked:
  - Phase 0 progress log records closeout passed with required verification
  - Phase 1 progress log records closeout passed, all Phase 1 IDs implemented, review gates complete, and current history includes `258abf2 phase-1: close phase` and `3c3f7fe phase-1: record closeout commit`
  - Phase 1 provided add/list, registry, metadata, source/outpost discovery, and safety foundations needed by Phase 2
- Toolchain evidence:
  - `cargo --version`: `cargo 1.94.0`
  - `rustc --version`: `rustc 1.94.0`
  - `git --version`: `git version 2.43.0`
  - `cargo metadata --no-deps --format-version 1`: passed
  - `cargo test -p outpost-core`: passed; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: passed; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test
  - `cargo test --workspace`: passed; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
- Spec/architecture/roadmap consistency: pass. Roadmap Phase 2 exactly matches the invocation scope and test ranges. Product behavior for locking, moving, removing, and pruning is reflected in architecture sections 5.9.9..5.9.13 and test inventory sections 11.4, 11.10, 11.11. Open questions OQ-1..OQ-4 are post-MVP and do not block Phase 2.
- Blocking issues: none
- Non-blocking cautions:
  - `Outpost::unpushed_commits` and `safety::check_no_unpushed` are documented and required for R-03/R-05 but are not implemented yet; adding them in Phase 2 must be explicitly justified here as required support for `ops::remove`.
  - Lock/unlock contextual CLI behavior from an outpost is product scope, but CLI dispatch remains Phase 5; Phase 2 should keep to core ops unless a test explicitly requires more.
  - Registry file locking is documented post-MVP, so Phase 2 should not broaden into concurrency behavior.
- Recommended first chunk: implement and test `ops::lock`, `ops::unlock`, and `ops::move` with `crates/core/tests/lock_move_unlock.rs` covering LMU-01..LMU-08.
- Required human decisions: none

## QA/Test Map

| ID | Scope | Status | Notes |
| --- | --- | --- | --- |
| LMU-01 | `lock --reason keep C` marks C locked in registry | implemented passing | `lock_with_reason_marks_registry_entry_locked` |
| LMU-02 | `unlock C` clears locked state and reason | implemented passing | `unlock_clears_registry_lock_state_and_reason` |
| LMU-03 | `move C D` moves outpost and updates registry path | implemented passing | `move_updates_filesystem_and_registry_path` |
| LMU-04 | `move C D` refuses locked C with `OutpostLocked` | implemented passing | `move_refuses_locked_outpost_without_force` |
| LMU-05 | `move --force C D` moves locked C and preserves lock state | implemented passing | `move_force_moves_locked_outpost_and_preserves_lock` |
| LMU-06 | `move C D` refuses dirty C; force succeeds | implemented passing | `move_refuses_dirty_outpost_but_force_succeeds` |
| LMU-07 | `move C D` refuses non-empty destination | implemented passing | `move_refuses_non_empty_destination` |
| LMU-08 | `lock`, `move`, and `unlock` reject paths not registered to current source | implemented passing | `lock_move_unlock_reject_unregistered_paths`, `lock_move_unlock_reject_wrong_source_registered_path` |
| R-01 | `remove C` clean fully-pushed C deletes path and registry entry | implemented passing | `remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry` |
| R-02 | `remove C` dirty C returns `DirtyTree { hint: "pass --force" }` | implemented passing | `remove_dirty_outpost_returns_dirty_tree_with_force_hint` |
| R-03 | `remove C` unpushed C returns `UnpushedCommits` | implemented passing | `remove_unpushed_outpost_returns_unpushed_commits` |
| R-04 | `remove --force C` succeeds despite dirty tree | implemented passing | `remove_force_deletes_dirty_outpost` |
| R-05 | `remove --force C` succeeds despite unpushed commits | implemented passing | `remove_force_deletes_outpost_with_unpushed_commits` |
| R-06 | `remove` on path not in registry returns `RegistryEntryNotFound` | implemented passing | `remove_unregistered_path_returns_registry_entry_not_found` |
| R-07 | unlocked registered-but-missing path is deregistered, no rmtree | implemented passing | `remove_unlocked_missing_registered_path_deregisters_without_rmtree` |
| R-08 | registry entry pointing at unrelated directory returns `RegistryEntryNotManaged` and deletes nothing | implemented passing | `remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed` |
| R-09 | wrong-source registered outpost returns `RegistryEntryNotManaged` | implemented passing | `remove_wrong_source_outpost_returns_not_managed` |
| R-10 | locked C refused; force deletes and deregisters | implemented passing | `remove_refuses_locked_outpost_unless_forced` |
| R-11 | locked missing path returns `OutpostLocked`; force deregisters without rmtree | implemented passing | `remove_locked_missing_path_requires_force_then_deregisters` |
| Pr-01 | `prune` removes registry entries whose paths no longer exist | planned | `crates/core/tests/prune.rs` |
| Pr-02 | `prune` keeps existing valid outposts | planned | `crates/core/tests/prune.rs` |
| Pr-03 | `prune` does not delete real directories or source branches | planned | `crates/core/tests/prune.rs` |
| Pr-04 | `prune` reports removed entries in `PruneReport` | planned | `crates/core/tests/prune.rs` |
| Pr-05 | `prune` keeps unrelated existing dirs and wrong-source outposts registered | planned | `crates/core/tests/prune.rs` |
| Pr-06 | `prune --dry-run` makes no registry changes | planned | `crates/core/tests/prune.rs` |
| Pr-07 | missing `outpost.sourceRepo` target reported in `orphaned_source_missing` | planned | `crates/core/tests/prune.rs` |
| Pr-08 | `prune` leaves locked stale entries registered and reports them | planned | `crates/core/tests/prune.rs` |
| Pr-09 | `ops::prune::run` includes each pruned entry in `PruneReport.removed_entries` | planned | `crates/core/tests/prune.rs`; CLI `-v` formatting out of scope |

## QA/Test Plan Gate

- QA subagent: `019e382d-1005-7310-b57c-f10da10b735a`
- Artifact: `.agents-artifacts/qa/phase-2/test-plan.md`
- Summary: QA owns core integration tests in `crates/core/tests/lock_move_unlock.rs`, `crates/core/tests/remove.rs`, and `crates/core/tests/prune.rs`; developers own production code and narrow helper unit tests.
- Planned test files:
  - `crates/core/tests/lock_move_unlock.rs`: LMU-01..LMU-08
  - `crates/core/tests/remove.rs`: R-01..R-11
  - `crates/core/tests/prune.rs`: Pr-01..Pr-09
- Developer-owned helper tests:
  - `Outpost::unpushed_commits`
  - `safety::check_no_unpushed`
  - narrow pure helper behavior for registry/path/error handling only when needed
- Fixture helpers likely needed:
  - registry assertion helper for one or all entries
  - direct registry lock helper for remove/prune setup, after LMU covers lock behavior
  - wrong-source outpost setup using a second fixture
  - unrelated registered directory setup for R-08 and Pr-05
  - missing-path setup by manually removing outpost directories
  - config rewrite helpers for source-missing and wrong-source metadata scenarios
- Blocked tests:
  - R-03 and R-05 need `Outpost::unpushed_commits` and `safety::check_no_unpushed`, scoped to remove safety support
- QA risks:
  - avoid testing helper mutations instead of op behavior; use registry/config mutation only for setup of corrupt states
  - keep all Phase 2 tests in `outpost-core` and call ops directly; no CLI/global `-C` behavior
  - assert destructive operations stay inside fixture temp roots and preserve unrelated dirs/source branches where required
  - do not add registry file locking/concurrency tests
- Recommended QA first step: implement `lock_move_unlock.rs` for LMU-01..LMU-08 with `ops::lock`, `ops::unlock`, and `ops::move`.

## Active Chunk

- `remove-safety`
- Scope: implement `ops::remove` and minimal unpushed safety support; add QA-owned core integration coverage for R-01..R-11.
- Test IDs: R-01, R-02, R-03, R-04, R-05, R-06, R-07, R-08, R-09, R-10, R-11
- Out of scope: CLI contextual path omission, CLI formatting/dispatch/global `-C`, prune behavior, Phase 3+ status/sync behavior, registry file locking/concurrency.
- Status: approved; next chunk pending
- QA worker: `019e384b-ddb5-7fd2-96b5-a99e956f0a8c`; write scope `crates/core/tests/remove.rs`.

## Remaining Chunks

Chunk Planning Gate:

- Planner subagent: `019e3831-2f3c-7802-b288-3715d05b79d6`
- Artifact: `.agents-artifacts/qa/phase-2/chunk-plan.md`
- Verdict: `ready_with_cautions`
- Recommended chunks:
  - `lock-move-unlock`: `ops::lock`, `ops::unlock`, `ops::move`; LMU-01..LMU-08
  - `remove-safety`: `ops::remove` plus minimal unpushed safety helpers; R-01..R-11
  - `prune`: `ops::prune`; Pr-01..Pr-09
- Dependencies:
  - `lock-move-unlock` depends on Phase 1 registry, discovery, and safety foundations
  - `remove-safety` depends on `lock-move-unlock` lock semantics and registry setup
  - `prune` depends on `lock-move-unlock` lock semantics and Phase 1 registry load/save
- Docs needed: no product, architecture, roadmap, README, or CLI docs changes expected; add only concise developer-facing docs if implementation introduces stable invariants not already covered by architecture.
- Risks/cautions:
  - lock/move/unlock must reject unregistered or wrong-source paths before mutation
  - `move --force` bypasses lock/dirty guards only, not managed-outpost or destination validation
  - `remove --force` bypasses lock/dirty/unpushed guards only, not managed-outpost validation
  - prune classification order must be locked, missing, source-missing, then keep
  - no CLI/global `-C`, Phase 3+, Phase 4, registry locking, or concurrency behavior
- Required human decisions: none

Remaining chunk order:

- `prune`

## Completed Chunks

- `lock-move-unlock` implementation evidence recorded:
  - Files changed: `crates/core/src/ops/mod.rs`, `crates/core/src/ops/lock.rs`, `crates/core/src/ops/move.rs`, `crates/core/src/ops/unlock.rs`, `crates/core/src/outpost.rs`, `crates/core/tests/lock_move_unlock.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-2.md`, `.agents-artifacts/reviews/phase-2/lock-move-unlock/evidence-pack.md`, `.agents-artifacts/qa/phase-2/lock-move-unlock.md`
  - Test IDs advanced: LMU-01..LMU-08
  - Evidence pack: `.agents-artifacts/reviews/phase-2/lock-move-unlock/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-2/lock-move-unlock.md`
  - Unit tests added: none; behavior covered by core integration tests
  - Integration tests added: `lock_with_reason_marks_registry_entry_locked`, `unlock_clears_registry_lock_state_and_reason`, `move_updates_filesystem_and_registry_path`, `move_refuses_locked_outpost_without_force`, `move_force_moves_locked_outpost_and_preserves_lock`, `move_refuses_dirty_outpost_but_force_succeeds`, `move_refuses_non_empty_destination`, `lock_move_unlock_reject_unregistered_paths`, `lock_move_unlock_reject_wrong_source_registered_path`
  - Docs updated: none; existing product and architecture document lock/move/unlock contracts
  - Architecture deviations: none for claimed lock/move/unlock behavior
  - Implementation commit: `700689e phase-2: add lock move unlock`
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-2/lock-move-unlock/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-2/lock-move-unlock/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-2/lock-move-unlock/independent-review.md`
  - Review verdicts: scope `approved with nits`; normal `approved`; independent `approved with nits`
  - Required review changes: none
  - Adopted nits: progress log now records checkpoint commit `786473d`
  - Residual notes: no dedicated LMU integration test for moving into an existing empty destination; shared destination-safety coverage exists and reviewers did not require a change
  - Status: approved
- `remove-safety` implementation evidence recorded:
  - Files changed: `crates/core/src/ops/mod.rs`, `crates/core/src/ops/remove.rs`, `crates/core/src/outpost.rs`, `crates/core/src/safety.rs`, `crates/core/tests/remove.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-2.md`, `.agents-artifacts/reviews/phase-2/remove-safety/evidence-pack.md`, `.agents-artifacts/qa/phase-2/remove-safety.md`
  - Test IDs advanced: R-01..R-11
  - Evidence pack: `.agents-artifacts/reviews/phase-2/remove-safety/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-2/remove-safety.md`
  - Unit tests added: `outpost::tests::unpushed_commits_reports_local_commits_ahead_of_source`, `safety::tests::check_no_unpushed_reports_unpushed_commits`
  - Integration tests added: `remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry`, `remove_dirty_outpost_returns_dirty_tree_with_force_hint`, `remove_unpushed_outpost_returns_unpushed_commits`, `remove_force_deletes_dirty_outpost`, `remove_force_deletes_outpost_with_unpushed_commits`, `remove_unregistered_path_returns_registry_entry_not_found`, `remove_unlocked_missing_registered_path_deregisters_without_rmtree`, `remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed`, `remove_wrong_source_outpost_returns_not_managed`, `remove_refuses_locked_outpost_unless_forced`, `remove_locked_missing_path_requires_force_then_deregisters`
  - Docs updated: none; existing product and architecture document remove safety ordering and unpushed support
  - Architecture deviations: none for claimed remove behavior
  - Implementation commit: `9d0348c phase-2: add remove safety`
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-2/remove-safety/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-2/remove-safety/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-2/remove-safety/independent-review.md`
  - Review verdicts: scope `approved`; normal `approved`; independent `approved`
  - Required review changes: none
  - Status: approved

## Verification Log

- Phase 2 readiness baseline:
  - `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test, 0 doctests
- `lock-move-unlock` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core --test lock_move_unlock`: pass; 9 lock/move/unlock integration tests
  - `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `remove-safety` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core --test remove`: pass; 11 remove integration tests
  - `cargo test -p outpost-core`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 45 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 11 remove integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass

## Review Log

- `lock-move-unlock` Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-2/lock-move-unlock/scope-review.md`; nit was to record checkpoint commit `786473d`.
- `lock-move-unlock` Normal Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-2/lock-move-unlock/normal-review.md`; no required changes.
- `lock-move-unlock` Independent Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-2/lock-move-unlock/independent-review.md`; residual edge-case test note, no required changes.
- `remove-safety` Scope Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-2/remove-safety/scope-review.md`; no required changes.
- `remove-safety` Normal Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-2/remove-safety/normal-review.md`; no required changes.
- `remove-safety` Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-2/remove-safety/independent-review.md`; no required changes.

## Docs Log

- `lock-move-unlock`: no docs changes; stable lock/move/unlock contracts already covered by product behavior and architecture sections 5.9.9..5.9.11.
- `remove-safety`: no docs changes; stable remove contracts and unpushed safety helpers already covered by product behavior and architecture sections 5.5, 5.8, 5.9.12, and 11.10.

## Commit Log

- `88e1b09 phase-2: record readiness and plan`
- `700689e phase-2: add lock move unlock`
- `786473d phase-2: record lock move unlock checkpoint`
- `9d0348c phase-2: add remove safety`

## Protected-Path Exception Log

- none

## Open Risks / Questions

- `Outpost::unpushed_commits` and `safety::check_no_unpushed` were added only as required support for `ops::remove` R-03/R-05.
- CLI/global `-C`/contextual omission behavior for lock/unlock remains Phase 5.
- Registry file locking remains post-MVP and out of scope.

## Next Recommended Action

- Commit `remove-safety` implementation evidence, then run scope, normal, and independent reviews.
