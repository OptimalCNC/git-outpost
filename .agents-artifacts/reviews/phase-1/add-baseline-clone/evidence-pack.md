# Evidence Pack: add-baseline-clone

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `add-baseline-clone`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.9.1 `ops/add.rs`, 10.2 fixture design, 11.2 C-01/C-02/C-10..C-12/C-14..C-16/C-20
- Roadmap test IDs advanced: C-01, C-02, C-10, C-11a, C-11b, C-11c, C-11d, C-12, C-14, C-15, C-16, C-20
- Roadmap test IDs supported for later chunks: C-03, C-04, C-05..C-09, C-13, C-17..C-19

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
  - Contains the `NewBranch` arm required by the public Phase 1 add API; branch-mode QA IDs are intentionally not claimed in this chunk and remain assigned to `add-branch-modes`.
- `tests/common/fixture.rs`
  - Adds `create_source_branch` for add integration tests.
- `tests/add.rs`
  - Adds QA-owned core integration coverage for the baseline add clone path and required clone/config/metadata/registry invariants.

## Tests Added / Updated

- Unit tests added:
  - `ops::add::tests::destination_parent_and_name_splits_bare_relative_path`
  - `ops::add::tests::destination_parent_and_name_splits_nested_relative_path`

## Integration Tests Added / Updated

- `add_without_branch_clones_current_branch_with_real_git_dir` covers C-01 and C-11b.
- `add_existing_branch_checks_out_branch_and_tracks_local_remote` covers C-02.
- `add_writes_outpost_metadata_keys` covers C-10.
- `add_configures_local_remote_and_non_shared_clone` covers C-11a, C-11c, and C-11d.
- `add_registers_outpost_path_in_source_registry` covers C-12.
- `add_sets_source_receive_deny_current_branch_update_instead` covers C-14.
- `add_reports_source_config_change` covers C-15.
- `add_clone_allows_user_file_protocol` covers C-16.
- `add_ignores_source_registry_directory_locally` covers C-20.

## Docs Added / Updated

- none
- Rationale: product and architecture already document the stable add behavior implemented here; no new user-facing behavior or invariant required a docs change.

## Verification

- `cargo test -p outpost-core --test add`: pass; 9 add integration tests
- `cargo fmt --check`: pass
- `cargo test -p outpost-core`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed baseline add behavior.
- C-03/C-04/C-05..C-09/C-13/C-17..C-19 remain assigned to later add chunks; this chunk does not claim branch-mode, custom-remote, missing-branch, destination-refusal, or unborn-HEAD completion.

## Residual Risks / Handoff Notes

- `ops::add::run` intentionally has no partial-add rollback, matching architecture section 5.9.1.
- The next chunk should add QA coverage for `AddCheckout::NewBranch`, custom `remote_name`, branch/destination refusal cases, and unborn source `HEAD`.
