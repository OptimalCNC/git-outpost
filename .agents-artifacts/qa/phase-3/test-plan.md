# QA/Test Plan: Phase 3 `ops::status`

## Scope

Phase 3 covers core-only `ops::status` using `RawMetadata` for status reporting. No CLI E2E, binary-name, stdout formatting, or global `-C` behavior is in scope; Phase 5 owns that.

## Test File Ownership

`crates/core/tests/status.rs` owns S-01..S-13 as core integration tests.

Use `ops::status::run_with(<target>, &fixture.git_env)` for hermetic test execution. `run(<target>)` can be covered narrowly if needed, but S coverage should prefer `run_with` to match existing fixture env patterns.

## S-01..S-13 Mapping

| ID | Proposed test name |
| --- | --- |
| S-01 | `status_reports_canonical_source_path` |
| S-02 | `status_reports_configured_remote_name` |
| S-03 | `status_reports_current_branch_and_none_when_detached` |
| S-04 | `status_reports_dirty_tree_for_untracked_files` |
| S-05 | `status_reports_outpost_ahead_behind_source_without_fetching` |
| S-06 | `status_reports_source_ahead_behind_origin` |
| S-07 | `status_uses_explicit_target_path_when_process_cwd_is_elsewhere` |
| S-08 | `status_from_unmanaged_repo_returns_not_an_outpost` |
| S-09 | `status_reports_missing_source_repo_config_as_problem` |
| S-10 | `status_reports_source_not_present_when_source_directory_missing` |
| S-11 | `status_reports_local_remote_mismatch` |
| S-12 | `status_uses_custom_remote_name_for_tracking_and_remote_url` |
| S-13 | `status_uses_raw_metadata_when_source_repo_config_is_missing` |

## Developer-Owned Unit/Helper Tests Expected

Developer should add focused unit tests for pure helper logic if introduced in `crates/core/src/ops/status.rs`.

Expected helper coverage:
- optional current branch helper maps detached `HEAD` to `None`, not `BranchNotFound`
- ahead/behind helper uses existing refs only and does not invoke `git fetch`
- remote URL/source path comparison canonicalizes local path URLs carefully
- `ConfigProblem` construction for missing `sourceRepo`, missing remote name, source missing, and remote mismatch
- parse/count helper tests if new parsing code is not reused from existing tested code

## Fixture/Helper Recommendations

Use existing `AbcFixture` A/B/C topology.

Recommended setup patterns:
- Add outpost with `fixture.add_outpost("C")`.
- Dirty state: write an untracked file in C, matching existing list/remove tests.
- Outpost ahead: `fixture.commit_in_outpost(&outpost, "outpost commit")`.
- Outpost behind: commit in B after C is added.
- Source vs origin ahead/behind: use `fixture.commit_in_source(...)`, `fixture.commit_in_upstream("main", ...)`, and avoid commands that hide whether status fetched.
- Missing `outpost.sourceRepo`: `git config --local --unset outpost.sourceRepo` in C.
- Missing source directory: rename or remove B after C exists; keep expected `source_path` from metadata.
- Remote mismatch: set `remote.<remote_name>.url` to a different local repo/path and assert `LocalRemoteMismatch`.
- Custom remote: add with `AddOptions { remote_name: RemoteName::parse("custom") }`, then assert status uses `custom`.

For read-only verification, compare refs before/after status where useful:
- `refs/remotes/local/main`
- `refs/remotes/custom/main`
- `refs/remotes/origin/main`
- relevant local branch refs in B and C

## Verification Commands

```bash
cargo test -p outpost-core --test status
cargo test -p outpost-core
cargo test -p outpost-core --tests
cargo test --workspace
```

## Risks / Cautions

- Status must be read-only: no fetch, pull, push, branch update, or ref update.
- Do not blindly reuse `Outpost::ahead_behind_source()` because it fetches.
- Detached `HEAD` must produce `current_branch = None`.
- S-09 and S-13 must explicitly prove degraded `RawMetadata` reporting for missing `outpost.sourceRepo`.
- `LocalRemoteMismatch` must handle canonical local paths and path-like remote URLs carefully.
- Keep CLI formatting, binary behavior, and global `-C` out of Phase 3.
- Blocked items: none.

## Recommended First QA Step

Start with S-09/S-13 plus S-08 to lock down the `RawMetadata` degraded-reporting contract before validating richer status fields.
