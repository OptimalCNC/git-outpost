# QA Notes: list-ahead-behind

## Scope

- Phase: `phase-1`
- Chunk: `list-ahead-behind`
- Test file: `crates/core/tests/list.rs`
- QA focus: core integration behavior for `ops::list`, not CLI E2E behavior.

## Covered IDs

- L-05: `list_reports_outpost_ahead_of_source`
- L-06: `list_reports_outpost_behind_source`

## Verification

- `cargo test -p outpost-core --test list`: pass; 11 tests
- `cargo test -p outpost-core --tests`: pass; 43 unit tests, 22 add integration tests, 11 list integration tests, 1 fixture smoke test
