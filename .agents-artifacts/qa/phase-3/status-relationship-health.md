# QA Note: status-relationship-health

## Summary

QA completed core integration coverage for status relationship and health reporting. The tests call `outpost_core::ops::status::run_with(<target>, &fixture.git_env)` and use real fixture repositories. Existing remote-tracking refs are set up in test fixtures before status runs so status can report ahead/behind without fetching or updating refs.

## Test IDs Addressed

- S-05
- S-06
- S-11
- S-12

## Additional Architecture Health Coverage

- `ConfigProblem::NotInRegistry`
- `ConfigProblem::PushWouldFail`

These are status report health variants from architecture 5.9.3. They are included in this final relationship-health chunk because all roadmap S-IDs are now covered and these variants complete the Phase 3 status problem surface without adding Phase 4 sync behavior.

## Test Coverage Map

| ID | File | Test Name | Behavior | Status |
| --- | --- | --- | --- | --- |
| S-05 | `crates/core/tests/status.rs` | `s05_run_with_reports_outpost_ahead_behind_source_from_existing_refs` | Status reports C ahead/behind B from existing `refs/remotes/<remote>/<branch>` and does not update that ref | implemented passing |
| S-06 | `crates/core/tests/status.rs` | `s06_run_with_reports_source_ahead_behind_upstream_from_existing_refs` | Status reports B ahead/behind A `origin` from existing local refs and does not update that ref | implemented passing |
| S-11 | `crates/core/tests/status.rs` | `s11_run_with_flags_local_remote_mismatch` | Status reports `LocalRemoteMismatch` when configured `sourceRepo` differs from `remote.<remote_name>.url` | implemented passing |
| S-12 | `crates/core/tests/status.rs` | `s12_run_with_uses_metadata_remote_name_for_custom_remote` | Status uses metadata remote name `custom` for remote URL and tracking refs, not hardcoded `local` | implemented passing |
| health | `crates/core/tests/status.rs` | `run_with_flags_not_in_registry_when_outpost_entry_is_missing` | Status reports `NotInRegistry` when B registry no longer contains C | implemented passing |
| health | `crates/core/tests/status.rs` | `run_with_flags_push_would_fail_when_source_refuses_checked_out_branch_update` | Status reports `PushWouldFail` when B refuses updates to the checked-out branch | implemented passing |

## Files Changed

- `crates/core/tests/status.rs`

## Fixture Changes

- none

## Production Code Changes

- none by QA

## Docs Added / Updated

- none; these tests exercise behavior already specified by product, architecture, and roadmap documents.

## Verification Run

- `cargo test -p outpost-core --test status`: pass; 15 status integration tests, run by QA worker and coordinator
- `cargo fmt --check`: pass, run by QA worker and coordinator

## Verification Not Run

- none for the assigned QA slice

## Blocked Tests

- none for S-05, S-06, S-11, S-12, `NotInRegistry`, or `PushWouldFail`

## Risks / Handoff Notes

- Test fixture setup uses `git fetch <remote> <refspec>` only to create local remote-tracking refs before invoking status. The production status path remains read-only and is separately verified by preserving remote-tracking ref OIDs across `run_with`.
