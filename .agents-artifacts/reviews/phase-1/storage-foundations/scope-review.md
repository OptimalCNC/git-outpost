# Scope Review - storage-foundations

## Verdict: approved

## Evidence Reviewed

- Commit `e80bd1e3f3361192048d59c39266ff2c64dbb9c0`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md` sections 5.6, 5.7, 7, 11.1, 12, `docs/src/roadmap.md` Phase 1 row
- Coordination/progress: `.agents-artifacts/progress/phase-1.md`
- Evidence pack: `.agents-artifacts/reviews/phase-1/storage-foundations/evidence-pack.md`
- Diff/stat/name-status for `e80bd1e^..e80bd1e`
- Targeted review of changed Rust and manifest files

## Path Matrix

| Path | Scope status | Reasoning |
|---|---:|---|
| `.agents-artifacts/progress/phase-1.md` | In scope | Phase coordination artifact; records active chunk, test map, verification, no product-doc refactor. |
| `.agents-artifacts/reviews/phase-1/storage-foundations/evidence-pack.md` | In scope | Required evidence artifact for this chunk. |
| `Cargo.toml` / `Cargo.lock` / `crates/core/Cargo.toml` | In scope | Adds storage dependencies allowed by architecture section 12: `chrono`, `serde`, `serde_json`, production `tempfile`. |
| `crates/core/src/lib.rs` | In scope | Minimal exports for storage types and storage carrier. |
| `crates/core/src/metadata.rs` | In scope | Implements Raw/validated metadata and U-05, U-06, U-14 coverage. |
| `crates/core/src/registry.rs` | In scope | Implements registry storage, JSON load/save, canonicalized entries, local ignore, Drop guard, and U-01..U-04/U-15 coverage. |
| `crates/core/src/source_repo.rs` | In scope | Minimal carrier for `work_tree`, registry path/access, and local exclude path. No discovery or command behavior. |

## Scope Reasoning

The implementation stays within the `storage-foundations` chunk. It adds `metadata.rs`, `registry.rs`, minimal storage dependencies/exports, and a minimal `SourceRepo` storage carrier needed by `Registry::load` / `SourceRepo::registry_mut`.

No `ops::add`, `ops::list`, safety gates, full source/outpost discovery, CLI behavior, e2e behavior, or global `-C` behavior were added. Registry `lock`, `unlock`, `update_path`, and `remove_by_path` are storage-layer methods specified in architecture section 5.7; they do not implement Phase 2 command behavior by themselves.

## Findings (severity, file/line, issue, required change)

None.

## Missing Evidence

I did not rerun the cargo test suite during this scope review. The evidence pack records passing `cargo fmt --check`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, and dependency checks.

No missing scope evidence found.

## Required Changes

None.

## Notes

The implementation evidence correctly calls out the intentional `SourceRepo` deviation as minimal registry coupling, with full discovery deferred to `source-outpost-discovery`. The changed product/architecture/roadmap docs were not modified, so there is no unrelated docs/refactor scope issue.
