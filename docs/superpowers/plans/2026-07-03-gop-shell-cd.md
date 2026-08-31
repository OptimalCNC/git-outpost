# `gop cd` Shell Function Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add shell integration so users can type `gop cd` to change from an outpost to its source repo and `gop cd <outpost>` to change to a resolved outpost path, while every other `gop ...` command still delegates to the real binary.

**Spec:** `docs/superpowers/specs/2026-07-03-gop-shell-cd-spec.md`

**Architecture:** A binary cannot change its parent shell's current directory, so the installed binary will provide `gop shell init [bash|zsh]`, which prints a marker-wrapped Bash/Zsh-compatible function named `gop`. The function removes any existing `gop` alias in the evaluating shell, intercepts only invocations whose first argument is exactly `cd`, and uses `command gop "$@"` to bypass the function and run the actual binary for every other invocation. `gop cd` depends on the Plan 1 `gop path` command for all path resolution, keeping shell code small and avoiding duplicate selector logic.

**Tech Stack:** Rust CLI subcommand, static shell script generation, Bash/Zsh shell function, existing CLI e2e test harness with `bash --noprofile --norc -c`.

## Global Constraints

- User-facing behavior is defined by `docs/superpowers/specs/2026-07-03-gop-shell-cd-spec.md`.
- This plan depends on the `gop path` spec and Plan 1: `gop path src` and `gop path <outpost>` must already work.
- Do not implement `cd` as a Rust binary command; it cannot change the user's current shell.
- Do not add binaries named `gcd`, `gsrc`, or `gout`.
- `gop shell init [SHELL]` must parse `SHELL` as a shell kind, currently `bash` or `zsh`.
- If `SHELL` is omitted, `gop shell init` must print the Bash/Zsh-compatible integration block.
- This milestone implements only `gop shell init`; do not add `gop shell install`, `gop shell uninstall`, shell startup file editing, or a Git Outpost-owned source file.
- The generated function must shadow `gop` only in the shell that evaluates it.
- The generated function must be wrapped in `# >>> git-outpost shell integration >>>` and `# <<< git-outpost shell integration <<<` comments and include a manual-removal comment.
- The generated function must remove an existing `gop` alias in that shell before defining the function.
- The generated function must intercept only invocations whose first argument is exactly `cd`.
- The generated function must delegate all non-`cd` invocations to the binary with `command gop "$@"`.
- The shell function must quote paths safely by storing command output in a variable and calling `cd "$target"`.
- Do not write shell files or edit user startup files in this plan.
- Keep shell support scoped to Bash and Zsh unless a later design explicitly adds Fish or PowerShell.
- Documentation changes must make the one-time setup explicit.

---

## Current Architecture Map

- `crates/cli/src/cli.rs`
  - Owns `clap` command parsing.
  - Add a `Shell(ShellArgs)` command with an `init` subcommand.

- `crates/cli/src/main.rs`
  - Owns dispatch.
  - `Command::Shell` should print static shell integration text and return without touching repository state.

- `crates/cli/src/output.rs`
  - Could print shell text, but a dedicated `crates/cli/src/shell.rs` keeps a long static script out of generic output formatting.

- `crates/cli/src/shell.rs`
  - New module responsible for shell integration text.
  - Should expose `pub fn init_script(shell: Option<ShellKind>) -> &'static str`.

- `crates/cli/tests/e2e.rs`
  - Add tests that run Bash, evaluate the generated function, and verify `pwd` changes inside the same shell process.
  - The tests must put the just-built binary directory on `PATH` before evaluating the function, because the function delegates through `command gop`.
  - Add a skip-if-missing Zsh smoke test; Bash remains the required automated shell because CI does not currently install Zsh.

- `crates/cli/tests/help.rs`
  - Add `shell` to root help and verify `shell init` help tokens.

- `README.md` and `docs/src/product.md`
  - Document one-time setup and daily usage.

---

### Task 1: Add `gop shell init`

**Files:**
- Create: `crates/cli/src/shell.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/cli.rs`
- Test: `crates/cli/tests/help.rs`

**Interfaces:**
- Produces:
  - `Command::Shell(ShellArgs)`
  - `ShellCommand::Init { shell: Option<ShellKind> }`
  - `ShellKind::{Bash, Zsh}`
  - `shell::init_script(shell: Option<ShellKind>) -> &'static str`

- [ ] **Step 1: Write failing help tests**

Modify `crates/cli/tests/help.rs`.

