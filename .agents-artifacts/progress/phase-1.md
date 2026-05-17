# Phase 1 Progress

## Phase

- `phase_id`: `phase-1`
- Roadmap scope: `source_repo.rs`, `outpost.rs`, `metadata.rs` (Raw+validated), `registry.rs` (incl. Drop guard), `safety.rs`, `ops::add` (incl. add ConfigChange `Reporter` event), `ops::list`
- Test IDs: U-01..U-06, U-10, U-13..U-15, C-01..C-20, L-01..L-10
- Progress log path: `.agents-artifacts/progress/phase-1.md`
- Review artifact root: `.agents-artifacts/reviews/phase-1/`
- QA artifact root: `.agents-artifacts/qa/phase-1/`
- Protected paths: none
- Protected exceptions: none
- Forbidden scope:
  - implementation outside Phase 1 unless required to make Phase 1 compile and explicitly justified here
  - Phase 2+ command behavior
  - CLI binary/e2e/global CLI behavior from Phase 5
  - unrelated documentation cleanup
  - unrelated refactors
- Required verification:
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`
- Invocation note: Phase 1 was started from the user's phase-by-phase instruction after Phase 0 closeout, using roadmap scope and coordinator defaults rather than a separate pasted Phase 1 invocation block.

## Source Docs

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- Last observed repo revision before Phase 1: `0778614 phase-0: close phase`

## Current Snapshot

- Branch: `main`
- Initial Phase 1 `git status --short`: clean
- Workspace: one member, `outpost-core`
- Existing implementation: Phase 0 modules only (`error.rs`, `git.rs`, `refname.rs`, `reporter.rs`) plus A/B fixture smoke test
- Missing Phase 1 files at start: `source_repo.rs`, `outpost.rs`, `metadata.rs`, `registry.rs`, `safety.rs`, `ops/add.rs`, `ops/list.rs`, Phase 1 integration tests
- Toolchain observed: `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
- Baseline verification before Phase 1 planning: `cargo test --workspace` passed with 10 unit tests, 1 fixture smoke test, 0 doctests

## Readiness Log

- Verdict: `ready with cautions`
- Auditor: native subagent `019e3745-04b5-7713-a6f0-0d851d607234`
- Phase reviewed: `phase-1`; roadmap scope `source_repo.rs`, `outpost.rs`, `metadata.rs` Raw+validated, `registry.rs` incl. Drop guard, `safety.rs`, `ops::add` incl. add `ConfigChange` `Reporter` event, `ops::list`; test IDs U-01..U-06, U-10, U-13..U-15, C-01..C-20, L-01..L-10
- Source documents reviewed:
  - `docs/src/roadmap.md` Phase table, especially Phase 1 row
  - `docs/src/product.md` Core Model, add behavior, list behavior, working-directory matrix
  - `docs/src/architecture.md` sections 5.4-5.9, 11.1-11.3, 12, 13
- Repo state evidence:
  - `git status --short --untracked-files=all` produced no output before Phase 1 artifacts
  - HEAD `0778614 phase-0: close phase`
  - `rg --files` showed only `outpost-core` crate plus docs/artifacts; no Phase 1 implementation files yet
  - `.agents-artifacts/progress` contained `phase-0.md` only; Phase 1 artifact roots did not exist yet
- Prerequisites checked:
  - Phase 0 progress log says all chunks complete, closeout gate passed, and closeout verification passed
  - Prior Phase 0 blocking review findings were fixed and rerun approvals recorded
  - C/outpost fixture helpers were intentionally deferred to Phase 1 because they require `ops::add`
- Toolchain evidence:
  - `cargo --version`: `cargo 1.94.0`
  - `rustc --version`: `rustc 1.94.0`
  - `git --version`: `git version 2.43.0`
  - `cargo metadata --no-deps --format-version 1` succeeds and reports workspace member `outpost-core`, Rust version `1.75`
