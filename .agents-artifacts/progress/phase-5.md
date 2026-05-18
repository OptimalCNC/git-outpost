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
| E-01 | both `git-outpost` and `gop` debug binaries are built | completed | `crates/cli/tests/flags.rs::e_01_build_produces_both_binaries` |
| E-02 | `git outpost status`, `git-outpost status`, and `gop status` produce identical stdout for same C | completed | `crates/cli/tests/e2e.rs::e_02_invocation_forms_produce_same_status_stdout` |
| E-03 | `gop --help` lists every subcommand exactly once and includes every long flag from the CLI surface | completed | `crates/cli/tests/help.rs::e_03_help_lists_commands_and_long_flags` |
| E-04 | add/status/push/list/remove round trip through CLI exits 0 | completed | `crates/cli/tests/e2e.rs::e_04_basic_cli_lifecycle_round_trip_exits_zero` |
| E-05 | `gop push` makes C commit visible in A | completed | `crates/cli/tests/e2e.rs::e_05_push_makes_outpost_commit_visible_upstream` |
| E-06 | two outposts round trip via source | completed | `crates/cli/tests/e2e.rs::e_06_two_outposts_round_trip_via_source` |
| E-07 | copied outpost remains Git-independent after deleting source and reports degraded status | completed | `crates/cli/tests/e2e.rs::e_07_copied_outpost_is_git_independent_when_source_is_missing` |
| E-08 | every `OutpostError` variant maps to documented exit code | completed | `crates/cli/tests/flags.rs::e_08_outpost_errors_map_to_documented_exit_codes`; reachable CLI broken-state coverage in `e_08_cli_errors_return_documented_exit_codes` |
| E-09 | `--no-color` and `NO_COLOR=1` output contains no ANSI escapes | completed | `crates/cli/tests/flags.rs::e_09_no_color_flag_and_env_do_not_emit_ansi_output` |
| E-10 | full Story flow exits 0 | completed | `crates/cli/tests/e2e.rs::e_10_story_flow_exits_zero` |
| E-11 | `merge local/main` and `rebase local/main` accept Story source-ref form | completed | `crates/cli/tests/e2e.rs::e_11_merge_and_rebase_accept_story_source_ref` |
| E-12 | global `-C <other-dir>` changes effective cwd | completed | `crates/cli/tests/flags.rs::e_12_global_c_changes_effective_cwd` |
| E-13 | removed `add --detach` returns clap usage error | completed | `crates/cli/tests/flags.rs::e_13_add_detach_is_rejected_by_clap` |
| E-14 | `gop add C -- -evil` returns `InvalidRefName`, not `GitFailed` | completed | `crates/cli/tests/flags.rs::e_14_add_target_branch_starting_with_dash_returns_invalid_ref` |
| E-15 | representative deferred/removed surfaces are rejected by clap | completed | `crates/cli/tests/flags.rs::e_15_deferred_and_removed_surfaces_are_rejected_by_clap` |
| H-01 | `git-outpost --help` renders `git-outpost` as program name | completed | `crates/cli/tests/help.rs::h_01_git_outpost_help_uses_git_outpost_name` |
| H-02 | `gop --help` renders `gop` as program name | completed | `crates/cli/tests/help.rs::h_02_gop_help_uses_gop_name` |
| H-03 | `git outpost -h` renders a non-`gop` program name; Git intercepts literal `git outpost --help` before dispatch | completed | `crates/cli/tests/help.rs::h_03_git_dispatch_help_does_not_use_gop_name` |

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

- `P5-C3-exit-color-platform-hardening`
- Scope: copied-outpost degradation, CLI exit-code coverage, `--no-color`/`NO_COLOR`, status health output hardening if needed, and cross-platform test hardening.
- Test IDs: E-07, E-08, E-09
- Out of scope: new command surfaces, global registry behavior, unrelated docs cleanup, unrelated refactors.
- Status: implemented locally; evidence recorded; review gate pending.

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

- none

## Completed Chunks

