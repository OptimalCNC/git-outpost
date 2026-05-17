# Normal Review: Phase 0 / git-and-ref-boundary

## Verdict

needs changes

## Evidence Reviewed

- Commit `c144b69`
- Evidence pack: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/evidence-pack.md`
- Scope review: `.agents-artifacts/reviews/phase-0/git-and-ref-boundary/scope-review.md`
- Progress log: `.agents-artifacts/progress/phase-0.md`
- Files: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/git.rs`, `crates/core/src/refname.rs`
- Source docs: `docs/src/roadmap.md` Phase 0 table, `docs/src/architecture.md` sections 5.2, 5.3, 10, 11.1, and Documentation Policy in `docs/coordinator-prompt.md`
- Verification evidence listed in the evidence pack

## Requirement Reasoning

Phase scope is appropriate: `git.rs`, `refname.rs`, exports, `test-helpers`, and colocated U-09/U-11/U-12 tests are within Phase 0. `GitInvoker` implements cwd pinning, env overrides, stdout/stderr capture, `run_capture`, `run_check`, `run_status`, and signal mapping. `refname.rs` implements the documented boundary types and validation shape. However, U-09 and architecture section 5.2 require failed argv to be preserved exactly/verbatim, and the implementation loses argv boundaries in `GitFailed.args`.

## Test Reasoning

Tests cover U-11 leading-dash positional handling through an explicit `--`, U-12 branch/remote validation examples, `RefName`, `SourceRemoteRef`, `UpstreamRef::short_branch`, and `run_status` exit-code semantics. They do not prove U-09's "exact argv preserved" requirement because the U-09 test expects a space-joined string for an argument containing spaces, which is indistinguishable from multiple separate argv elements.

## Docs Reasoning

No new docs were required because the stable module/API contracts already exist in `docs/src/architecture.md`. Documentation quality is acceptable, but the implementation currently does not match the documented "exact argv preserved" contract, making the code the issue rather than missing docs.

## Verification Reasoning

Supplied verification shows `cargo fmt --check`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, and `cargo test -p outpost-core --features test-helpers` all passing. These commands prove the current tests pass, but they do not close the U-09 semantic gap above.

## Findings

- Severity: blocking. Evidence: `docs/src/architecture.md` section 5.2 requires `GitFailed` with exact argv preserved; U-09 requires failed argv preserved verbatim. `crates/core/src/git.rs` converts argv elements with `to_string_lossy()` and `.join(" ")`. The U-09 test passes `["--literal", "value with spaces"]` but expects `"--literal value with spaces"`. Issue: argument boundaries are not preserved, and non-UTF-8 argv would also be lossy. Why it matters: the central U-09 contract is not actually satisfied; failure diagnostics can misrepresent the command that was run.

## Missing Evidence

none

## Required Changes

Preserve failed argv unambiguously in `GitFailed.args`/`GitTerminatedBySignal.args` or otherwise align the error representation with the documented "exact argv" contract, and update U-09 to distinguish a single argv element containing spaces from multiple argv elements.

## Notes

`test-helpers` is feature-gated and verified directly, but it is not yet exercised by an integration test; acceptable for this chunk because fixture/integration work remains in a later Phase 0 chunk.
