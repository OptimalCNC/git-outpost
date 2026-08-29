# Git Outpost

Documentation: [GitHub Pages](https://optimalcnc.github.io/git-outpost/)

[![CI](https://github.com/OptimalCNC/git-outpost/actions/workflows/ci.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/OptimalCNC/git-outpost/graph/badge.svg)](https://codecov.io/gh/OptimalCNC/git-outpost)
[![Development](https://github.com/OptimalCNC/git-outpost/actions/workflows/dev.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/dev.yml)
[![Integration](https://github.com/OptimalCNC/git-outpost/actions/workflows/integration.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/integration.yml)
[![Release](https://github.com/OptimalCNC/git-outpost/actions/workflows/release.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/release.yml)
[![Publish Docs](https://github.com/OptimalCNC/git-outpost/actions/workflows/docs.yml/badge.svg)](https://github.com/OptimalCNC/git-outpost/actions/workflows/docs.yml)
[![crates.io: git-outpost](https://img.shields.io/crates/v/git-outpost.svg?label=git-outpost)](https://crates.io/crates/git-outpost)
[![crates.io: outpost-core](https://img.shields.io/crates/v/outpost-core.svg?label=outpost-core)](https://crates.io/crates/outpost-core)

Git Outpost is a Rust command-line tool for creating self-contained Git
checkouts from an existing local repository. It gives you a `git worktree`-like
workflow, but each outpost is a normal clone with its own `.git` directory, so
editors and devcontainers can open it without extra repository metadata mounts.

Detailed user documentation is published on [GitHub Pages](https://optimalcnc.github.io/git-outpost/)
and maintained in [docs/src/product.md](docs/src/product.md).

## Usage

Install the CLI:

```bash
cargo install git-outpost
```

Create a new outpost from a source repository:

```bash
cd /path/to/source-repo
gop config set outpost-container ..
gop add -b feature/my-change
cd ../my-change
git status
```

Check out a branch that may exist only on `origin`:

```bash
gop add ../review-docs docs/review-update
```

When the branch is missing locally, an interactive terminal asks before
fetching it. Non-interactive callers must grant that consent explicitly with
`--fetch-missing`; otherwise `add` remains local-only and fails without
fetching.

Enable shell navigation in the current shell:

```bash
eval "$(gop shell init bash)"   # Bash
eval "$(gop shell init zsh)"    # Zsh
```

For persistent setup, let Git Outpost manage a small source block in your shell
startup file:

```bash
gop shell install bash          # writes ~/.bashrc + ~/.config/git-outpost/shell.bash
gop shell install zsh           # writes ~/.zshrc + ~/.config/git-outpost/shell.zsh
```

Run the install command again after upgrading Git Outpost to refresh the
generated shell integration. Remove the managed block and generated file with:

```bash
gop shell uninstall bash
gop shell uninstall zsh
```

If you run `gop cd` before enabling shell integration, the binary prints setup
instructions and exits without changing directories.

Then:

```bash
gop cd        # from an outpost, cd to its source repository
gop cd ../my-change
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

## Agent skill

Install the `using-git-outpost` skill globally with the
[Vercel Skills CLI](https://github.com/vercel-labs/skills):

```bash
npx skills add OptimalCNC/git-outpost \
  --skill using-git-outpost \
  --global
```

The skill uses one context-aware `gop status` call to identify a source or
managed outpost and report the optional `outpost-container`. It maps ordinary
worktree and parallel-checkout requests to `gop add`, leaves whether and where
to configure the container to the agent, and covers navigation,
synchronization, publication, and lifecycle safety.

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
