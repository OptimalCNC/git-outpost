# Roadmap

## Roadmap Status

Roadmap entries use these status values:

- **Done**: implemented and verified by the referenced phase closeout.
- **Present**: already in the repository as part of the delivery path, but not
  tracked as a product feature phase.
- **Planned**: agreed in the product and architecture docs, but not yet
  implemented.

New product work should be added as a numbered phase only after
`docs/src/product.md`, `docs/src/architecture.md`, and this roadmap agree on the
behavior, storage model, test IDs, and closeout gates.

## Planning New Phases

Use the phase table for the normal traceability record. Add a dedicated
subsection only when the phase needs roadmap-level planning context, such as
chunk sequencing, cross-phase dependencies, deployment steps, or scope
boundaries that are likely to be misunderstood.

A planned phase should name the outcome, tests, evidence path, and closeout
gate. Product behavior belongs in `docs/src/product.md`; implementation details,
invariants, and test IDs belong in `docs/src/architecture.md`.

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
add, pull, source pull, merge, rebase, push, list, lock, unlock, move, remove, prune, status, analyze
```

The first version should prioritize predictable behavior and clear errors
over clever automation. More advanced global registry behavior can be added
once the basic workflow is proven.

## Deployment Scope

The MVP deploys as a Cargo-delivered local CLI. It does not deploy a hosted
service, daemon, database, API server, or provider-specific integration.

| Artifact | Status | Deployment / use |
|---|---|---|
| `outpost-core` crate | Present | Library crate for Git Outpost core behavior. It is published before the CLI crate because `git-outpost` depends on it. |
| `git-outpost` crate/package | Present | End-user package installed with `cargo install git-outpost`; installs both `git-outpost` and `gop`. |
| `git-outpost` binary | Present | Canonical binary; Git dispatches `git outpost ...` to it. |
| `gop` binary | Present | Short alias for everyday use; same CLI entrypoint as `git-outpost`. |
| GitHub CI / release workflows | Present | Validate formatting, clippy, tests, docs, packaging, cross-platform integration, and crates.io publishing on release paths. |

## Implementation Phases

Each phase ends with green tests for everything in scope. Command phases include
the command's core behavior plus narrow command-specific CLI parsing/formatting
checks where listed; Phase 5 covers whole-binary, e2e, and global CLI behavior.

| Phase | Status | Scope delivered | Tests in scope | Evidence |
|---|---|---|---|---|
| 0 | Done | Cargo workspace skeleton, `error.rs`, `git.rs`, `refname.rs`, `reporter.rs`, fixture | U-07..U-09, U-11, U-12 | `.agents-artifacts/progress/phase-0.md` |
| 1 | Done | `source_repo.rs`, `outpost.rs`, `metadata.rs` (Raw+validated), `registry.rs` (incl. Drop guard), `safety.rs`, `ops::add` (incl. add ConfigChange `Reporter` event), `ops::list` | U-01..U-06, U-10, U-13..U-15, C-01..C-20, L-01..L-10 | `.agents-artifacts/progress/phase-1.md` |
| 2 | Done | `ops::lock`, `ops::move`, `ops::unlock`, `ops::remove`, `ops::prune` | LMU-01..LMU-08, R-01..R-11, Pr-01..Pr-09 | `.agents-artifacts/progress/phase-2.md` |
| 3 | Done | `ops::status` (use `RawMetadata` for status reporting) | S-01..S-13 | `.agents-artifacts/progress/phase-3.md` |
| 4 | Done | `ops::source`, `ops::pull` (UpstreamRef-driven), `ops::merge`, `ops::rebase`, `ops::push`, sync `Reporter` events | SP-01..SP-05, P-01..P-09, MR-01..MR-06, Pu-01..Pu-10 | `.agents-artifacts/progress/phase-4.md` |
| 5 | Done | CLI binaries, exit codes, `--no-color`, global `-C`, E2E, cross-platform | E-01..E-15, H-01..H-03 | `.agents-artifacts/progress/phase-5.md` |

The phases are review milestones, not Cargo features.