Add `"shell"` to the root command list:

```rust
for command in [
    "add", "pull", "source", "merge", "rebase", "push", "list", "path", "lock", "unlock",
    "move", "remove", "prune", "status", "analyze", "config", "shell",
] {
```

Add this block after the config help assertions:

```rust
let shell_help = help_for(&["shell", "--help"]);
for token in ["init", "shell integration"] {
    assert!(
        shell_help.contains(token),
        "expected {token} in shell help:\n{shell_help}"
    );
}

let shell_init_help = help_for(&["shell", "init", "--help"]);
for token in ["Print shell integration", "gop cd", "SHELL", "bash", "zsh"] {
    assert!(
        shell_init_help.contains(token),
        "expected {token} in shell init help:\n{shell_init_help}"
    );
}
```

- [ ] **Step 2: Run the focused help test and verify it fails**

Run:

```bash
cargo test -p git-outpost --test help e_03_help_lists_commands_and_long_flags --locked
```

Expected: fail because `shell` is not a command.

- [ ] **Step 3: Add CLI argument types**

Modify `crates/cli/src/cli.rs`.

Add to `Command`:

```rust
    /// Print shell integration helpers.
    Shell(ShellArgs),
```

Update `validate_refs`:

```rust
            | Command::Shell(_)
            | Command::Path(_)
```

Update the `clap` import to include `ValueEnum`:

```rust
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
```

Add these types near `ConfigArgs`:

```rust
#[derive(Debug, Args)]
#[command(about = "Print shell integration helpers.")]
pub struct ShellArgs {
    #[command(subcommand)]
    pub command: ShellCommand,
}

#[derive(Debug, Subcommand)]
pub enum ShellCommand {
    /// Print shell integration for `gop cd`.
    Init {
        /// Shell syntax to print.
        #[arg(value_enum, value_name = "SHELL")]
        shell: Option<ShellKind>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShellKind {
    Bash,
    Zsh,
}
```

- [ ] **Step 4: Add the generated shell function**

Create `crates/cli/src/shell.rs`:

```rust
use crate::cli::ShellKind;

pub fn init_script(shell: Option<ShellKind>) -> &'static str {
    match shell {
        Some(ShellKind::Bash) | Some(ShellKind::Zsh) | None => BASH_ZSH_INIT_SCRIPT,
    }
}

const BASH_ZSH_INIT_SCRIPT: &str = r#"# >>> git-outpost shell integration >>>
# Git Outpost shell integration for Bash and Zsh.
# Evaluate with:
#   eval "$(gop shell init bash)"
#   eval "$(gop shell init zsh)"
# Remove this marked block if you manually paste it into a shell startup file.
unalias gop 2>/dev/null || true
gop() {
    if [ "$#" -gt 0 ] && [ "$1" = "cd" ]; then
        shift
        if [ "$#" -eq 0 ]; then
            local _gop_target
            _gop_target="$(command gop path src)" || return
            cd "$_gop_target"
            return
        fi

        if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
            printf '%s\n' 'Usage: gop cd [OUTPOST]'
            printf '%s\n' 'With no OUTPOST, change to the associated source repository.'
            printf '%s\n' 'With OUTPOST, change to the path printed by: gop path OUTPOST'
            return 0
        fi

        local _gop_target
        _gop_target="$(command gop path "$@")" || return
        cd "$_gop_target"
        return
    fi

    command gop "$@"
}
# <<< git-outpost shell integration <<<
"#;
```

- [ ] **Step 5: Wire shell dispatch**

Modify `crates/cli/src/main.rs`.

Add module declaration:

```rust
mod shell;
```

Update imports:

```rust
use cli::{Cli, Command, ConfigCommand, PathTargetArg, ShellCommand, SourceCommand};
```

Add dispatch arm:

```rust
        Command::Shell(args) => match args.command {
            ShellCommand::Init { shell: shell_kind } => {
                print!("{}", shell::init_script(shell_kind));
            }
        },
```

- [ ] **Step 6: Run focused help test**

Run:

```bash
cargo test -p git-outpost --test help e_03_help_lists_commands_and_long_flags --locked
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/cli/src/cli.rs crates/cli/src/main.rs crates/cli/src/shell.rs crates/cli/tests/help.rs
git commit -m "feat: add shell init command"
```

---

### Task 2: Verify The Function Changes Current Working Directory

**Files:**
- Modify: `crates/cli/tests/e2e.rs`

