# P5-C2 Dispatch E2E Independent Re-review

## Verdict

Pass.

The prior independent review blockers are resolved in the current P5-C2 state:

- The `CliError` architecture mismatch is resolved. `rg -n "CliError" crates/cli crates/core docs/src/architecture.md` returns no matches. CLI results are now `OutpostResult`, and `exit::report` accepts `OutpostError` directly.
- The new dispatch-context errors are first-class `OutpostError` variants, documented in the architecture, mapped by `OutpostError::exit_code()`, and covered by both core unit tests and CLI wrong-context tests.
- Matrix-edge coverage now includes list-from-outpost, lock/unlock from source and outpost contexts, move, prune, representative wrong-context failures, and `add` with global `-C`.
- Progress/evidence were updated for the review fixes and the P5-C3 boundary remains explicit.

## Remaining Blockers

None.

## Nits

- No action required for P5-C2. P5-C3 should still treat E-08 as open at the CLI edge; the new core `exit_code_maps_each_variant` unit test is useful evidence, but it is not a replacement for the planned exhaustive CLI exit-code matrix.
- `status` still intentionally takes the core degraded-status path instead of `require_outpost`. That preserves the P5-C3 E-07 path and is not a P5-C2 blocker.

## Evidence

Reviewed commits:

- `6f68b95 phase-5: add dispatch e2e`
- `91f0052 phase-5: record dispatch e2e commit`
- `56eadac phase-5: fix dispatch e2e review findings`
- `fe717b6 phase-5: record dispatch review-fix commit`

Error architecture:

- `crates/core/src/error.rs:18-26` defines `OutpostError::WrongContext` and `OutpostError::MissingOutpostPath`.
- `crates/core/src/error.rs:102-130` maps both variants to exit code 2 through `OutpostError::exit_code()`.
- `crates/core/src/error.rs:158-170` covers their display strings.
- `crates/core/src/error.rs:298-317` covers their exit-code mapping.
- `crates/cli/src/exit.rs:3-9` aliases `CliResult<T>` to `OutpostResult<T>` and reports `OutpostError` directly.
- `crates/cli/src/main.rs:173-192` emits `WrongContext` for source-only and outpost-only dispatch failures.
- `crates/cli/src/main.rs:202-222` emits `MissingOutpostPath` when source-context lock/unlock omit the outpost path.
- `docs/src/architecture.md:181-185` documents the new variants in the error model.
- `docs/src/architecture.md:1694-1722` documents the exit-code mapping and the single `OutpostError` table.

Matrix coverage:

- `crates/cli/tests/e2e.rs:53-104` covers list from outpost, lock/unlock from outpost with omitted path, lock/unlock from source with explicit relative path, move from source, and prune from source.
- `crates/cli/tests/e2e.rs:106-139` covers representative wrong-context failures for source-only, outpost-only, and missing outpost path cases, all with exit code 2.
- `crates/cli/tests/flags.rs:17-64` covers global `-C`, including `add` with a relative destination resolved against the effective source cwd.
- `crates/cli/tests/e2e.rs:19-51` still covers the add/status/push/list/remove lifecycle.

Progress and scope:

- `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md:12-17` records the dispatch implementation and review-fix error-model change.
- `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md:34-45` records the new matrix tests.
- `.agents-artifacts/progress/phase-5.md:184-197` records P5-C2 files, tests, docs, and review fixes.
- `.agents-artifacts/progress/phase-5.md:248-251` records the adopted independent-review fixes.
- `.agents-artifacts/progress/phase-5.md:127`, `.agents-artifacts/progress/phase-5.md:140`, and `.agents-artifacts/progress/phase-5.md:276` keep E-07, E-08, and E-09 in P5-C3 scope.
- `.agents-artifacts/qa/phase-5/P5-C2-dispatch-e2e.md:23-25` keeps copied-outpost degradation, exhaustive CLI exit-code behavior, and color stripping as remaining Phase 5 QA.

Local verification:

- `cargo test -p outpost-core error::tests --lib`: pass; 2 tests.
- `cargo test -p git-outpost --tests`: pass; 8 e2e tests, 5 flags tests, 4 help tests.
- `cargo test --workspace`: pass; full CLI and core workspace tests.
- `git diff --check`: pass.
- Cargo repeated the existing warning that `crates/cli/src/main.rs` is present in both bin targets, which matches the current Phase 5 binary layout and was not introduced by this review.
