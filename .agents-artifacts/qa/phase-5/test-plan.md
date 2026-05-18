**Summary**

Phase 5 QA is CLI-first: add integration tests under `crates/cli/tests/*.rs`
that spawn `git-outpost`, `gop`, and Git dispatch where needed, while reusing
the existing A/B/C fixture behavior through CLI-local helpers. Core tests remain
untouched unless a tiny helper exposure is needed to share hermetic Git fixture
setup.

**Phase Test IDs Covered**

E-01, E-02, E-03, E-04, E-05, E-06, E-07, E-08, E-09, E-10, E-11, E-12, E-13,
E-14, E-15, H-01, H-02, H-03

**Test Coverage Map**

| ID | test file | test name | behavior | status |
|---|---|---|---|---|
| E-01 | `crates/cli/tests/e2e.rs` | `build_creates_both_debug_binaries` | `cargo build -p git-outpost` creates `git-outpost[.exe]` and `gop[.exe]`. | planned |
| E-02 | `crates/cli/tests/e2e.rs` | `all_invocation_forms_produce_same_status_stdout` | `git outpost status`, `git-outpost status`, and `gop status` match for the same outpost. | planned |
| E-03 | `crates/cli/tests/help.rs` | `gop_help_lists_commands_and_long_flags_once` | Help lists every subcommand once and includes all long flags from the CLI surface. | planned |
| E-04 | `crates/cli/tests/e2e.rs` | `basic_cli_lifecycle_round_trip_exits_zero` | `add`, `status`, `push`, `list`, and `remove` all exit 0 in the documented contexts. | planned |
| E-05 | `crates/cli/tests/e2e.rs` | `push_makes_outpost_commit_visible_upstream` | `gop push` publishes C commit through B to A. | planned |
| E-06 | `crates/cli/tests/e2e.rs` | `two_outposts_sync_through_source` | C1 push then C2 pull makes the change visible in C2 through B. | planned |
| E-07 | `crates/cli/tests/e2e.rs` | `copied_outpost_is_git_independent_when_source_is_missing` | Copied C remains normal Git repo after B deletion; `gop status` reports degraded source-missing state. | planned |
| E-08 | `crates/cli/tests/flags.rs` | `outpost_errors_map_to_documented_exit_codes` | Representative CLI scenarios cover every `OutpostError` exit-code mapping. | planned |
| E-09 | `crates/cli/tests/flags.rs` | `no_color_flag_and_env_strip_ansi_output` | `--no-color` and `NO_COLOR=1` produce output with no ANSI escapes. | planned |
| E-10 | `crates/cli/tests/e2e.rs` | `story_flow_exits_zero` | `add -b`, commit, `source pull`, `rebase local/main`, and `push` exit 0. | planned |
| E-11 | `crates/cli/tests/e2e.rs` | `merge_and_rebase_accept_story_source_ref` | `merge local/main` and `rebase local/main` accept the Story source-ref form. | planned |
| E-12 | `crates/cli/tests/flags.rs` | `global_c_changes_effective_cwd` | `gop -C <other-dir> status` behaves as if started in that directory. | planned |
| E-13 | `crates/cli/tests/flags.rs` | `add_detach_is_rejected_by_clap` | `gop add --detach C main` returns clap usage error. | planned |
| E-14 | `crates/cli/tests/flags.rs` | `add_target_branch_starting_with_dash_returns_invalid_ref` | `gop add C -- -evil` returns `InvalidRefName`, not `GitFailed`. | planned |
| E-15 | `crates/cli/tests/flags.rs` | `deferred_and_removed_surfaces_are_rejected_by_clap` | `--json`, `--quiet`, `list --all`, `prune --expire`, and pull strategy flags are usage errors. | planned |
| H-01 | `crates/cli/tests/help.rs` | `git_outpost_help_uses_git_outpost_program_name` | `git-outpost --help` renders `git-outpost` as program name. | planned |
| H-02 | `crates/cli/tests/help.rs` | `gop_help_uses_gop_program_name` | `gop --help` renders `gop` as program name. | planned |
| H-03 | `crates/cli/tests/help.rs` | `git_dispatch_help_does_not_render_gop_program_name` | `git outpost --help` renders `git outpost` or `git-outpost`, but not `gop`. | planned |

**Fixture Changes Needed**

`crates/cli/tests/common/mod.rs` should provide CLI-oriented helpers: temp
A/B/C setup, hermetic Git env, binary path lookup for both debug binaries,
command builders using `assert_cmd`, cross-platform `.exe` suffix handling,
ANSI stripping helper, file commit helpers, and recursive copy support for
E-07.

If feasible, reuse or mirror the existing core A/B/C fixture shape. Expose
only tiny core test helper APIs if duplication becomes risky; do not move
Phase 5 behavior assertions into core tests.

**Tests To Write Before Implementation**

After minimal CLI crate scaffolding exists, write failing CLI integration tests
for E-01, E-03, E-12, E-13, E-15, H-01, H-02, and H-03 first. These pin binary
targets, clap surface, global flags, removed/deferred surfaces, and help naming
before command dispatch behavior is filled in.

**Tests To Write After API Stabilizes**

Write E-02, E-04, E-05, E-06, E-07, E-08, E-09, E-10, E-11, and E-14 after CLI
dispatch, output formatting, reporter rendering, color policy, and
error-reporting shape are stable enough to avoid locking tests to throwaway
text.

**Blocked Tests**

None permanently blocked. All Phase 5 tests are temporarily blocked on creating
`crates/cli` and adding the `git-outpost` package to the workspace.

**Verification Commands**

```bash
cargo build -p git-outpost
cargo test -p git-outpost --tests
cargo test -p outpost-core
cargo test -p outpost-core --tests
cargo test --workspace
```

**Risks**

Cross-platform confidence depends on CI runners beyond local Linux. E-08 may
become brittle if it asserts full stderr; prefer exit code plus a focused error
substring. E-07 must use a Rust copy helper such as `fs_extra`, not shell tools.
Color tests should strip ANSI with `strip-ansi-escapes` rather than matching
literal escape bytes.
