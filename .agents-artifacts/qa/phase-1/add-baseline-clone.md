# QA Notes: add-baseline-clone

## Scope

- Phase: `phase-1`
- Chunk: `add-baseline-clone`
- Test file: `crates/core/tests/add.rs`
- QA focus: core integration behavior for `ops::add`, not CLI E2E behavior.

## Covered IDs

- C-01: `add_without_branch_clones_current_branch_with_real_git_dir`
- C-02: `add_existing_branch_checks_out_branch_and_tracks_local_remote`
- C-03: `add_new_branch_from_target_creates_source_branch_and_tracks_it`
- C-04: `add_new_branch_without_target_uses_source_current_branch`
- C-05: `add_rejects_existing_non_empty_directory`
- C-06: `add_rejects_existing_file`
- C-07: `add_outside_git_repo_cannot_discover_source`
- C-08: `add_rejects_destination_inside_existing_repo`
- C-09: `add_rejects_missing_existing_branch_before_clone`
- C-10: `add_writes_outpost_metadata_keys`
- C-11a/C-11c/C-11d: `add_configures_local_remote_and_non_shared_clone`
- C-11b: `add_without_branch_clones_current_branch_with_real_git_dir`
- C-12: `add_registers_outpost_path_in_source_registry`
- C-13: `add_custom_remote_name_replaces_origin_and_updates_metadata`
- C-14: `add_sets_source_receive_deny_current_branch_update_instead`
- C-15: `add_reports_source_config_change`
- C-16: `add_clone_allows_user_file_protocol`
- C-17: `add_rejects_unborn_source_head_before_clone`
- C-18: `add_new_branch_rejects_missing_target_before_clone`
- C-19: `add_new_branch_does_not_switch_source_checkout`
- C-20: `add_ignores_source_registry_directory_locally`

## Deferred IDs

- none for `ops::add`; C-01..C-20 are implemented and passing.

## Regression Coverage

- `add_rejects_relative_destination_inside_source_repo`: relative destination `C` resolves under the source repo and is refused consistently.
- `add_relative_sibling_destination_uses_same_resolved_path_for_all_steps`: relative destination `../C` uses the same resolved sibling path for clone, metadata, registry, and returned `Outpost`.

## Verification

- `cargo test -p outpost-core --test add`: pass; 22 tests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 1 fixture smoke test
