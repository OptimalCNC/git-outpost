# P5-C2 Dispatch E2E Normal Re-review

## Verdict

Pass with one remaining nit.

I reviewed the P5-C2 state after `56eadac phase-5: fix dispatch e2e review findings`
and `fe717b6 phase-5: record dispatch review-fix commit`, with spot checks back
to `6f68b95 phase-5: add dispatch e2e` and `91f0052 phase-5: record dispatch
e2e commit`.

The material review fixes are adopted. The dispatch-context failures now live in
`OutpostError`, `exit.rs` no longer maintains a separate CLI error mapping, the
matrix-edge tests cover the previous `-C` and context gaps, and progress/evidence
now records the changed scope accurately.

## Findings

No blocking findings.

## Required Changes

None.

## Nits

1. Low: The prior status health wording nit remains open. Product docs ask for a
   short health line of `ok` or blocking configuration problems, while
   `crates/cli/src/output.rs` still prints `problems: none` for the healthy
   case. This is a wording/product alignment issue only; I did not find a
   dispatch or exit-code bug.

## Review Notes

- `crates/core/src/error.rs:18` and `crates/core/src/error.rs:25` add
  `OutpostError::WrongContext` and `OutpostError::MissingOutpostPath`.
- `crates/core/src/error.rs:103` maps those variants to exit code 2, with unit
  coverage for display text and exit codes.
- `crates/cli/src/exit.rs:5` now aliases `CliResult<T>` to `OutpostResult<T>`,
  and `crates/cli/src/exit.rs:7` reports `OutpostError` directly. The previous
  separate `CliError::exit_code` path is gone.
- `crates/cli/src/main.rs:173`, `crates/cli/src/main.rs:184`, and
  `crates/cli/src/main.rs:202` dispatch wrong-context and missing-path failures
  through the new `OutpostError` variants.
- `crates/cli/tests/flags.rs:51` covers `add` with global `-C` and a relative
  destination rooted at the effective source cwd.
- `crates/cli/tests/e2e.rs:54` covers list from source/outpost, lock/unlock
  from source and outpost contexts, move relative paths, and source prune.
- `crates/cli/tests/e2e.rs:107` covers representative wrong-context failures:
  source-only from an outpost, outpost-only from a source repo, and missing
  contextual outpost path from source.
- `docs/src/architecture.md:181` and `docs/src/architecture.md:1698` document
  the new dispatch-context variants and their exit-code mapping.
- `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md:12`
  through `.agents-artifacts/reviews/phase-5/P5-C2-dispatch-e2e/evidence-pack.md:17`
  accurately summarize the adopted dispatch, error-model, and `-C` path changes.

## Tests Reviewed

- `cargo test -p git-outpost --test e2e dispatch_matrix -- --nocapture`: pass.
- `cargo test -p git-outpost --test flags e_12_global_c_changes_effective_cwd -- --nocapture`: pass.
- `cargo test -p outpost-core error::tests -- --nocapture`: pass.
- `cargo fmt --check`: pass.
- `git diff --check 6f68b95..fe717b6`: pass.
- `cargo test -p git-outpost --tests`: pass; 8 E2E tests, 5 flags tests, 4 help tests.
- `cargo test --workspace`: pass; full CLI and core workspace tests, 0 doctests.

Cargo still warns that `crates/cli/src/main.rs` is present in both `git-outpost`
and `gop` binary targets; that matches the current Phase 5 architecture.

## Worktree Note

I did not edit production code or commit. While reviewing, unrelated local
changes appeared in the worktree (`crates/cli/Cargo.toml`, `.github/`, and
`README.md`); I left them untouched.
