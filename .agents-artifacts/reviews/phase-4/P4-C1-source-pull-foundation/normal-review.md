**Verdict**: approved with nits

**Evidence Reviewed**:
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Review artifacts: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C1-source-pull-foundation/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C1-source-pull-foundation.md`
- Commit: `9d491be phase-4: add source pull foundation`
- Files inspected: `crates/core/src/source_repo.rs`, `crates/core/src/ops/source.rs`, `crates/core/src/ops/pull.rs`, `crates/core/src/safety.rs`, `crates/core/src/ops/mod.rs`, `crates/core/tests/source.rs`, `crates/core/tests/pull.rs`, `crates/core/tests/common/fixture.rs`
- Commands inspected/run: `git status --short`, `git show --stat --oneline --decorate --name-only 9d491be`, `git show --format=fuller --no-ext-diff --unified=80 9d491be -- ...`, focused `rg`/`sed`/`nl` reads, `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-normal-review-target cargo test -p outpost-core --test source`, `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-normal-review-target cargo test -p outpost-core --test pull`

**Correctness Findings**: none. `SourceRepo::fast_forward_branch_from_origin` correctly prechecks local B branch existence, fetches `origin <branch>:refs/remotes/origin/<branch>`, treats equal/source-ahead as no-op, fast-forwards source-behind branches, rejects true divergence with typed `Divergence`, uses checked-out worktree `git merge --ff-only` for checked-out B branches, and uses guarded `git update-ref` for unchecked-out refs. `ops::source::pull` and `ops::pull::run` follow the documented sequencing and error behavior for the reviewed cases.

**Architecture / Scope Reasoning**: the commit fits Phase 4 chunk scope. It adds core-only `ops::source` and `ops::pull`, source refresh support, C/B divergence checking, and reporter events without adding Phase 5 CLI behavior, global `-C`, E2E behavior, or merge/rebase/push implementation. `ops::pull::run` builds the source remote from `outpost.metadata().remote_name`, so custom remote behavior is not hardcoded to `local`.

**Test Reasoning**: SP-01..SP-05 and P-01..P-09 are present and map to the requested scenarios. The tests cover unchecked-out B ref updates, checked-out B worktree updates, B/origin divergence, missing source branch, source fetch events, source-before-outpost pull sequencing, B-only updates without touching A, missing source repo, detached HEAD, custom remote, C/B divergence, ordered reporter events, and missing matching source branch before C fast-forward. Source-ahead, source-behind, and divergence are directly covered; the exact equal no-op case is assessed by code inspection rather than a dedicated named test.

**Docs Reasoning**: no product or architecture doc update is required for this chunk. The existing docs already describe `pull`, `source pull`, `Reporter` events, `SourceRepo::fast_forward_branch_from_origin`, `safety::check_no_divergence`, and SP/P test expectations. One API-shape nit is noted below.

**Verification Reasoning**:
- `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-normal-review-target cargo test -p outpost-core --test source`: passed, 5/5 tests.
- `CARGO_TARGET_DIR=/tmp/git-outpost-p4c1-normal-review-target cargo test -p outpost-core --test pull`: passed, 9/9 tests.
- I used `/tmp` for `CARGO_TARGET_DIR` to avoid writing build artifacts into the repo. Full workspace verification was not rerun during this normal review; it is recorded as passing in the evidence pack.

**Findings**: none.

**Missing Evidence**: no dedicated equal B/origin no-op integration test. This is low risk because the implementation checks `local_oid == remote_oid` before ancestor classification, and source-ahead no-op is covered by P-02.

**Required Changes**: none.

**Nits**:
- `docs/src/architecture.md` still shows `SourceRepo::fast_forward_branch_from_origin` returning `OutpostResult<()>`, while the implementation returns `OutpostResult<bool>` so reports can expose `updated`. This is harmless for behavior but should be reconciled when docs are next touched.
- `.agents-artifacts/progress/phase-4.md` still says the P4-C1 implementation/evidence commit is pending even though this review targets committed hash `9d491be`.