**Interfaces:**
- Consumes:
  - Plan 1 `gop path`
  - Task 1 `gop shell init`
- Produces:
  - e2e proof that `gop cd` changes `pwd` inside the same shell process.
  - e2e proof that shell selection and generated block markers behave as specified.

- [ ] **Step 1: Add Bash e2e test helpers**

Append to `crates/cli/tests/e2e.rs` near the new shell tests:

```rust
#[cfg(unix)]
fn shell_path() -> std::ffi::OsString {
    let bin_dir = common::binary_path("gop")
        .parent()
        .expect("binary directory")
        .to_path_buf();
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin_dir).chain(std::env::split_paths(&existing));

    std::env::join_paths(paths).expect("join PATH")
}

#[cfg(unix)]
fn bash_script(script: &str, fixture: &common::CliFixture) -> std::process::Output {
    let mut command = std::process::Command::new("bash");
    command
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(script)
        .env("GOP_BIN", common::binary_path("gop"))
        .env("PATH", shell_path())
        .env("SOURCE_DIR", &fixture.source)
        .env("ROOT_DIR", &fixture.root);
    common::run(&mut command)
}
```

- [ ] **Step 2: Add generated-script shape tests**

Append to `crates/cli/tests/e2e.rs`:

```rust
#[cfg(unix)]
#[test]
fn shell_init_accepts_supported_shells_and_prints_removable_block() {
    let fixture = common::CliFixture::new();

    for shell in ["bash", "zsh"] {
        let output = fixture.command(["shell", "init", shell]).output().expect("run gop");
        common::assert_success(&output, "gop shell init");
        let stdout = common::stdout(&output);
        assert!(
            stdout.contains("# >>> git-outpost shell integration >>>")
                && stdout.contains("# <<< git-outpost shell integration <<<")
                && stdout.contains("Remove this marked block")
                && stdout.contains("command gop \"$@\""),
            "generated integration should be marker-wrapped and delegate to command gop:\n{stdout}"
        );
    }
}

#[cfg(unix)]
#[test]
fn shell_init_rejects_unsupported_shell() {
    let fixture = common::CliFixture::new();

    let output = fixture.command(["shell", "init", "fish"]).output().expect("run gop");

    assert!(
        !output.status.success(),
        "unsupported shell should fail before shell code is printed"
    );
    assert_eq!(common::stdout(&output), "");
}
```

- [ ] **Step 3: Add failing shell behavior test**

Append to `crates/cli/tests/e2e.rs`:

```rust
#[cfg(unix)]
#[test]
fn shell_gop_cd_changes_directory_in_current_shell() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let source_display = common::displayed_path(&fixture.source);
    let outpost_display = common::displayed_path(&outpost);

    let script = r#"
set -eu
eval "$("$GOP_BIN" shell init bash)"
cd "$ROOT_DIR/C"
gop cd
pwd
gop cd "$ROOT_DIR/C"
pwd
gop status >/dev/null
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop cd");
    let stdout = common::stdout(&output);
    assert_eq!(stdout, format!("{source_display}\n{outpost_display}\n"));
}
```

- [ ] **Step 4: Add alias-shadowing and delegation behavior tests**

Append to `crates/cli/tests/e2e.rs`:

```rust
#[cfg(unix)]
#[test]
fn shell_gop_cd_removes_existing_gop_alias() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let source_display = common::displayed_path(&fixture.source);

    let script = r#"
set -eu
shopt -s expand_aliases
alias gop='printf alias-was-used\n'
eval "$("$GOP_BIN" shell init bash)"
cd "$ROOT_DIR/C"
gop cd
pwd
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop alias shadowing");
    assert_eq!(common::stdout(&output), format!("{source_display}\n"));
}

#[cfg(unix)]
#[test]
fn shell_gop_delegates_non_cd_commands_to_binary() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let script = r#"
set -eu
eval "$("$GOP_BIN" shell init bash)"
cd "$ROOT_DIR/C"
gop status | sed -n '1p'
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop status delegation");
    let stdout = common::stdout(&output);
    assert!(
        stdout.starts_with(&format!("outpost: {}", common::displayed_path(&outpost))),
        "delegated status should print outpost status:\n{stdout}"
    );
}

#[cfg(unix)]
#[test]
fn shell_gop_passes_through_when_first_arg_is_not_cd() {
    let fixture = common::CliFixture::new();

    let script = r#"
set -eu
eval "$("$GOP_BIN" shell init bash)"
gop --help | sed -n '1p'
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop --help passthrough");
    let stdout = common::stdout(&output);
    assert!(
        stdout.contains("Manage self-contained Git outposts"),
        "non-cd calls should pass through to the binary:\n{stdout}"
    );
}
```

