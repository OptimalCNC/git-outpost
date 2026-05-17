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
| U-10 | `safety.rs` dirty detection staged/unstaged/untracked | implemented passing | `check_clean_reports_staged_changes_as_dirty`, `check_clean_reports_unstaged_changes_as_dirty`, `check_clean_reports_untracked_changes_as_dirty` |
| U-13 | `safety.rs` managed outpost path gate rejects invalid/wrong-source paths | implemented passing | `managed_outpost_gate_rejects_path_with_no_git_repo`, `managed_outpost_gate_rejects_managed_false`, `managed_outpost_gate_rejects_different_source` |
| U-14 | `metadata.rs` `RawMetadata::read` ignores global config | implemented passing | `raw_metadata_read_ignores_global_outpost_managed_config` |
| U-15 | `registry.rs` unsaved `RegistryMut` Drop guard | implemented passing | `dropping_dirty_registry_mut_trips_debug_drop_guard`; release behavior test is cfg-gated |
| C-01 | `ops::add` current-branch clone | implemented passing | `add_without_branch_clones_current_branch_with_real_git_dir` |
| C-02 | `ops::add` existing branch checkout/tracking | implemented passing | `add_existing_branch_checks_out_branch_and_tracks_local_remote` |
| C-03 | `ops::add -b <new> <path> <target>` branch creation | implemented passing | `add_new_branch_from_target_creates_source_branch_and_tracks_it` |
| C-04 | `ops::add -b <new> <path>` uses source current branch | implemented passing | `add_new_branch_without_target_uses_source_current_branch` |
| C-05 | `ops::add` rejects non-empty destination directory | implemented passing | `add_rejects_existing_non_empty_directory` |
| C-06 | `ops::add` rejects existing destination file | implemented passing | `add_rejects_existing_file` |
| C-07 | `ops::add` source discovery outside repo returns `NotARepo` | implemented passing | `add_outside_git_repo_cannot_discover_source` |
| C-08 | `ops::add` rejects destination inside existing repo | implemented passing | `add_rejects_destination_inside_existing_repo` |
| C-09 | `ops::add` rejects missing existing branch before clone | implemented passing | `add_rejects_missing_existing_branch_before_clone` |
| C-10 | `ops::add` outpost metadata config | implemented passing | `add_writes_outpost_metadata_keys` |
| C-11a..C-11d | `ops::add` remote/real-git-dir/no-shared clone invariants | implemented passing | `add_configures_local_remote_and_non_shared_clone`; C-11b also covered by C-01 test |
| C-12 | `ops::add` source registry entry | implemented passing | `add_registers_outpost_path_in_source_registry` |
| C-13 | `ops::add` custom remote name | implemented passing | `add_custom_remote_name_replaces_origin_and_updates_metadata` |
| C-14 | `ops::add` source `receive.denyCurrentBranch` config | implemented passing | `add_sets_source_receive_deny_current_branch_update_instead` |
| C-15 | `ops::add` config change reporter event | implemented passing | `add_reports_source_config_change` |
| C-16 | `ops::add` file protocol clone override | implemented passing | `add_clone_allows_user_file_protocol` |
| C-17 | `ops::add` rejects unborn source `HEAD` before clone | implemented passing | `add_rejects_unborn_source_head_before_clone` |
| C-18 | `ops::add -b` rejects missing target before clone | implemented passing | `add_new_branch_rejects_missing_target_before_clone` |
| C-19 | `ops::add -b` leaves source checkout unchanged | implemented passing | `add_new_branch_does_not_switch_source_checkout` |
| C-20 | `ops::add` local `.outpost/` exclude | implemented passing | `add_ignores_source_registry_directory_locally` |
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