- `P5-C1-cli-surface` implementation evidence recorded:
  - Files changed: `Cargo.toml`, `Cargo.lock`, `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/cli/src/cli.rs`, `crates/cli/tests/common/mod.rs`, `crates/cli/tests/flags.rs`, `crates/cli/tests/help.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-5.md`, `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/evidence-pack.md`, `.agents-artifacts/qa/phase-5/P5-C1-cli-surface.md`
  - Test IDs advanced: E-01, E-03, E-13, E-15, H-01, H-02, H-03
  - Evidence pack: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-5/P5-C1-cli-surface.md`
  - Unit tests added: none
  - CLI integration tests added: `e_01_build_produces_both_binaries`, `e_03_help_lists_commands_and_long_flags`, `e_13_add_detach_is_rejected_by_clap`, `e_15_deferred_and_removed_surfaces_are_rejected_by_clap`, `h_01_git_outpost_help_uses_git_outpost_name`, `h_02_gop_help_uses_gop_name`, `h_03_git_dispatch_help_does_not_use_gop_name`
  - Docs updated: `docs/src/architecture.md` H-03 now specifies `git outpost -h`, because Git intercepts literal `git outpost --help` as a manpage request before external command dispatch.
  - Architecture deviations: none for claimed `P5-C1-cli-surface` behavior after review fix. Git 2.43 intercepts `git outpost --help` as a manpage request, so H-03 uses `git outpost -h` to exercise forwarded external-command help; `docs/src/architecture.md` now records that acceptance detail.
  - Implementation/evidence commit: `00f48c7 phase-5: add cli surface`
  - Review fixes committed in `a885c59 phase-5: fix cli surface review findings`:
    - `clap` dependency constrained to `>=4.5, <4.6`; resolved `clap 4.5.61` is compatible with Rust 1.75.
    - H-03 acceptance docs and artifacts updated to `git outpost -h`.
    - E-03 strengthened with actual subcommand help assertions.
    - E-15 expanded with representative removed/deferred add, list, and push flags.
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/independent-review.md`
    - Scope Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/scope-rereview.md`
    - Normal Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/normal-rereview.md`
    - Independent Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/independent-rereview.md`
  - Review verdicts after fixes: scope `pass`; normal `pass`; independent `pass`
  - Required review changes: none open
  - Adopted nits: progress log now records review-fix commit `a885c59`; future evidence should prefer `git diff --check HEAD^..HEAD` for committed diffs.
  - Status: approved
- `P5-C2-dispatch-e2e` implementation evidence recorded:
  - Files changed: `Cargo.lock`, `crates/cli/Cargo.toml`, `crates/cli/src/cli.rs`, `crates/cli/src/main.rs`, `crates/cli/src/exit.rs`, `crates/cli/src/output.rs`, `crates/cli/src/reporter_impls.rs`, `crates/cli/tests/common/mod.rs`, `crates/cli/tests/e2e.rs`, `crates/cli/tests/flags.rs`, `crates/core/src/error.rs`, `docs/src/architecture.md`
  - Artifact files changed: `.agents-artifacts/progress/phase-5.md`, `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md`, `.agents-artifacts/qa/phase-5/P5-C2-dispatch-e2e.md`
  - Test IDs advanced: E-02, E-04, E-05, E-06, E-10, E-11, E-12, E-14
  - Evidence pack: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-5/P5-C2-dispatch-e2e.md`
  - CLI integration tests added: `e_02_invocation_forms_produce_same_status_stdout`, `e_04_basic_cli_lifecycle_round_trip_exits_zero`, `e_05_push_makes_outpost_commit_visible_upstream`, `e_06_two_outposts_round_trip_via_source`, `e_10_story_flow_exits_zero`, `e_11_merge_and_rebase_accept_story_source_ref`, `dispatch_matrix_contextual_source_and_outpost_commands`, `dispatch_matrix_rejects_wrong_contexts`, `e_12_global_c_changes_effective_cwd`, `e_14_add_target_branch_starting_with_dash_returns_invalid_ref`
  - Docs updated: `docs/src/architecture.md` now includes `OutpostError::WrongContext` and `OutpostError::MissingOutpostPath` in the error model and exit-code mapping.
  - Architecture deviations: none for claimed `P5-C2-dispatch-e2e` behavior. CLI fixture uses sibling paths such as `../C` from source repo to preserve the core invariant that outposts are separate checkouts outside the source work tree.
  - Implementation/evidence commit: `6f68b95 phase-5: add dispatch e2e`
  - Review fixes committed in `56eadac phase-5: fix dispatch e2e review findings`:
    - Replaced CLI-only `CliError` context failures with first-class `OutpostError` variants to preserve the architecture's single error/exit-code model.
    - Added matrix-edge CLI tests for list-from-outpost, lock/unlock explicit and default path dispatch, move/prune dispatch, representative wrong-context failures, and `add` under global `-C`.
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/independent-review.md`
    - Scope Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/scope-rereview.md`
    - Normal Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/normal-rereview.md`
    - Independent Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/independent-rereview.md`
  - Review verdicts after fixes: scope `pass`; normal `pass with nit`; independent `pass`
  - Required review changes: none open
  - Adopted nits: `add` with global `-C` and representative matrix negative/dual-context tests. Remaining low-severity nit: status healthy output currently prints `problems: none` instead of literal `ok`; P5-C3 may address if output hardening touches status formatting.
  - Status: approved
