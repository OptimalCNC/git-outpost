# `gop remove` Cleanup Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce interactive branch-cleanup evidence collection to one useful remote snapshot on the normal path and reuse it throughout `gop analyze`.

**Architecture:** Add a semantic cleanup-evidence seam in core with GitHub, generic Git, and fake adapters. Branch analysis captures local eligibility first, consumes one snapshot, fetches only missing ancestry objects, and carries the observation to analyze; removal trusts the final exact lease instead of performing a second observation.

**Tech Stack:** Rust 2024 workspace, `std::process::Command`, `serde`/`serde_json`, Git CLI, GitHub CLI GraphQL, mdBook.

**Spec:** `docs/superpowers/specs/2026-08-14-gop-remove-cleanup-performance-design.md`

## Global Constraints

- Preserve all existing cleanup eligibility and proof rules.
- Do not require `upstream_oid == source_oid`.
- Keep generic non-GitHub fallback and fail closed on incomplete evidence.
- Keep source deletion guarded by exact `update-ref` and remote deletion guarded by exact `--force-with-lease`.
- Do not add dependencies.
- Do not edit unrelated files in the existing untracked `docs/superpowers/` tree.
- Do not commit, push, publish, or open a pull request without separate authorization.
- Every production behavior change follows an observed failing test first.

---

### Task 1: Define and parse one cleanup evidence snapshot

**Files:**
- Create: `crates/core/src/ops/cleanup_evidence.rs`
- Modify: `crates/core/src/ops/mod.rs`
- Modify: `crates/core/src/source_repo.rs`

**Interfaces:**
- Produces: `CleanupEvidenceRequest { upstream_remote, upstream_url, branch, source_oid }`.
- Produces: `ObservedRemoteBranch { branch, oid }` and `CleanupEvidenceSnapshot { default_branch, upstream_oid, merged_pull_request }`.
- Produces: `CleanupEvidenceProvider::snapshot(&CleanupEvidenceRequest) -> OutpostResult<Option<CleanupEvidenceSnapshot>>`.
- Produces: `collect(source, request, provider) -> CleanupEvidenceCollection`, which tries the provider once and then the generic Git adapter when needed.
- Produces: `SourceRepo::fetch_remote_branches(&RemoteName, &[BranchName])` and `SourceRepo::has_commit_oid(&str)` for conditional object hydration.

- [ ] **Step 1: Write parser tests for the batched generic Git response**

Add unit tests in `cleanup_evidence.rs` with literal `ls-remote --symref` output covering a default branch plus candidate, an absent candidate, and malformed/missing HEAD metadata. Each test names the parser mutation it catches and asserts typed identities rather than parser internals.

- [ ] **Step 2: Run the parser tests and verify RED**

Run: `cargo test -p outpost-core cleanup_evidence --lib --locked`

Expected: compilation fails because the module, types, and parser do not exist.

- [ ] **Step 3: Implement the snapshot types, provider interface, and generic adapter**

The generic adapter must execute exactly:

```text
git ls-remote --symref <remote> HEAD refs/heads/<branch>
```

Parse the `ref: refs/heads/<default> HEAD` line, the `<oid> HEAD` line, and the optional `<oid> refs/heads/<candidate>` line into one snapshot. Keep merged PR proof absent in this adapter. Return typed errors for malformed output and a snapshot with unknown default only when the remote genuinely supplies no symref.

- [ ] **Step 4: Add conditional object and batched fetch helpers**

Implement `has_commit_oid` with `git rev-parse --verify --quiet <oid>^{commit}`. Implement `fetch_remote_branches` by constructing one `git fetch <remote>` command with one exact `+refs/heads/X:refs/remotes/<remote>/X` refspec per unique branch; make the existing single-default fetch delegate to the exact helper where possible without changing its public result.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run: `cargo test -p outpost-core cleanup_evidence --lib --locked`

Expected: all cleanup-evidence parser and helper unit tests pass.

### Task 2: Make branch analysis consume one snapshot

**Files:**
- Modify: `crates/core/src/ops/branch_analysis.rs`
- Modify: `crates/core/tests/remove.rs`
- Modify: `crates/core/tests/analyze.rs`

**Interfaces:**
- Consumes: `CleanupEvidenceProvider`, `CleanupEvidenceRequest`, and `CleanupEvidenceSnapshot` from Task 1.
- Produces: `BranchCleanupAnalysis::evidence`, an optional operation-scoped request/snapshot pair for downstream reuse.
- Preserves: `BranchCleanupCandidate`, `BranchCleanupProof`, findings, and skip reasons.

- [ ] **Step 1: Replace the fake PR callback with a counting snapshot fake in tests**