- `add-baseline-clone`
- Scope: add `ops::add` for Phase 1, including baseline clone path, branch creation mode, custom remote, destination/branch refusal behavior, registry/metadata setup, local safety prechecks, reporter config event, and QA-owned core integration coverage for C-01..C-20
- Test IDs: C-01..C-20
- Status: normal-review fix implemented; rerun review pending
- Scope update note: initial Scope Reviewer found `AddCheckout::NewBranch` execution exceeded the baseline-only chunk claim. Coordinator resolved this by expanding the current add chunk evidence and QA coverage to all C-01..C-20 instead of leaving a temporary unimplemented public enum arm.

## Remaining Chunks

Chunk Planning Gate:

- Planner subagent: `019e3749-d3f4-7340-afe6-6801af8c7cdf`
- Verdict: `ready with cautions`
- Recommended chunks:
  - `storage-foundations`: `metadata.rs`, `registry.rs`, minimal exports/dependencies; U-01..U-06, U-14, U-15
  - `source-outpost-discovery`: `source_repo.rs`, `outpost.rs`, minimal fixture support; supports C/L behavior; no command orchestration
  - `safety-gates`: `safety.rs`; U-10, U-13; supports add refusal tests
  - `add-baseline-clone`: `ops/mod.rs`, `ops/add.rs`, add integration tests for existing-branch clone path; C-01, C-02, C-10..C-12, C-14..C-16, C-20
  - `add-branch-modes`: absorbed into `add-baseline-clone` scope-review fix; C-03..C-09, C-13, C-17..C-19
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

- `list-basic-summaries`
- `list-ahead-behind`

## Completed Chunks

- `storage-foundations` implementation evidence recorded:
  - Files changed: `Cargo.toml`, `Cargo.lock`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/metadata.rs`, `crates/core/src/registry.rs`, `crates/core/src/source_repo.rs`
  - Test IDs advanced: U-01, U-02, U-03, U-04, U-05, U-06, U-14, U-15
  - Evidence pack: `.agents-artifacts/reviews/phase-1/storage-foundations/evidence-pack.md`
  - Review artifacts: `.agents-artifacts/reviews/phase-1/storage-foundations/scope-review.md`, `.agents-artifacts/reviews/phase-1/storage-foundations/normal-review.md`, `.agents-artifacts/reviews/phase-1/storage-foundations/independent-review.md`, `.agents-artifacts/reviews/phase-1/storage-foundations/normal-review-rerun.md`, `.agents-artifacts/reviews/phase-1/storage-foundations/independent-review-rerun.md`
  - Unit tests added: `metadata_write_sets_local_outpost_config_keys`, `raw_metadata_on_non_managed_repo_promotes_to_not_an_outpost`, `raw_metadata_read_ignores_global_outpost_managed_config`, `empty_registry_serializes_to_expected_json_and_round_trips`, `add_readd_remove_and_add_round_trips_by_canonical_path`, `load_missing_registry_returns_empty_registry`, `load_malformed_json_returns_bad_registry`, `dropping_dirty_registry_mut_trips_debug_drop_guard`, `dropping_dirty_registry_mut_does_not_panic_in_release_builds`, `failed_save_returns_error_without_drop_guard_panic`, `update_path_handles_registered_old_path_after_rename`, `remove_by_path_handles_registered_missing_path`
  - Integration tests touched: none; QA-owned and scheduled after APIs stabilize
  - Docs updated: none; existing architecture documents storage contracts
  - Architecture deviations: minimal `SourceRepo` storage carrier added because registry APIs require it; full discovery remains deferred
  - Status: complete; post-fix normal and independent reviewers approved
- `source-outpost-discovery` implementation evidence recorded:
  - Files changed: `Cargo.lock`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/source_repo.rs`, `crates/core/src/outpost.rs`, `crates/core/tests/common/fixture.rs`, `crates/core/tests/fixture_smoke.rs`
  - Test IDs advanced: none directly; supports U-10, U-13, C-01..C-20, L-01..L-10
  - Evidence pack: `.agents-artifacts/reviews/phase-1/source-outpost-discovery/evidence-pack.md`
  - Unit tests added: `source_at_canonicalizes_paths_and_reads_current_branch`, `source_discover_rejects_non_repo`, `source_dirty_detects_untracked_files`, `source_branch_helpers_read_local_heads_upstream_and_worktrees`, `outpost_at_rejects_unmanaged_repo`, `outpost_at_reads_metadata_and_source_repo`, `outpost_reports_missing_source_repo_from_metadata`
  - Integration tests touched: `abc_fixture_builds_a_b_with_hermetic_git_env` now verifies `AbcFixture::source_repo` and normal integration-test access to `SourceRepo::test_invoker().argv_log()`
  - Docs updated: none; existing architecture documents the stable `SourceRepo`, `Outpost`, and fixture env-threading contracts
  - Architecture deviations: none; ahead/behind behavior remains deferred to the planned `list-ahead-behind` chunk
  - Review-fix delta: added documented `outpost-core` self dev-dependency with `test-helpers` for integration tests; updated lockfile for that self edge
  - Status: complete; post-fix scope, normal, and independent reviewers approved
