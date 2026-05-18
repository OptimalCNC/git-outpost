# Normal Review: status-report-core

- **Verdict**: `approved`
- **Evidence Reviewed**: docs `product` status section, architecture `RawMetadata`/`ops::status`/S-07..S-13 sections, roadmap Phase 3 row; progress log, QA note, evidence pack, scope review; implementation files `ops/mod.rs`, `ops/status.rs`, `tests/status.rs`; commit file lists for `252e2f1` and `a33b050`; commands listed under Verification Reasoning.
- **Correctness Findings**: none
- **Architecture / Scope Reasoning**: Fits Phase 3. `status::run_with` discovers the work tree from the explicit target path, reads `RawMetadata` directly, returns `NotAnOutpost` when `managed != Some(true)`, and builds degraded reports without `Metadata::from_raw`. The touched status code only runs `rev-parse --show-toplevel` plus local metadata reads; no fetch/pull/push/stash/ref update, CLI, or global `-C` behavior was introduced.
- **Test Reasoning**: S-07 through S-13 claimed slice is covered by focused integration tests in `crates/core/tests/status.rs`: explicit target path, unmanaged repo rejection, missing `sourceRepo` problem reporting, and degraded report availability. Remaining S-01..S-06 and S-10..S-12 are correctly left for later Phase 3 chunks.
- **Docs Reasoning**: No docs change required. Existing product docs already define status as read-only, architecture defines `StatusReport`, `ConfigProblem`, `RawMetadata` degraded reporting, and roadmap assigns `ops::status` to Phase 3.
- **Verification Reasoning**: Ran and passed: `cargo fmt --check`, `cargo check -p outpost-core`, `cargo test -p outpost-core --lib ops::status`, `cargo test -p outpost-core --test status`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, `git diff --check`. Workspace status was clean.
- **Findings**: none
- **Missing Evidence**: none
- **Required Changes**: none
- **Nits**: none
