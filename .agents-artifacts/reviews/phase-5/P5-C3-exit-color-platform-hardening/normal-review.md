# P5-C3 Exit Color Platform Hardening Normal Review

## Verdict

Changes requested.

Reviewed commits:

- `47d10fd phase-5: harden exit color platform behavior`
- `858f61e phase-5: record exit color hardening commit`

Assumption: existing dirty worktree entries (`crates/cli/Cargo.toml`, `.github/`, `README.md`) are unrelated to these commits. I left them untouched.

## Findings

1. High: `GitFailed` negative process codes map to CLI success on Windows.

   `crates/core/src/error.rs:127` clamps `GitFailed { code }` with `(*code).clamp(0, 125)`, so any negative code becomes `0`. P5-C3 then locks that behavior into the new E-08 test at `crates/cli/tests/flags.rs:152`, expecting `GitFailed { code: -1 }` to return exit code `0`.

   This is a cross-platform CLI error regression risk. Rust's Windows `ExitStatus::code()` returns `Some(self.0 as i32)` for the process `u32` status, so high-bit Windows process statuses are represented as negative `i32` values rather than `None`. If Git exits with one of those statuses, `GitInvoker` reports `GitFailed`, `exit::report` prints `error: ...`, and the process exits successfully. Map negative `GitFailed` codes to a non-zero failure code, then update the E-08 assertion so the test guards that behavior instead of preserving success.

2. Medium: E-08 black-box CLI checks do not assert error text.

   The Phase 5 QA/test plan says E-08 should assert exit code plus focused error substrings (`.agents-artifacts/progress/phase-5.md:117`), but the new CLI smoke test only checks buckets: examples include `crates/cli/tests/flags.rs:177`, `crates/cli/tests/flags.rs:180`, `crates/cli/tests/flags.rs:183`, `crates/cli/tests/flags.rs:220`, `crates/cli/tests/flags.rs:223`, and `crates/cli/tests/flags.rs:239`.

   That means wrong error routing can pass as long as the code bucket matches, especially within bucket 2 and bucket 3 where multiple distinct failures share an exit code. Add focused stderr assertions for the representative black-box cases so CLI output/error regressions are covered.

3. Low: Progress metadata remains stale after the record commit.

   The reviewed progress-record commit updates the implementation/evidence commit, but `.agents-artifacts/progress/phase-5.md:306` still says `pending phase-5: start exit color platform hardening`, and `.agents-artifacts/progress/phase-5.md:320` still recommends committing the start marker. That conflicts with the recorded implementation commit at `.agents-artifacts/progress/phase-5.md:217` and can mislead later reviewers or closeout.

## Verification

Commands run:

- `cargo fmt --check`: pass
- `cargo build -p git-outpost`: pass; Cargo emitted the existing warning that `src/main.rs` is used by both `git-outpost` and `gop`
- `cargo test -p git-outpost --tests`: pass; 9 e2e, 8 flags, 4 help tests
- `cargo test -p outpost-core`: pass; full core suite and 0 doctests
- `cargo test -p outpost-core --tests`: pass
- `cargo test --workspace`: pass
- `git diff --check HEAD^..HEAD`: pass
- `git diff --check 47d10fd^..858f61e`: pass

Inspected:

- `git diff --no-renames 47d10fd^ 47d10fd -- ...`
- `git diff --no-renames 858f61e^ 858f61e -- .agents-artifacts/progress/phase-5.md`
- `crates/cli/src/output.rs`
- `crates/cli/tests/common/mod.rs`
- `crates/cli/tests/e2e.rs`
- `crates/cli/tests/flags.rs`
- `.agents-artifacts/reviews/phase-5/P5-C3-exit-color-platform-hardening/evidence-pack.md`
- `.agents-artifacts/qa/phase-5/P5-C3-exit-color-platform-hardening.md`
- relevant product/architecture docs for status output, E-07/E-08/E-09, exit-code mapping, and cross-platform notes
- local Rust std Windows `ExitStatus::code()` implementation to validate the negative-code path