- `safety-gates` implementation evidence recorded:
  - Files changed: `crates/core/src/lib.rs`, `crates/core/src/safety.rs`
  - Test IDs advanced: U-10, U-13
  - Evidence pack: `.agents-artifacts/reviews/phase-1/safety-gates/evidence-pack.md`
  - Unit tests added: `check_clean_reports_staged_changes_as_dirty`, `check_clean_reports_unstaged_changes_as_dirty`, `check_clean_reports_untracked_changes_as_dirty`, `check_clean_allows_clean_work_tree`, `managed_outpost_gate_rejects_path_with_no_git_repo`, `managed_outpost_gate_rejects_managed_false`, `managed_outpost_gate_rejects_different_source`, `managed_outpost_gate_accepts_matching_source`, `destination_clean_rejects_existing_file_and_non_empty_dir`, `destination_clean_allows_missing_and_empty_dir_outside_repo`, `destination_clean_rejects_target_inside_existing_repo`, `destination_clean_allows_relative_sibling_outside_repo`, `destination_clean_resolves_relative_path_under_parent_before_exists_check`
  - Integration tests touched: none; QA-owned and scheduled after ops APIs stabilize
  - Docs updated: none; existing architecture documents safety contracts
  - Architecture deviations: `check_no_unpushed` and divergence helpers remain deferred because this chunk is scoped to U-10/U-13 and destination gating
  - Review-fix delta: relative destination resolution now anchors under canonicalized `parent` before existence/canonicalization checks
  - Status: complete; post-fix scope, normal, and independent reviewers approved
- `add-baseline-clone` implementation evidence recorded:
  - Files changed: `crates/core/src/lib.rs`, `crates/core/src/source_repo.rs`, `crates/core/src/ops/mod.rs`, `crates/core/src/ops/add.rs`, `crates/core/tests/add.rs`, `crates/core/tests/common/fixture.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-1.md`, `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`, `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
  - Test IDs advanced: C-01..C-20
  - Evidence pack: `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
  - Unit tests added: `ops::add::tests::destination_parent_and_name_splits_bare_relative_path`, `ops::add::tests::destination_parent_and_name_splits_nested_relative_path`
  - Integration tests added: `add_without_branch_clones_current_branch_with_real_git_dir`, `add_existing_branch_checks_out_branch_and_tracks_local_remote`, `add_new_branch_from_target_creates_source_branch_and_tracks_it`, `add_new_branch_without_target_uses_source_current_branch`, `add_rejects_existing_non_empty_directory`, `add_rejects_existing_file`, `add_outside_git_repo_cannot_discover_source`, `add_rejects_destination_inside_existing_repo`, `add_rejects_missing_existing_branch_before_clone`, `add_writes_outpost_metadata_keys`, `add_configures_local_remote_and_non_shared_clone`, `add_custom_remote_name_replaces_origin_and_updates_metadata`, `add_registers_outpost_path_in_source_registry`, `add_sets_source_receive_deny_current_branch_update_instead`, `add_reports_source_config_change`, `add_rejects_unborn_source_head_before_clone`, `add_new_branch_rejects_missing_target_before_clone`, `add_new_branch_does_not_switch_source_checkout`, `add_clone_allows_user_file_protocol`, `add_ignores_source_registry_directory_locally`
  - Docs updated: none; existing product and architecture document add contracts
  - Architecture deviations: none
  - Review-fix delta: scope-review fix expanded add evidence and QA coverage to C-01..C-20; `NewBranch` now has direct integration coverage, tracking setup, missing-target preclone rejection, and source-checkout-preservation coverage
  - Normal-review fix delta: add resolves the caller destination once before validation and uses that effective path for clone, metadata, registry, and return; added regressions for relative source-internal `C` refusal and relative sibling `../C` success
  - Status: normal-review fix implemented; awaiting rerun review

