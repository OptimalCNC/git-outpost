**Verdict**

Changes requested. The CLI scaffold mostly matches the documented command
surface, and the local CLI tests pass, but two blocking acceptance issues were
found: the new dependency set breaks the documented MSRV, and H-03 does not
test the documented `git outpost --help` behavior.

**Findings**

1. **High: New `clap` resolution violates the documented Rust 1.75 MSRV.**
   Workspace MSRV is `1.75` in `Cargo.toml`, and the architecture reiterates
   MSRV `1.75` plus `clap` min `4.5`. This commit adds `clap = "^4.6"` in
   `Cargo.toml`, and `Cargo.lock` resolves `clap 4.6.1`. The resolved crate
   declares `rust-version = "1.85"`, so the new CLI package is not buildable
   under the documented MSRV.
2. **High: H-03 is not actually covered as specified.** H-03 requires
   `git outpost --help` in `docs/src/architecture.md`. The test uses
   `git outpost -h` instead. Manual check confirms `git outpost --help` exits
   with Git's manpage lookup, while `git outpost -h` reaches the binary.
3. **Medium: E-03 can pass while the real parser surface drifts.** Root help
   lists command-specific flags through a hard-coded `ROOT_AFTER_HELP` string,
   and the E-03 test only checks `help.contains(flag)`. This can pass even if
   the actual clap args are removed, renamed, or moved.
4. **Low: E-15 coverage is narrower than the deferred/removed surface list.**
   The test covers a useful subset, but obvious deferred surfaces such as
   removed `add`, `list`, and `push` flags are not pinned.

**Required Changes**

- Resolve the MSRV conflict by selecting a `clap` version compatible with Rust
  1.75, or explicitly update the project MSRV and docs.
- Resolve the H-03 contract: either support/test the documented
  `git outpost --help` path, or update the acceptance docs to specify
  `git outpost -h` because Git intercepts `--help`.
- Strengthen E-03 so command-specific long flags are asserted against actual
  subcommand help and/or positive parse behavior, not only the manual root help
  string.

**Suggested Nits**

- Expand E-15 with at least one removed `add` flag, one deferred list
  formatting flag, and one removed push flag.
- Consider documenting that non-help commands are parse/validate-only in this
  chunk directly near `main.rs`, because they currently exit 0 without
  dispatch by design.

**Test/Verification Assessment**

The reviewer independently ran `cargo test -p git-outpost --tests`; all 7 CLI
tests passed. The reviewer also manually checked `gop --help`,
`git-outpost --help`, `git outpost -h`, rejected flags, and
`git outpost --help`. The full workspace and an actual Rust 1.75 toolchain were
not rerun by the reviewer; the MSRV issue is based on resolved dependency
metadata.

**Residual Risk**

Real command dispatch, `-C` behavior, `--no-color`, output formatting, and
exit-code mapping remain deferred. The parse-only scaffold means later chunks
must be careful not to let current success exits mask missing dispatch paths.
Cross-platform behavior for Git dispatch help also needs CI confirmation.
