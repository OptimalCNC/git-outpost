# Verdict

Changes required. The main happy path is implemented, and the current Pu-01..Pu-10 tests pass, but focused probes found subtle push-publication gaps around non-fast-forward preconditions and first-publication commit counts.

# Evidence Reviewed

- Commit under review: `03ee3f9 phase-4: add push publication`.
- Source docs:
  - `docs/src/product.md` `push`
  - `docs/src/architecture.md` sections 5.8, 5.9.8, and 11.9
  - `docs/src/roadmap.md`
- Artifacts:
  - `.agents-artifacts/reviews/phase-4/P4-C3-push-publication/evidence-pack.md`
  - `.agents-artifacts/qa/phase-4/P4-C3-push-publication.md`
  - `.agents-artifacts/progress/phase-4.md`
- Code reviewed:
  - `crates/core/src/ops/push.rs`
  - `crates/core/tests/push.rs`
  - supporting helpers in `crates/core/src/safety.rs`, `crates/core/src/source_repo.rs`, `crates/core/src/outpost.rs`, `crates/core/src/error.rs`, and `crates/core/src/ops/add.rs`
- Verification run:
  - `cargo test -p outpost-core --test push`: passed, 10/10 tests.
  - `cargo test -p outpost-core`: passed.
- Scratch probes were added temporarily and reverted before this artifact was written. They checked C-behind-B push, origin-ahead push, linked source worktree `updateInstead`, and first push to an absent origin branch.

# Findings

1. `push` lets a pure C-behind-B case fall through to `GitFailed` after emitting `OutpostPush`.

   `crates/core/src/ops/push.rs:40-63` relies on `safety::check_no_divergence`, but that helper only rejects both-sides-unique histories. If B/source has advanced and C has no unique commits, the check passes; then `reporter.step(OutpostPush, ...)` is emitted and `git push <metadata.remote_name> <branch>:<branch>` fails as a non-fast-forward `GitFailed`.

   Scratch result: after adding a source-side commit after C was created, `run` returned `GitFailed { args: "[\"push\", \"local\", \"main:main\"]", code: 1, stderr: "... non-fast-forward ..." }` and reporter steps were `[OutpostPush]`.

   This is a predictable push precondition and should be surfaced as typed `Divergence { branch: "main" }` before push events or ref updates.

2. `push` can mutate B before discovering that `origin/<branch>` is ahead or divergent.

   `crates/core/src/ops/push.rs:44-81` validates only C<->B before pushing C to B. It does not validate B/C against `origin/<branch>` until the source-to-origin push itself fails. When origin has advanced independently, the C->B push succeeds, B is updated, `SourcePush` is emitted, and the B->origin push returns `GitFailed`.

   Scratch result: with one commit in C and one independent commit already pushed to A/origin, `run` returned `GitFailed { args: "[\"push\", \"origin\", \"main:main\"]", code: 1, stderr: "... fetch first ..." }`; reporter steps were `[OutpostPush, SourcePush]`; B/source moved from its pre-push commit to C's commit while A/origin remained unchanged.

   For a two-hop publication command, origin non-fast-forward is predictable from `ls-remote origin refs/heads/<branch>` and should be rejected as typed `Divergence` before the C->B side effect if the operation is expected to fail as a unit.

3. `source_to_origin` overreports commits on first push to an absent origin branch.

   `crates/core/src/ops/push.rs:141-148` computes the absent-origin case as `git rev-list --count <after>`. That counts the entire reachable history from the branch tip, not just commits newly published by this push or newly introduced relative to the source branch baseline.

   Scratch result: B had `feature/new` at the initial commit, A/origin had no `feature/new` ref, C added one commit on `feature/new`, and `gop push` reported `source_to_origin: Pushed { commits: 2 }`. The expected operation delta is one commit; the extra count is the already-existing initial commit reachable from A's `main`.

   This is specifically missing from the existing count assertions, which only cover one-commit pushes to an origin branch that already exists.

# Missing Evidence

- No test covers C behind B/source without C having unique commits. Existing Pu-04 covers only true divergence where both C and B have unique commits.
- No test covers A/origin ahead or divergent before `push`, so the suite does not pin typed error behavior or no-side-effect ordering for the second hop.
- No test covers first publication of a B-existing branch to an absent `origin/<branch>` and its `PushReport` commit count.
- Push-specific tests do not prove global `receive.denyCurrentBranch=refuse` is ignored, though the implementation uses `read_optional_config(... --local ...)`.
- Push-specific tests do not cover dirty linked source worktrees, though a clean linked source worktree with local `updateInstead` succeeded in scratch verification.

# Required Changes

- In `crates/core/src/ops/push.rs:40-63`, add a push-specific fast-forward precondition for C->B. It should reject both divergence and pure C-behind-B as `OutpostError::Divergence { branch }` before `OutpostPush` is emitted.
- In `crates/core/src/ops/push.rs:46-81`, preflight `origin/<branch>` before mutating B. If origin exists and the publication would be non-fast-forward, return typed `Divergence` before `OutpostPush`.
- In `crates/core/src/ops/push.rs:141-148`, fix absent-origin commit counting so first publication of a branch reports the actual operation delta, not the full reachable history.
- Add focused integration coverage in `crates/core/tests/push.rs` for the three cases above, with assertions on error variants, reporter events, B/A ref movement, and commit counts.

# Nits

None.
