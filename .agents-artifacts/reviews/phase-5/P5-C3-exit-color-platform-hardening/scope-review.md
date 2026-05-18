# P5-C3 Exit Color Platform Hardening Scope Review

## Verdict

Changes requested.

The committed P5-C3 implementation and the `47d10fd..858f61e` record commit are
limited to the declared P5-C3 work. However, the current working tree contains
uncommitted/untracked work outside `P5-C3-exit-color-platform-hardening`, and the
review request explicitly included the working tree as needed.

## Findings

### P5-C3-S01 - Out-of-scope current working tree changes must be removed from this review boundary

Severity: required change

`git status --short` shows current working tree changes beyond the P5-C3 artifact
work: `crates/cli/Cargo.toml`, untracked `.github/`, and untracked `README.md`.
Those files are not part of the P5-C3 evidence pack's changed-file list
(`.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:18`)
and are explicitly called out there as unrelated local changes
(`.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:49`).

Concrete out-of-scope surfaces inspected:

- `.github/workflows/ci.yml:1`, `.github/workflows/dev.yml:1`,
  `.github/workflows/integration.yml:1`, and `.github/workflows/release.yml:1`
  add CI/development/integration/release workflows. P5-C3 scope is copied-outpost
  degradation, exit-code coverage, `--no-color`/`NO_COLOR`, status health output
  hardening, and cross-platform test hardening
  (`.agents-artifacts/progress/phase-5.md:124`), not repository CI or release
  process wiring. The architecture also explicitly leaves CI configuration and
  release / `cargo publish` process out of the document
  (`docs/src/architecture.md:2160`).
- `README.md:1` adds repository-level README content and workflow badges. A
  README is part of the repo layout (`docs/src/architecture.md:66`), but creating
  or updating it is not in P5-C3's declared scope or evidence.
- `crates/cli/Cargo.toml:17` changes the `outpost-core` dependency to include a
  version. This appears related to package/release preparation, not to E-07,
  E-08, E-09, status health output, or platform-test hardening.

Required change: before P5-C3 can pass scope review, remove/stash these unrelated
working tree changes from the P5-C3 review boundary, or move them into a separately
approved chunk with its own evidence.

## Scope Notes

- The requested range `47d10fd..858f61e` contains only
  `.agents-artifacts/progress/phase-5.md`, updating the P5-C3 implementation
  commit reference from `pending` to `47d10fd`
  (`.agents-artifacts/progress/phase-5.md:217`).
- The implementation commit named by progress and evidence, `47d10fd`, was
  inspected because it contains the actual P5-C3 code and test changes. Its
  changed files match the evidence pack:
  `crates/cli/src/output.rs`, `crates/cli/tests/common/mod.rs`,
  `crates/cli/tests/e2e.rs`, `crates/cli/tests/flags.rs`, the QA note, the
  progress log, and the evidence pack
  (`.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:18`).
- The committed P5-C3 code changes align with the declared test IDs:
  E-07 is implemented in
  `crates/cli/tests/e2e.rs:177`; E-08 coverage is in
  `crates/cli/tests/flags.rs:20` and `crates/cli/tests/flags.rs:172`; E-09 is in
  `crates/cli/tests/flags.rs:243`.
- The status output hardening is narrowly scoped to `health: ok` /
  `health: problems` in `crates/cli/src/output.rs:64`.
- The recursive copy helper for E-07 uses Rust filesystem APIs and
  platform-specific symlink handling in `crates/cli/tests/common/mod.rs:240`.
- No new command surfaces, global registry behavior, `--json`, `--quiet`,
  porcelain output, hooks, or other post-MVP/Phase 6+ behavior were found in the
  committed P5-C3 implementation.

## Required Artifacts

- Evidence pack exists and records scope, changed files, tests, verification, and
  deviations:
  `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:1`.
- QA note exists and records E-07/E-08/E-09 coverage:
  `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md:1`.
- Phase 5 progress records P5-C3 as review-pending and records implementation
  commit `47d10fd`: `.agents-artifacts/progress/phase-5.md:208`.

Non-blocking artifact note: the progress log's commit-log section still says
`pending phase-5: start exit color platform hardening`
(`.agents-artifacts/progress/phase-5.md:306`) even though the completed-chunk
section records `47d10fd`. This is stale coordination metadata, not a separate
scope blocker.

## Verification

Commands and inspections performed:

- `git status --short`
- `git log --oneline --decorate 47d10fd..858f61e`
- `git diff --name-status 47d10fd..858f61e`
- `git diff --stat 47d10fd..858f61e`
- `git show --stat --name-status --oneline 47d10fd`
- `git show --stat --name-status --oneline 858f61e`
- `git diff -- crates/cli/Cargo.toml`
- `find .github -maxdepth 3 -type f -print`
- `git diff --check 47d10fd..858f61e`: pass
- `git diff --check`: pass for tracked working tree diff
- Inspected source docs:
  `docs/src/product.md`, `docs/src/architecture.md`, and `docs/src/roadmap.md`
- Inspected progress and evidence:
  `.agents-artifacts/progress/phase-5.md`,
  `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`,
  and `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`

I did not rerun the full Cargo test suite for this scope review. The evidence
pack records `cargo fmt --check`, `cargo build -p git-outpost`,
`cargo test -p git-outpost --tests`, `cargo test -p outpost-core`,
`cargo test -p outpost-core --tests`, `cargo test --workspace`, and
`git diff --check` as passing
(`.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md:35`).
