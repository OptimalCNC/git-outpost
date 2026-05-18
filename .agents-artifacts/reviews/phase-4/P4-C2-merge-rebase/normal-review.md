# Normal Review: P4-C2-merge-rebase

**Verdict**

Pass. Commit `4a68f15` correctly implements the Phase 4 merge/rebase core operations and tests for MR-01..MR-06 within the documented scope. I found no required changes.

**Evidence Reviewed**

- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`.
- Phase artifacts: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`.
- Commit diff for `4a68f15`, especially:
  - `crates/core/src/ops/merge.rs`
  - `crates/core/src/ops/rebase.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/tests/merge.rs`
  - `crates/core/tests/rebase.rs`
- Supporting APIs inspected: `Outpost::current_branch`, `SourceRemoteRef`, `GitInvoker`, `Reporter`, and existing `ops::pull` behavior.

**Correctness Findings**

- Attached-branch precondition runs before any merge/rebase fetch. Both `merge::run` and `rebase::run` call `outpost.current_branch()` first and map detached `HEAD` to `NoUpstreamTracking { branch: "HEAD" }`; tests assert no reporter steps and unchanged `refs/remotes/local/main` for MR-06.
- Wrong remote validation runs before any fetch. Both ops compare `opts.source_ref.remote` to `outpost.metadata().remote_name` before emitting `OutpostFetch` or running `git fetch`; MR-04 tests add a decoy `origin` remote and assert `InvalidRefName`, no event, unchanged `local/main`, and no `origin/main`.
- There is no B-from-origin refresh inside merge/rebase. The implementation only runs outpost-side `git fetch <metadata remote> <branch>:refs/remotes/<remote>/<branch>` and then `git merge` or `git rebase`; it does not call `source_repo()` or `SourceRepo::fast_forward_branch_from_origin`, matching product lines 298-310.
- Custom remote metadata is honored. Fetch refs and merge/rebase targets are built from `SourceRemoteRef.remote`, after equality validation against `metadata.remote_name`; MR-03 covers custom remote names and verifies no reliance on `local`.
- `OutpostFetch` is emitted before the user-visible outpost fetch. Both ops call `reporter.step(StepKind::OutpostFetch, ...)` immediately before `fetch_source_ref`; MR-05 captures the event.
- Success behavior matches the architecture. Merge fetches B's branch into C's remote-tracking ref and merges `<remote>/<branch>`; rebase fetches the same ref and rebases onto it. MR-01 and MR-02 assert the fetched tracking ref and resulting history.

**Architecture / Scope Reasoning**

- The implementation matches architecture sections 5.9.6 and 5.9.7: attached branch, metadata remote match, `OutpostFetch`, explicit C-side fetch refspec, then native Git merge/rebase.
- `ops::mod` only exports the new `merge` and `rebase` modules. No CLI, global `-C`, end-to-end, push, or Phase 5 behavior is introduced.
- The error mapping follows the existing `ops::pull` pattern for detached `HEAD`.
- Argument construction remains through `GitInvoker` with separate argv entries; source refs are parsed through validated `SourceRemoteRef`.

**Test Reasoning**

- MR-01: `mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref` checks the remote-tracking ref and that both source and outpost commits are ancestors of `HEAD`.
- MR-02: `mr02_rebase_fetches_source_branch_and_rebases_current_branch` checks the remote-tracking ref, rebased parent, changed outpost commit id, and file contents.
- MR-03: merge and rebase custom-remote tests exercise `custom/main` and verify no `local` remote dependency.
- MR-04: merge and rebase wrong-remote tests verify `InvalidRefName` before fetching.
- MR-05: merge and rebase reporter tests verify the captured `OutpostFetch` event.
- MR-06: merge and rebase detached-HEAD tests verify the attached-branch error before fetching.

**Docs Reasoning**

- Product docs already specify that `merge <source-ref>` and `rebase <source-ref>` fetch the source-repo branch named by the configured source remote form, and explicitly do not refresh B from `origin`.
- Architecture docs already define the new `MergeOptions`, `RebaseOptions`, report structs, exact operation sequence, and MR-01..MR-06 tests.
- Roadmap places `ops::merge` and `ops::rebase` in Phase 4, while Phase 5 owns whole-binary, e2e, and global CLI behavior. No additional docs changes appear necessary for this chunk.

**Verification Reasoning**

- Ran `cargo test -p outpost-core --test merge`: passed, 5 tests.
- Ran `cargo test -p outpost-core --test rebase`: passed, 5 tests.
- These focused runs directly cover the committed merge/rebase behavior. I did not rerun the full workspace because the evidence pack already records `cargo test -p outpost-core` passing and the review focus is this chunk.

**Findings**

- None.

**Missing Evidence**

- No direct argv-log assertion proves `OutpostFetch` is emitted before the actual `git fetch`; the implementation order is straightforward and the focused tests cover event presence plus precondition no-event paths.
- Conflict behavior is intentionally delegated to Git and not separately exercised in MR-01..MR-06.

**Required Changes**

- None.

**Nits**

- `merge.rs` and `rebase.rs` duplicate small helper functions. This is acceptable at this chunk size and does not justify a refactor now.
