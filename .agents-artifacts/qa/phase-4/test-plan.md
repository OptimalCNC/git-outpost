- **Summary**: Phase 4 QA will cover the sync operations as core integration tests that call `outpost_core::ops::{source,pull,merge,rebase,push}` directly against real temporary Git repositories, primarily the A/B/C fixture where A is bare upstream, B is source, and C is managed outpost. The plan keeps CLI binary, global `-C`, stdout/stderr formatting, and full Story E2E behavior out of scope for Phase 5, while asserting cross-repository `Reporter` events through captured `StepKind` values.

- **Phase Test IDs Covered**: SP-01, SP-02, SP-03, SP-04, SP-05, P-01, P-02, P-03, P-04, P-05, P-06, P-07, P-08, P-09, MR-01, MR-02, MR-03, MR-04, MR-05, MR-06, Pu-01, Pu-02, Pu-03, Pu-04, Pu-05, Pu-06, Pu-07, Pu-08, Pu-09, Pu-10.

- **Test Coverage Map**:

| ID | test file | test name | behavior | status |
| --- | --- | --- | --- | --- |
| SP-01 | `crates/core/tests/source.rs` | `sp01_source_pull_fast_forwards_unchecked_out_source_branch_without_switching` | Calls `ops::source::pull` from C for a non-checked-out B branch after A advances it; asserts B branch ref fast-forwards and B remains on its original branch. | planned |
| SP-02 | `crates/core/tests/source.rs` | `sp02_source_pull_updates_checked_out_source_worktree` | Calls `ops::source::pull` for B's checked-out `main` after A advances it; asserts B `HEAD` and working tree update through fast-forward. | planned |
| SP-03 | `crates/core/tests/source.rs` | `sp03_source_pull_returns_divergence_when_source_and_origin_diverge` | Creates unique commits on B's branch and A's matching branch, then asserts `ops::source::pull` returns `OutpostError::Divergence`. | planned |
| SP-04 | `crates/core/tests/source.rs` | `sp04_source_pull_missing_branch_returns_branch_not_found` | Calls `ops::source::pull` with a branch absent from B and asserts `OutpostError::BranchNotFound` before branch update. | planned |
| SP-05 | `crates/core/tests/source.rs` | `sp05_source_pull_records_source_fetch_event` | Calls `ops::source::pull` with a capturing reporter and asserts a `StepKind::SourceFetch` event is emitted before the source fast-forward. | planned |
| P-01 | `crates/core/tests/pull.rs` | `p01_pull_fast_forwards_source_from_origin_then_outpost_from_source` | After A advances `main`, calls `ops::pull::run` in C and asserts B then C fast-forward to the upstream commit. | planned |
| P-02 | `crates/core/tests/pull.rs` | `p02_pull_fast_forwards_outpost_from_source_without_touching_origin` | After B advances `main` locally, calls `ops::pull::run` in C and asserts C matches B while A remains unchanged. | planned |
| P-03 | `crates/core/tests/pull.rs` | `p03_pull_returns_divergence_when_source_and_origin_diverge` | Creates unique commits on B and A for the attached branch, then asserts `ops::pull::run` returns `OutpostError::Divergence` during the source-refresh step. | planned |
| P-04 | `crates/core/tests/pull.rs` | `p04_pull_returns_divergence_when_outpost_and_source_diverge` | Creates unique commits on C and B for the attached branch, then asserts `ops::pull::run` returns `OutpostError::Divergence` before `git pull --ff-only`. | planned |
| P-05 | `crates/core/tests/pull.rs` | `p05_pull_with_missing_source_returns_source_missing` | Moves or deletes B after C exists, opens C, calls `ops::pull::run`, and asserts `OutpostError::SourceMissing`. | planned |
| P-06 | `crates/core/tests/pull.rs` | `p06_pull_on_detached_head_returns_no_upstream_tracking_head` | Detaches C `HEAD`, calls `ops::pull::run`, and asserts `OutpostError::NoUpstreamTracking { branch: "HEAD" }`. | planned |
| P-07 | `crates/core/tests/pull.rs` | `p07_pull_uses_custom_source_remote_name` | Adds C with `--remote-name custom`, advances A or B, calls `ops::pull::run`, and asserts C updates through `custom`, not hardcoded `local`. | planned |
| P-08 | `crates/core/tests/pull.rs` | `p08_pull_records_source_fetch_and_outpost_fetch_events` | Calls `ops::pull::run` with a capturing reporter and asserts ordered `StepKind::SourceFetch` then `StepKind::OutpostFetch` events. | planned |
| P-09 | `crates/core/tests/pull.rs` | `p09_pull_missing_matching_source_branch_returns_branch_not_found_before_outpost_ff` | Deletes B's matching branch while C remains attached, calls `ops::pull::run`, and asserts `OutpostError::BranchNotFound` before C fast-forward. | planned |
| MR-01 | `crates/core/tests/merge.rs` | `mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref` | Calls `ops::merge::run` with `local/main` from a feature branch in C after B advances `main`; asserts `refs/remotes/local/main` updates and C contains the merge result. | planned |
| MR-02 | `crates/core/tests/rebase.rs` | `mr02_rebase_fetches_source_branch_and_rebases_current_branch` | Calls `ops::rebase::run` with `local/main` from a feature branch in C after B advances `main`; asserts C's commits are replayed onto `local/main`. | planned |
| MR-03 | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` | `mr03_merge_and_rebase_use_custom_source_remote_name` | Adds C with `--remote-name custom`, then verifies `ops::merge::run(custom/main)` and `ops::rebase::run(custom/main)` fetch and integrate from `custom`. | planned |
| MR-04 | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` | `mr04_merge_and_rebase_reject_wrong_remote_before_fetching` | From an outpost whose source remote is `local`, calls merge and rebase with `origin/main`; asserts `OutpostError::InvalidRefName` and no source fetch ref is updated. | planned |
| MR-05 | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` | `mr05_merge_and_rebase_record_outpost_fetch_event` | Calls merge and rebase with capturing reporters and asserts each emits `StepKind::OutpostFetch` before fetching B into C. | planned |
| MR-06 | `crates/core/tests/merge.rs`, `crates/core/tests/rebase.rs` | `mr06_merge_and_rebase_on_detached_head_return_attached_branch_error_before_fetching` | Detaches C `HEAD`, calls merge and rebase, and asserts the clear attached-branch error occurs before any outpost fetch. | planned |
| Pu-01 | `crates/core/tests/push.rs` | `pu01_push_sends_outpost_branch_to_source_then_origin` | Commits on C, calls `ops::push::run`, and asserts the commit appears on B's matching branch and A's matching branch. | planned |
| Pu-02 | `crates/core/tests/push.rs` | `pu02_push_records_outpost_push_and_source_push_events` | Calls `ops::push::run` with a capturing reporter and asserts ordered `StepKind::OutpostPush` then `StepKind::SourcePush` events. | planned |
| Pu-03 | `crates/core/tests/push.rs` | `pu03_push_from_outpost_only_branch_returns_ambiguous_branch_creation` | Creates and checks out a branch only in C, calls `ops::push::run`, and asserts `OutpostError::AmbiguousBranchCreation`. | planned |
| Pu-04 | `crates/core/tests/push.rs` | `pu04_push_when_source_diverged_from_outpost_returns_divergence` | Creates unique commits on B and C for the same branch, calls `ops::push::run`, and asserts `OutpostError::Divergence` before pushing. | planned |
| Pu-05 | `crates/core/tests/push.rs` | `pu05_push_dirty_outpost_succeeds` | Leaves untracked or modified files in C, calls `ops::push::run`, and asserts committed C changes still push to B and A. | planned |
| Pu-06 | `crates/core/tests/push.rs` | `pu06_push_with_missing_source_returns_source_missing` | Moves or deletes B after C exists, calls `ops::push::run`, and asserts `OutpostError::SourceMissing`. | planned |
| Pu-07 | `crates/core/tests/push.rs` | `pu07_push_uses_custom_remote_for_outpost_to_source_and_origin_for_source_to_upstream` | Adds C with `--remote-name custom`, commits in C, calls `ops::push::run`, and asserts C->B uses `custom` while B->A still updates `origin`. | planned |
| Pu-08 | `crates/core/tests/push.rs` | `pu08_push_into_dirty_checked_out_source_branch_surfaces_update_instead_git_failed` | Dirties B's checked-out target branch while `receive.denyCurrentBranch=updateInstead`, then asserts `ops::push::run` returns `OutpostError::GitFailed` with Git stderr. | planned |
| Pu-09 | `crates/core/tests/push.rs` | `pu09_push_with_deny_current_branch_refuse_returns_push_into_checked_out_branch` | Sets B `receive.denyCurrentBranch=refuse` with target branch checked out, calls `ops::push::run`, and asserts `OutpostError::PushIntoCheckedOutBranch`. | planned |
| Pu-10 | `crates/core/tests/push.rs` | `pu10_push_on_detached_head_returns_no_upstream_tracking_head_before_push` | Detaches C `HEAD`, calls `ops::push::run`, and asserts `OutpostError::NoUpstreamTracking { branch: "HEAD" }` before any push event. | planned |

- **Fixture Changes Needed**:
  - `crates/core/tests/common/fixture.rs`: add helpers to create an outpost with a custom `RemoteName`, add a new source branch from a target, checkout/detach branches, delete or rename source branches safely, read branch/ref OIDs from A/B/C, update or assert remote-tracking refs, configure `receive.denyCurrentBranch`, and create file-backed commits where a working-tree assertion is needed.
  - `crates/core/tests/common/fixture.rs`: add a `CapturingReporter` test helper that records `Vec<(StepKind, String)>` and warnings for SP-05, P-08, MR-05, and Pu-02.
  - `crates/core/tests/source.rs`: new Phase 4 integration test file for SP-01..SP-05.
  - `crates/core/tests/pull.rs`: new Phase 4 integration test file for P-01..P-09.
  - `crates/core/tests/merge.rs`: new Phase 4 integration test file for merge-side MR-01, MR-03, MR-04, MR-05, and MR-06 coverage.
  - `crates/core/tests/rebase.rs`: new Phase 4 integration test file for MR-02 plus rebase-side MR-03, MR-04, MR-05, and MR-06 coverage.
  - `crates/core/tests/push.rs`: new Phase 4 integration test file for Pu-01..Pu-10.

- **Tests To Write Before Implementation**:
  - SP-01..SP-05 in `crates/core/tests/source.rs`, because `SourceRepo::fast_forward_branch_from_origin` is the narrow foundation for source refresh and `pull`.
  - P-01, P-03, P-05, P-06, P-08, and P-09 in `crates/core/tests/pull.rs`, because they define `pull` sequencing, early errors, and event visibility before implementation details can drift.
  - MR-04, MR-05, and MR-06 across `crates/core/tests/merge.rs` and `crates/core/tests/rebase.rs`, because they pin down remote-name validation, reporter behavior, and detached-head preconditions before Git fetch/merge/rebase wiring.
  - Pu-02, Pu-03, Pu-04, Pu-09, and Pu-10 in `crates/core/tests/push.rs`, because they constrain push sequencing, divergence detection, branch-creation refusal, checked-out branch refusal, and detached-head behavior.

- **Tests To Write After API Stabilizes**:
  - P-02 and P-07 after the `ops::pull::PullReport` shape and custom-remote invocation details are stable enough to assert both ref movement and returned update flags without overfitting.
  - MR-01, MR-02, and MR-03 after `ops::merge::MergeReport`, `ops::rebase::RebaseReport`, and conflict/error propagation are stable enough to assert successful merge/rebase outcomes precisely.
  - Pu-01, Pu-05, Pu-06, Pu-07, and Pu-08 after `ops::push::PushReport`/`StepResult` and `GitFailed` propagation are stable enough to assert pushed commit counts and stderr behavior without locking in transient implementation details.

- **Blocked Tests**: none.

- **Verification Commands**:

```bash
cargo test -p outpost-core --test source
cargo test -p outpost-core --test pull
cargo test -p outpost-core --test merge
cargo test -p outpost-core --test rebase
cargo test -p outpost-core --test push
cargo test -p outpost-core --tests
cargo test -p outpost-core
cargo test --workspace
```

- **Risks**:
  - `SourceRepo::fast_forward_branch_from_origin` must update checked-out source branches through a worktree fast-forward and unchecked-out branches through ref updates; tests need to distinguish ref-only correctness from working-tree correctness.
  - Divergence tests can accidentally pass through raw `GitFailed` if remote-tracking refs are stale; P-04 and Pu-04 should assert typed `Divergence`.
  - Custom remote tests must prove `local` is not hardcoded for C->B while `origin` remains hardcoded only for B->A.
  - Reporter tests must assert events emitted before cross-repo operations without depending on exact human-facing message text.
  - Merge/rebase tests should use simple non-conflicting file-backed commits where outcome assertions need tree/content evidence, avoiding conflict behavior that is not the Phase 4 contract.
  - Push tests involving non-bare checked-out branches are sensitive to Git's `receive.denyCurrentBranch` behavior and dirty-worktree stderr; assertions should match typed errors and only the necessary stderr presence/content.
