# Evidence Pack: Phase 1 / storage-foundations

## Scope

- Phase: `phase-1`
- Chunk: `storage-foundations`
- Roadmap scope: `metadata.rs`, `registry.rs`, minimal exports/dependencies
- Test IDs in this chunk: U-01, U-02, U-03, U-04, U-05, U-06, U-14, U-15
- Source docs reviewed: `docs/src/product.md`, `docs/src/architecture.md` sections 5.6, 5.7, 7, 11.1, 12, `docs/src/roadmap.md`
- Progress log: `.agents-artifacts/progress/phase-1.md`

## Changed Files

- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/storage-foundations/evidence-pack.md`
- `.agents-artifacts/reviews/phase-1/storage-foundations/independent-review.md`
- `.agents-artifacts/reviews/phase-1/storage-foundations/normal-review.md`
- `.agents-artifacts/reviews/phase-1/storage-foundations/scope-review.md`
- `Cargo.toml`
- `Cargo.lock`
- `crates/core/Cargo.toml`
- `crates/core/src/lib.rs`
- `crates/core/src/metadata.rs`
- `crates/core/src/registry.rs`
- `crates/core/src/source_repo.rs`

## Moves / Renames

- none

## Diff Summary

- Added architecture-approved storage dependencies: `chrono`, `serde`, `serde_json`, and promoted `tempfile` to a production dependency for atomic registry writes.
- Added `metadata.rs` with `RawMetadata`, `Metadata`, `RawMetadata::read`, `Metadata::from_raw`, and `Metadata::write`.
- Added `registry.rs` with registry JSON load/save, `RegistryEntry`, `RegistryMut`, canonical-path add/update/remove/lock/unlock helpers, same-directory tempfile persistence, local exclude installation for `.outpost/`, and the dirty unsaved Drop guard.
- Added a minimal `source_repo.rs` storage carrier with `work_tree`, `registry_path`, `registry`, `registry_mut`, and local-exclude path support. Full source discovery is intentionally deferred to `source-outpost-discovery`.
- Exported storage types from `lib.rs`.
- Updated `Cargo.lock` for the new storage dependency tree.

## Review Fixes

- Normal review finding: `RegistryMut::save()` could mask a typed save error with the dirty Drop guard panic in debug builds. Fix: `RegistryMut::save()` marks the consumed guard as an attempted save before calling the fallible save; a failed save now returns the original `OutpostResult` error and does not trip the Drop guard.
- Normal review finding: `RegistryMut::update_path()` canonicalized the old path, which failed after filesystem rename. Fix: lookup accepts an already-recorded canonical old path when the filesystem path is gone, while still canonicalizing/storing the new path.
- Independent review finding: `RegistryMut::remove_by_path()` canonicalized the path, which failed for stale registered paths. Fix: removal uses the same existing-or-recorded lookup so callers can remove a missing registered canonical path.

## Tests Added / Updated

- `metadata::tests::metadata_write_sets_local_outpost_config_keys` (U-05)
- `metadata::tests::raw_metadata_on_non_managed_repo_promotes_to_not_an_outpost` (U-06)
- `metadata::tests::raw_metadata_read_ignores_global_outpost_managed_config` (U-14)
- `registry::tests::empty_registry_serializes_to_expected_json_and_round_trips` (U-01)
- `registry::tests::add_readd_remove_and_add_round_trips_by_canonical_path` (U-02)
- `registry::tests::load_missing_registry_returns_empty_registry` (U-03)
- `registry::tests::load_malformed_json_returns_bad_registry` (U-04)
- `registry::tests::dropping_dirty_registry_mut_trips_debug_drop_guard` (U-15, debug builds)
- `registry::tests::dropping_dirty_registry_mut_does_not_panic_in_release_builds` (U-15, release builds)
- `registry::tests::failed_save_returns_error_without_drop_guard_panic` (review fix)
- `registry::tests::update_path_handles_registered_old_path_after_rename` (review fix)
- `registry::tests::remove_by_path_handles_registered_missing_path` (review fix)

## Integration Tests

- none; QA-owned add/list integration tests are scheduled after APIs stabilize.

## Docs Added / Updated

- none. Product, architecture, and roadmap already document the storage contracts and dependency policy; this chunk added only coordination artifacts.

## Verification

- `cargo fmt --check`: pass.
- `cargo test -p outpost-core`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --tests`: pass; 21 unit tests, 1 fixture smoke test.
- `cargo test --workspace`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --features test-helpers`: pass; 21 unit tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core registry::tests::`: pass; 8 registry unit tests.
- `cargo metadata --format-version 1 --no-deps --offline`: pass; workspace member `outpost-core`, Rust version `1.75`, storage dependencies present.
- `cargo tree -p outpost-core --offline`: pass; active target dependency tree includes `chrono v0.4.44`, `serde v1.0.228`, `serde_json v1.0.149`, `tempfile v3.10.0`, and existing `thiserror`.
- Local registry manifest audit for active target dependency tree: locked active crates checked have `rust-version <= 1.75` (`chrono 1.62`, `serde/serde_core 1.56`, `serde_derive 1.61`, `serde_json 1.68`, `itoa 1.68`, `memchr 1.61`, `num-traits 1.60`, `iana-time-zone 1.62`, `proc-macro2/quote/syn/unicode-ident/zmij 1.68-1.71`).

## Verification Not Run / Not Required

- `cargo metadata --format-version 1` without `--no-deps` failed under restricted network because Cargo tried to download target-specific crates such as `windows-link v0.2.1`. The required verification commands and Linux active dependency tree passed; all-target dependency materialization is not part of the Phase 1 closeout gate.
- `cargo tree -p outpost-core --target all --offline` failed because target-specific crates such as `android_system_properties v0.1.5` were not cached. Active target tree passed offline.

## Protected Path Exceptions

- none

## Architecture Deviations

- Minimal `SourceRepo` storage carrier added in this chunk because `Registry::load` and `SourceRepo::registry_mut` are coupled by the architecture. Full `SourceRepo` discovery, git-dir/common-dir helpers, env threading, branch helpers, and `test_invoker` remain deferred to the planned `source-outpost-discovery` chunk.

## Residual Risks / Handoff Notes

- Reviewers should inspect whether `RawMetadata::read` correctly treats `git config --local --get` exit 1 as absent and avoids global config leakage.
- Reviewers should inspect whether `RegistryMut` dirty/saved semantics match the architecture, especially the debug Drop guard.
- Reviewers should inspect the dependency lockfile for MSRV and scope. The active Linux tree is audited; target-specific cached gaps are documented above.
