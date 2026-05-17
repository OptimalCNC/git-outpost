# Normal Review: Phase 0 / phase0-fixture-scaffold

## Verdict

needs changes

## Evidence Reviewed

- Commit `2361d90`
- Evidence pack: `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/evidence-pack.md`
- Scope review: `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/scope-review.md`
- Files: `Cargo.toml`, `Cargo.lock`, `crates/core/Cargo.toml`, `crates/core/tests/common/mod.rs`, `crates/core/tests/common/fixture.rs`, `crates/core/tests/fixture_smoke.rs`, `crates/core/src/git.rs`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md` sections 2, 5.2, 10.1-10.3, 11.13, 12; `docs/src/roadmap.md`
- Progress log
- Local `cargo metadata --no-deps --format-version 1`
- Local registry manifests for locked `tempfile 3.27.0`, `getrandom 0.4.2`, `rustix 1.1.4`

## Requirement Reasoning

Phase 0 fixture scaffold is in roadmap scope. The A/B topology matches architecture: bare A, cloned B, explicit `main`, initial commit through B pushed to A. Hermetic Git env is threaded through `GitInvoker::with_env` with empty global/system config, fixed author/committer, and terminal prompts disabled. `core.autocrlf=false` is set on B. C/outpost helpers are intentionally omitted and reasonably deferred because they require Phase 1 APIs. However, the new `tempfile = "^3.0"` lockfile resolves to `tempfile 3.27.0` and `getrandom 0.4.2`; `getrandom 0.4.2` declares `edition = "2024"` and `rust-version = "1.85"`, conflicting with workspace/docs MSRV Rust 1.75.

## Test Reasoning

The smoke test proves the fixture creates A/B, A HEAD points at `main`, B disables autocrlf, B has the initial commit, source/upstream commit helpers produce 40-character object IDs, and global Git config is isolated. It does not prove MSRV compatibility, Windows behavior, C/outpost helpers, Phase 1 ops integration, or that helper return values are valid object IDs beyond length.

## Docs Reasoning

No product/architecture/roadmap docs were required for the fixture implementation; existing architecture already documents the fixture and the deferral is captured in evidence. Docs quality is adequate. The dependency/MSRV mismatch conflicts with architecture section 12's Rust 1.75 requirement.

## Verification Reasoning

Evidence reports `cargo fmt --check`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, and `cargo test --workspace` passing on the local Rust 1.94 toolchain. Missing verification is an MSRV check with Rust 1.75 or equivalent dependency audit; local dependency manifests show the current lockfile cannot satisfy declared MSRV.

## Findings

- Severity: high. `Cargo.lock` locks `getrandom 0.4.2`, and its registry manifest declares `edition = "2024"` / `rust-version = "1.85"` while `Cargo.toml` declares workspace `rust-version = "1.75"` and architecture section 12 requires Rust 1.75. The project will not build on its declared MSRV, so the dependency change is not architecture-compatible despite passing on Rust 1.94.

## Missing Evidence

MSRV verification for Rust 1.75 after adding `tempfile`, or evidence that all locked dependency `rust-version` values remain compatible with Rust 1.75.

## Required Changes

Resolve the `tempfile` dependency/lockfile to Rust 1.75-compatible versions, then record MSRV-compatible verification; do not raise the project MSRV unless that is an explicit product/architecture decision.

## Notes

Fixture implementation itself is otherwise appropriately scoped and aligned with the Phase 0 scaffold.
