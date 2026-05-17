# Independent Review - storage-foundations

## Verdict: changes requested

## Evidence Reviewed

- Commit `e80bd1e3f3361192048d59c39266ff2c64dbb9c0`
- Source docs: `product.md`, `architecture.md` sections 5.6, 5.7, 7, 11.1, 12; `roadmap.md` Phase 1 row; `coordinator-prompt.md`
- Artifacts: `phase-1.md`, `evidence-pack.md`, current `scope-review.md`
- Code/manifests: `metadata.rs`, `registry.rs`, `source_repo.rs`, `lib.rs`, `Cargo.toml`, `Cargo.lock`
- Independently reran:
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core`: pass, 18 unit + 1 integration smoke
  - `cargo test -p outpost-core --tests`: pass
  - `cargo test --workspace`: pass

## Independent Findings (severity, file/line, issue, required change)

| Severity | File/line | Issue | Required change |
|---|---|---|---|
| Medium | `crates/core/src/registry.rs:213` | `RegistryMut::remove_by_path` always calls `fs::canonicalize`, so it cannot remove a registered path after the outpost directory is gone. That conflicts with `prune`/missing-path requirements in `product.md` and `architecture.md` where stale registry entries must be removable. | Adjust the registry removal API so callers can remove already-registered canonical paths that no longer exist, and add a focused unit test for removing a missing registered path. |

## Regression/Scope Risks

- The chunk stays within Phase 1 storage scope; no CLI, `ops::add`, `ops::list`, Phase 2 command behavior, or product docs were modified.
- The stale-path removal issue is a future integration hazard for Phase 2 `remove`/`prune`; with the current API, those ops will need to either change registry internals later or fail stale cleanup.
- Dependency/MSRV evidence is acceptable for the active Linux target. All-target dependency verification remains incomplete because target-specific crates were not cached offline; I did not find a blocking Phase 1 issue from that.
- `tempfile = "=3.10.0"` remains an exact pin while architecture section 12 says compatible-version ranges. This appears inherited, but now `tempfile` is a production dependency, so it should be reconciled in a later dependency-policy pass.

## Required Changes

- Fix `RegistryMut::remove_by_path` or add an equivalent storage-layer removal method that can deregister missing paths.
- Add a unit test covering a registered path that existed at insertion, is deleted, and is then successfully removed from the registry.

## Notes

`metadata.rs` matches the important local-config requirements: `--local` is used for reads/writes, absent config returns unmanaged, and source paths are canonicalized on write. Registry JSON shape, same-directory tempfile persistence, local exclude installation, lock preservation on re-add, and the debug Drop guard are otherwise consistent with the architecture.
