# Roadmap

## Implementation Direction

The tool is implemented as a Rust workspace with one core library crate and one
CLI crate/package with two binary targets sharing one `src/main.rs` entrypoint:

- `cargo install git-outpost` is the primary installation method.
- The CLI crate/package produces two binary names, `git-outpost` and `gop`, so Git can
  dispatch `git outpost ...` to the long name while users type `gop ...` for
  everyday work.
- A standard Rust CLI parsing crate handles arguments and subcommands.
- Regular `git` commands are invoked through the standard library's process
  API.
- One CLI crate/package with two binary targets sharing one entrypoint keeps installation, distribution, and updates
  simple; there is no interpreter or runtime to manage on the host.

The implementation should start with the core lifecycle commands:

```text
add, pull, source pull, merge, rebase, push, list, lock, unlock, move, remove, prune, status
```

The first version should prioritize predictable behavior and clear errors
over clever automation. More advanced global registry behavior can be added
once the basic workflow is proven.

## Implementation Phases

Each phase ends with green tests for everything in scope. Command phases include
the command's core behavior plus narrow command-specific CLI parsing/formatting
checks where listed; Phase 5 covers whole-binary, e2e, and global CLI behavior.

| Phase | Scope | Tests in scope |
|---|---|---|
| 0 | Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture | U-07..U-09, U-11, U-12 |
| 1 | `source_repo.rs`, `outpost.rs`, `metadata.rs` (Raw+validated), `registry.rs` (incl. Drop guard), `safety.rs`, `ops::add` (incl. add ConfigChange `Reporter` event), `ops::list` | U-01..U-06, U-10, U-13..U-15, C-01..C-20, L-01..L-10 |
| 2 | `ops::lock`, `ops::move`, `ops::unlock`, `ops::remove`, `ops::prune` | LMU-01..LMU-08, R-01..R-11, Pr-01..Pr-09 |
| 3 | `ops::status` (use `RawMetadata` for status reporting) | S-01..S-13 |
| 4 | `ops::source`, `ops::pull` (UpstreamRef-driven), `ops::merge`, `ops::rebase`, `ops::push`, sync `Reporter` events | SP-01..SP-05, P-01..P-09, MR-01..MR-06, Pu-01..Pu-10 |
| 5 | CLI binaries, exit codes, `--no-color`, global `-C`, E2E, cross-platform | E-01..E-15, H-01..H-03 |

The phases are review milestones, not Cargo features.