## Verification Log

- Baseline before Phase 1 planning:
  - `cargo test --workspace`: pass; 10 unit tests, 1 fixture smoke test, 0 doctests
- `storage-foundations` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 21 unit tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core registry::tests::`: pass; 8 registry unit tests
  - `cargo metadata --format-version 1 --no-deps --offline`: pass
  - `cargo tree -p outpost-core --offline`: pass; active target storage dependency tree audited for Rust 1.75-compatible locked manifests
  - `cargo metadata --format-version 1`: failed under restricted network while trying to download target-specific crates; not required for the gate
  - `cargo tree -p outpost-core --target all --offline`: failed because uncached target-specific crates were unavailable offline; not required for the gate
- `source-outpost-discovery` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 28 unit tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `source-outpost-discovery` review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core --tests`: pass; 28 unit tests, 1 fixture smoke test
  - `cargo test -p outpost-core`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test --workspace`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `safety-gates` local verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core safety::tests::`: pass; 12 safety tests
  - `cargo test -p outpost-core`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 40 unit tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `safety-gates` review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core safety::tests::`: pass; 13 safety tests
  - `cargo test -p outpost-core`: pass; 41 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 41 unit tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 41 unit tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 41 unit tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `add-baseline-clone` local verification:
  - `cargo test -p outpost-core --test add`: pass; 9 add integration tests
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `add-baseline-clone` scope-review-fix verification:
  - `cargo test -p outpost-core --test add`: pass; 20 add integration tests
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- `add-baseline-clone` normal-review-fix verification:
  - `cargo test -p outpost-core --test add`: pass; 22 add integration tests
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 43 unit tests, 22 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 22 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 22 add integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass

## Review Log

- `storage-foundations`:
  - Scope Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-1/storage-foundations/scope-review.md`
  - Normal Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-1/storage-foundations/normal-review.md`
  - Independent Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-1/storage-foundations/independent-review.md`
  - Blocking findings fixed locally: `RegistryMut::save()` failure no longer trips Drop guard; `update_path()` and `remove_by_path()` handle already-recorded canonical paths after the filesystem path is missing.
  - Normal Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/storage-foundations/normal-review-rerun.md`
  - Independent Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/storage-foundations/independent-review-rerun.md`
