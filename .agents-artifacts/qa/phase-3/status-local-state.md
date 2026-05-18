# QA Note: status-local-state

## Summary

QA extended the core status integration suite for local state reporting. The tests call `outpost_core::ops::status::run_with(<target>, &fixture.git_env)` against real fixture repositories and cover source path, remote name, current/detached branch, dirty state, and missing source reporting.

## Test IDs Addressed

- S-01
- S-02
- S-03
- S-04
- S-10

## Test Coverage Map

| ID | File | Test Name | Behavior | Status |
| --- | --- | --- | --- | --- |
| S-01 | `crates/core/tests/status.rs` | `s01_run_with_from_inside_outpost_reports_canonical_source_path` | Running status from a nested path inside C reports canonical B as `source_path` | implemented passing |
| S-02 | `crates/core/tests/status.rs` | `s02_run_with_reports_local_remote_name` | Running status on C reports `remote_name = local` | implemented passing |
| S-03 | `crates/core/tests/status.rs` | `s03_run_with_reports_current_branch_and_detached_head` | Status reports `main` on a normal checkout and `None` after detached HEAD checkout | implemented passing |
| S-04 | `crates/core/tests/status.rs` | `s04_run_with_reports_dirty_state_for_untracked_files` | Status reports clean before an untracked file and dirty after the file exists | implemented passing |
| S-10 | `crates/core/tests/status.rs` | `s10_run_with_reports_missing_source_problem` | Moving B makes status report `source_present=false` and `ConfigProblem::SourceMissing` | implemented passing |

## Files Changed

- `crates/core/tests/status.rs`

## Fixture Changes

- none

## Production Code Changes

- none by QA

## Docs Added / Updated

- none; these tests exercise behavior already specified by product, architecture, and roadmap documents.

## Verification Run

- `cargo test -p outpost-core --test status`: pass; 9 status integration tests, run by QA worker and coordinator

## Verification Not Run

- none for the assigned QA slice

## Blocked Tests

- none for S-01, S-02, S-03, S-04, S-10

## Risks / Handoff Notes

- S-05, S-06, S-11, and S-12 remain planned for `status-relationship-health`.
