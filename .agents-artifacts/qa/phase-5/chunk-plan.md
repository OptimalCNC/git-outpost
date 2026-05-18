**Verdict**

Ready to chunk with one gating caution: implementation should start only after
the QA test plan is recorded. Recommended split: 3 chunks.

**Recommended Chunks**

`P5-C1-cli-surface`

- Scope: Add `git-outpost` CLI crate, workspace membership, both binaries,
  clap command tree, help rendering, deferred/removed flag rejection, initial
  CLI test harness.
- Test IDs: E-01, E-03, E-13, E-15, H-01, H-02, H-03.
- Files likely touched: `Cargo.toml`, `Cargo.lock`, `crates/cli/Cargo.toml`,
  `crates/cli/src/main.rs`, `crates/cli/src/cli.rs`,
  `crates/cli/tests/common/mod.rs`, `crates/cli/tests/help.rs`,
  `crates/cli/tests/flags.rs`.
- Acceptance criteria: `cargo build -p git-outpost` creates `git-outpost` and
  `gop`; help names match invocation; all documented commands/long flags
  appear once; removed/deferred surfaces fail with clap usage exit 2; no core
  behavior dispatch required beyond parse/help.

`P5-C2-dispatch-e2e`

- Scope: Wire CLI dispatch to `outpost-core` ops, context classification,
  global `-C`, stdout/stderr rendering for normal reports, `StderrReporter`,
  and CLI E2E fixture.
- Test IDs: E-02, E-04, E-05, E-06, E-10, E-11, E-12, E-14.
- Files likely touched: `crates/cli/src/main.rs`, `crates/cli/src/cli.rs`,
  `crates/cli/src/output.rs`, `crates/cli/src/reporter_impls.rs`,
  `crates/cli/tests/common/mod.rs`, `crates/cli/tests/e2e.rs`.
- Acceptance criteria: `git outpost`, `git-outpost`, and `gop` produce
  equivalent command behavior; add/status/push/list/remove and full Story flow
  pass through the binary; merge/rebase accept `local/main`; `-C` controls
  effective cwd; invalid branch/ref inputs surface `InvalidRefName`, not raw
  `GitFailed`.

`P5-C3-exit-color-platform-hardening`

- Scope: Complete CLI error reporting, exit-code coverage, `--no-color` /
  `NO_COLOR`, degraded status output, copy-independence test, and
  cross-platform test hardening.
- Test IDs: E-07, E-08, E-09.
- Files likely touched: `crates/cli/src/exit.rs`, `crates/cli/src/output.rs`,
  `crates/cli/src/main.rs`, `crates/cli/tests/flags.rs`,
  `crates/cli/tests/e2e.rs`, `crates/cli/tests/common/mod.rs`, possibly
  `crates/cli/Cargo.toml`.
- Acceptance criteria: every current `OutpostError` variant has documented CLI
  exit-code coverage; `--no-color` and `NO_COLOR=1` output has no ANSI escapes;
  copied outpost remains normal-Git usable after source deletion and
  `gop status` reports degraded source-missing state; tests use
  cross-platform path/env/copy rules.

**Dependencies Between Chunks**

`P5-C1` must land first because the CLI package and binaries do not exist.
`P5-C2` depends on `P5-C1` parser/bin structure. `P5-C3` depends on real
dispatch/output from `P5-C2`.

**Out-of-Scope Guardrails**

Do not change core command semantics except for a narrowly justified
compile/API issue exposed by CLI wiring. Do not refactor existing core tests or
fixture layout unless CLI tests cannot be made cross-platform otherwise. Do not
add post-MVP surfaces like `--json`, `--quiet`, `list --all`, `add --detach`,
or pull/push strategy flags. Do not add global registry behavior or unrelated
docs cleanup.

**Risks**

E-08 may be the hardest: some variants, especially process-termination cases,
may need a careful non-brittle strategy. CLI fixtures may duplicate core
fixture logic unless a small shared test support approach is approved.
Cross-platform confidence is design/test-level locally; full Windows/macOS
proof needs CI runners.

**Suggested First Assignment**

Assign `P5-C1-cli-surface` after QA records the Phase 5 test plan. Ownership
should be limited to workspace/CLI scaffold, clap surface,
binary/help/parse-rejection tests, and dependency setup.
