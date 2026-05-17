# Phase 0 Progress

## Phase

- `phase_id`: `phase-0`
- Roadmap scope: Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture
- Test IDs: U-07, U-08, U-09, U-11, U-12
- Forbidden scope: any implementation outside Phase 0 unless required to make Phase 0 compile and explicitly justified here; Phase 1+ command behavior; unrelated documentation cleanup; unrelated refactors
- Protected paths: none

## Source Docs

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- Last observed repo revision: `a971f25 docs: use agents artifact directory`

## Current Snapshot

- Branch: `main`
- Initial `git status --short`: clean
- Repository files observed: documentation tree plus `AGENTS.md`, `.gitignore`, and `story-add-entry-branches.png`
- Cargo workspace status before Phase 0: no `Cargo.toml` present; this is expected Phase 0 scope
- Toolchain observed: `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
- Commit identity observed: `Wei Hu <whuae@connect.ust.hk>`

## Readiness Log

- Verdict: `ready with cautions`
- Auditor: native subagent `019e36e7-2c35-7f21-a8db-237512bce479`
- Phase reviewed: `phase-0`; roadmap scope "Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture"; test IDs U-07, U-08, U-09, U-11, U-12
- Evidence:
  - `git status --short` currently shows `?? .agents-artifacts/`
  - `git status --short --untracked-files=all` shows only `?? .agents-artifacts/progress/phase-0.md`
  - branch `main`; HEAD `a971f25 docs: use agents artifact directory`
  - `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
  - `cargo metadata --no-deps --format-version 1` fails before Phase 0 because no `Cargo.toml` exists; expected starting state
- Prerequisites: no prior roadmap phase prerequisite; missing workspace is intentionally Phase 0 scope
- Consistency: source docs pass; roadmap Phase 0 maps to architecture sections for the workspace, `OutpostError`, `GitInvoker`, refname newtypes, `Reporter`, fixture, and U-07/U-08/U-09/U-11/U-12
- Blocking issues: none
- Non-blocking cautions:
  - The auditor cannot independently reconstruct the coordinator's pre-artifact clean-state claim, but current tracked state shows no modified or deleted tracked files.
  - Ignored local artifacts exist outside Phase 0 work (`.claude/`, `.playwright-mcp/`, `docs/book/`, `story-add-entry-branches.png`); do not include them in commits.
- Required human decisions: none
- Recommended first chunk: minimal Cargo workspace and `outpost-core` crate skeleton with Phase 0 modules and fixture directory

## QA/Test Map

| ID | Scope | Status | Notes |
| --- | --- | --- | --- |
| U-07 | `error.rs` display strings | implemented passing | `display_strings_match_snapshot` in `crates/core/src/error.rs` |
| U-08 | `OutpostError::exit_code` mapping | implemented passing | `exit_code_maps_each_variant` in `crates/core/src/error.rs` |
| U-09 | `GitInvoker::run_check` preserves failed argv | pending | Unit test expected in `crates/core/src/git.rs` |
| U-11 | `GitInvoker::run_capture` handles `-` argv after `--` | pending | Unit test expected in `crates/core/src/git.rs` |
| U-12 | refname validation | pending | Unit test expected in `crates/core/src/refname.rs` |

QA/Test Plan Gate:

- QA subagent: `019e36ea-1838-7b42-9abc-436e652cac8a`
- Summary: Phase 0 has no QA-owned core integration behavior tests yet; roadmap IDs U-07/U-08/U-09/U-11/U-12 are developer-owned colocated unit tests. QA-relevant scope is shared A/B/C fixture scaffolding.
- Coverage:
  - U-07: `crates/core/src/error.rs::tests::display_strings_match_snapshot`, planned
  - U-08: `crates/core/src/error.rs::tests::exit_code_maps_each_variant`, planned
  - U-09: `crates/core/src/git.rs::tests::run_check_preserves_failed_argv`, planned
  - U-11: `crates/core/src/git.rs::tests::run_capture_accepts_dash_prefixed_arg_after_separator`, planned
  - U-12: `crates/core/src/refname.rs::tests::refname_validation_rejects_flag_injection_shapes`, planned
