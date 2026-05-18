# Phase 5 Progress

## Phase

- `phase_id`: `phase-5`
- Roadmap scope: CLI binaries, exit codes, `--no-color`, global `-C`, E2E, cross-platform
- Test IDs: E-01..E-15, H-01..H-03
- Progress log path: `.agents-artifacts/progress/phase-5.md`
- Review artifact root: `.agents-artifacts/reviews/phase-5/`
- QA artifact root: `.agents-artifacts/qa/phase-5/`
- Protected paths: none
- Protected exceptions: none
- Forbidden scope:
  - implementation outside Phase 5 unless required to make Phase 5 compile and explicitly justified here
  - unrelated documentation cleanup
  - unrelated refactors
- Required verification:
  - `cargo test -p outpost-core`
  - `cargo test -p outpost-core --tests`
  - `cargo test --workspace`

## Source Docs

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`
- Last observed repo revision before Phase 5: `a1bdd72 phase-4: close phase`

## Current Snapshot

- Branch: `main`
- Initial Phase 5 `git status --short --branch`: `## main...origin/main [ahead 14]`, with no modified or untracked files before readiness artifact creation
- Workspace at readiness: one member, `outpost-core`
- Existing implementation: Phase 0/1/2/3/4 core library behavior and integration tests
- Missing Phase 5 files at start: `crates/cli/**`, CLI package manifest, binary targets, CLI integration tests
- Toolchain observed by readiness: `cargo 1.94.0`, `rustc 1.94.0`, `git version 2.43.0`
- Baseline verification before Phase 5 planning: required verification passed during Phase 5 readiness with 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
- `/goal` tool status: unable to create a new Phase 5 goal because the thread already contains the completed Phase 0 goal; this progress log is the durable Phase 5 coordination state.

## Readiness Log

- Verdict: `ready with cautions`
- Auditor: native subagent `019e3963-3065-75b0-8c48-97c3ac1ee243`
- Artifact: `.agents-artifacts/reviews/phase-5/readiness/readiness-audit.md`
- Phase reviewed: `phase-5`; roadmap scope CLI binaries, exit codes, `--no-color`, global `-C`, E2E, cross-platform; test IDs E-01..E-15, H-01..H-03
- Source documents reviewed:
  - `docs/src/product.md`
  - `docs/src/architecture.md`
  - `docs/src/roadmap.md`
  - `docs/coordinator-prompt.md`
  - `.agents-artifacts/progress/phase-0.md` through `.agents-artifacts/progress/phase-4.md`
- Repo state evidence:
  - cwd `/home/huwei/projects/git-outpost`
  - branch `main`
  - HEAD `a1bdd72 phase-4: close phase`
  - `git status --short --branch`: `## main...origin/main [ahead 14]`, with no modified or untracked files before artifact creation
  - `Cargo.toml` workspace members currently `["crates/core"]`
  - `crates/` contains only `core`
  - `cargo metadata --no-deps --format-version 1`: passed with one package, `outpost-core`
- Prerequisites checked:
  - Phase 4 closeout commit is HEAD
  - Phase 4 progress log records closeout passed
  - Phase 4 test IDs SP-01..SP-05, P-01..P-09, MR-01..MR-06, and Pu-01..Pu-10 are implemented and passing
  - No Phase 5 CLI crate or CLI tests exist before implementation, as expected
- Toolchain evidence:
  - `cargo --version`: `cargo 1.94.0`
  - `rustc --version`: `rustc 1.94.0`
  - `git --version`: `git version 2.43.0`
  - `cargo test -p outpost-core`: passed; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: passed with the same test binaries excluding doctests
  - `cargo test --workspace`: passed with the same workspace coverage
  - `cargo test -p git-outpost --tests`: not checked because the CLI crate does not exist yet