- `P5-C3-exit-color-platform-hardening` implementation evidence recorded:
  - Files changed: `crates/cli/src/output.rs`, `crates/core/src/error.rs`, `crates/cli/tests/common/mod.rs`, `crates/cli/tests/e2e.rs`, `crates/cli/tests/flags.rs`
  - Artifact files changed: `.agents-artifacts/progress/phase-5.md`, `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`, `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`
  - Test IDs advanced: E-07, E-08, E-09
  - Evidence pack: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`
  - QA note: `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`
  - CLI integration tests added: `e_07_copied_outpost_is_git_independent_when_source_is_missing`, `e_08_outpost_errors_map_to_documented_exit_codes`, `e_08_cli_errors_return_documented_exit_codes`, `e_09_no_color_flag_and_env_do_not_emit_ansi_output`
  - Docs updated: none. Status output already matched product source text after changing the CLI line to `health: ok` / `health: problems`.
  - Architecture deviations: E-09 uses an equivalent stricter ESC-byte assertion instead of adding `strip-ansi-escapes`; E-08 keeps `GitTerminatedBySignal` table-driven because reliably forcing that process-control state through the CLI would be brittle.
  - Implementation/evidence commit: `47d10fd phase-5: harden exit color platform behavior`
  - Review fixes committed in `c93bb8c phase-5: fix exit color review findings`:
    - `GitFailed` negative process status values now clamp to exit code 1 instead of success exit code 0.
    - E-08 CLI coverage now uses focused broken-state fixtures and stderr assertions for reachable `OutpostError` variants.
    - Progress commit log and P5-C3 evidence/QA notes were updated.
  - Review artifacts:
    - Scope Reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/scope-review.md`
    - Normal Reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/normal-review.md`
    - Independent Reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/independent-review.md`
    - Scope Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/scope-rereview.md`
    - Normal Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/normal-rereview.md`
    - Independent Re-reviewer: `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/independent-rereview.md`
  - Review verdicts after fixes: scope `pass`; normal `pass`; independent `pass`
  - Review verdicts before fixes: scope `changes requested`; normal `changes requested`; independent `changes requested`
  - Required review changes before fixes: scope requested unrelated working tree changes removed from review boundary; normal requested negative `GitFailed` mapping fix, focused stderr assertions, and stale progress metadata fix; independent requested stronger E-08 broken-state traceability.
  - Adopted re-review note: `docs/src/architecture.md` exit-code snippet now reflects `GitFailed` clamping to `1..=125`.
  - Status: approved

## Verification Log

- Phase 5 readiness baseline:
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same test binaries excluding doctests
  - `cargo test --workspace`: pass with the same workspace coverage