- Spec/architecture/roadmap consistency: pass. Product requires outpost metadata, source registry, add setup, and list output; architecture maps those to the exact Phase 1 modules and tests; roadmap Phase 1 row matches the invocation assumption.
- Blocking issues: none
- Non-blocking cautions:
  - Phase 1 progress/review/QA artifact roots did not exist yet; created by coordinator after readiness.
  - Invocation was derived from the user's phase-by-phase instruction instead of a separately pasted Phase 1 block; auditor judged this non-blocking because the invocation assumption records exact phase id, artifact paths, forbidden scope, required verification, roadmap row, and test IDs.
  - CLI/global/e2e behavior remains Phase 5 scope.
- Recommended first chunk: storage foundations (`metadata.rs`, `registry.rs`) with unit tests U-01..U-06, U-14, U-15, plus minimal exports/dependencies.
- Required human decisions: none

## QA/Test Map

| ID | Scope | Status | Notes |
| --- | --- | --- | --- |
| U-01 | `registry.rs` empty registry JSON/round-trip | implemented passing | `empty_registry_serializes_to_expected_json_and_round_trips` |
| U-02 | `registry.rs` add/re-add/remove round-trip | implemented passing | `add_readd_remove_and_add_round_trips_by_canonical_path` |
| U-03 | `registry.rs` missing file loads empty | implemented passing | `load_missing_registry_returns_empty_registry` |
| U-04 | `registry.rs` malformed JSON returns `BadRegistry` | implemented passing | `load_malformed_json_returns_bad_registry` |
| U-05 | `metadata.rs` writes local outpost config keys | implemented passing | `metadata_write_sets_local_outpost_config_keys` |
| U-06 | `metadata.rs` unmanaged raw metadata promotes to `NotAnOutpost` | implemented passing | `raw_metadata_on_non_managed_repo_promotes_to_not_an_outpost` |
| U-10 | `safety.rs` dirty detection staged/unstaged/untracked | planned |  |
| U-13 | `safety.rs` managed outpost path gate rejects invalid/wrong-source paths | planned |  |
| U-14 | `metadata.rs` `RawMetadata::read` ignores global config | implemented passing | `raw_metadata_read_ignores_global_outpost_managed_config` |
| U-15 | `registry.rs` unsaved `RegistryMut` Drop guard | implemented passing | `dropping_dirty_registry_mut_trips_debug_drop_guard`; release behavior test is cfg-gated |
| C-01..C-20 | `ops::add` core integration behavior | planned | QA-owned integration tests |
| L-01..L-10 | `ops::list` core integration behavior | planned | QA-owned integration tests |

QA/Test Plan Gate:

- QA subagent: `019e3749-d36f-7ec1-8262-371386306758`
- Summary: no tests need to be written before implementation. Developers own Phase 1 colocated unit/function tests; QA owns core integration tests in `crates/core/tests/add.rs` and `crates/core/tests/list.rs` once APIs stabilize.
- Developer-owned unit tests:
  - `registry.rs`: U-01, U-02, U-03, U-04, U-15
  - `metadata.rs`: U-05, U-06, U-14
  - `safety.rs`: U-10, U-13
- QA-owned integration tests:
  - `crates/core/tests/add.rs`: C-01..C-20
  - `crates/core/tests/list.rs`: L-01..L-10
- Fixture changes needed:
  - `AbcFixture::source_repo()` using `SourceRepo::at_with(&self.source, &self.git_env)`
  - `AbcFixture::add_outpost(name)` using `ops::add::run`
  - `AbcFixture::dirty_outpost(name)` for L-04
  - `RecordingReporter` helper for C-15
  - small helpers for `commit_in_outpost`, source branch setup, canonical path assertions, and direct registry lock-state setup for L-10
- Fixture helpers still deferred: `outpost_with_unpushed(name)` until Phase 2 remove/push-style safety needs it
- Blocked tests:
  - C-11d and C-16 need public test-helper access to recorded `GitInvoker` argv from `SourceRepo`/`Outpost`
  - C-15 needs finalized add `Reporter` event shape
  - L-10 needs registry lock fields/write helpers, but must not call Phase 2 `ops::lock`
- QA risks:
  - Keep all Phase 1 QA tests in `outpost-core` and call ops directly; no Phase 5 CLI/e2e/global `-C` behavior
  - Assert only required argv elements/order for clone flags to avoid brittle Git argv tests
  - Inject registry lock state directly for L-10 to avoid Phase 2 behavior
  - Always use hermetic `_with` constructors from fixture-created repos
