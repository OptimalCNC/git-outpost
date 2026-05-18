# Evidence Pack: status-report-core

## Phase And Chunk

- Phase: `phase-3`
- Chunk: `status-report-core`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `status`: read-only status summary and target semantics
  - Architecture 5.9.3 `ops/status.rs`: `StatusReport`, `ConfigProblem`, `run`, `run_with`, `RawMetadata` degraded reporting
  - Architecture 11.5: status integration tests S-01..S-13
  - Roadmap Phase 3: `ops::status` using `RawMetadata` for status reporting
- Roadmap test IDs advanced: S-07, S-08, S-09, S-13

## Changed Files

- `.agents-artifacts/progress/phase-3.md`
- `.agents-artifacts/qa/phase-3/status-report-core.md`
- `.agents-artifacts/reviews/phase-3/status-report-core/evidence-pack.md`
- `crates/core/src/ops/mod.rs`
- `crates/core/src/ops/status.rs`
- `crates/core/tests/status.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/mod.rs`
  - Exports the new `status` op module.
- `ops/status.rs`
  - Adds public `StatusReport` fields matching architecture 5.9.3.
  - Adds status-scoped `ConfigProblem` variants matching architecture 5.9.3.
  - Adds `run(target_path)` and `run_with(target_path, env)`.
  - Discovers the Git work tree from the explicit target path.
  - Reads `RawMetadata` before validated `Metadata`.
  - Returns `NotAnOutpost` when `outpost.managed` is missing or not true.
  - Reports missing `outpost.sourceRepo` and `outpost.remoteName` as `ConfigProblem` entries.
  - Leaves local branch, dirty state, ahead/behind, source-missing, remote mismatch, and registry health behavior for later Phase 3 chunks.
- `tests/status.rs`
  - Adds QA-owned core integration tests for S-07, S-08, S-09, and S-13.
  - Uses `run_with(<target>, &fixture.git_env)` directly; no CLI/E2E/global `-C` tests.

## Patch Excerpts

```rust
pub struct StatusReport {
    pub outpost_path: PathBuf,
    pub source_path: Option<PathBuf>,
    pub source_present: bool,
    pub remote_name: Option<RemoteName>,
    pub current_branch: Option<BranchName>,
    pub outpost_dirty: bool,
    pub source_ahead_behind_upstream: Option<AheadBehind>,
    pub outpost_ahead_behind_source: Option<AheadBehind>,
    pub problems: Vec<ConfigProblem>,
}
```

```rust
pub fn run_with(
    target_path: &Path,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<StatusReport> {
    let outpost_path = discover_work_tree(target_path, env)?;
    let git = invoker_at(&outpost_path, env);
    let raw = RawMetadata::read(&git)?;

    if raw.managed != Some(true) {
        return Err(OutpostError::NotAnOutpost(outpost_path));
    }

    Ok(report_from_raw(outpost_path, raw))
}
```

```rust
#[test]
fn s13_missing_source_repo_config_keeps_degraded_report_available() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.sourceRepo");

    let report = run_with(&outpost, &fixture.git_env).expect("degraded status report");

    assert_eq!(report.source_path, None);
    assert!(!report.source_present);
    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("local")
    );
    assert!(report
        .problems
        .contains(&ConfigProblem::MissingSourceRepoConfig));
}
```

## Tests Added / Updated

- Unit tests added:
  - `ops::status::tests::report_from_raw_records_missing_metadata_problems`

## Integration Tests Added / Updated

- `s07_run_with_accepts_explicit_outpost_target_path` covers S-07.
- `s08_unmanaged_repo_returns_not_an_outpost` covers S-08.
- `s09_missing_source_repo_config_is_reported_as_problem` covers S-09.
- `s13_missing_source_repo_config_keeps_degraded_report_available` covers S-13.

## Docs Added / Updated

- none
- Rationale: product and architecture already document the stable status report shape, `RawMetadata` degraded reporting requirement, and S-07/S-08/S-09/S-13 behavior. This chunk does not introduce a new stable concept beyond those source docs.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --lib ops::status`: pass; 1 status unit test
- `cargo test -p outpost-core --test status`: pass; 4 status integration tests
- `cargo test -p outpost-core`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 4 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 4 status integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 4 status integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `status-report-core` behavior.

## Residual Risks / Handoff Notes

- `current_branch`, `outpost_dirty`, source presence details, ahead/behind, remote mismatch, registry health, and custom remote behavior are intentionally left for later Phase 3 chunks.
- Status remains read-only in this chunk; no fetch/pull/push/stash/checkout/branch update/ref update operations are introduced.
- CLI dispatch/global `-C` and user-facing status formatting remain Phase 5 scope.
