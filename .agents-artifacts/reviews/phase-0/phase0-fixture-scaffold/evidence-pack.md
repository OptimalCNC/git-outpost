# Evidence Pack: Phase 0 / phase0-fixture-scaffold

## Scope

- Phase: `phase-0`
- Chunk: `phase0-fixture-scaffold`
- Roadmap scope: Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture
- Test IDs in this chunk: none directly
- Source docs reviewed: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Progress log: `.agents-artifacts/progress/phase-0.md`

## Changed Files

- `.agents-artifacts/progress/phase-0.md`
- `.agents-artifacts/reviews/phase-0/phase0-fixture-scaffold/evidence-pack.md`
- `Cargo.toml`
- `Cargo.lock`
- `crates/core/Cargo.toml`
- `crates/core/tests/common/mod.rs`
- `crates/core/tests/common/fixture.rs`
- `crates/core/tests/fixture_smoke.rs`

## Moves / Renames

- none

## Diff Summary

- Added `tempfile` as a workspace dev dependency for temporary Git repositories and empty config files.
- Added `AbcFixture` with temporary root, bare upstream A, cloned source B, and hermetic Git environment values.
- Added `AbcFixture::invoker` that threads hermetic env through `GitInvoker::with_env`.
- Added helpers `commit_in_source` and `commit_in_upstream`.
- Added a fixture smoke integration test that verifies A HEAD, B `core.autocrlf=false`, initial commit, commit helpers, and empty global Git config behavior.
- Deliberately omitted `add_outpost`, `dirty_outpost`, and `outpost_with_unpushed` because they require Phase 1 `ops::add` and outpost behavior.

## Tests Added / Updated

- `crates/core/tests/fixture_smoke.rs::abc_fixture_builds_a_b_with_hermetic_git_env`

## Integration Tests

- Added one core integration smoke test for fixture scaffold behavior.

## Docs Added / Updated

- none. Product, architecture, and roadmap edits were out of scope; fixture intent is already documented in the architecture.

## Verification

- `cargo fmt --check`: pass.
- `cargo test -p outpost-core`: pass; 10 unit tests, 1 fixture smoke test, 0 doctests.
- `cargo test -p outpost-core --tests`: pass; 10 unit tests, 1 fixture smoke test.
- `cargo test --workspace`: pass; 10 unit tests, 1 fixture smoke test, 0 doctests.

## Protected Path Exceptions

- none

## Architecture Deviations

- Full architecture fixture lists C/outpost helpers. Phase 0 implements only A/B and hermetic env scaffold; C/outpost helpers are intentionally deferred because they depend on Phase 1 APIs and command behavior.

## Residual Risks / Handoff Notes

- Phase 1 should extend the fixture with outpost helpers once `ops::add` and outpost metadata behavior exist.
- No Phase 1+ command behavior was added.
