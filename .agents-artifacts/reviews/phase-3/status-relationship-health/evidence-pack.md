# Evidence Pack: status-relationship-health

## Phase And Chunk

- Phase: `phase-3`
- Chunk: `status-relationship-health`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`
- Relevant source sections:
  - Product `status`: read-only ahead/behind and health reporting based on existing local refs
  - Architecture 5.9.3 `ops/status.rs`: `StatusReport`, `ConfigProblem`, `run`, `run_with`, `RawMetadata` degraded reporting
  - Architecture 11.5: status integration tests S-01..S-13
  - Roadmap Phase 3: `ops::status` using `RawMetadata` for status reporting
- Roadmap test IDs advanced: S-05, S-06, S-11, S-12
- Additional status health variants completed: `NotInRegistry`, `PushWouldFail`

## Changed Files

- `.agents-artifacts/progress/phase-3.md`
- `.agents-artifacts/qa/phase-3/status-relationship-health.md`
- `.agents-artifacts/reviews/phase-3/status-relationship-health/evidence-pack.md`
- `crates/core/src/ops/status.rs`
- `crates/core/tests/status.rs`

## Moves / Renames

- none

## Diff Summary

- `ops/status.rs`
  - Computes outpost-vs-source ahead/behind with `git rev-list --left-right --count` over existing local and remote-tracking refs only.
  - Computes source-vs-upstream ahead/behind from B's existing local refs and remote-tracking refs only.
  - Uses metadata `remote_name` for remote URL checks and outpost remote-tracking refs.
  - Reports `LocalRemoteMismatch` when `remote.<remote_name>.url` canonicalizes differently from configured `outpost.sourceRepo`.
  - Reports `NotInRegistry` when the source registry does not contain the outpost path.
  - Reports `PushWouldFail` when B's `receive.denyCurrentBranch` is not `updateInstead` and the current branch is checked out in B.
  - Leaves degraded behavior intact for missing `sourceRepo`, missing `remoteName`, missing source, detached HEAD, and missing tracking refs.
  - Does not call `fetch`, `pull`, `push`, `stash`, `checkout`, `branch`, `update-ref`, or write any config/registry/ref state.
- `tests/status.rs`
  - Adds QA-owned coverage for S-05, S-06, S-11, S-12, `NotInRegistry`, and `PushWouldFail`.
  - Verifies status does not update existing remote-tracking refs by comparing ref OIDs before and after status.

## Patch Excerpts

```rust
if let (Some(source_path), Some(remote_name)) = (source_path.as_ref(), remote_name.as_ref()) {
    if source_present {
        check_local_remote(git, &outpost_path, source_path, remote_name, &mut problems)?;
        let source = SourceRepo::at_with(source_path, env)?;
        check_registry(&source, &outpost_path, &mut problems)?;
        if let Some(branch) = current_branch.as_ref() {
            outpost_ahead_behind_source =
                ahead_behind_outpost_source(git, branch, remote_name, &mut problems)?;
            source_ahead_behind_upstream =
                ahead_behind_source_upstream(source_path, branch, env, &mut problems)?;
            check_push_would_fail(&source, branch, &mut problems)?;
        }
    }
}
```

```rust
fn ahead_behind_existing_refs(
    git: &GitInvoker,
    local_ref: &str,
    remote_ref: &str,
) -> OutpostResult<Option<AheadBehind>> {
    if !ref_exists(git, local_ref)? || !ref_exists(git, remote_ref)? {
        return Ok(None);
    }

    let range = format!("{local_ref}...{remote_ref}");
    let output = git.run_capture(["rev-list", "--left-right", "--count", &range])?;
    parse_ahead_behind(git.cwd(), &output).map(Some)
}
```

```rust
#[test]
fn s05_run_with_reports_outpost_ahead_behind_source_from_existing_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source_seen = fixture
        .commit_in_source("source seen by outpost")
        .expect("source seen commit");
    update_remote_tracking_ref(&fixture, &outpost, "local", "main", &source_seen);
    fixture
        .commit_in_outpost(&outpost, "outpost commit")
        .expect("outpost commit");
    fixture
        .commit_in_source("source not fetched by status")
        .expect("source unseen commit");
    let remote_ref_before = rev_parse(&fixture, &outpost, "refs/remotes/local/main");

    let report = run_with(&outpost, &fixture.git_env).expect("status report");

    assert_eq!(
        report.outpost_ahead_behind_source,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        rev_parse(&fixture, &outpost, "refs/remotes/local/main"),
        remote_ref_before
    );
}
```

## Tests Added / Updated

- Unit tests added/updated: none

## Integration Tests Added / Updated

- `s05_run_with_reports_outpost_ahead_behind_source_from_existing_refs` covers S-05.
- `s06_run_with_reports_source_ahead_behind_upstream_from_existing_refs` covers S-06.
- `s11_run_with_flags_local_remote_mismatch` covers S-11.
- `s12_run_with_uses_metadata_remote_name_for_custom_remote` covers S-12.
- `run_with_flags_not_in_registry_when_outpost_entry_is_missing` covers architecture status health variant `NotInRegistry`.
- `run_with_flags_push_would_fail_when_source_refuses_checked_out_branch_update` covers architecture status health variant `PushWouldFail`.

## Docs Added / Updated

- none
- Rationale: product and architecture already document status relationship fields, health line problems, read-only behavior, and all `ConfigProblem` variants. This chunk implements those stable concepts without changing their public meaning.

## Verification

- `cargo fmt --check`: pass
- `cargo check -p outpost-core`: pass
- `cargo test -p outpost-core --lib ops::status`: pass; 1 status unit test
- `cargo test -p outpost-core --test status`: pass; 15 status integration tests
- `cargo test -p outpost-core`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `cargo test -p outpost-core --tests`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 15 status integration tests, 1 fixture smoke test
- `cargo test --workspace`: pass; 46 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 9 prune integration tests, 11 remove integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `git diff --check`: pass

## Verification Not Run

- none for this chunk

## Protected-Path Exceptions

- none

## Architecture Deviations

- none for the claimed `status-relationship-health` behavior.

## Residual Risks / Handoff Notes

- Status relationship reporting is intentionally based on existing local refs. It does not fetch, so stale remote-tracking refs can make status stale by design; this matches the product requirement that status is read-only and based on existing local refs.
- Test fixture setup uses `git fetch <remote> <refspec>` to create local remote-tracking refs before invoking status. Production status itself does not fetch or update refs.
- CLI dispatch/global `-C` and user-facing status formatting remain Phase 5 scope.