Write failing removal and analyze tests whose fake records every request and returns a literal snapshot. Assert that a matching merged PR creates the same candidate and the provider receives exactly one request containing the expected remote, URL, branch, and source OID.

- [ ] **Step 2: Add zero-call local-gate tests and verify RED**

Add tests for a checked-out branch and another representative ineligible path with a panic/counting provider. Assert zero snapshot calls. Run:

`cargo test -p outpost-core --test remove --test analyze --locked`

Expected: compilation fails against the old `BranchCleanupProvider` interface.

- [ ] **Step 3: Refactor branch analysis around the request and snapshot**

Keep all outpost/source-local checks before `collect`. Resolve the source upstream remote and URL, build one request, collect evidence once, reject the default branch, validate an exact merged PR, then fall back to ancestry. Store the successful collection in `BranchCleanupAnalysis` for analyze.

- [ ] **Step 4: Fetch the default branch only for missing ancestry objects**

Before ancestry proof, call `has_commit_oid(snapshot.default_branch.oid)`. Fetch that exact named branch only when absent, confirm the original observed OID became available, and run `is_ancestor_oid` against that OID. Provider PR proof must return before object hydration.

- [ ] **Step 5: Run focused branch-analysis tests and verify GREEN**

Run: `cargo test -p outpost-core --test remove --test analyze --locked`

Expected: existing proof behavior and new call-count assertions pass.

### Task 3: Remove the redundant upstream recheck and prove lease safety

**Files:**
- Modify: `crates/core/src/ops/remove.rs`
- Modify: `crates/core/tests/remove.rs`

**Interfaces:**
- Consumes: `BranchCleanupCandidate::upstream_oid` captured in Task 2.
- Preserves: `SourceRepo::delete_remote_branch_if_oid`, whose exact lease is the final guard.

- [ ] **Step 1: Write a remote-movement regression test**

Create a pushed feature branch and a prompt adapter that moves the bare remote branch after evidence collection but before returning approval for upstream deletion. Assert that removal deletes the source branch, the leased push records a warning, and the moved remote branch remains at its new literal OID.

- [ ] **Step 2: Write an argv regression test for no post-source observation**

Expose the existing test-only `GitInvoker` argv log through `SourceRepo` under `test-helpers`. Assert that after the source-delete `update-ref`, the only remote deletion command is `push` with `--force-with-lease=refs/heads/<branch>:<snapshot-oid>` and there is no intervening `ls-remote`.

- [ ] **Step 3: Run the regression tests and verify RED**

Run: `cargo test -p outpost-core --test remove --locked`

Expected: the old pre-prompt recheck observes the move and prevents the leased push, so the expected push argv is absent.

- [ ] **Step 4: Delete the redundant recheck**

After successful source deletion, prompt whenever the snapshot contains an upstream OID. Pass that OID unchanged to `delete_remote_branch_if_oid`; retain existing warning and outcome behavior.

- [ ] **Step 5: Run removal tests and verify GREEN**

Run: `cargo test -p outpost-core --test remove --locked`

Expected: all removal tests pass, including the moved-remote lease failure.

### Task 4: Reuse cleanup evidence in `gop analyze`

**Files:**
- Modify: `crates/core/src/ops/analyze.rs`
- Modify: `crates/core/tests/analyze.rs`

**Interfaces:**
- Consumes: `BranchCleanupAnalysis::evidence` from Task 2.
- Produces: the existing `AnalyzeReport` fields without changing the public report shape.

- [ ] **Step 1: Write an analyze snapshot-reuse test**

Use a counting fake snapshot with upstream/default OIDs that already exist in the fixture. Assert one provider call and that `upstream_remote`, `upstream_branch`, `upstream_default_branch`, both ahead/behind probes, and `safe_delete` all reflect that single snapshot.

- [ ] **Step 2: Write a conditional batched-fetch test**

Advance both candidate and default refs in an upstream fixture so their observed OIDs are absent locally. Assert analyze fetches the required exact refs in one `fetch` argv and compares against the snapshot identities.

- [ ] **Step 3: Run analyze tests and verify RED**

Run: `cargo test -p outpost-core --test analyze --locked`

Expected: old analyze performs its independent remote probes and does not expose/reuse the fake's snapshot identities.

- [ ] **Step 4: Reorder and reuse analysis evidence**

Run branch cleanup analysis before upstream network probes. Prefer its request for remote name/URL and its snapshot for remote identities. Check object availability, batch-fetch missing named refs once, then calculate ahead/behind directly from the observed OIDs. Retain current best-effort probes when local cleanup eligibility produced no evidence request.

- [ ] **Step 5: Run analyze tests and verify GREEN**

Run: `cargo test -p outpost-core --test analyze --locked`

Expected: all analyze tests and the one-snapshot assertion pass.

### Task 5: Implement the one-call GitHub GraphQL adapter and cache

