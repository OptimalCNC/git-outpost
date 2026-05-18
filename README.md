# Git Outpost

[![CI](https://github.com/OptimalCNC/git-outpost/actions/workflows/ci.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/ci.yml)
[![Development](https://github.com/OptimalCNC/git-outpost/actions/workflows/dev.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/dev.yml)
[![Integration](https://github.com/OptimalCNC/git-outpost/actions/workflows/integration.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/integration.yml)
[![Release](https://github.com/OptimalCNC/git-outpost/actions/workflows/release.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/release.yml)
[![crates.io: git-outpost](https://img.shields.io/crates/v/git-outpost.svg?label=git-outpost)](https://crates.io/crates/git-outpost)
[![crates.io: outpost-core](https://img.shields.io/crates/v/outpost-core.svg?label=outpost-core)](https://crates.io/crates/outpost-core)

Git Outpost is a Rust command-line tool for creating self-contained Git
checkouts from an existing local repository. It gives you a `git worktree`-like
workflow, but each outpost is a normal clone with its own `.git` directory, so
editors and devcontainers can open it without extra repository metadata mounts.

Detailed user documentation lives in [docs/src/product.md](docs/src/product.md).

## Usage

Install the CLI:

```bash
cargo install git-outpost
```

Create a new outpost from a source repository:

```bash
cd /path/to/source-repo
gop add -b feature/my-change ../my-change main
cd ../my-change
git status
```

Publish the current outpost branch through the source repository:

```bash
gop push
```

The installed commands are equivalent:

```bash
git-outpost status
git outpost status
gop status
```

## Contributing

This is a Cargo workspace with the implementation split between:

- `crates/core`: Git Outpost library logic.
- `crates/cli`: CLI parsing, output, and binary entry points.

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked
cargo test --workspace --locked
```

Development details are in [docs/src/architecture.md](docs/src/architecture.md),
and planned work is tracked in [docs/src/roadmap.md](docs/src/roadmap.md).

## License

Git Outpost is licensed under the [MIT License](LICENSE).