- [ ] **Step 5: Add quote-safety and generated-help tests**

Append to `crates/cli/tests/e2e.rs`:

```rust
#[cfg(unix)]
#[test]
fn shell_gop_cd_handles_paths_with_spaces() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C space");
    let outpost_display = common::displayed_path(&outpost);

    let script = r#"
set -eu
eval "$("$GOP_BIN" shell init bash)"
cd "$SOURCE_DIR"
gop cd "$ROOT_DIR/C space"
pwd
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop cd path with spaces");
    assert_eq!(common::stdout(&output), format!("{outpost_display}\n"));
}

#[cfg(unix)]
#[test]
fn shell_gop_cd_help_is_handled_by_function() {
    let fixture = common::CliFixture::new();

    let script = r#"
set -eu
eval "$("$GOP_BIN" shell init bash)"
gop cd --help
"#;

    let output = bash_script(script, &fixture);

    common::assert_success(&output, "bash gop cd --help");
    let stdout = common::stdout(&output);
    assert!(
        stdout.contains("Usage: gop cd [OUTPOST]")
            && stdout.contains("With no OUTPOST"),
        "gop cd --help should describe the shell function:\n{stdout}"
    );
}
```

- [ ] **Step 6: Add skip-if-missing Zsh smoke test**

Append to `crates/cli/tests/e2e.rs`:

```rust
#[cfg(unix)]
#[test]
fn shell_gop_cd_smoke_test_zsh_when_available() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let source_display = common::displayed_path(&fixture.source);

    let mut command = std::process::Command::new("zsh");
    command
        .arg("-f")
        .arg("-c")
        .arg(
            r#"
set -eu
eval "$("$GOP_BIN" shell init zsh)"
cd "$ROOT_DIR/C"
gop cd
pwd
"#,
        )
        .env("GOP_BIN", common::binary_path("gop"))
        .env("PATH", shell_path())
        .env("ROOT_DIR", &fixture.root);

    let output = match command.output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!("run zsh: {err}"),
    };

    common::assert_success(&output, "zsh gop cd");
    assert_eq!(common::stdout(&output), format!("{source_display}\n"));
}
```

- [ ] **Step 7: Run focused shell tests and verify they fail before implementation**

If Task 1 is not implemented yet:

```bash
cargo test -p git-outpost --test e2e shell_gop_ --locked
```

Expected: fail because `gop shell init` or `gop shell init bash` is not implemented.

If Task 1 is implemented, the tests should pass. In that case, record that the failing-test check was satisfied by Task 1's prior failure.

- [ ] **Step 8: Run focused shell tests after Task 1**

Run:

```bash
cargo test -p git-outpost --test e2e shell_gop_ --locked
```

Expected: all `shell_gop_` tests pass on Unix; the Zsh smoke test passes when `zsh` is installed and returns early when it is not.

- [ ] **Step 9: Commit**

```bash
git add crates/cli/tests/e2e.rs
git commit -m "test: verify gop cd shell function"
```

---

### Task 3: Document Setup And Daily Use

**Files:**
- Modify: `README.md`
- Modify: `docs/src/product.md`
- Modify: `docs/src/roadmap.md`

**Interfaces:**
- Consumes:
  - `gop shell init`
  - shell function semantics from Task 1
- Produces:
  - clear user-facing setup instructions.

- [ ] **Step 1: Update README**

In `README.md`, after the `gop path` usage added by Plan 1, add:

````markdown
Enable shell navigation in Bash or Zsh for the current shell:

```bash
eval "$(gop shell init bash)"   # Bash
eval "$(gop shell init zsh)"    # Zsh
```

For one-time setup, manually add the matching line to `~/.bashrc` or
`~/.zshrc`. The generated shell block is wrapped in `git-outpost shell
integration` comments, so removing that marked block or line removes the
integration from future shells. `gop shell install` and `gop shell uninstall`
are not part of this milestone.

Then:

```bash
gop cd        # from an outpost, cd to its source repository
gop cd ../my-change
```
````

- [ ] **Step 2: Update product Story**

In `docs/src/product.md`, after the `gop path` navigation paragraph from Plan 1, add:

