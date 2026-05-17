- **Verdict**: approved with nits

- **Evidence Reviewed**: updated evidence pack, progress log, prior reviews, `fd66377` and `bad1609` diffs/name-status, current `git status`, source docs, Documentation Policy, and code/tests in `crates/core`. Verification rerun: `cargo fmt --check`, `cargo test -p outpost-core --tests`, `cargo test -p outpost-core`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`, `git diff --check`, `git diff --cached --check`, and `git diff HEAD --check`; all passed. Current worktree has staged review/progress artifacts only.

- **Review Reasoning**: The review fix is in chunk scope: `crates/core/Cargo.toml` adds the documented self dev-dependency feature wiring, `Cargo.lock` records it, and `fixture_smoke.rs` proves normal `cargo test -p outpost-core --tests` can call `SourceRepo::test_invoker().argv_log()`. `SourceRepo` and `Outpost` accessors are public under the same `test-helpers` gate. Discovery/opening, canonical paths, metadata validation, source resolution, env threading, branch/upstream/worktree helpers, and fixture access match the product/architecture scope for this chunk. Deferred ahead/behind behavior remains outside this slice.

- **Verification And Risk Reasoning**: Tests prove the source/outpost opening paths, unmanaged/missing-source errors, dirty detection, branch/upstream/worktree helpers, hermetic fixture source opening, and integration-test helper access. Residual risk is limited to uncalled-but-same-gate `Outpost::test_invoker()` integration use and future command-level paths not implemented in this chunk.

- **Docs Reasoning**: No docs changes were needed. The stable contracts are already covered by architecture sections for `GitInvoker`, `SourceRepo`, `Outpost`, and hermetic fixture env threading, and the review fix brings implementation in line with that documented feature-wiring contract.

- **Findings**: none

- **Missing Evidence**: none

- **Required Changes**: none

- **Notes**: Nit: the evidence pack's changed-file list still omits the prior review artifact files added by `bad1609`, and current staged scope-rerun bookkeeping is outside the pack because it was added afterward. These are review/progress artifacts, not implementation behavior gaps.
