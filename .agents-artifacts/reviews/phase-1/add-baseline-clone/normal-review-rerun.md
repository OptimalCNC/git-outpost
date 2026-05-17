# Normal Review Rerun: add-baseline-clone

## Verdict

approved

## Evidence Reviewed

- `e7d7a47..HEAD`
- Clean `git status --short`
- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- Prior review artifacts
- Implementation in `crates/core/src/ops/add.rs`
- Tests in `crates/core/tests/add.rs`
- Rerun commands: `git diff --check e7d7a47..HEAD`, `cargo fmt --check`, `cargo test -p outpost-core --test add`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`

## Requirement Reasoning

Phase 1 includes `ops::add` and C-01..C-20 per `docs/src/roadmap.md`. Product add behavior is documented in `docs/src/product.md`, including existing/new branch modes, empty destination rule, metadata, registry, local ignore, remote name, and `receive.denyCurrentBranch=updateInstead`. Architecture section 5.9.1 specifies the add API and ordered algorithm.

The implementation matches the required flow: destination precheck before clone, branch resolution before clone, `git -c protocol.file.allow=user clone --no-shared --`, remote rename, checkout modes, metadata write, config-change reporter event, source config write, registry save, and returned outpost.

The prior normal finding is fixed. `run` resolves `AddOptions.destination` once, anchors relative paths under `source.work_tree()`, and then uses the resolved path for safety, clone destination, post-clone invoker, registry insertion, and return.

## Test Reasoning

The committed add suite covers C-01..C-20 as mapped in the QA note. The two required relative-destination regressions are present:

- `add_rejects_relative_destination_inside_source_repo`
- `add_relative_sibling_destination_uses_same_resolved_path_for_all_steps`

These tests directly cover the prior cwd-split regression. They do not cover CLI/global `-C`, which is outside this chunk's QA scope.

## Docs Reasoning

No docs changes were required for this fix. The stable add contract is already covered by product docs and architecture section 5.9.1; the normal-review fix is an implementation consistency fix for that contract, backed by regression tests. Documentation Policy is satisfied.

## Verification Reasoning

Supplied evidence records the normal-review fix verification, including 22 add integration tests and workspace/core suites passing. Reviewer reran the same relevant commands locally; all passed. `git status --short` remained clean after verification.

## Findings

none

## Missing Evidence

none

## Required Changes

none

## Notes

none
