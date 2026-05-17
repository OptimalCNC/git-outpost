# Independent Review Rerun - storage-foundations

## Verdict: `approved`

## Evidence Reviewed

Reviewed commits `e80bd1e` and `98591c6`, the requested source docs/artifacts, and the storage code. Key code evidence: `crates/core/src/registry.rs`.

Ran verification without modifying files:

- `CARGO_TARGET_DIR=/tmp/git-outpost-review-target cargo test -p outpost-core registry::tests:: --locked`
- `CARGO_TARGET_DIR=/tmp/git-outpost-review-target cargo test -p outpost-core --locked`
- `CARGO_TARGET_DIR=/tmp/git-outpost-review-target cargo test --workspace --locked`

All passed. `git status --short` remained clean.

## Previous Findings Status

Resolved. `RegistryMut::remove_by_path()` now uses `find_existing_or_recorded()`, which first canonicalizes existing paths and, if canonicalization fails, matches an already-recorded canonical path exactly. The regression test `remove_by_path_handles_registered_missing_path` registers a path, deletes the directory, removes by the saved canonical path, saves, and reloads an empty registry.

## Independent Findings (severity, file/line, issue, required change)

None.

## Regression/Scope Risks

No new blocking regression or scope risk found. The fix is scoped to registry storage helpers and tests. It does not add Phase 2 command behavior, CLI behavior, or unrelated docs changes.

## Required Changes

None.

## Notes

The review-fix also addresses adjacent storage hazards from the normal review: save errors no longer trip the dirty Drop guard, and `update_path()` can update an already-registered old path after filesystem rename.
