# QA Note: status-report-core

## Summary

QA added focused core integration coverage for the first Phase 3 status slice. The tests call `outpost_core::ops::status::run_with(<target>, &fixture.git_env)` against real temporary Git repositories and cover explicit target-path handling, unmanaged repository rejection, and degraded `RawMetadata` reporting when `outpost.sourceRepo` is missing.

## Test IDs Addressed

- S-07
- S-08
- S-09
- S-13

## Test Coverage Map

| ID | File | Test Name | Behavior | Status |
| --- | --- | --- | --- | --- |
| S-07 | `crates/core/tests/status.rs` | `s07_run_with_accepts_explicit_outpost_target_path` | `run_with(&C, env)` reports C even when the process cwd is outside C | implemented passing |
| S-08 | `crates/core/tests/status.rs` | `s08_unmanaged_repo_returns_not_an_outpost` | Running status on non-managed B returns `OutpostError::NotAnOutpost` | implemented passing |
| S-09 | `crates/core/tests/status.rs` | `s09_missing_source_repo_config_is_reported_as_problem` | Missing `outpost.sourceRepo` is reported in `problems` | implemented passing |
| S-13 | `crates/core/tests/status.rs` | `s13_missing_source_repo_config_keeps_degraded_report_available` | Missing `outpost.sourceRepo` still returns a degraded report using `RawMetadata` | implemented passing |

## Files Changed

- `crates/core/tests/status.rs`

## Fixture Changes

- none

## Production Code Changes

- none by QA

## Docs Added / Updated

- none; these tests exercise behavior already specified by product, architecture, and roadmap documents.

## Verification Run

- `rustfmt crates/core/tests/status.rs`: pass, run by QA worker
- `cargo test -p outpost-core --test status`: pass; 4 status integration tests, run by QA worker and coordinator

## Verification Not Run

- none for the assigned QA slice

## Blocked Tests

- none for S-07, S-08, S-09, S-13

## Risks / Handoff Notes

- S-01..S-06 and S-10..S-12 remain planned for later Phase 3 chunks.
