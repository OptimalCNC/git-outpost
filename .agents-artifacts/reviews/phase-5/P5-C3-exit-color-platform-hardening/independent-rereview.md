# P5-C3 Independent Re-review: Exit / Color / Platform Hardening

## Verdict

Pass.

E-08 now meets the architecture/test intent. The fix keeps the exhaustive
`OutpostError::exit_code()` table and adds reachable CLI broken-state coverage
with focused stderr assertions for the variants that can be surfaced through a
stable CLI fixture. The remaining table-only case, `GitTerminatedBySignal`, is
explicitly documented as an exception in the QA/evidence notes because forcing a
child `git` process to die by signal through a black-box CLI test would be
brittle.

## Findings

None.

## Re-review Notes

- E-08: `crates/cli/tests/flags.rs::e_08_outpost_errors_map_to_documented_exit_codes`
  still enumerates every current `OutpostError` variant, including
  `GitTerminatedBySignal`, `GitFailed` lower-bound clamping, and upper-bound
  clamping.
- E-08: `crates/cli/tests/flags.rs::e_08_cli_errors_return_documented_exit_codes`
  now exercises CLI broken states for the reachable variants: context errors,
  destination errors, dirty/unpushed/diverged histories, missing or invalid
  refs, checked-out-branch push rejection, ambiguous branch creation, locks,
  registry failures, bad registry/metadata, a real `GitFailed`, and `IoAt`.
  `assert_failure_code_contains` checks both the exit code and a focused stderr
  substring, so same-bucket routing mistakes are less likely to pass.
- E-08: `GitFailed` no longer maps negative process status values to success;
  `crates/core/src/error.rs` now clamps to `1..=125`, and the CLI/core tests
  assert that behavior.
- E-07: not reopened. The review-fix commits did not modify the copied-outpost
  E2E implementation, and the full CLI integration suite still passes it.
- E-09: only rechecked because `crates/cli/tests/flags.rs` was edited nearby.
  The no-color test remains intact and passing.

## Open Questions

- `docs/src/architecture.md` still shows the older illustrative
  `GitFailed { code, .. } => (*code).clamp(0, 125) as u8` snippet. The code,
  tests, QA note, and evidence pack now use the safer `1..=125` behavior. I do
  not consider this blocking for the requested re-review because the fix
  explicitly addressed a review-identified bug, but the architecture snippet
  should be aligned in a follow-up docs cleanup if that section is treated as
  authoritative source text.
- Local verification is Linux-only; cross-platform confidence still depends on
  CI runners for Windows/macOS behavior.

## Verification

- Inspected prior artifact:
  `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/independent-review.md`.
- Inspected review-fix commits:
  `c93bb8c phase-5: fix exit color review findings` and
  `671c7a4 phase-5: record exit color review fixes`.
- Inspected key files: `crates/cli/tests/flags.rs`,
  `crates/core/src/error.rs`,
  `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`,
  `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`,
  and `.agents-artifacts/progress/phase-5.md`.
- Ran `cargo test -p git-outpost --test flags`: pass, 8 tests.
- Ran `cargo test -p git-outpost --tests`: pass, 9 E2E tests, 8 flags tests,
  4 help tests.
- Ran `cargo test -p outpost-core --lib exit_code`: pass, 1 selected test.
- Ran `cargo test --workspace`: pass; CLI integration, core unit/integration,
  and doctest targets all passed.
- Cargo emitted the existing duplicate `src/main.rs` bin-target warning for
  `git-outpost`/`gop`; no new warning was introduced by this re-review.
- Existing unrelated local changes were present before review and left
  untouched: `crates/cli/Cargo.toml`, `.github/`, and `README.md`.