```markdown
Because a child process cannot change its parent shell's current directory,
`gop cd` is provided by shell integration rather than by the binary itself.
After evaluating `gop shell init bash` or `gop shell init zsh`, the generated
function shadows `gop` in that shell, intercepts only invocations whose first
argument is exactly `cd`, and delegates every other `gop ...` invocation to the
installed binary with `command gop "$@"`. `gop cd` changes to the associated
source repository, and `gop cd <outpost>` changes to the path resolved by
`gop path <outpost>`.
```

- [ ] **Step 3: Update product Synopsis**

In `docs/src/product.md`, add these lines to the Synopsis after `gop path <src|outpost>`:

```text
gop shell init [bash|zsh]
gop cd [<outpost>]   # after evaluating gop shell init
```

- [ ] **Step 4: Update product Working Directory Matrix**

In `docs/src/product.md`, add a `shell init` row near `config`:

```markdown
| `shell init [bash\|zsh]` | Prints shell integration; does not inspect repo state | Prints shell integration; does not inspect repo state |
```

Do not add a matrix row for `gop cd`, because `gop cd` is a shell function after setup rather than a binary subcommand.

- [ ] **Step 5: Add product command reference**

In `docs/src/product.md`, before the `status` section, add:

````markdown
### `shell init [bash|zsh]`

Print Bash/Zsh shell integration. Pass `bash` or `zsh` to select the generated
shell syntax. Evaluate it in the current shell to define a `gop` function that
intercepts only invocations whose first argument is exactly `cd` and delegates
every other `gop ...` invocation to the binary with `command gop "$@"`.

```bash
eval "$(gop shell init bash)"
eval "$(gop shell init zsh)"
```

For one-time setup, add the matching line to `~/.bashrc` or `~/.zshrc`. The
generated shell block is wrapped in marker comments and says how to remove the
marked block if it is manually pasted into a startup file. `gop shell install`
and `gop shell uninstall` are not part of this milestone.

After setup, `gop cd` changes to the associated source repository. `gop cd
<outpost>` changes to the path resolved by `gop path <outpost>`. Existing
aliases named `gop` are removed by the generated integration in the shell that
evaluates it.
````

- [ ] **Step 6: Update roadmap deployment scope**

In `docs/src/roadmap.md`, add a Present row to the deployment table after the `gop` binary row:

```markdown
| `gop shell init [bash\|zsh]` | Present | Prints marker-wrapped Bash/Zsh shell integration that shadows `gop` only to implement `gop cd`; calls whose first argument is not exactly `cd` delegate to the binary. It does not install or uninstall shell startup configuration. Bash behavior is covered in CI; Zsh is smoke-tested when available. |
```

- [ ] **Step 7: Build docs**

Run:

```bash
mdbook build docs
```

Expected: docs build succeeds.

- [ ] **Step 8: Commit**

```bash
git add README.md docs/src/product.md docs/src/roadmap.md
git commit -m "docs: document gop cd shell integration"
```

---

### Task 4: Final Verification For Plan 2

**Files:**
- No additional edits.

**Interfaces:**
- Consumes Plan 1 plus all shell-function tasks.
- Produces final evidence that shell integration works.

- [ ] **Step 1: Run formatting**

```bash
cargo fmt --all -- --check
```

Expected: pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features --locked
```

Expected: pass.

- [ ] **Step 3: Run tests**

```bash
cargo test --workspace --locked
```

Expected: pass.

- [ ] **Step 4: Build docs**

```bash
mdbook build docs
```

Expected: pass.

- [ ] **Step 5: Manual smoke test in one shell**

Run in a disposable Bash or Zsh session:

```bash
eval "$(gop shell init bash)"
cd /path/to/an/outpost
pwd
gop cd
pwd
gop cd /path/to/an/outpost
pwd
gop status
```

Expected:
- First `pwd` prints the outpost path.
- After `gop cd`, `pwd` prints the associated source repository path.
- After `gop cd /path/to/an/outpost`, `pwd` prints the outpost path.
- `gop status` still delegates to the binary and prints the normal status report.

- [ ] **Step 6: Commit if verification required a correction**

```bash
git add README.md docs/src/product.md docs/src/roadmap.md crates/cli/src/cli.rs crates/cli/src/main.rs crates/cli/src/shell.rs crates/cli/tests/e2e.rs crates/cli/tests/help.rs
git commit -m "fix: align shell integration behavior"
```

Only run this commit command if final verification forced additional tracked edits.
