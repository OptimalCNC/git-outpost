# P5-C2 Dispatch E2E Independent Review

## Verdict

Changes requested.

The happy-path CLI dispatch added in `6f68b95` works for the claimed E2E tests, and local verification passes. The implementation wires real core operations through the CLI and keeps P5-C3 copy/color tests out of this chunk.

I am not approving the chunk as-is because the dispatch layer introduces a CLI-only error model that conflicts with the architecture's documented `OutpostError`-only CLI error contract, and the evidence over-claims Working Directory Matrix coverage for commands and contexts that are not covered by P5-C2 tests.

## Blocking Findings

### 1. CLI context errors bypass the documented `OutpostError` error model

Architecture §9 says the CLI uses `OutpostError` internally and that exit codes are encoded as an exhaustive method on `OutpostError`, with no separate table to drift. `6f68b95` adds `CliError::WrongContext` and `CliError::MissingOutpostPath` plus a separate `CliError::exit_code()` mapping in `crates/cli/src/exit.rs:9-29`.

This is not just cosmetic. Working Directory Matrix failures are a core part of P5-C2 dispatch behavior, but they now sit outside the documented error type and outside the future E-08 wording, which only covers `OutpostError` variants. Either the implementation should model these dispatch failures in the documented error system, or the architecture/test plan/progress must explicitly accept CLI-only errors and require P5-C3 to cover their exit behavior too.

### 2. P5-C2 evidence claims full dispatch/context coverage that tests do not prove

The evidence pack says dispatch was added "for every Phase 5 command, using `outpost-core` operations and the product Working Directory Matrix" (`evidence-pack.md:12`). The added tests cover important happy paths, but they do not exercise several dispatch edges this chunk owns:

- `lock` / `unlock` from source with explicit relative paths.
- `lock` / `unlock` from an outpost with omitted path defaulting to the current outpost.
- `move` relative old/new path resolution from the source repo.
- `prune` source-only dispatch.
- `list` from an outpost auto-resolving the source repo.
- Wrong-context rejection for source-only and outpost-only commands.

Code inspection suggests most of these paths are plausible (`crates/cli/src/main.rs:76-127`, `204-224`), but the committed QA/evidence does not prove the matrix claim. This is especially relevant because the review prompt specifically calls out add/remove/move/lock/unlock relative paths and status/list dual contexts.

## Nonblocking Findings

### 1. `status` uses a different context path than other outpost-only commands

`pull`, `source pull`, `merge`, `rebase`, and `push` call `require_outpost`, while `status` directly calls `ops::status::run(&cwd)` (`crates/cli/src/main.rs:44-68`, `129-131`). That preserves degraded status behavior, but source-repo status failures return core `NotAnOutpost` rather than the CLI `WrongContext` path used elsewhere.

This may be intentional, but it should be documented or covered with a test so reviewers know the inconsistency is deliberate.

### 2. E-04 uses sibling `../C` instead of the literal inventory text `gop add C`

Architecture E-04 says `gop add C`, while the P5-C2 test uses `../C` because core add intentionally rejects destinations inside the source work tree (`crates/cli/tests/e2e.rs:24-31`; core rejection at `crates/core/tests/add.rs:169-183`). The evidence pack notes this and I agree with the implementation choice, but the architecture inventory remains misleading for CLI E2E readers.

### 3. Progress log omits the progress commit under review

The current range includes `91f0052 phase-5: record dispatch e2e commit`, but `.agents-artifacts/progress/phase-5.md` lists `6f68b95` and then a pending review-record commit (`phase-5.md:248-257`). This is a bookkeeping nit, not an implementation blocker.

## Required Changes

1. Resolve the `CliError` architecture mismatch:
   - Prefer adding documented `OutpostError` variants for CLI dispatch context failures, or
   - Update architecture/progress/evidence to explicitly allow CLI-only dispatch errors and make P5-C3 E-08 cover them.

2. Add focused CLI tests, or narrow the evidence claim, for the untested dispatch edges:
   - `lock` and `unlock` relative explicit paths from source.
   - `lock` and `unlock` omitted path from current outpost.
   - `move` relative old/new path resolution from source.
   - `list` from an outpost producing the same stdout as list from source.
   - Representative wrong-context failures.

3. Update progress/evidence after fixes so commit metadata and any accepted architecture deviations are accurate.

## Evidence

Reviewed:

- Commit range: `0b4ea9c..HEAD`
- Implementation commit: `6f68b95 phase-5: add dispatch e2e`
- Progress commit: `91f0052 phase-5: record dispatch e2e commit`
- Product docs: `docs/src/product.md`, especially Working Directory Matrix and command sections.
- Architecture docs: `docs/src/architecture.md`, especially CLI surface, dispatch, error handling, and E-02/E-04/E-06/E-10/E-11/E-12/E-14 inventory.
- Implementation files: `crates/cli/src/main.rs`, `crates/cli/src/cli.rs`, `crates/cli/src/exit.rs`, `crates/cli/src/output.rs`, `crates/cli/src/reporter_impls.rs`.
- Tests: `crates/cli/tests/e2e.rs`, `crates/cli/tests/flags.rs`, `crates/cli/tests/common/mod.rs`.
- Evidence/progress artifacts under `.agents-artifacts/qa/phase-5/` and `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/`.

Local verification run:

- `cargo test -p git-outpost --tests`: pass; 6 E2E tests, 5 flags tests, 4 help tests.
- `cargo test --workspace`: pass; full CLI and core workspace tests.

P5-C3 scope check:

- E-07 copied-outpost degradation remains unimplemented and marked planned.
- E-08 full exit-code matrix remains planned.
- E-09 `--no-color` / `NO_COLOR` ANSI assertions remain planned.
- No production implementation for copied-outpost independence or color stripping was added in this range.