- Fixture changes needed: `crates/core/tests/common/mod.rs`, `crates/core/tests/common/fixture.rs`
- Tests to write before implementation: none
- Tests to write after API stabilizes: Phase 1 `add.rs` and `list.rs` integration tests once `ops::add` and `ops::list` exist
- Blocked tests: none
- QA risks:
  - Fixture helpers described by architecture include later API dependencies, especially `ops::add`; Phase 0 should avoid implementing Phase 1 behavior just to complete the fixture.
  - `GitInvoker` tests use real Git and must avoid user/global Git config leakage.

## Active Chunk

- `core-foundation` selected after Chunk Planning Gate
- Scope: minimal Cargo workspace/core crate skeleton plus `error.rs`, `reporter.rs`, and public exports
- Test IDs: U-07, U-08
- Status: implementation complete; evidence recorded; pending milestone commit and review
- Developer subagent: `019e36ee-8703-7de3-812a-beb5e1d25fdf`
- Coordinator supplement: added `.gitignore` `/target/` entry because Phase 0 introduces Cargo build output

## Remaining Chunks

- `git-and-ref-boundary`: implement `git.rs` and `refname.rs`; covers U-09, U-11, U-12
- `phase0-fixture-scaffold`: add A/B fixture scaffold and hermetic Git environment support without Phase 1 ops behavior

Chunk Planning Gate:

- Planner subagent: `019e36eb-ee16-7df0-9954-e17fd88c265e`
- Verdict: ready with cautions
- Recommended chunks:
  - `core-foundation`: workspace/core skeleton, `error.rs`, `reporter.rs`, U-07, U-08
  - `git-and-ref-boundary`: `git.rs`, `refname.rs`, U-09, U-11, U-12
  - `phase0-fixture-scaffold`: `crates/core/tests/common/*` fixture support, no direct Phase 0 test IDs
- Planner risks:
  - Keep fixture from drifting into Phase 1 `ops::add` behavior.
  - If a CLI package skeleton is needed for workspace shape, it must be compile-only and contain no Phase 5 command behavior.
  - Keep `GitInvoker` and refname code operation-agnostic and injection-safe.
  - Explicitly stage files to avoid ignored/unrelated local artifacts.
- Open questions: none
- Recommendation accepted: start with `core-foundation`

## Completed Chunks

- `core-foundation` implementation evidence recorded:
  - Files changed: `.gitignore`, `Cargo.toml`, `Cargo.lock`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/error.rs`, `crates/core/src/reporter.rs`
  - Test IDs advanced: U-07, U-08
  - Evidence pack: `.agents-artifacts/reviews/phase-0/core-foundation/evidence-pack.md`
  - Unit tests added: `display_strings_match_snapshot`, `exit_code_maps_each_variant`
  - Integration tests touched: none; QA-owned and not needed for these IDs
  - Docs updated: none; existing architecture already documents the stable contract
  - Architecture deviations: none; `PushIntoCheckedOutBranch` uses raw field identifier `r#source` internally to avoid `thiserror` source-field handling, while construction remains `source: ...`

## Verification Log

- Required for phase closeout:
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`
- Pre-readiness `cargo metadata --no-deps --format-version 1`: failed because no workspace exists yet; expected Phase 0 starting state
- `core-foundation` local verification:
  - `cargo test -p outpost-core`: pass; 2 unit tests passed, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 2 unit tests passed
  - `cargo test --workspace`: pass; 2 unit tests passed, 0 doctests

## Review Log

- `core-foundation`:
  - Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-0/core-foundation/scope-review.md`
  - Scope review nit adopted: evidence pack changed-file list now includes progress/evidence artifact paths

## Docs Log

- none

## Commit Log

- `7128367 phase-0: add core foundation`
  - Milestone: `core-foundation` implementation evidence recorded before review
  - Includes Phase 0 progress log, evidence pack, workspace/core skeleton, `error.rs`, `reporter.rs`, U-07/U-08 tests, and `.gitignore` `/target/`

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Non-blocking readiness cautions above must remain excluded from commits where unrelated.

## Next Recommended Action

- Commit `core-foundation` implementation milestone, then run Scope Reviewer.
