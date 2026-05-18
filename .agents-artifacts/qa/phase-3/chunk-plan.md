# Chunk Plan: Phase 3

## Verdict

- `ready_with_cautions`

## Recommended Chunks

| Chunk | Owner | Scope | S IDs | Likely Files | Review Focus |
| --- | --- | --- | --- | --- | --- |
| `status-report-core` | Dev | Add `ops::status` public report types and basic `run`/`run_with` flow using `RawMetadata`; support explicit target path; return `NotAnOutpost` for unmanaged repos; report missing `sourceRepo`/`remoteName` as problems. | S-07, S-08, S-09, S-13 | `crates/core/src/ops/status.rs`, `crates/core/src/ops/mod.rs`, possibly `crates/core/src/lib.rs` | `RawMetadata` is read before validation; degraded managed outposts do not crash; no CLI/global `-C` behavior added. |
| `status-local-state` | Dev | Populate source path, remote name, current branch/detached state, dirty state, and missing-source state without mutating refs. | S-01, S-02, S-03, S-04, S-10 | `crates/core/src/ops/status.rs`, colocated unit/helper tests if useful | Detached `HEAD` becomes `current_branch=None`; dirty check includes untracked files; missing source reports `source_present=false` plus problem. |
| `status-relationship-health` | Dev | Add read-only ahead/behind reporting and relationship health checks: outpost vs source, source vs upstream, remote mismatch, custom remote name. | S-05, S-06, S-11, S-12 | `crates/core/src/ops/status.rs`, narrow helper additions in existing core modules only if required | No fetch/pull/push/ref updates; do not reuse helpers that fetch; path canonicalization for `LocalRemoteMismatch`; all git invocations use configured remote name. |
| `status-integration-qa` | QA | Add/own integration coverage for all S-01..S-13 in `crates/core/tests/status.rs`, using existing ABC fixture patterns. | S-01..S-13 | `crates/core/tests/status.rs`, `crates/core/tests/common/*` only if test helper gaps require it | Tests prove read-only behavior where practical, RawMetadata degraded reporting, detached HEAD, deleted source, mismatch canonicalization, and custom remote behavior. |
| `phase-3-verification` | Dev + QA | Run required verification and collect evidence pack after implementation and QA tests land. | S-01..S-13 | no production scope expected | Required commands: `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`. |

## Dependencies / Order

1. Accept this chunk plan before implementation.
2. `status-report-core` first, because later chunks depend on report types and degraded metadata flow.
3. `status-local-state` second.
4. `status-relationship-health` third.
5. `status-integration-qa` may start after report types exist, then fill gaps as later chunks land.
6. `phase-3-verification` last.

## Docs Expectations

- No product, architecture, or roadmap edits expected for Phase 3 unless implementation discovers a real spec ambiguity.
- No unrelated documentation cleanup.
- Phase progress/review/QA artifacts should record chunk evidence, cautions handled, and verification results.

## Risks / Cautions

- `status` must remain read-only: no fetch, pull, push, stash, checkout, branch updates, or ref updates.
- Avoid `Outpost::ahead_behind_source()` if it fetches; add status-specific read-only logic if needed.
- Detached `HEAD` is report data, not a fatal branch error.
- Missing `outpost.sourceRepo` must be reported via `RawMetadata` degraded mode.
- `LocalRemoteMismatch` needs careful canonicalization because remote URLs may be path strings.
- Do not implement Phase 4 sync behavior or Phase 5 CLI/global `-C`/E2E behavior.

## Required Human Decisions

- none
