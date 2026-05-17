# Evidence Pack: source-outpost-discovery

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `source-outpost-discovery`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.4 `SourceRepo`, 5.5 `Outpost`, 10.2 hermetic fixture env, 11.1 unit tests, 11.2/11.3 integration support
- Roadmap test IDs directly advanced: none
- Roadmap test IDs supported: U-10, U-13, C-01..C-20, L-01..L-10

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `crates/core/src/lib.rs`
- `crates/core/src/outpost.rs`
- `crates/core/src/source_repo.rs`
- `crates/core/tests/common/fixture.rs`
- `crates/core/tests/fixture_smoke.rs`

## Moves / Renames

- none

## Diff Summary

- `lib.rs`
  - Exports the new `outpost` module plus `Outpost` and `AheadBehind`.
- `source_repo.rs`
  - Expands `SourceRepo` from the storage carrier into a Git-backed repository handle with canonicalized `work_tree`, `git_dir`, `git_common_dir`, internal `GitInvoker`, and remembered hermetic env.
  - Adds `discover`, `discover_with`, `at`, and `at_with` constructors using `git rev-parse --show-toplevel`, `--git-dir`, and `--git-common-dir`.
  - Adds accessors, env-threaded `outpost_at`, test-helper `test_invoker`, current-branch, dirty-tree, local branch existence, upstream config, and worktree checked-out branch helpers.
  - Preserves the test-only `from_storage_paths` constructor needed by registry unit tests.
  - Does not add Phase 4 `fast_forward_branch_from_origin` behavior.
- `outpost.rs`
  - Adds `Outpost` with Git-backed discovery/opening, metadata validation via `RawMetadata::read` and `Metadata::from_raw`, canonicalized paths, remembered env, `source_repo`, branch/dirty helpers, upstream tracking, and test-helper `test_invoker`.
  - Adds the `AheadBehind` data type only; ahead/behind behavior is intentionally deferred to the planned `list-ahead-behind` chunk.
- `tests/common/fixture.rs`
  - Adds `AbcFixture::source_repo()` using `SourceRepo::at_with(&self.source, &self.git_env)` so later integration tests inherit the hermetic Git env.
- `tests/fixture_smoke.rs`
  - Verifies the new fixture helper opens the source repo at its canonical work tree.

## Tests Added / Updated

- Unit tests added:
  - `source_at_canonicalizes_paths_and_reads_current_branch`
  - `source_discover_rejects_non_repo`
  - `source_dirty_detects_untracked_files`
  - `source_branch_helpers_read_local_heads_upstream_and_worktrees`
  - `outpost_at_rejects_unmanaged_repo`
  - `outpost_at_reads_metadata_and_source_repo`
  - `outpost_reports_missing_source_repo_from_metadata`
- Integration tests updated:
  - `abc_fixture_builds_a_b_with_hermetic_git_env` now covers `AbcFixture::source_repo`.

## Docs Added / Updated

- none
- Rationale: the stable public contracts for `SourceRepo`, `Outpost`, and fixture env threading are already described in `docs/src/architecture.md`; this chunk implements that contract without adding a new stable concept.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 28 unit tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 28 unit tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- none
- Note: `Outpost::ahead_behind_source` and `Outpost::unpushed_commits` remain deferred to the already planned `list-ahead-behind` chunk to avoid implementing that behavior before its reviewable slice.

## Residual Risks / Handoff Notes

- `SourceRepo::branch_exists` checks only `refs/heads/<branch>`, matching add/list needs for local source branches.
- `checked_out_branches` ignores detached worktrees because there is no local branch to protect in that case.
- Reviewers should inspect env threading through `SourceRepo::outpost_at`, `Outpost::source_repo`, and `AbcFixture::source_repo`.
