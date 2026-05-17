**Verdict**: needs changes

**Evidence Reviewed**: commit `85119de`, `git status --short`, `git show --stat`, `git show` diff for `crates/core/src/lib.rs` and `crates/core/src/safety.rs`, evidence pack, approved scope review, progress log, `docs/src/product.md`, `docs/src/architecture.md` §§5.8, 5.9.1, 11.1, `docs/src/roadmap.md` Phase 1 row, Documentation Policy, and reviewer-run `cargo test -p outpost-core safety::tests::`.

**Requirement Reasoning**: U-10 is implemented: `check_clean` uses `git status --porcelain=v1 --untracked-files=normal` and returns `DirtyTree` on non-empty output, matching architecture §5.8.

U-13 is implemented: `check_path_is_managed_outpost_of` canonicalizes the candidate, opens it via `source.outpost_at`, resolves metadata source, and compares `work_tree` to the current source.

Deferred `check_no_unpushed` and divergence helpers are acceptable for this chunk because the evidence pack and progress log scope this chunk to U-10/U-13 plus destination gating.

`check_destination_clean` does not fully satisfy the documented add/move destination safety behavior because relative destinations can be resolved against the process cwd before being anchored under `parent`.

**Test Reasoning**: The 12 safety tests pass. They prove staged, unstaged, and untracked dirty detection; clean-tree acceptance; managed-outpost rejection for no repo, `managed=false`, and wrong source; matching-source acceptance; and several destination cases.

They do not prove that relative destination paths are resolved against `parent` before existence/canonicalization. Current destination tests cover absolute existing paths and a missing relative sibling, but not a relative destination whose name exists in the process cwd or under `parent`.

**Docs Reasoning**: No product/architecture/roadmap doc changes are required for the intended safety contracts. Architecture §5.8 already documents `check_clean`, `check_path_is_managed_outpost_of`, and `check_destination_clean`; the Documentation Policy is satisfied once implementation matches those contracts.

**Verification Reasoning**: Supplied evidence records `cargo fmt --check`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`, and `git diff --check` as passing. I reran `cargo test -p outpost-core safety::tests::`; it passed with 12 tests. The verification gap is the missing relative-destination regression described below.

**Findings**:
Blocking. Evidence: architecture says `check_destination_clean(parent, dest)` ensures the add/move target is absent or empty and not inside another Git work tree, using `git -C <parent> rev-parse --show-toplevel` for containment checks; product says add destinations must be absent or empty. Code checks `dest.exists()` before joining a relative `dest` to canonicalized `parent`, while later applying existence and containment checks to that resolved path. Issue: if a relative destination also exists in the process cwd, the helper can inspect the cwd path instead of `parent/dest`, causing false refusal or allowing the actual target under `parent` to go unchecked. Why it matters: this is the destination safety gate for later `add` and `move`; safety behavior must not depend on the test runner or caller process cwd.

**Missing Evidence**: Test evidence for `check_destination_clean(parent, relative_dest)` where the relative path must be resolved under `parent` before any `exists` or canonicalization check.

**Required Changes**: Fix `resolve_destination` so relative destinations are joined to canonicalized `parent` before checking existence/canonicalizing. Add a regression test proving relative destination resolution is independent of process cwd and catches an existing/non-empty `parent/relative_dest`.

**Notes**: Current workspace has staged artifact-only deltas for the scope review/progress log; I did not treat those as implementation changes.
