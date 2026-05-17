# QA Notes: add-baseline-clone

## Scope

- Phase: `phase-1`
- Chunk: `add-baseline-clone`
- Test file: `crates/core/tests/add.rs`
- QA focus: core integration behavior for baseline `ops::add`, not CLI E2E behavior.

## Covered IDs

- C-01: `add_without_branch_clones_current_branch_with_real_git_dir`
- C-02: `add_existing_branch_checks_out_branch_and_tracks_local_remote`
- C-10: `add_writes_outpost_metadata_keys`
- C-11a/C-11c/C-11d: `add_configures_local_remote_and_non_shared_clone`
- C-11b: `add_without_branch_clones_current_branch_with_real_git_dir`
- C-12: `add_registers_outpost_path_in_source_registry`
- C-14: `add_sets_source_receive_deny_current_branch_update_instead`
- C-15: `add_reports_source_config_change`
- C-16: `add_clone_allows_user_file_protocol`
- C-20: `add_ignores_source_registry_directory_locally`

## Deferred IDs

- C-03, C-04, C-18, C-19: branch creation modes, assigned to `add-branch-modes`.
- C-05, C-06, C-07, C-08, C-09, C-13, C-17: refusal/custom-remote cases, assigned to later add chunks.

## Verification

- `cargo test -p outpost-core --test add`: pass; 9 tests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 9 add integration tests, 1 fixture smoke test