- `P5-C1-cli-surface` local verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 3 `flags` tests and 4 `help` tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 7 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass
- `P5-C1-cli-surface` review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 3 `flags` tests and 4 `help` tests with hardened E-03/E-15 coverage
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 7 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass
- `P5-C2-dispatch-e2e` local verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 8 E2E tests, 5 flags tests, 4 help tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 17 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass
- `P5-C3-exit-color-platform-hardening` local verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 9 E2E tests, 8 flags tests, 4 help tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 21 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass
- `P5-C3-exit-color-platform-hardening` review-fix verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 9 E2E tests, 8 flags tests, 4 help tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 21 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass
- `P5-C3-exit-color-platform-hardening` review-record targeted verification:
  - `cargo test -p outpost-core exit_code_maps_each_variant`: pass
  - `cargo test -p git-outpost --test flags e_08`: pass; 2 selected E-08 tests
  - `git diff --check`: pass
- Phase 5 closeout verification:
  - `cargo fmt --check`: pass
  - `cargo build -p git-outpost`: pass; builds `git-outpost` and `gop`; Cargo warns that `src/main.rs` is present in both bin targets, matching the Phase 5 architecture
  - `cargo test -p git-outpost --tests`: pass; 9 E2E tests, 8 flags tests, 4 help tests
  - `cargo test -p outpost-core`: pass; 48 unit tests, 22 add integration tests, 11 list integration tests, 9 lock/move/unlock integration tests, 6 merge integration tests, 9 prune integration tests, 9 pull integration tests, 13 push integration tests, 6 rebase integration tests, 11 remove integration tests, 5 source integration tests, 15 status integration tests, 1 fixture smoke test, 0 doctests
  - `cargo test -p outpost-core --tests`: pass with the same core test binaries excluding doctests
  - `cargo test --workspace`: pass; 21 CLI integration tests plus existing core coverage, 0 doctests
  - `git diff --check`: pass

## Review Log

- Readiness Auditor: `ready with cautions`; artifact `.agents-artifacts/reviews/phase-5/readiness/readiness-audit.md`; no blocking issues and no required human decisions.
- `P5-C1-cli-surface` Scope Reviewer: `approved with nits`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/scope-review.md`; nits were stale progress commit-log text and preserving the `git outpost -h` H-03 nuance.
- `P5-C1-cli-surface` Normal Reviewer: `conditional pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/normal-review.md`; required H-03 acceptance/spec mismatch resolution.
- `P5-C1-cli-surface` Independent Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/independent-review.md`; required MSRV-compatible `clap` dependency, H-03 contract resolution, and E-03 hardening.
- Adopted `P5-C1-cli-surface` review fixes:
  - `clap` constrained to `>=4.5, <4.6`, resolving to `clap 4.5.61` whose registry metadata declares `rust-version = "1.74"`.
  - H-03 acceptance docs and artifacts now use `git outpost -h`, because Git intercepts literal `git outpost --help` before dispatching external commands.
  - E-03 now checks actual subcommand help for command-owned long flags.
  - E-15 now includes representative removed/deferred add, list, and push flags in addition to the original global/list/prune/pull cases.
- `P5-C1-cli-surface` Scope Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/scope-rereview.md`; nit was stale progress commit-log text.
- `P5-C1-cli-surface` Normal Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/normal-rereview.md`; nit was stale progress metadata around docs updated and review-fix status.
- `P5-C1-cli-surface` Independent Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/independent-rereview.md`; nit was future evidence should prefer `git diff --check HEAD^..HEAD`.
- Blocking review findings: none open for `P5-C1-cli-surface`.
- `P5-C2-dispatch-e2e` Scope Reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/scope-review.md`; no required changes or nits.
- `P5-C2-dispatch-e2e` Normal Reviewer: `pass with nits`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/normal-review.md`; nits were add-with-`-C`, negative matrix tests, and possible status health wording alignment.
- `P5-C2-dispatch-e2e` Independent Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/independent-review.md`; required replacing CLI-only dispatch errors or documenting them, and adding/narrowing matrix-edge coverage.
- Adopted `P5-C2-dispatch-e2e` review fixes:
  - `OutpostError::WrongContext` and `OutpostError::MissingOutpostPath` now own CLI dispatch context failures and map to exit code 2.
  - Added matrix-edge coverage for list from outpost, lock/unlock from source and outpost contexts, move/prune from source, representative wrong-context failures, and `add` with global `-C`.
  - Evidence/progress updated to reflect docs and coverage changes.
