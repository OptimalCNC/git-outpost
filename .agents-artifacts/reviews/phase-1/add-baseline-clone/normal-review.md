# Normal Review: add-baseline-clone

## Verdict

needs changes

## Evidence Reviewed

- `e7d7a47..HEAD` diff and status
- `crates/core/src/ops/add.rs`
- `crates/core/src/source_repo.rs`
- `crates/core/tests/add.rs`
- `docs/src/product.md` add section
- `docs/src/architecture.md` sections 5.9.1 and 11.2
- `docs/src/roadmap.md` Phase 1 row
- `.agents-artifacts/progress/phase-1.md`
- `.agents-artifacts/reviews/phase-1/add-baseline-clone/evidence-pack.md`
- `.agents-artifacts/qa/phase-1/add-baseline-clone.md`
- Scope review and rerun artifacts
- Commands rerun: `cargo test -p outpost-core --test add`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`, `cargo fmt --check`, `git diff --check e7d7a47..HEAD`

## Requirement Reasoning

Phase 1 includes `ops::add` and C-01..C-20. The implementation covers the documented add API shape, branch validation, non-shared local clone, remote rename, checkout modes, metadata, source config event, and registry save. Existing-branch and `-b` branch behavior match the product and architecture for absolute destinations.

Blocking issue: relative destinations are not resolved consistently between validation, clone, and post-clone operations, so the destination safety requirement is not fully satisfied.

## Test Reasoning

The 20 add integration tests pass and cover C-01..C-20 as mapped in the QA note. They prove absolute-path clone behavior, existing and new branch modes, destination/file refusal, absolute inside-repo refusal, metadata, registry, remote naming, `updateInstead`, reporter event, unborn HEAD, clone argv, and local exclude behavior.

They do not prove `ops::add::run` handles relative destinations consistently; current tests use absolute fixture paths such as `fixture.root.join("C")` and `fixture.source.join("C")`.

## Docs Reasoning

No new product or architecture docs are required for the intended add behavior because `product.md` already documents the add command, destination rules, metadata, registry, and source config behavior, and `architecture.md` documents the add algorithm and C-01..C-20 inventory. Documentation Policy is satisfied for the intended contract. If the fix chooses to require absolute `AddOptions.destination`, that API invariant must be documented and enforced.

## Verification Reasoning

Supplied verification evidence records the required add and workspace commands passing, and reviewer reruns confirmed the same. The verification gap is behavioral: the committed suite does not exercise relative destination handling through `ops::add::run`, which is where the implementation currently diverges.

## Findings

- Blocking. Evidence: `SourceRepo::at_with` stores its `GitInvoker` rooted at `work_tree`, and `ops::add::run` invokes `git clone` through `source.git()` with the raw `destination`. Post-clone `outpost_git` is built from the same raw `destination` relative to the process cwd. But preflight validation splits a relative destination into parent `"."` plus name and calls `safety::check_destination_clean` on that process-relative parent. Issue: a relative destination can be checked relative to the process cwd, cloned relative to the source work tree, then configured/opened relative to the process cwd. Why it matters: destination safety and command semantics become cwd-dependent; with `-C`-style dispatch or a library caller not already chdir'd to the source root, the add operation can validate one path and clone another, including bypassing the intended inside-repo refusal.

## Missing Evidence

- Committed test evidence for `ops::add::run` with relative destinations, including a relative path that would be inside the source repo and a relative sibling path, proving validation, clone, registry, and returned `Outpost` all use the same resolved destination.

## Required Changes

- Resolve `AddOptions.destination` once to a single effective path before validation, clone, post-clone Git operations, registry insertion, and return; or explicitly require/enforce an absolute destination API.
- Add regression tests for the relative-destination cases above.

## Notes

Scope expansion to all C-01..C-20 is properly recorded and scope-approved.
