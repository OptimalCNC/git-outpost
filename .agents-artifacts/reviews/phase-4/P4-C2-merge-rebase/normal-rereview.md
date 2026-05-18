# Normal Re-review: P4-C2-merge-rebase

## Verdict

Pass. Current HEAD `6d5ee16` fixes the prior ambiguity finding for `P4-C2-merge-rebase` without observed regressions in the focused merge/rebase behavior.

## Evidence Reviewed

- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`.
- Phase artifacts: `.agents-artifacts/progress/phase-4.md`, `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/evidence-pack.md`, `.agents-artifacts/qa/phase-4/P4-C2-merge-rebase.md`.
- Prior reviews in `.agents-artifacts/reviews/phase-4/P4-C2-merge-rebase/`, especially the independent review finding that final merge/rebase operands were ambiguous short refs.
- Commit reviewed: `6d5ee16 phase-4: fix merge rebase review findings`.
- Focus files reviewed:
  - `crates/core/src/ops/merge.rs`
  - `crates/core/src/ops/rebase.rs`
  - `crates/core/tests/merge.rs`
  - `crates/core/tests/rebase.rs`
- Supporting implementation checked: `crates/core/src/refname.rs`, `crates/core/src/git.rs`, `crates/core/src/outpost.rs`, `crates/core/tests/common/fixture.rs`.

## Correctness Findings

- The prior ambiguity issue is fixed. `merge::run` now uses the `String` returned by `fetch_source_ref` and invokes `git merge refs/remotes/<remote>/<branch>` (`crates/core/src/ops/merge.rs:33`-`34`). `rebase::run` does the same for `git rebase refs/remotes/<remote>/<branch>` (`crates/core/src/ops/rebase.rs:33`-`34`). This avoids Git resolving a short `local/main` operand to `refs/heads/local/main`.
- The fetch target remains the expected remote-tracking ref. Both `fetch_source_ref` helpers build `refs/remotes/{remote}/{branch}`, fetch `<branch>:refs/remotes/<remote>/<branch>`, and return that full ref for the final operation.
- Existing precondition ordering is preserved. Attached-branch validation still happens before remote validation, reporter events, and fetch; mismatched source remotes are still rejected before reporter/fetch side effects.
- Custom remote behavior remains intact because both the fetch refspec and final full ref are built from the validated `SourceRemoteRef.remote`, not hard-coded to `local`.
- The change remains scoped to core merge/rebase behavior and evidence/test artifacts. I found no source-origin refresh added to merge/rebase, no push behavior, and no Phase 5 CLI/E2E behavior.

## Test/Verification Findings

- `cargo fmt --check`: pass.
- `cargo test -p outpost-core --test merge`: pass, 6 tests.
- `cargo test -p outpost-core --test rebase`: pass, 6 tests.
- `git diff --check HEAD^..HEAD`: pass.
- Regression coverage now exists for both operations:
  - `merge_uses_full_remote_tracking_ref_when_local_branch_name_collides` creates `refs/heads/local/main`, runs `merge local/main`, and asserts the fetched source commit is an ancestor of `HEAD`.
  - `rebase_uses_full_remote_tracking_ref_when_local_branch_name_collides` creates `refs/heads/local/main`, runs `rebase local/main`, and asserts `HEAD^` is the fetched source commit.

## Required Changes

None.

## Nits

None.