**Files:**
- Modify: `crates/cli/src/gh.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/output.rs`

**Interfaces:**
- Consumes: `CleanupEvidenceProvider` and snapshot types from Task 1.
- Produces: one cached GraphQL bundle containing the core snapshot and `Vec<PullRequestSummary>`.
- Preserves: `GhStatus::provider`, `GhStatus::analyze`, and user-facing unavailable/fallback diagnostics.

- [ ] **Step 1: Write GraphQL parser tests with a complete literal fixture**

The fixture must include default/candidate refs; matching and nonmatching merged PRs from both aliases; state, draft, base/head, review decision; and nested CheckRun/StatusContext nodes. Assert exact proof selection and the existing `PullRequestSummary` normalization.

- [ ] **Step 2: Write a fake-executable process-count test**

Use a Unix test script that appends each invocation to a file and emits the fixture. Call provider snapshot, then `GhStatus::analyze`, and assert exactly one invocation whose arguments begin with `api graphql`; assert no `--version` or `pr list` invocation.

- [ ] **Step 3: Write unavailable and unsupported-remote tests**

Assert a missing executable is classified from the useful snapshot call and retains the fallback diagnostic. Assert a non-GitHub/local URL returns `Ok(None)` without invoking the fake executable.

- [ ] **Step 4: Run CLI unit tests and verify RED**

Run: `cargo test -p git-outpost --bin gop gh::tests --locked`

Expected: compilation fails because `GhProbe` still implements the old PR-only provider and performs eager version detection.

- [ ] **Step 5: Implement remote parsing, GraphQL execution, and cache**

Recognize standard GitHub HTTPS/SSH/scp-like URLs and explicit `GH_HOST` enterprise URLs. Execute one `gh api graphql` command with explicit owner, repository, host, qualified candidate ref, and repository-scoped SHA-search variables. Parse the response into the core snapshot plus summaries; deduplicate the two PR aliases. Cache success or failure for the operation so analyze and diagnostics do not execute another command.

- [ ] **Step 6: Remove eager availability probing and wire cached analysis**

Make `GhStatus::detect` construct the lazy probe without `gh --version`. In `main.rs`, remove the pre-analysis availability command and let the useful snapshot determine status. In `output.rs`, obtain diagnostics through `GhStatus` so a cached missing/auth/GraphQL failure still explains the generic Git fallback.

- [ ] **Step 7: Run CLI unit tests and verify GREEN**

Run: `cargo test -p git-outpost --bin gop --locked`

Expected: all CLI unit tests pass with one fake GraphQL invocation.

### Task 6: Update behavioral documentation

**Files:**
- Modify: `docs/src/product.md`
- Modify: `docs/src/architecture.md`

**Interfaces:**
- Documents: one provider snapshot, one generic fallback observation, conditional fetch, analyze reuse, and final-lease race behavior.

- [ ] **Step 1: Update the `remove` and `analyze` descriptions**

State that remote/default/PR evidence is batched, default fetch is conditional, analyze reuses the observation, and a moved remote is rejected by the final lease. Remove the architecture statement requiring a separate pre-prompt recheck.

- [ ] **Step 2: Build the book**

Run: `mdbook build docs`

Expected: exit 0 with no broken include or Markdown errors.

### Task 7: Full verification and scoped review

**Files:**
- Review only: all files changed by Tasks 1-6.

**Interfaces:**
- Verifies: the complete design and all global constraints.

- [ ] **Step 1: Format and check formatting**

Run: `cargo fmt --all`

Run: `cargo fmt --all -- --check`

Expected: formatting check exits 0.

- [ ] **Step 2: Run Clippy**

Run: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`

Expected: exit 0 apart from no pre-existing Cargo manifest warning being promoted to a Rust lint.

- [ ] **Step 3: Run the full workspace suite**

Run: `cargo test --workspace --locked`

Expected: all unit, integration, end-to-end, and doc tests pass.

- [ ] **Step 4: Rebuild documentation**

Run: `mdbook build docs`

Expected: exit 0.

- [ ] **Step 5: Audit the scoped diff and operation-count requirements**

Run: `git status --short`

Run: `git diff --check`

Run: `git diff -- crates/core/src/ops/cleanup_evidence.rs crates/core/src/ops/branch_analysis.rs crates/core/src/ops/remove.rs crates/core/src/ops/analyze.rs crates/core/src/source_repo.rs crates/core/tests/remove.rs crates/core/tests/analyze.rs crates/cli/src/gh.rs crates/cli/src/main.rs crates/cli/src/output.rs docs/src/product.md docs/src/architecture.md`

Expected: every changed production line maps to the performance design, no unrelated user files are modified, no post-source remote observation remains, and tests prove snapshot/fetch/process counts.