- Recommended QA first step: after `ops::add` signatures compile, add `RecordingReporter` plus C-01.

## Active Chunk

- `storage-foundations`
- Scope: `metadata.rs`, `registry.rs`, minimal storage dependencies/exports, and minimal `SourceRepo` storage carrier
- Test IDs: U-01, U-02, U-03, U-04, U-05, U-06, U-14, U-15
- Status: implementation complete; evidence recorded; pending milestone commit and review

## Remaining Chunks

Chunk Planning Gate:

- Planner subagent: `019e3749-d3f4-7340-afe6-6801af8c7cdf`
- Verdict: `ready with cautions`
- Recommended chunks:
  - `storage-foundations`: `metadata.rs`, `registry.rs`, minimal exports/dependencies; U-01..U-06, U-14, U-15
  - `source-outpost-discovery`: `source_repo.rs`, `outpost.rs`, minimal fixture support; supports C/L behavior; no command orchestration
  - `safety-gates`: `safety.rs`; U-10, U-13; supports add refusal tests
  - `add-baseline-clone`: `ops/mod.rs`, `ops/add.rs`, add integration tests for existing-branch clone path; C-01, C-02, C-10..C-12, C-14..C-16, C-20
  - `add-branch-modes`: `ops/add.rs` branch creation/custom remote/refusal paths; C-03..C-09, C-13, C-17..C-19
  - `list-basic-summaries`: `ops/list.rs`; L-01..L-04, L-07..L-10
  - `list-ahead-behind`: `Outpost::ahead_behind_source` support and `ops/list.rs`; L-05, L-06
- Dependencies:
  - `storage-foundations` blocks registry-backed `add` and `list`
  - `source-outpost-discovery` blocks safety, `add`, and `list`
  - `safety-gates` should land before full add refusal tests
  - `add-baseline-clone` depends on storage, discovery, and safety
  - `add-branch-modes` depends on add baseline
  - `list-basic-summaries` depends on storage/discovery/safety and can use real outposts after add baseline
  - `list-ahead-behind` depends on list basic and ahead/behind helper support
- Docs needed: no product, architecture, roadmap, README, or CLI docs changes expected; add only concise developer-facing docs if implementation introduces stable invariants not already covered by architecture.
- Risks/cautions:
  - `RawMetadata::read` must use `git config --local`
  - registry mutations must be explicitly saved
  - add prevalidation must happen before clone where specified
  - `git switch` uses validated branch names, not `--`
  - clone argv must include `-c protocol.file.allow=user` and `--no-shared`
  - no rollback/remove/move/prune/CLI/e2e behavior in Phase 1
- Required human decisions: none

Remaining chunk order:

- `storage-foundations`
- `source-outpost-discovery`
- `safety-gates`
- `add-baseline-clone`
- `add-branch-modes`
- `list-basic-summaries`
- `list-ahead-behind`

## Completed Chunks

- none; `storage-foundations` implementation is pending review

## Verification Log

- Baseline before Phase 1 planning:
  - `cargo test --workspace`: pass; 10 unit tests, 1 fixture smoke test, 0 doctests
- `storage-foundations` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 18 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 18 unit tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 18 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 18 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo metadata --format-version 1 --no-deps --offline`: pass
  - `cargo tree -p outpost-core --offline`: pass; active target storage dependency tree audited for Rust 1.75-compatible locked manifests
  - `cargo metadata --format-version 1`: failed under restricted network while trying to download target-specific crates; not required for the gate
  - `cargo tree -p outpost-core --target all --offline`: failed because uncached target-specific crates were unavailable offline; not required for the gate

## Review Log

- none

## Docs Log

- none

## Commit Log

- `3de288d phase-1: record readiness and plan`
  - Milestone: Phase 1 readiness, QA/Test Plan Gate, and Chunk Planning Gate recorded before implementation

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Phase 1 is broad; chunk plan should keep storage foundations, source/outpost/safety, add integration, and list integration reviewable.
- Keep CLI/e2e behavior out of Phase 1.

## Next Recommended Action

- Commit `storage-foundations` implementation evidence, then run the three-reviewer gate.
