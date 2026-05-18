# P5-C2 Dispatch E2E Normal Review

## Verdict

Pass with nits.

I reviewed `0b4ea9c..HEAD`, focusing on `6f68b95 phase-5: add dispatch e2e`
and `91f0052 phase-5: record dispatch e2e commit`. I found no blocking
dispatch, `-C`, path handling, context classification, reporter, or E2E test
defects in this range.

Assumptions:

- `P5-C2-dispatch-e2e` owns CLI dispatch, effective cwd/global `-C`,
  `StderrReporter`, human-readable output, and E-02/E-04/E-05/E-06/E-10/E-11/E-12/E-14.
- E-07 copied-outpost degradation, E-08 exhaustive exit-code coverage, and
  E-09 color/NO_COLOR assertions remain P5-C3 scope, as recorded in the
  progress and evidence artifacts.

## Findings

No blocking findings.

## Required Changes

None.

## Nits

1. Low: E-12 covers `status` with `-C` and one relative `remove` path, but it
   does not cover `add` with `-C`. `add` resolves its destination through a
   separate helper in `crates/cli/src/cli.rs:142`, while `remove` uses the
   dispatch helper in `crates/cli/src/main.rs:153`. A future test like
   `gop -C <source> add ../C main` would close that small regression gap.

2. Low: The Working Directory Matrix implementation is present in
   `crates/cli/src/main.rs:38`, `crates/cli/src/main.rs:166`,
   `crates/cli/src/main.rs:175`, and `crates/cli/src/main.rs:186`, but P5-C2
   tests mostly exercise allowed contexts. Consider adding compact negative
   tests for at least one source-only command from an outpost and one
   outpost-only command from a source, plus `list` from an outpost. The matrix
   contract is in `docs/src/product.md:211` and `docs/src/architecture.md:1557`.

3. Low: `status` output is usable and human-readable, but the product wording
   says the output should include a short health line of `ok` or the blocking
   configuration problems (`docs/src/product.md:413`). The current formatter
   prints `problems: none` for the healthy case in `crates/cli/src/output.rs:64`.
   If literal product wording matters, either align the formatter or clarify
   the docs before the output shape hardens.

## Tests Reviewed

- E-02: `crates/cli/tests/e2e.rs:4` verifies `gop`, `git-outpost`, and
  `git outpost` produce identical `status` stdout for the same outpost.
- E-04: `crates/cli/tests/e2e.rs:20` covers add, `-C` status, `-C` push, list,
  and remove happy path exits.
- E-05: `crates/cli/tests/e2e.rs:54` verifies `gop push` makes the outpost
  commit visible in upstream A.
- E-06: `crates/cli/tests/e2e.rs:69` verifies two outposts round-trip via the
  source repository.
- E-10: `crates/cli/tests/e2e.rs:89` covers the Story flow with `add -b`,
  `source pull`, `rebase local/main`, and `push`.
- E-11: `crates/cli/tests/e2e.rs:127` verifies `merge local/main` and
  `rebase local/main` accept the Story source-ref form.
- E-12: `crates/cli/tests/flags.rs:18` verifies effective cwd for `status` and
  relative path rooting for `remove` after global `-C`.
- E-14: `crates/cli/tests/flags.rs:59` verifies leading-dash target branches
  surface `InvalidRefName` instead of `GitFailed`.

## Implementation Reviewed

- CLI dispatch: `crates/cli/src/main.rs:34`
- Effective cwd and path rooting: `crates/cli/src/main.rs:138`,
  `crates/cli/src/main.rs:153`, `crates/cli/src/cli.rs:142`
- Context classification and matrix enforcement: `crates/cli/src/main.rs:166`,
  `crates/cli/src/main.rs:175`, `crates/cli/src/main.rs:186`,
  `crates/cli/src/main.rs:197`, `crates/cli/src/main.rs:204`
- Output/reporting: `crates/cli/src/output.rs:5`,
  `crates/cli/src/reporter_impls.rs:5`
- CLI-only errors: `crates/cli/src/exit.rs:8`
- Fixture and Git dispatch helpers: `crates/cli/tests/common/mod.rs:17`,
  `crates/cli/tests/common/mod.rs:152`

## Verification

- `cargo fmt --check`: pass
- `git diff --check 0b4ea9c..HEAD`: pass
- `cargo test -p git-outpost --tests`: pass
- `cargo test --workspace`: pass

Cargo still warns that `crates/cli/src/main.rs` is present in both `git-outpost`
and `gop` binary targets; that matches the Phase 5 architecture for two binary
names sharing one entrypoint.