- Spec/architecture/roadmap consistency: pass. Product, architecture, and roadmap agree that Phase 5 owns package `git-outpost`, binaries `git-outpost` and `gop`, Git dispatch via `git outpost`, global `-C`, `--no-color`/`NO_COLOR`, exit codes, help rendering, rejected deferred flags, E2E behavior, and cross-platform test considerations.
- Blocking issues: none
- Non-blocking cautions:
  - `crates/cli` and CLI dependencies are absent; the first chunk must add the package, workspace member, binary targets, and test dependencies before CLI tests can run.
  - `Cargo.lock` does not currently include `clap`, `assert_cmd`, `predicates`, `strip-ansi-escapes`, `fs_extra`, or `anyhow`; dependency resolution will update the lockfile.
  - Cross-platform behavior can be designed locally, but real Windows/macOS confidence depends on CI or platform runners outside this local Linux run.
- Recommended first chunk: scaffold the minimal CLI crate with both binary targets, documented dependencies, command/help surface, and E-01 plus H-01..H-03/help-surface coverage before deeper command E2E.
- Required human decisions: none

## QA/Test Map

| ID | Scope | Status | Notes |
| --- | --- | --- | --- |
| E-01 | both `git-outpost` and `gop` debug binaries are built | planned | `crates/cli/tests/e2e.rs::build_creates_both_debug_binaries` |
| E-02 | `git outpost status`, `git-outpost status`, and `gop status` produce identical stdout for same C | planned | `crates/cli/tests/e2e.rs::all_invocation_forms_produce_same_status_stdout` |
| E-03 | `gop --help` lists every subcommand exactly once and includes every long flag from the CLI surface | planned | `crates/cli/tests/help.rs::gop_help_lists_commands_and_long_flags_once` |
| E-04 | add/status/push/list/remove round trip through CLI exits 0 | planned | `crates/cli/tests/e2e.rs::basic_cli_lifecycle_round_trip_exits_zero` |
| E-05 | `gop push` makes C commit visible in A | planned | `crates/cli/tests/e2e.rs::push_makes_outpost_commit_visible_upstream` |
| E-06 | two outposts round trip via source | planned | `crates/cli/tests/e2e.rs::two_outposts_sync_through_source` |
| E-07 | copied outpost remains Git-independent after deleting source and reports degraded status | planned | `crates/cli/tests/e2e.rs::copied_outpost_is_git_independent_when_source_is_missing` |
| E-08 | every `OutpostError` variant maps to documented exit code | planned | `crates/cli/tests/flags.rs::outpost_errors_map_to_documented_exit_codes` |
| E-09 | `--no-color` and `NO_COLOR=1` output contains no ANSI escapes | planned | `crates/cli/tests/flags.rs::no_color_flag_and_env_strip_ansi_output` |
| E-10 | full Story flow exits 0 | planned | `crates/cli/tests/e2e.rs::story_flow_exits_zero` |
| E-11 | `merge local/main` and `rebase local/main` accept Story source-ref form | planned | `crates/cli/tests/e2e.rs::merge_and_rebase_accept_story_source_ref` |
| E-12 | global `-C <other-dir>` changes effective cwd | planned | `crates/cli/tests/flags.rs::global_c_changes_effective_cwd` |
| E-13 | removed `add --detach` returns clap usage error | planned | `crates/cli/tests/flags.rs::add_detach_is_rejected_by_clap` |
| E-14 | `gop add C -- -evil` returns `InvalidRefName`, not `GitFailed` | planned | `crates/cli/tests/flags.rs::add_target_branch_starting_with_dash_returns_invalid_ref` |
| E-15 | representative deferred/removed surfaces are rejected by clap | planned | `crates/cli/tests/flags.rs::deferred_and_removed_surfaces_are_rejected_by_clap` |
| H-01 | `git-outpost --help` renders `git-outpost` as program name | planned | `crates/cli/tests/help.rs::git_outpost_help_uses_git_outpost_program_name` |
| H-02 | `gop --help` renders `gop` as program name | planned | `crates/cli/tests/help.rs::gop_help_uses_gop_program_name` |
| H-03 | `git outpost --help` renders a non-`gop` program name | planned | `crates/cli/tests/help.rs::git_dispatch_help_does_not_render_gop_program_name` |

## QA/Test Plan Gate

