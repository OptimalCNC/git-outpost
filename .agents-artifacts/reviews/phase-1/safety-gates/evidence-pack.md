# Evidence Pack: safety-gates

## Phase And Chunk

- Phase: `phase-1`
- Chunk: `safety-gates`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant architecture sections: 5.8 `safety.rs`, 11.1 U-10/U-13, add/move/remove safety references in 5.9
- Roadmap test IDs advanced: U-10, U-13
- Roadmap test IDs supported: C-05, C-06, C-08, remove/move safety behavior in later phases

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/safety-gates/evidence-pack.md`
- `crates/core/src/lib.rs`
- `crates/core/src/safety.rs`

## Moves / Renames

- none

## Diff Summary

- `lib.rs`
  - Exports the new `safety` module.
- `safety.rs`
  - Adds `check_clean(work_tree, git)` using `git status --porcelain=v1 --untracked-files=normal` and returning `DirtyTree { hint: "pass --force" }` for any output.
  - Adds `check_path_is_managed_outpost_of(source, candidate)` that canonicalizes the candidate, opens it through `source.outpost_at()` to preserve hermetic env, resolves its stored source repo, and verifies the resolved `work_tree` matches the source.
  - Adds `check_destination_clean(parent, dest)` for later add/move use, rejecting existing files, non-empty directories, and destinations inside an existing Git work tree.
  - Does not implement `check_no_unpushed` or divergence helpers; those are command-flow behaviors outside this chunk.

## Tests Added / Updated

- Unit tests added:
  - `check_clean_reports_staged_changes_as_dirty`
  - `check_clean_reports_unstaged_changes_as_dirty`
  - `check_clean_reports_untracked_changes_as_dirty`
  - `check_clean_allows_clean_work_tree`
  - `managed_outpost_gate_rejects_path_with_no_git_repo`
  - `managed_outpost_gate_rejects_managed_false`
  - `managed_outpost_gate_rejects_different_source`
  - `managed_outpost_gate_accepts_matching_source`
  - `destination_clean_rejects_existing_file_and_non_empty_dir`
  - `destination_clean_allows_missing_and_empty_dir_outside_repo`
  - `destination_clean_rejects_target_inside_existing_repo`
  - `destination_clean_allows_relative_sibling_outside_repo`

## Integration Tests Added / Updated

- none; QA-owned add/list integration tests are scheduled after ops APIs stabilize.

## Docs Added / Updated

- none
- Rationale: architecture section 5.8 already documents the stable safety contracts implemented in this chunk.

## Verification

- `cargo fmt --check`: pass
- `cargo test -p outpost-core safety::tests::`: pass; 12 safety tests
- `cargo test -p outpost-core`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 40 unit tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --features test-helpers`: pass; 40 unit tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none

## Protected-Path Exceptions

- none

## Architecture Deviations

- `check_no_unpushed` and divergence helpers remain unimplemented in this chunk because `safety-gates` was scoped to U-10 and U-13 plus add/move destination support. Those helpers require ahead/behind and command-flow behavior planned for later chunks/phases.

## Residual Risks / Handoff Notes

- `check_path_is_managed_outpost_of` intentionally maps candidate open/source-ownership failures to `RegistryEntryNotManaged`, matching later remove/move safety behavior.
- `check_destination_clean` returns the user-supplied destination path in typed errors while using a resolved path internally for containment checks.
