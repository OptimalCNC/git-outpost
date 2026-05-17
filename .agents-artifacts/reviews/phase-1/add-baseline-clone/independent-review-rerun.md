# Independent Review Rerun: add-baseline-clone

## Verdict

approved

## Evidence Reviewed

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- `docs/coordinator-prompt.md` Documentation Policy
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- Prior review artifacts
- `e7d7a47..HEAD` name/status and implementation/test files
- Verified commands:
  - `git diff --check e7d7a47..HEAD`: pass
  - `cargo fmt --check`: pass
  - `cargo test -p outpost-core --test add`: pass; 22 tests
  - `cargo test -p outpost-core`: pass
  - `cargo test -p outpost-core --tests`: pass
  - `cargo test --workspace`: pass
  - `cargo test -p outpost-core --features test-helpers`: pass
  - `git status --short --untracked-files=all`: clean after verification

## Review Reasoning

Phase scope matches Phase 1: the range adds `ops::add`, narrow source-repo support, add integration tests, fixture helper, and review/progress/QA artifacts. No CLI/e2e/global `-C`, Phase 2+ command behavior, or unrelated docs cleanup was introduced.

Product and architecture behavior match the add contract: destination validation before clone, existing and `-b` branch modes, no source checkout switch, `clone --no-shared` with file protocol override, remote rename/custom remote metadata, metadata write, source config reporter event, `receive.denyCurrentBranch=updateInstead`, registry save, and `.outpost/` local ignore.

The prior scope finding is addressed by explicitly expanding this chunk to C-01..C-20. The prior normal finding is addressed by resolving `AddOptions.destination` once and using that effective path consistently.

## Verification And Risk Reasoning

The tests prove C-01..C-20 plus the relative-destination regressions: relative `C` is refused inside the source repo, while relative `../C` succeeds as a sibling and uses the same resolved path for clone, metadata, registry, and return. Broader core/workspace tests also pass.

Residual risk is limited to deferred surfaces: CLI dispatch, global `-C`, full e2e behavior, and later lifecycle commands. Partial-add rollback remains intentionally absent per architecture.

## Docs Reasoning

No docs changes were required. The stable add behavior is already documented in `product.md`; the algorithm and C-01..C-20 test inventory are documented in `architecture.md`; Phase 1 scope is documented in `roadmap.md`. The Documentation Policy is satisfied by not duplicating stable docs.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

none