- `source-outpost-discovery`:
  - Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/scope-review.md`
  - Normal Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/normal-review.md`
  - Independent Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/independent-review.md`
  - Blocking finding to fix: integration tests must be able to call `SourceRepo::test_invoker()` / `Outpost::test_invoker()` under `cargo test -p outpost-core --tests`, not only under an explicit `--features test-helpers` command.
  - Scope Reviewer rerun: `approved with nits`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/scope-review-rerun.md`
  - Normal Reviewer rerun: `approved with nits`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/normal-review-rerun.md`
  - Independent Reviewer rerun: `approved with nits`; artifact `.agents-artifacts/reviews/phase-1/source-outpost-discovery/independent-review-rerun.md`
- `safety-gates`:
  - Scope Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/scope-review.md`
  - Normal Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/normal-review.md`
  - Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/independent-review.md`
  - Blocking finding to fix: `check_destination_clean(parent, relative_dest)` must resolve relative destinations under `parent` before any existence/canonicalization check, with regression coverage.
  - Scope Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/scope-review-rerun.md`
  - Normal Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/normal-review-rerun.md`
  - Independent Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/safety-gates/independent-review-rerun.md`
- `add-baseline-clone`:
  - Scope Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-1/add-baseline-clone/scope-review.md`
  - Blocking finding fixed locally: `AddCheckout::NewBranch` behavior exceeded the baseline-only chunk claim; evidence and QA coverage now explicitly include all C-01..C-20 add behavior, with direct tests for branch modes, custom remote, refusal paths, and unborn `HEAD`.
  - Scope Reviewer rerun: `approved`; artifact `.agents-artifacts/reviews/phase-1/add-baseline-clone/scope-review-rerun.md`
  - Normal Reviewer: `needs changes`; artifact `.agents-artifacts/reviews/phase-1/add-baseline-clone/normal-review.md`
  - Blocking finding fixed locally: relative add destinations were validated/cloned/opened relative to different directories; add now resolves one effective destination path up front and regression tests cover source-internal and sibling relative destinations.
  - Normal Reviewer rerun: pending
  - Independent Reviewer: `approved`; artifact `.agents-artifacts/reviews/phase-1/add-baseline-clone/independent-review.md`

## Docs Log

- `source-outpost-discovery`: no docs changes; stable contracts already covered by architecture sections 5.4, 5.5, and 10.2.
- `safety-gates`: no docs changes; stable contracts already covered by architecture section 5.8.
- `add-baseline-clone`: no docs changes; stable add contracts already covered by product `add` behavior and architecture section 5.9.1.

## Commit Log

- `3de288d phase-1: record readiness and plan`
  - Milestone: Phase 1 readiness, QA/Test Plan Gate, and Chunk Planning Gate recorded before implementation
- `e80bd1e phase-1: add storage foundations`
  - Milestone: `storage-foundations` implementation evidence recorded before review
- `98591c6 phase-1: address storage review findings`
  - Milestone: fixed Normal and Independent Reviewer findings for `storage-foundations`
- `fd66377 phase-1: add source and outpost discovery`
  - Milestone: `source-outpost-discovery` implementation evidence recorded before review
- `bad1609 phase-1: address discovery review finding`
  - Milestone: fixed Normal Reviewer finding for `source-outpost-discovery`
- `6e38079 phase-1: record discovery review approvals`
  - Milestone: `source-outpost-discovery` reviewers approved after rerun
- `85119de phase-1: add safety gates`
  - Milestone: `safety-gates` implementation evidence recorded before review
- `7045f59 phase-1: address safety review finding`
  - Milestone: fixed Normal Reviewer finding for `safety-gates`
- `e7d7a47 phase-1: record safety review approvals`
  - Milestone: `safety-gates` reviewers approved after rerun
- `8b894d9 phase-1: add baseline add clone`
  - Milestone: `add-baseline-clone` implementation evidence recorded before review
- `2d377aa phase-1: record add baseline checkpoint`
  - Milestone: recorded `add-baseline-clone` implementation checkpoint hash before review
- `1227687 phase-1: address add scope review`
  - Milestone: fixed Scope Reviewer finding by expanding add coverage to C-01..C-20
- `7691332 phase-1: record add scope approval`
  - Milestone: recorded `add-baseline-clone` scope rerun approval
- pending `phase-1: address add normal review`
  - Milestone: fixed Normal Reviewer relative destination finding

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Phase 1 is broad; chunk plan should keep storage foundations, source/outpost/safety, add integration, and list integration reviewable.
- Keep CLI/e2e behavior out of Phase 1.

## Next Recommended Action

- Commit `add-baseline-clone` normal-review fix, then rerun normal review.
