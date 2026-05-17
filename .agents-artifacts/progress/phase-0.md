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
| U-09 | `GitInvoker::run_check` preserves failed argv | implemented passing | `run_check_bad_command_preserves_failed_argv` in `crates/core/src/git.rs` |
| U-11 | `GitInvoker::run_capture` handles `-` argv after `--` | implemented passing | `run_capture_keeps_leading_dash_value_positional_after_separator` in `crates/core/src/git.rs` |
| U-12 | refname validation | implemented passing | `branch_parse_rejects_leading_dash_and_accepts_feature_branch` and `remote_parse_rejects_shell_like_value` in `crates/core/src/refname.rs` |

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

- none
- Status: all Phase 0 chunks complete; phase closeout pending

## Remaining Chunks

- none for Phase 0 after `phase0-fixture-scaffold` review/closeout

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
  - Review artifacts: `.agents-artifacts/reviews/phase-0/core-foundation/scope-review.md`, `.agents-artifacts/reviews/phase-0/core-foundation/normal-review.md`, `.agents-artifacts/reviews/phase-0/core-foundation/independent-review.md`
  - Unit tests added: `display_strings_match_snapshot`, `exit_code_maps_each_variant`
  - Integration tests touched: none; QA-owned and not needed for these IDs
  - Docs updated: none; existing architecture already documents the stable contract
  - Architecture deviations: none; `PushIntoCheckedOutBranch` uses raw field identifier `r#source` internally to avoid `thiserror` source-field handling, while construction remains `source: ...`
- `git-and-ref-boundary` implementation evidence recorded:
  - Files changed: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/git.rs`, `crates/core/src/refname.rs`
  - Test IDs advanced: U-09, U-11, U-12
  - Evidence pack: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md`
  - Review artifacts: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/scope-review.md`, `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/normal-review.md`, `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/independent-review.md`, `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/normal-review-rerun.md`, `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/independent-review-rerun.md`
  - Unit tests added: `run_check_bad_command_preserves_failed_argv`, `run_capture_keeps_leading_dash_value_positional_after_separator`, `run_status_distinguishes_exit_one_from_real_failure`, `branch_parse_rejects_leading_dash_and_accepts_feature_branch`, `remote_parse_rejects_shell_like_value`, `ref_parse_uses_full_ref_validation`, `source_remote_ref_parses_remote_and_branch`, `upstream_short_branch_returns_only_heads_refs`
  - Integration tests touched: none; QA-owned and not needed for these IDs
  - Docs updated: none; existing architecture already documents the stable contract
  - Architecture deviations: none
  - Review fix applied: `GitFailed.args`/`GitTerminatedBySignal.args` now render argv as a bracketed per-argument list to preserve argument boundaries; U-09 test now distinguishes one arg containing spaces from multiple args
- `phase0-fixture-scaffold` implementation evidence recorded:
  - Files changed: `Cargo.toml`, `Cargo.lock`, `crates/core/Cargo.toml`, `crates/core/tests/common/mod.rs`, `crates/core/tests/common/fixture.rs`, `crates/core/tests/fixture_smoke.rs`
  - Test IDs advanced: none directly
  - Evidence pack: `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/evidence-pack.md`
  - Review artifacts: `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/scope-review.md`, `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/normal-review.md`, `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/independent-review.md`, `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/normal-review-rerun.md`, `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/independent-review-rerun.md`
  - Fixture changes: A/B temp fixture, hermetic Git env, `invoker`, `commit_in_source`, `commit_in_upstream`
  - Integration tests added: `abc_fixture_builds_a_b_with_hermetic_git_env`
  - Docs updated: none; existing architecture documents fixture intent
  - Architecture deviations: full C/outpost helpers deferred because they require Phase 1 APIs and command behavior
  - Review fix applied: `tempfile` pinned to `=3.10.0`; lockfile no longer contains `getrandom`/Rust 1.85 dependency chain; audited locked fixture dependencies are Rust 1.75-compatible
  - Status: complete; all reviewer reruns approved after MSRV fix

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
- `git-and-ref-boundary` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 10 unit tests passed, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 10 unit tests passed
  - `cargo test --workspace`: pass; 10 unit tests passed, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 10 unit tests passed, 0 doctests
- `git-and-ref-boundary` review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 10 unit tests passed, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 10 unit tests passed
  - `cargo test --workspace`: pass; 10 unit tests passed, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 10 unit tests passed, 0 doctests
- `phase0-fixture-scaffold` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 10 unit tests passed, 1 fixture smoke test passed, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 10 unit tests passed, 1 fixture smoke test passed
  - `cargo test --workspace`: pass; 10 unit tests passed, 1 fixture smoke test passed, 0 doctests
- `phase0-fixture-scaffold` review-fix verification:
  - `cargo metadata --format-version 1 --no-deps`: pass; `tempfile` requirement `=3.10.0`, workspace `rust_version` `1.75`
  - `cargo tree -p outpost-core --offline`: pass; dev dependency tree uses `tempfile v3.10.0`, `rustix v0.38.31`, `linux-raw-sys v0.4.13`
  - Local registry manifest audit: locked fixture dependencies checked have `rust-version <= 1.75`; `Cargo.lock` no longer contains `getrandom`, `wasip*`, `wit-*`, or `wasm*`
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 10 unit tests passed, 1 fixture smoke test passed, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 10 unit tests passed, 1 fixture smoke test passed
  - `cargo test --workspace`: pass; 10 unit tests passed, 1 fixture smoke test passed, 0 doctests

## Review Log

- `core-foundation`:
  - Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-0/core-foundation/scope-review.md`
  - Scope review nit adopted: evidence pack changed-file list now includes progress/evidence artifact paths
  - Normal Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/core-foundation/normal-review.md`
  - Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/core-foundation/independent-review.md`