- `P5-C2-dispatch-e2e` Scope Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/scope-rereview.md`; no required changes or nits.
- `P5-C2-dispatch-e2e` Normal Re-reviewer: `pass with one low-severity nit`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/normal-rereview.md`; nit was status healthy wording `problems: none` vs product's `ok` wording.
- `P5-C2-dispatch-e2e` Independent Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/independent-rereview.md`; prior blockers resolved.
- Blocking review findings: none open for `P5-C2-dispatch-e2e`.
- `P5-C3-exit-color-platform-hardening` Scope Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/scope-review.md`; finding was unrelated current worktree files must remain outside the committed P5-C3 review boundary.
- `P5-C3-exit-color-platform-hardening` Normal Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/normal-review.md`; required negative `GitFailed` process-code mapping fix, focused E-08 stderr assertions, and stale progress metadata fix.
- `P5-C3-exit-color-platform-hardening` Independent Reviewer: `changes requested`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/independent-review.md`; required stronger E-08 broken-state traceability.
- Adopted `P5-C3-exit-color-platform-hardening` review fixes:
  - `GitFailed` negative process codes clamp to exit code 1.
  - E-08 uses focused reachable CLI broken-state fixtures with stderr assertions.
  - P5-C3 progress, evidence, and QA artifacts updated.
  - `docs/src/architecture.md` exit-code snippet aligned with `1..=125` `GitFailed` clamping.
- `P5-C3-exit-color-platform-hardening` Scope Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/scope-rereview.md`; P5-C3 commits exclude unrelated local files.
- `P5-C3-exit-color-platform-hardening` Normal Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/normal-rereview.md`; prior blockers resolved.
- `P5-C3-exit-color-platform-hardening` Independent Re-reviewer: `pass`; artifact `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/independent-rereview.md`; prior E-08 blocker resolved.
- Blocking review findings: none open for `P5-C3-exit-color-platform-hardening`.

## Docs Log

- `P5-C1-cli-surface`: `docs/src/architecture.md` updated H-03 to specify `git outpost -h`, because Git intercepts literal `git outpost --help` as a manpage request before external command dispatch.
- `P5-C2-dispatch-e2e`: `docs/src/architecture.md` updated with dispatch-context `OutpostError` variants and exit-code mapping. Evidence records that E2E paths use sibling outposts such as `../C` to honor the existing separate-checkout invariant.
- `P5-C3-exit-color-platform-hardening`: `docs/src/architecture.md` updated the `GitFailed` exit-code snippet to clamp into `1..=125`, matching the review-fixed implementation and tests.

## Commit Log

- `1042a8e phase-5: record readiness and plan`
- `270cdde phase-5: start cli surface`
- `00f48c7 phase-5: add cli surface`
- `a885c59 phase-5: fix cli surface review findings`
- `cc22594 phase-5: record cli surface reviews`
- `0b4ea9c phase-5: start dispatch e2e`
- `6f68b95 phase-5: add dispatch e2e`
- `56eadac phase-5: fix dispatch e2e review findings`
- `fe717b6 phase-5: record dispatch review-fix commit`
- `6631f42 phase-5: record dispatch e2e reviews`
- `f0c41c7 phase-5: start exit color platform hardening`
- `47d10fd phase-5: harden exit color platform behavior`
- `858f61e phase-5: record exit color hardening commit`
- `c93bb8c phase-5: fix exit color review findings`
- `d12f0d7 phase-5: record exit color reviews`
- `95e5770 phase-5: close phase`

## Protected-Path Exception Log

- none

## Open Risks / Questions

- H-03 should continue to use `git outpost -h`; literal `git outpost --help` is Git's manpage path on Git 2.43.
- Local execution is Linux; cross-platform rules must be encoded in tests and CI-friendly code, but Windows/macOS behavior cannot be fully proven locally without runners.
- Existing unrelated local changes remain outside Phase 5 commits: `crates/cli/Cargo.toml`, `.github/`, and `README.md`.

## Next Recommended Action

- Phase 5 is closed. `docs/src/roadmap.md` currently defines phases 0 through 5 only, so there is no Phase 6 readiness gate to run until the roadmap is extended or a new phase invocation is provided.
