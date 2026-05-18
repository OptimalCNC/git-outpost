# P5-C3 Independent Review: Exit / Color / Platform Hardening

## Verdict

Changes requested.

E-07 is genuinely satisfied by the current CLI integration test, and E-09 is satisfied for the current no-coloring implementation. E-08 is not yet satisfied against the architecture's stricter wording because the exhaustive part is a direct `OutpostError::exit_code()` table, while the CLI edge is only covered by representative exit-code buckets.

## Findings

### Required: E-08 lacks per-variant broken-state / CLI-edge traceability

`docs/src/architecture.md:2048` requires every §5.1 `OutpostError` variant to map to its §9 exit code "using one crafted broken state per variant for traceability." The implementation added in `47d10fd` covers all variants by constructing enum values directly in `crates/cli/tests/flags.rs:19` and asserting `error.exit_code()` through `crates/cli/tests/flags.rs:169`. That proves the mapping method, but it does not exercise real CLI dispatch, `main()` return handling, `exit::report`, or real broken repository/process states for most variants.

The black-box CLI test starts at `crates/cli/tests/flags.rs:172` and covers only representative buckets: exit 2 at `crates/cli/tests/flags.rs:177` and `crates/cli/tests/flags.rs:180`, exit 3 at `crates/cli/tests/flags.rs:183` and `crates/cli/tests/flags.rs:198`, exit 4 at `crates/cli/tests/flags.rs:220`, exit 5 at `crates/cli/tests/flags.rs:223`, and exit 6 at `crates/cli/tests/flags.rs:237`. It does not provide one crafted CLI/broken-state path per variant, and does not black-box the Git exit-code clamping, `GitTerminatedBySignal`, `IoAt`, `BadMetadata`, `RegistryEntryNotManaged`, `RegistryEntryNotFound`, `DirtyTree`, `UnpushedCommits`, `Divergence`, `NoUpstreamTracking`, `UpstreamNotABranch`, or `AmbiguousBranchCreation` cases.

The artifacts also acknowledge the weaker strategy: `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md:6` says black-box coverage is only for buckets 2 through 6, and `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md:13` says the exhaustive variant creation stays table-driven. `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:47` and `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:48` record deviations from the planned `strip-ansi-escapes` and per-variant black-box strategy, but the source docs were not updated to accept that narrower E-08 interpretation.

Requested change: either add traceable broken-state coverage for each current variant at the CLI edge where practical, with explicit documented exceptions for process-control-only cases, or update the source/QA acceptance criteria to state that E-08 is satisfied by exhaustive `OutpostError::exit_code()` coverage plus representative CLI bucket smoke tests.

## Satisfied Scope

E-07 copied-outpost independence is covered well. The product requires outposts to be normal self-contained clones with their own `.git` directory and no shared object-store dependency (`docs/src/product.md:6`, `docs/src/product.md:22`, `docs/src/product.md:33`). The add implementation clones with `--no-shared` in `crates/core/src/ops/add.rs:43`, and the earlier core add test checks a real `.git` directory, no `objects/info/alternates`, and `--no-shared` argv in `crates/core/tests/add.rs:278` through `crates/core/tests/add.rs:286`. P5-C3 then adds the required copy/delete test in `crates/cli/tests/e2e.rs:176`: it copies C with Rust filesystem APIs, removes B at `crates/cli/tests/e2e.rs:184`, verifies `git status`, `git log`, `git diff HEAD~1`, and `git checkout -b new-branch` at `crates/cli/tests/e2e.rs:186` through `crates/cli/tests/e2e.rs:193`, and verifies degraded `gop status` output at `crates/cli/tests/e2e.rs:195` through `crates/cli/tests/e2e.rs:209`.

E-09 is satisfied for the current implementation. The product requires `--no-color` and `NO_COLOR` support at `docs/src/product.md:454` through `docs/src/product.md:456`, and the architecture test row requires no ANSI escapes for both forms at `docs/src/architecture.md:2049`. The test runs `gop --no-color status` and `NO_COLOR=1 gop status` in `crates/cli/tests/flags.rs:243` through `crates/cli/tests/flags.rs:265`, then rejects any ESC byte on both stdout and stderr in `crates/cli/tests/flags.rs:386` through `crates/cli/tests/flags.rs:400`. This is stricter than matching CSI-only escapes. The caveat is that `Cli::no_color` is parsed but not otherwise referenced (`crates/cli/src/cli.rs:27` through `crates/cli/src/cli.rs:29`), and current output is uncolored by construction in `crates/cli/src/output.rs:32` through `crates/cli/src/output.rs:71`; so the test proves absence of ANSI today, not an actively plumbed color policy.

Cross-platform assumptions are mostly reasonable but not fully proven locally. The recursive copy helper avoids shell tools and has Unix/Windows symlink branches in `crates/cli/tests/common/mod.rs:240` through `crates/cli/tests/common/mod.rs:282`; the fixture uses an empty config file rather than `/dev/null` in `crates/cli/tests/common/mod.rs:21` through `crates/cli/tests/common/mod.rs:30`; binary lookup uses `env::consts::EXE_EXTENSION` in `crates/cli/tests/common/mod.rs:257` through `crates/cli/tests/common/mod.rs:266`. Local verification was Linux only, so Windows/macOS confidence still depends on CI runners as anticipated by `.agents-artifacts/qa/phase-5/chunk-plan.md:74`.

## Open Questions

- Should E-08 remain as written in `docs/src/architecture.md:2048`, or should the project explicitly accept exhaustive direct mapping plus representative CLI bucket coverage?
- Is the MVP intentionally uncolored everywhere? If yes, E-09's no-op color policy should be considered acceptable; if color is planned soon, `--no-color` and `NO_COLOR` need an actual output/reporter policy path.
- Are Linux, macOS, and Windows CI jobs already running this Phase 5 suite? I only verified on the local Linux environment.

## Verification Run / Inspected

- Inspected source docs: `docs/src/product.md`, `docs/src/architecture.md`, `docs/src/roadmap.md`.
- Inspected commits: `47d10fd phase-5: harden exit color platform behavior`; `858f61e phase-5: record exit color hardening commit`.
- Inspected artifacts: `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`, `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`, `.agents-artifacts/progress/phase-5.md`.
- Inspected implementation/tests: `crates/cli/src/cli.rs`, `crates/cli/src/main.rs`, `crates/cli/src/exit.rs`, `crates/cli/src/output.rs`, `crates/cli/tests/common/mod.rs`, `crates/cli/tests/e2e.rs`, `crates/cli/tests/flags.rs`, `crates/core/src/error.rs`, `crates/core/src/ops/add.rs`, `crates/core/src/ops/status.rs`, and related core tests.
- Ran `cargo test -p git-outpost --tests`: pass; 9 E2E tests, 8 flags tests, 4 help tests. Cargo emitted the expected duplicate `src/main.rs` bin-target warning.
- Ran `cargo test --workspace`: pass; CLI tests plus existing core suite and doctests.
- Ran `git diff --check`: pass before writing this review artifact.
- Initial working tree already had unrelated local changes: `crates/cli/Cargo.toml`, `.github/`, and `README.md`; I did not inspect or modify them for this review.