- `git-and-ref-boundary`:
  - Scope Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/scope-review.md`
  - Normal Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/normal-review.md`
  - Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/independent-review.md`
  - Blocking finding fixed: U-09 failed argv preservation now keeps argv boundaries unambiguous and test distinguishes a single arg containing spaces from multiple args
  - Normal Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/normal-review-rerun.md`
  - Independent Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/independent-review-rerun.md`
- `phase0-fixture-scaffold`:
  - Scope Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/scope-review.md`
  - Normal Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/normal-review.md`
  - Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/independent-review.md`
  - Blocking finding fixed: `tempfile` pinned to `=3.10.0`; audited locked dependency tree is Rust 1.75-compatible and Rust 1.85 chain is absent
  - Normal Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/normal-review-rerun.md`
  - Independent Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/independent-review-rerun.md`

## Docs Log

- none

## Commit Log

- `7128367 phase-0: add core foundation`
  - Milestone: `core-foundation` implementation evidence recorded before review
  - Includes Phase 0 progress log, evidence pack, workspace/core skeleton, `error.rs`, `reporter.rs`, U-07/U-08 tests, and `.gitignore` `/target/`
- `3dafbfd phase-0: address core foundation scope review`
  - Milestone: adopted Scope Reviewer nit and recorded scope review artifact
- `85c6563 phase-0: record core foundation reviews`
  - Milestone: recorded Normal Reviewer and Independent Reviewer approvals for `core-foundation`
- `c144b69 phase-0: add git and ref boundary`
  - Milestone: `git-and-ref-boundary` implementation evidence recorded before review
  - Includes `git.rs`, `refname.rs`, exports, `test-helpers` feature, U-09/U-11/U-12 tests, and evidence/progress artifacts
- `83a1f74 phase-0: address git argv review finding`
  - Milestone: fixed Normal Reviewer blocking finding for `git-and-ref-boundary` U-09 argv boundary preservation and recorded review artifacts
- `9f2fc35 phase-0: record git boundary review approvals`
  - Milestone: recorded post-fix Normal Reviewer and Independent Reviewer approvals for `git-and-ref-boundary`
- `2361d90 phase-0: add fixture scaffold`
  - Milestone: `phase0-fixture-scaffold` implementation evidence recorded before review
  - Includes A/B fixture scaffold, hermetic Git env, smoke integration test, `tempfile` dev-dependency, and evidence/progress artifacts
- `383a2e8 phase-0: address fixture msrv review finding`
  - Milestone: fixed Normal Reviewer blocking finding for `phase0-fixture-scaffold` MSRV-compatible fixture dependency resolution and recorded review artifacts

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Non-blocking readiness cautions above must remain excluded from commits where unrelated.

## Next Recommended Action

- Commit `phase0-fixture-scaffold` review approval artifacts, then run and record the Phase 0 closeout gate.
