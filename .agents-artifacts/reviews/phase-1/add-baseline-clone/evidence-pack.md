# Evidence Pack: add-baseline-clone

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `add-baseline-clone`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.9.1 `ops/add.rs`, 10.2 fixture design, 11.2 C-01..C-20
- Roadmap test IDs advanced: C-01..C-20
- Scope update: scope review found that the public `AddCheckout::NewBranch` execution path exceeded the original baseline-only evidence claim. The review-fix response expands this chunk's evidence and QA coverage to the full Phase 1 add surface (C-01..C-20), which remains inside Phase 1 and avoids temporary unimplemented public operation behavior.

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- `crates/core/src/lib.rs`
- `crates/core/src/source_repo.rs`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/add.rs`
- `crates/core/tests/add.rs`
- `crates/core/tests/common/fixture.rs`

## Moves / Renames

- none

## Diff Summary

- `lib.rs`
  - Exports the new `ops` module.
- `source_repo.rs`
  - Adds crate-private `SourceRepo::git()` so operation code uses the source repo's hermetic `GitInvoker`; this also lets integration tests inspect recorded clone argv through existing test-helper APIs.
- `ops/mod.rs`
  - Adds the `add` operation module.
- `ops/add.rs`
  - Adds the Phase 1 `AddCheckout`, `AddOptions`, and `run` API shape from architecture section 5.9.1.
  - Validates destination cleanliness through `safety::check_destination_clean` before cloning.
  - Resolves omitted target branches from `SourceRepo::current_branch()` and requires target branches to exist before cloning.
  - Runs `git -c protocol.file.allow=user clone --no-shared -- <source.work_tree> <destination>`.
  - Renames the clone's default remote to `remote_name` when needed, checks out the target branch, writes outpost metadata, emits the required `ConfigChange` reporter event, configures source `receive.denyCurrentBranch=updateInstead`, writes the registry entry, and returns `source.outpost_at(destination)`.
  - Splits destination parent/name for safety checks so nested relative paths are not double-prefixed.
  - Implements `NewBranch` by creating the source branch from the resolved target without switching the source checkout, fetching it into the outpost, and checking it out with tracking.
  - Maps destination refusal errors back to the caller-supplied destination path while keeping parent-relative safety resolution.
  - Treats an omitted target on an unborn source `HEAD` as `BranchNotFound { branch: "HEAD" }` before cloning.
- `tests/common/fixture.rs`
  - Adds `create_source_branch` for add integration tests.
- `tests/add.rs`
  - Adds QA-owned core integration coverage for all Phase 1 add IDs C-01..C-20.

## Tests Added / Updated

- Unit tests added:
  - `ops::add::tests::destination_parent_and_name_splits_bare_relative_path`
  - `ops::add::tests::destination_parent_and_name_splits_nested_relative_path`

## Integration Tests Added / Updated

- `add_without_branch_clones_current_branch_with_real_git_dir` covers C-01 and C-11b.
- `add_existing_branch_checks_out_branch_and_tracks_local_remote` covers C-02.
- `add_new_branch_from_target_creates_source_branch_and_tracks_it` covers C-03.
- `add_new_branch_without_target_uses_source_current_branch` covers C-04.
- `add_rejects_existing_non_empty_directory` covers C-05.
- `add_rejects_existing_file` covers C-06.
- `add_outside_git_repo_cannot_discover_source` covers C-07.
- `add_rejects_destination_inside_existing_repo` covers C-08.
- `add_rejects_missing_existing_branch_before_clone` covers C-09.
- `add_writes_outpost_metadata_keys` covers C-10.
- `add_configures_local_remote_and_non_shared_clone` covers C-11a, C-11c, and C-11d.
- `add_registers_outpost_path_in_source_registry` covers C-12.
- `add_custom_remote_name_replaces_origin_and_updates_metadata` covers C-13.
- `add_sets_source_receive_deny_current_branch_update_instead` covers C-14.
- `add_reports_source_config_change` covers C-15.
- `add_clone_allows_user_file_protocol` covers C-16.
- `add_rejects_unborn_source_head_before_clone` covers C-17.
- `add_new_branch_rejects_missing_target_before_clone` covers C-18.
- `add_new_branch_does_not_switch_source_checkout` covers C-19.
- `add_ignores_source_registry_directory_locally` covers C-20.

## Docs Added / Updated

- none
- Rationale: product and architecture already document the stable add behavior implemented here; no new user-facing behavior or invariant required a docs change.

## Verification

- Initial implementation verification:
  - `cargo test -p outpost-core --test add`: pass; 9 add integration tests
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass
- Scope-review fix verification:
  - `cargo test -p outpost-core --test add`: pass; 20 add integration tests
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test
  - `cargo test --workspace`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 20 add integration tests, 1 fixture smoke test, 0 doctests
  - `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none; the scope-review fix expands evidence and QA coverage to the full Phase 1 add operation surface.

## Residual Risks / Handoff Notes

- `ops::add::run` intentionally has no partial-add rollback, matching architecture section 5.9.1.
- The previously planned `add-branch-modes` chunk has been absorbed into this review-fix path. Next remaining Phase 1 work is list behavior.
