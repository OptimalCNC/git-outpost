# Evidence Pack: status-local-state

## Phase And Chunk

- Phase: `phase-3`
- Chunk: `status-local-state`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `status`: reports source path, source remote name, current branch or detached state, clean/dirty state, and source presence problems
  - Architecture 5.9.3 `ops/status.rs`: `StatusReport`, `ConfigProblem`, `run`, `run_with`, `RawMetadata` degraded reporting
  - Architecture 11.5: status integration tests S-01..S-13
  - Roadmap Phase 3: `ops::status` using `RawMetadata` for status reporting
- Roadmap test IDs advanced: S-01, S-02, S-03, S-04, S-10

## Changed Files

- `.agents-artifacts/progress/phase-3.md`
- `.agents-artifacts/qa/phase-3/status-local-state.md`
- `.agents-artifacts/reviews/phase-3/status-local-state/evidence-pack.md`
- `crates/core/src/ops/status.rs`
- `crates/core/tests/status.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/status.rs`
  - Canonicalizes configured existing `source_path`.
  - Preserves a stable canonical missing path when configured source path no longer exists.
  - Reports `source_present=false` and `ConfigProblem::SourceMissing(path)` for missing configured source repositories.
  - Reads current branch with `git symbolic-ref --quiet --short HEAD` and maps detached HEAD exit code 1 to `current_branch=None`.
  - Reads dirty state with the existing `source_repo::is_dirty` helper, which uses `git status --porcelain=v1 --untracked-files=normal`.
  - Keeps ahead/behind and relationship-health fields as `None` for the later Phase 3 chunk.
- `tests/status.rs`
  - Adds QA-owned core integration coverage for S-01, S-02, S-03, S-04, and S-10.

## Patch Excerpts

```rust
let source_path = match raw.source_repo {
    Some(path) => Some(canonicalize_existing_or_missing(&path)?),
    None => {
        problems.push(ConfigProblem::MissingSourceRepoConfig);
        None
    }
};

let source_present = source_path.as_ref().is_some_and(|path| path.exists());
if let Some(path) = source_path.as_ref().filter(|_| !source_present) {
    problems.push(ConfigProblem::SourceMissing(path.clone()));
}
```

```rust
fn current_branch_or_detached(git: &crate::GitInvoker) -> OutpostResult<Option<BranchName>> {
    match git.run_capture(["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(branch) => BranchName::parse(branch).map(Some),
        Err(OutpostError::GitFailed { code: 1, .. }) => Ok(None),
        Err(err) => Err(err),
    }
}
```

```rust
#[test]
fn s04_run_with_reports_dirty_state_for_untracked_files() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let clean_report = run_with(&outpost, &fixture.git_env).expect("clean status report");
    assert!(!clean_report.outpost_dirty);

    fs::write(outpost.join("untracked.txt"), "new").expect("write untracked file");

    let dirty_report = run_with(&outpost, &fixture.git_env).expect("dirty status report");
    assert!(dirty_report.outpost_dirty);
}
```

## Tests Added / Updated

- Unit tests updated:
  - `ops::status::tests::report_from_raw_records_missing_metadata_problems` now uses a temporary Git repository because local-state reporting reads branch and dirty state.

## Integration Tests Added / Updated

- `s01_run_with_from_inside_outpost_reports_canonical_source_path` covers S-01.
- `s02_run_with_reports_local_remote_name` covers S-02.
- `s03_run_with_reports_current_branch_and_detached_head` covers S-03.
- `s04_run_with_reports_dirty_state_for_untracked_files` covers S-04.
- `s10_run_with_reports_missing_source_problem` covers S-10.

## Docs Added / Updated

- none
- Rationale: product and architecture already document the stable status report fields, detached HEAD behavior, dirty state, source-missing problem, and S-01/S-02/S-03/S-04/S-10 behavior. This chunk does not introduce a new stable concept beyond those source docs.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --lib ops::status`: pass; 1 status unit test
- `cargo test -p outpost-core --test status`: pass; 9 status integration tests
- `cargo test -p outpost-core`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 9 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 9 status integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 9 status integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `status-local-state` behavior.

## Residual Risks / Handoff Notes

- Ahead/behind, upstream/source relationship health, local remote mismatch, registry health, push-would-fail, and custom remote tracking behavior remain for `status-relationship-health`.
- Status remains read-only in this chunk; no fetch/pull/push/stash/branch update/ref update operations are introduced.
- CLI dispatch/global `-C` and user-facing status formatting remain Phase 5 scope.