- QA subagent: `019e396a-09ec-7131-8690-5bf43edbbe04`
- Artifact: `.agents-artifacts/qa/phase-5/test-plan.md`
- Summary: QA will cover Phase 5 as CLI integration tests under `crates/cli/tests/*.rs`, spawning `git-outpost`, `gop`, and Git dispatch where needed. CLI-local helpers will provide A/B/C setup, hermetic Git env, binary path lookup, cross-platform `.exe` handling, ANSI stripping, file commit helpers, and recursive copy support for E-07.
- Planned test files:
  - `crates/cli/tests/e2e.rs`: E-01, E-02, E-04, E-05, E-06, E-07, E-10, E-11
  - `crates/cli/tests/flags.rs`: E-08, E-09, E-12, E-13, E-14, E-15
  - `crates/cli/tests/help.rs`: E-03, H-01, H-02, H-03
  - `crates/cli/tests/common/mod.rs`: CLI fixture and command helpers
- Blocked tests: none permanently; all Phase 5 tests are temporarily blocked until the CLI crate is created.
- QA risks:
  - E-08 should assert exit code plus focused error substrings, not full stderr snapshots.
  - E-07 must use Rust copy helpers rather than shell tools.
  - Color assertions should use `strip-ansi-escapes`.
  - Real Windows/macOS confidence requires CI or platform runners beyond local Linux.

## Active Chunk

- none; readiness, QA, and chunk planning gates have passed.

## Remaining Chunks

Chunk Planning Gate:

- Planner subagent: `019e396a-0a8a-7ad3-b9fa-bcc9eb225698`
- Artifact: `.agents-artifacts/qa/phase-5/chunk-plan.md`
- Verdict: ready to chunk after QA/Test Plan Gate
- Recommended chunks:
  - `P5-C1-cli-surface`: add `git-outpost` CLI crate, workspace membership, both binaries, clap command tree, help rendering, deferred/removed flag rejection, and initial CLI test harness; test IDs E-01, E-03, E-13, E-15, H-01, H-02, H-03
  - `P5-C2-dispatch-e2e`: wire CLI dispatch to `outpost-core` ops, context classification, global `-C`, stdout/stderr rendering, `StderrReporter`, and E2E fixture; test IDs E-02, E-04, E-05, E-06, E-10, E-11, E-12, E-14
  - `P5-C3-exit-color-platform-hardening`: complete CLI error reporting, exit-code coverage, `--no-color`/`NO_COLOR`, degraded status output, copy-independence test, and cross-platform test hardening; test IDs E-07, E-08, E-09
- Dependencies:
  - `P5-C1-cli-surface` must land first because the CLI package and binaries do not exist.
  - `P5-C2-dispatch-e2e` depends on `P5-C1` parser/bin structure.
  - `P5-C3-exit-color-platform-hardening` depends on real dispatch/output from `P5-C2`.
- Out-of-scope guardrails:
  - no core command semantic changes except narrowly justified compile/API issues exposed by CLI wiring
  - no post-MVP surfaces such as `--json`, `--quiet`, `list --all`, `add --detach`, or pull/push strategy flags
  - no global registry behavior
  - no unrelated docs cleanup or refactors

Remaining chunk order:

- `P5-C1-cli-surface`
- `P5-C2-dispatch-e2e`
- `P5-C3-exit-color-platform-hardening`

## Completed Chunks

- none

## Verification Log

- Phase 5 readiness baseline:
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same test binaries excluding doctests
  - `cargo test --workspace`: pass with the same workspace coverage

## Review Log

- Readiness Auditor: `ready with cautions`; artifact `.agents-artifacts/reviews/phase-5/readiness/readiness-audit.md`; no blocking issues and no required human decisions.

## Docs Log

- none

## Commit Log

- pending `phase-5: record readiness and plan`

## Protected-Path Exception Log

- none

## Open Risks / Questions

- Adding CLI dependencies will update `Cargo.lock`.
- Local execution is Linux; cross-platform rules must be encoded in tests and CI-friendly code, but Windows/macOS behavior cannot be fully proven locally without runners.

## Next Recommended Action

- Dispatch QA/Test Developer for the Phase 5 test plan, then dispatch Planner for chunking.
