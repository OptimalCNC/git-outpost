# Normal Review - storage-foundations

## Verdict: changes requested

## Evidence Reviewed

- Commit `e80bd1e3f3361192048d59c39266ff2c64dbb9c0`
- Uncommitted `.agents-artifacts/reviews/phase-1/storage-foundations/scope-review.md`
- Source docs listed in the request, evidence pack, progress artifact, and changed Rust/manifests
- Ran `cargo test --workspace --offline`: passed
- Ran `cargo metadata --format-version 1 --no-deps --offline`: passed
- Ran `cargo tree -p outpost-core --offline`: passed

## Findings (severity, file/line, issue, required change)

| Severity | File/line | Issue | Required change |
|---|---|---|---|
| High | `crates/core/src/registry.rs:228` | `RegistryMut::save()` returns early on `self.inner.save()?` before clearing `dirty` or marking `saved`. If the save fails, the consumed guard is dropped dirty and triggers the debug Drop assertion at line 239, masking the intended `OutpostResult` error with a panic. | Return the original save error without tripping the unsaved-change guard after an attempted save, and add a regression test that forces `Registry::save()` to fail after a mutation. |
| Medium | `crates/core/src/registry.rs:173` | `update_path()` canonicalizes `old` before lookup. Product move semantics update the registry after the filesystem move succeeds, when the old path no longer exists, so this storage helper is not usable for its intended move support path. | Make old-path lookup work for the already-registered canonical path even after the old filesystem path is gone, while still canonicalizing/storing the new path. Add a test that registers `C`, renames it to `D`, then updates the registry. |

## Test/Verification Gaps

- No test covers `RegistryMut::save()` failure behavior.
- No test covers `update_path()` after an actual filesystem rename.
- Offline Linux dependency verification passed; all-target dependency materialization was not verified, matching the evidence pack's stated network limitation.

## Required Changes

- Fix `RegistryMut::save()` so save errors remain typed errors, not debug panics.
- Fix or redesign `RegistryMut::update_path()` so it supports the documented move flow.
- Add focused unit tests for both cases.

## Notes

Scope boundaries otherwise look correct: no command behavior, CLI behavior, or unrelated product/architecture doc edits were added. The storage dependencies match the architecture-approved crate set for this chunk.
