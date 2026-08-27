# `gop shell install|uninstall` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persistent shell integration management with `gop shell install <bash|zsh>` and `gop shell uninstall <bash|zsh>`, plus a dummy binary `gop cd [<outpost>]` command for discoverability and setup guidance.

**Spec:** `docs/superpowers/specs/2026-07-04-gop-shell-install-uninstall-spec.md`

**Architecture:** Keep real `gop cd` behavior in the existing generated `gop shell init` script, because only a shell function can change the caller's current directory. Add a dummy binary `gop cd [<outpost>]` command that is visible in help and returns setup guidance when shell integration is not active. Add a shell installation layer in the CLI crate that writes the generated script to a Git Outpost-owned file and manages a small marker-wrapped source block in the selected shell startup file.

**Tech Stack:** Rust workspace, `clap` CLI parsing, existing `ShellKind`, existing `shell::init_script`, standard filesystem APIs, `tempfile` for safer writes, existing CLI e2e test harness.

## Global Constraints

- User-facing behavior is defined by `docs/superpowers/specs/2026-07-04-gop-shell-install-uninstall-spec.md`.
- Reuse `gop shell init <shell>` output for the generated script file; do not duplicate `gop cd` function behavior.
- Add a top-level dummy binary `gop cd [<outpost>]` command for help/discovery and setup guidance only.
- The dummy binary `gop cd` command must not change directories, resolve `<outpost>`, call `gop path`, inspect Git state, or mutate files.
- When the binary receives `gop cd [<outpost>]`, it must print shell setup guidance to stderr and exit non-zero.
- `gop shell install <shell>` and `gop shell uninstall <shell>` require an explicit `bash` or `zsh` shell kind.
- Keep `gop shell init [shell]` behavior unchanged.
- `install` and `uninstall` must not require source or outpost context.
- `install` and `uninstall` own only the block between `# >>> git-outpost shell install >>>` and `# <<< git-outpost shell install <<<`.
- Fail without modifying the startup file when managed block markers are malformed or duplicated.
- Do not remove manually pasted `gop shell init` output or manual `eval "$(gop shell init ...)"` lines.
- Relative `--rc-file` and `--script-file` paths must resolve against the effective current directory, including global `-C`.
- Documentation must present `install` as the recommended persistent setup and `init` as the current-shell/scriptable primitive.

---

## Current Architecture Map

- `crates/cli/src/cli.rs`
  - Owns `clap` command parsing.
  - Already defines `ShellArgs`, `ShellCommand::Init { shell: Option<ShellKind> }`, and `ShellKind::{Bash, Zsh}`.
  - Add top-level `Command::Cd(CdArgs)` so root help lists `cd`.
  - Add `ShellCommand::Install(ShellManageArgs)` and `ShellCommand::Uninstall(ShellManageArgs)`.

- `crates/cli/src/main.rs`
  - Owns dispatch and computes `effective_cwd`.
  - `Command::Cd` should return CLI-only setup guidance before any source/outpost context checks.
  - Shell install/uninstall should dispatch without calling source/outpost context helpers.
  - Use `resolve_path_arg(&cwd, path)` for relative `--rc-file` and `--script-file` overrides.

- `crates/cli/src/exit.rs`
  - Currently aliases `CliResult<T>` to `OutpostResult<T>`.
  - Introduce a small CLI error wrapper so `gop cd` can return a CLI-only setup-guidance error without adding shell-specific variants to `outpost-core`.

- `crates/cli/src/shell.rs`
  - Owns the generated shell function returned by `init_script`.
  - Add a submodule for installation mechanics instead of growing this file into rc-file parsing and IO logic.
  - Re-export `InstallOptions`, `ShellInstallReport`, `install`, and `uninstall`.

- `crates/cli/src/shell/install.rs`
  - New focused module for path defaults, shell quoting, managed block rendering, managed block parsing, and file operations.

- `crates/cli/tests/e2e.rs`
  - Owns whole-binary behavior tests.
  - Add tests proving binary `gop cd` fails with setup guidance and does not require repo context.
  - Add tests using explicit temp `--rc-file` and `--script-file` paths so no real user startup file is touched.

- `crates/cli/tests/help.rs`
  - Owns help listing expectations.
  - Add `install` and `uninstall` help coverage under `shell`.

- `README.md`, `docs/src/product.md`, `docs/src/roadmap.md`
  - Update persistent shell setup docs from manual `eval` guidance to `gop shell install <shell>`.

---

### Task 1: Add CLI Shape, Dummy `cd`, And Help Coverage

**Files:**
- Modify: `crates/cli/src/cli.rs`
- Modify: `crates/cli/src/exit.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/tests/e2e.rs`
- Modify: `crates/cli/tests/help.rs`

**Interfaces:**
- Produces:
  - `Command::Cd(CdArgs)`
  - `CdArgs { outpost: Option<PathBuf> }`
  - `exit::CliError::ShellCdRequiresIntegration { outpost: Option<PathBuf> }`
  - `ShellCommand::Install(ShellManageArgs)`
  - `ShellCommand::Uninstall(ShellManageArgs)`
  - `ShellManageArgs { shell: ShellKind, rc_file: Option<PathBuf>, script_file: Option<PathBuf> }`

- [ ] **Step 1: Write failing help tests**

Modify `crates/cli/tests/help.rs`.

Add `"cd"` to the root command list:

```rust
for command in [
    "add", "pull", "source", "merge", "rebase", "push", "list", "lock", "unlock", "move",
    "remove", "prune", "status", "analyze", "config", "path", "cd", "shell",
] {
```

Add `gop cd --help` coverage after the existing command-specific help checks:

```rust
let cd_help = help_for(&["cd", "--help"]);
for token in ["shell integration", "gop shell install", "gop shell init", "OUTPOST"] {
    assert!(
        cd_help.contains(token),
        "expected {token} in cd help:\n{cd_help}"
    );
}
```

Update the shell help token list:

```rust
let shell_help = help_for(&["shell", "--help"]);
for token in ["init", "install", "uninstall", "shell integration"] {
    assert!(
        shell_help.contains(token),
        "expected {token} in shell help:\n{shell_help}"
    );
}
```

Add install and uninstall help checks after the existing `shell init` help check:

```rust
let shell_install_help = help_for(&["shell", "install", "--help"]);
for token in [
    "Install shell integration",
    "SHELL",
    "bash",
    "zsh",
    "--rc-file",
    "--script-file",
] {
    assert!(
        shell_install_help.contains(token),
        "expected {token} in shell install help:\n{shell_install_help}"
    );
}

let shell_uninstall_help = help_for(&["shell", "uninstall", "--help"]);
for token in [
    "Uninstall shell integration",
    "SHELL",
    "bash",
    "zsh",
    "--rc-file",
    "--script-file",
] {
    assert!(
        shell_uninstall_help.contains(token),
        "expected {token} in shell uninstall help:\n{shell_uninstall_help}"
    );
}
```

- [ ] **Step 2: Run the focused help test and verify it fails**

Run:

```bash
cargo test -p git-outpost --test help e_03_help_lists_commands_and_long_flags --locked
```

Expected: fail because `cd`, `shell install`, and `shell uninstall` are not commands yet.

- [ ] **Step 3: Add dummy `cd` and shell management argument types**

Modify `crates/cli/src/cli.rs`.

Add `Command::Cd` before `Command::Path`:

```rust
    /// Explain how to enable shell-backed directory changes.
    Cd(CdArgs),
```

Update `validate_refs`:

```rust
            | Command::Cd(_)
            | Command::Shell(_)
            | Command::Path(_)
```

Add `CdArgs` near `PathArgs`:

```rust
const CD_AFTER_HELP: &str = "\
This binary command cannot change your current shell directory.
Enable the shell function with `gop shell install bash` or `gop shell install zsh`.
For a temporary shell, run `eval \"$(gop shell init bash)\"` or `eval \"$(gop shell init zsh)\"`.
";

#[derive(Debug, Args)]
#[command(
    about = "Explain how to enable shell integration for `gop cd`.",
    after_help = CD_AFTER_HELP
)]
pub struct CdArgs {
    #[arg(value_name = "OUTPOST")]
    pub outpost: Option<PathBuf>,
}
```

Update `ShellCommand`:

```rust
#[derive(Debug, Subcommand)]
pub enum ShellCommand {
    /// Print shell integration for `gop cd`.
    Init {
        /// Shell syntax to print.
        #[arg(value_enum, value_name = "SHELL")]
        shell: Option<ShellKind>,
    },

    /// Install shell integration into a startup file.
    Install(ShellManageArgs),

    /// Uninstall shell integration from a startup file.
    Uninstall(ShellManageArgs),
}
```

Add the shared argument struct near `ShellCommand`:

```rust
#[derive(Debug, Args)]
pub struct ShellManageArgs {
    /// Shell startup syntax to manage.
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: ShellKind,

    /// Startup file to edit instead of the shell default.
    #[arg(long, value_name = "PATH")]
    pub rc_file: Option<PathBuf>,

    /// Generated integration script path instead of the Git Outpost default.
    #[arg(long, value_name = "PATH")]
    pub script_file: Option<PathBuf>,
}
```

- [ ] **Step 4: Add CLI-only setup guidance error**

Modify `crates/cli/src/exit.rs`.

Replace the `CliResult` type alias with a CLI error wrapper:

```rust
use std::path::PathBuf;
use std::process::ExitCode;

use outpost_core::OutpostError;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    Outpost(OutpostError),
    ShellCdRequiresIntegration { outpost: Option<PathBuf> },
}

impl From<OutpostError> for CliError {
    fn from(value: OutpostError) -> Self {
        Self::Outpost(value)
    }
}

pub fn report(err: CliError) -> ExitCode {
    match err {
        CliError::Outpost(err) => {
            eprintln!("error: {err}");
            ExitCode::from(err.exit_code())
        }
        CliError::ShellCdRequiresIntegration { outpost } => {
            eprintln!("error: `gop cd` is provided by shell integration");
            eprintln!();
            eprintln!("A binary command cannot change your current shell directory.");
            if let Some(outpost) = outpost {
                eprintln!("Requested target: {}", outpost.display());
            }
            eprintln!();
            eprintln!("For persistent setup, run one of:");
            eprintln!("  gop shell install bash");
            eprintln!("  gop shell install zsh");
            eprintln!();
            eprintln!("For the current shell only, run one of:");
            eprintln!("  eval \"$(gop shell init bash)\"");
            eprintln!("  eval \"$(gop shell init zsh)\"");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 5: Add dummy `cd` dispatch**

Modify `crates/cli/src/main.rs`.

Because `CliResult<T>` is now `Result<T, CliError>`, update direct `Err(OutpostError::...)` returns in this file to convert into `CliError`:

```rust
return Err(OutpostError::ConfigKeyUnset {
    key: args.key.as_str().to_owned(),
}
.into());
```

For helper functions with expression-style errors, wrap the existing `OutpostError` in `.into()`:

```rust
Context::Outpost(_) => Err(OutpostError::WrongContext {
    command,
    expected: "source repository",
    cwd: cwd.to_path_buf(),
}
.into()),
```

Apply the same pattern to the existing `WrongContext` and `MissingOutpostPath` returns. Do not change match arms that already use `?`; `From<OutpostError> for CliError` handles those.

Add this arm before `Command::Path`:

```rust
        Command::Cd(args) => {
            return Err(exit::CliError::ShellCdRequiresIntegration {
                outpost: args.outpost,
            });
        }
```

This branch must run before any source/outpost context helper, so `gop cd` works the same outside a Git repository.

- [ ] **Step 6: Add dummy `cd` behavior tests**

Append to `crates/cli/tests/e2e.rs` near existing shell tests:

```rust
#[test]
fn cd_without_shell_integration_prints_setup_guidance() {
    let fixture = common::CliFixture::new();

    let output = common::run(fixture.gop().current_dir(&fixture.root).arg("cd"));

    common::assert_failure_code(&output, 2, "gop cd without shell integration");
    assert_eq!(common::stdout(&output), "");
    let stderr = common::stderr(&output);
    for token in [
        "`gop cd` is provided by shell integration",
        "gop shell install bash",
        "gop shell install zsh",
        "gop shell init bash",
        "gop shell init zsh",
    ] {
        assert!(stderr.contains(token), "expected {token} in stderr:\n{stderr}");
    }
}

#[test]
fn cd_with_outpost_arg_prints_setup_guidance_without_resolving_target() {
    let fixture = common::CliFixture::new();

    let output = common::run(
        fixture
            .gop()
            .current_dir(&fixture.root)
            .args(["cd", "../does-not-need-to-exist"]),
    );

    common::assert_failure_code(&output, 2, "gop cd target without shell integration");
    assert_eq!(common::stdout(&output), "");
    let stderr = common::stderr(&output);
    assert!(stderr.contains("Requested target: ../does-not-need-to-exist"), "{stderr}");
    assert!(stderr.contains("gop shell install bash"), "{stderr}");
}
```

- [ ] **Step 7: Run focused dummy `cd` tests**

Run:

```bash
cargo test -p git-outpost --test e2e cd_without_shell_integration --locked
cargo test -p git-outpost --test e2e cd_with_outpost_arg --locked
```

Expected: pass.

- [ ] **Step 8: Run the focused help test**

Run:

```bash
cargo test -p git-outpost --test help e_03_help_lists_commands_and_long_flags --locked
```

Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add crates/cli/src/cli.rs crates/cli/src/exit.rs crates/cli/src/main.rs crates/cli/tests/e2e.rs crates/cli/tests/help.rs
git commit -m "feat: add shell setup command shape"
```

---

### Task 2: Add Shell Install Data Model And Pure Helpers

**Files:**
- Modify: `crates/cli/src/shell.rs`
- Create: `crates/cli/src/shell/install.rs`
- Modify: `crates/cli/Cargo.toml`

**Interfaces:**
- Consumes:
  - `crate::cli::ShellKind`
  - `super::init_script(shell: Option<ShellKind>) -> &'static str`
- Produces:
  - `shell::InstallOptions`
  - `shell::ShellInstallReport`
  - `shell::default_rc_file(shell: ShellKind) -> OutpostResult<PathBuf>`
  - `shell::default_script_file(shell: ShellKind) -> OutpostResult<PathBuf>`
  - `shell::managed_source_block(shell: ShellKind, script_file: &Path) -> String`

- [ ] **Step 1: Add `tempfile` to CLI runtime dependencies**

Modify `crates/cli/Cargo.toml`.

Move `tempfile.workspace = true` from dev-only usage into `[dependencies]` by adding it there:

```toml
[dependencies]
clap.workspace = true
outpost-core = { path = "../core", version = "0.2.1" }
serde.workspace = true
serde_json.workspace = true
tempfile.workspace = true
```

Leave the existing `[dev-dependencies]` entry only if tests still need it directly. Cargo accepts a dependency appearing in both sections, but remove the dev-dependency entry if it becomes redundant in this crate.

- [ ] **Step 2: Split shell installation mechanics into a submodule**

Modify `crates/cli/src/shell.rs`.

Add at the top:

```rust
mod install;

pub use install::{
    InstallOptions, ShellInstallReport, default_rc_file, default_script_file,
    managed_source_block,
};
```

Keep the existing `init_script` and `BASH_ZSH_INIT_SCRIPT` in `shell.rs`.

- [ ] **Step 3: Add the install module skeleton and pure helper tests**

Create `crates/cli/src/shell/install.rs`:

```rust
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use outpost_core::{OutpostError, OutpostResult};

use crate::cli::ShellKind;

pub const INSTALL_START: &str = "# >>> git-outpost shell install >>>";
pub const INSTALL_END: &str = "# <<< git-outpost shell install <<<";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub shell: ShellKind,
    pub rc_file: PathBuf,
    pub script_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ShellInstallReport {
    pub shell: ShellKind,
    pub rc_file: PathBuf,
    pub script_file: PathBuf,
    pub changed: bool,
}

impl ShellKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
        }
    }

    fn default_rc_name(&self) -> &'static str {
        match self {
            ShellKind::Bash => ".bashrc",
            ShellKind::Zsh => ".zshrc",
        }
    }

    fn default_script_name(&self) -> &'static str {
        match self {
            ShellKind::Bash => "shell.bash",
            ShellKind::Zsh => "shell.zsh",
        }
    }
}

pub fn default_rc_file(shell: ShellKind) -> OutpostResult<PathBuf> {
    Ok(home_dir()?.join(shell.default_rc_name()))
}

pub fn default_script_file(shell: ShellKind) -> OutpostResult<PathBuf> {
    Ok(config_home()?.join("git-outpost").join(shell.default_script_name()))
}

pub fn managed_source_block(shell: ShellKind, script_file: &Path) -> String {
    let quoted = shell_single_quote(script_file);
    format!(
        "{INSTALL_START}\n\
         # Managed by Git Outpost. Remove with: gop shell uninstall {}\n\
         # Sources the generated Git Outpost shell integration.\n\
         if [ -f {quoted} ]; then\n\
             . {quoted}\n\
         fi\n\
         {INSTALL_END}\n",
        shell.as_str()
    )
}

fn home_dir() -> OutpostResult<PathBuf> {
    non_empty_env_path("HOME").map_err(|source| OutpostError::IoAt {
        path: PathBuf::from("$HOME"),
        source,
    })
}

fn config_home() -> OutpostResult<PathBuf> {
    match env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        Some(value) => Ok(PathBuf::from(value)),
        None => Ok(home_dir()?.join(".config")),
    }
}

fn non_empty_env_path(name: &str) -> Result<PathBuf, std::io::Error> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{name} is not set"),
            )
        })
}

fn shell_single_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_source_block_quotes_script_path() {
        let block = managed_source_block(ShellKind::Bash, Path::new("/tmp/a dir/it's/gop.bash"));

        assert!(block.contains(INSTALL_START), "{block}");
        assert!(block.contains(INSTALL_END), "{block}");
        assert!(block.contains("gop shell uninstall bash"), "{block}");
        assert!(block.contains("'/tmp/a dir/it'\"'\"'s/gop.bash'"), "{block}");
    }
}
```

- [ ] **Step 4: Run the new unit test**

Run:

```bash
cargo test -p git-outpost shell::install::tests::managed_source_block_quotes_script_path --locked
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/shell.rs crates/cli/src/shell/install.rs
git commit -m "feat: add shell install helper model"
```

---

### Task 3: Implement Managed Block Parsing And File Operations

**Files:**
- Modify: `crates/cli/src/shell.rs`
- Modify: `crates/cli/src/shell/install.rs`

**Interfaces:**
- Consumes:
  - Task 2 `InstallOptions`
  - Task 2 `managed_source_block`
  - `super::init_script(Some(shell))`
- Produces:
  - working `shell::install(options) -> OutpostResult<ShellInstallReport>`
  - working `shell::uninstall(options) -> OutpostResult<ShellInstallReport>`

- [ ] **Step 1: Add parser and idempotency tests**

Append tests to `crates/cli/src/shell/install.rs`:

```rust
#[cfg(test)]
mod operation_tests {
    use super::*;

    fn paths(root: &Path) -> InstallOptions {
        InstallOptions {
            shell: ShellKind::Bash,
            rc_file: root.join(".bashrc"),
            script_file: root.join("git-outpost").join("shell.bash"),
        }
    }

    #[test]
    fn install_appends_then_replaces_managed_block() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        fs::write(&options.rc_file, "# user content\n").expect("write rc");

        let first = install(options.clone()).expect("install");
        let first_rc = fs::read_to_string(&options.rc_file).expect("read rc");
        let first_script = fs::read_to_string(&options.script_file).expect("read script");

        let second = install(options.clone()).expect("install again");
        let second_rc = fs::read_to_string(&options.rc_file).expect("read rc");
        let second_script = fs::read_to_string(&options.script_file).expect("read script");

        assert!(first.changed);
        assert!(second.changed);
        assert_eq!(first.rc_file, options.rc_file);
        assert_eq!(first.script_file, options.script_file);
        assert_eq!(first_rc, second_rc);
        assert_eq!(first_script, second_script);
        assert_eq!(second_rc.matches(INSTALL_START).count(), 1, "{second_rc}");
        assert!(second_script.contains("gop()"), "{second_script}");
    }

    #[test]
    fn uninstall_removes_managed_block_and_script_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        fs::write(
            &options.rc_file,
            format!(
                "# manual init remains\n{}\n# after\n",
                super::super::init_script(Some(ShellKind::Bash))
            ),
        )
        .expect("write rc");
        install(options.clone()).expect("install");

        let report = uninstall(options.clone()).expect("uninstall");
        let rc = fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(report.changed);
        assert!(!options.script_file.exists());
        assert!(!rc.contains(INSTALL_START), "{rc}");
        assert!(rc.contains("# manual init remains"), "{rc}");
        assert!(rc.contains("# >>> git-outpost shell integration >>>"), "{rc}");
        assert!(rc.contains("# after"), "{rc}");
    }

    #[test]
    fn uninstall_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());

        let report = uninstall(options).expect("uninstall absent");

        assert!(!report.changed);
    }

    #[test]
    fn malformed_markers_fail_without_modifying_rc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let options = paths(tmp.path());
        let original = format!("# before\n{INSTALL_START}\nmissing end\n");
        fs::write(&options.rc_file, &original).expect("write rc");

        let err = install(options.clone()).expect_err("malformed markers should fail");
        let after = fs::read_to_string(&options.rc_file).expect("read rc");

        assert!(err.to_string().contains("missing git-outpost shell install end marker"));
        assert_eq!(after, original);
        assert!(!options.script_file.exists());
    }
}
```

- [ ] **Step 2: Run parser tests and verify they fail**

Run:

```bash
cargo test -p git-outpost shell::install::operation_tests --locked
```

Expected: fail because `install` and `uninstall` are not implemented yet.

- [ ] **Step 3: Export operation functions from the shell module**

Modify the `pub use install::{ ... }` list in `crates/cli/src/shell.rs`:

```rust
pub use install::{
    InstallOptions, ShellInstallReport, default_rc_file, default_script_file, install,
    managed_source_block, uninstall,
};
```

- [ ] **Step 4: Implement marker parsing**

In `crates/cli/src/shell/install.rs`, add these helpers before `install`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockRange {
    Absent,
    Present { start: usize, end: usize },
}

fn managed_block_range(path: &Path, contents: &str) -> OutpostResult<BlockRange> {
    let starts: Vec<_> = contents.match_indices(INSTALL_START).map(|(idx, _)| idx).collect();
    let ends: Vec<_> = contents.match_indices(INSTALL_END).map(|(idx, _)| idx).collect();

    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(BlockRange::Absent),
        ([_], []) => Err(invalid_marker(path, "missing git-outpost shell install end marker")),
        ([], [_]) => Err(invalid_marker(path, "missing git-outpost shell install start marker")),
        ([start], [end]) if start < end => {
            let mut end = end + INSTALL_END.len();
            if contents[end..].starts_with("\r\n") {
                end += 2;
            } else if contents[end..].starts_with('\n') {
                end += 1;
            }
            Ok(BlockRange::Present { start: *start, end })
        }
        ([_], [_]) => Err(invalid_marker(
            path,
            "git-outpost shell install end marker appears before start marker",
        )),
        _ => Err(invalid_marker(
            path,
            "multiple git-outpost shell install blocks found",
        )),
    }
}

fn invalid_marker(path: &Path, message: &'static str) -> OutpostError {
    OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    }
}
```

- [ ] **Step 5: Implement text transformations**

Add these helpers:

```rust
fn install_contents(path: &Path, contents: &str, block: &str) -> OutpostResult<String> {
    match managed_block_range(path, contents)? {
        BlockRange::Absent => {
            let mut next = String::from(contents);
            if !next.is_empty() && !next.ends_with('\n') {
                next.push('\n');
            }
            if !next.is_empty() {
                next.push('\n');
            }
            next.push_str(block);
            Ok(next)
        }
        BlockRange::Present { start, end } => {
            let mut next = String::new();
            next.push_str(&contents[..start]);
            next.push_str(block);
            next.push_str(&contents[end..]);
            Ok(next)
        }
    }
}

fn uninstall_contents(path: &Path, contents: &str) -> OutpostResult<(String, bool)> {
    match managed_block_range(path, contents)? {
        BlockRange::Absent => Ok((contents.to_owned(), false)),
        BlockRange::Present { start, end } => {
            let mut next = String::new();
            next.push_str(&contents[..start]);
            next.push_str(&contents[end..]);
            Ok((next, true))
        }
    }
}
```

- [ ] **Step 6: Implement file IO helpers**

Add these helpers:

```rust
fn read_optional(path: &Path) -> OutpostResult<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_text(path: &Path, contents: &str) -> OutpostResult<()> {
    let parent = path.parent().ok_or_else(|| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
    })?;
    fs::create_dir_all(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|source| OutpostError::IoAt {
        path: parent.to_path_buf(),
        source,
    })?;
    temp.write_all(contents.as_bytes()).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    temp.persist(path).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: source.error,
    })?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> OutpostResult<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(OutpostError::IoAt {
            path: path.to_path_buf(),
            source,
        }),
    }
}
```

- [ ] **Step 7: Implement `install` and `uninstall`**

Replace the unimplemented functions:

```rust
pub fn install(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    let rc_before = read_optional(&options.rc_file)?;
    let block = managed_source_block(options.shell, &options.script_file);
    let rc_after = install_contents(&options.rc_file, &rc_before, &block)?;
    let script = super::init_script(Some(options.shell));

    write_text(&options.script_file, script)?;
    write_text(&options.rc_file, &rc_after)?;

    Ok(ShellInstallReport {
        shell: options.shell,
        rc_file: options.rc_file,
        script_file: options.script_file,
        changed: true,
    })
}

pub fn uninstall(options: InstallOptions) -> OutpostResult<ShellInstallReport> {
    let rc_before = read_optional(&options.rc_file)?;
    let (rc_after, rc_changed) = uninstall_contents(&options.rc_file, &rc_before)?;
    if rc_changed {
        write_text(&options.rc_file, &rc_after)?;
    }
    let script_removed = remove_file_if_exists(&options.script_file)?;

    Ok(ShellInstallReport {
        shell: options.shell,
        rc_file: options.rc_file,
        script_file: options.script_file,
        changed: rc_changed || script_removed,
    })
}
```

- [ ] **Step 8: Run operation tests**

Run:

```bash
cargo test -p git-outpost shell::install --locked
```

Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add crates/cli/src/shell.rs crates/cli/src/shell/install.rs
git commit -m "feat: manage shell integration files"
```

---

### Task 4: Wire Dispatch And E2E Behavior

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/tests/e2e.rs`

**Interfaces:**
- Consumes:
  - Task 1 `ShellManageArgs`
  - Task 3 `shell::InstallOptions`
  - Task 3 `shell::install`
  - Task 3 `shell::uninstall`
- Produces:
  - working `gop shell install <shell>`
  - working `gop shell uninstall <shell>`

- [ ] **Step 1: Add e2e tests for install/uninstall**

Append to `crates/cli/tests/e2e.rs` near existing shell tests:

```rust
#[cfg(unix)]
#[test]
fn shell_install_writes_script_and_managed_rc_block() {
    let fixture = common::CliFixture::new();
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "install", "bash", "--rc-file"])
            .arg(&rc_file)
            .arg("--script-file")
            .arg(&script_file),
    );

    common::assert_success(&output, "gop shell install bash");
    let stdout = common::stdout(&output);
    let rc = std::fs::read_to_string(&rc_file).expect("read rc");
    let script = std::fs::read_to_string(&script_file).expect("read script");

    assert!(stdout.contains("installed bash shell integration"), "{stdout}");
    assert!(stdout.contains(&common::displayed_path(&rc_file)), "{stdout}");
    assert!(stdout.contains(&common::displayed_path(&script_file)), "{stdout}");
    assert!(rc.contains("# >>> git-outpost shell install >>>"), "{rc}");
    assert!(rc.contains("# <<< git-outpost shell install <<<"), "{rc}");
    assert!(rc.contains("gop shell uninstall bash"), "{rc}");
    assert!(script.contains("# >>> git-outpost shell integration >>>"), "{script}");
    assert!(script.contains("command gop \"$@\""), "{script}");
}

#[cfg(unix)]
#[test]
fn shell_install_replaces_existing_managed_block() {
    let fixture = common::CliFixture::new();
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");

    for _ in 0..2 {
        let output = common::run(
            fixture
                .gop()
                .args(["shell", "install", "bash", "--rc-file"])
                .arg(&rc_file)
                .arg("--script-file")
                .arg(&script_file),
        );
        common::assert_success(&output, "gop shell install bash");
    }

    let rc = std::fs::read_to_string(&rc_file).expect("read rc");
    assert_eq!(rc.matches("# >>> git-outpost shell install >>>").count(), 1, "{rc}");
    assert_eq!(rc.matches("# <<< git-outpost shell install <<<").count(), 1, "{rc}");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_removes_managed_block_and_script_only() {
    let fixture = common::CliFixture::new();
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");
    std::fs::create_dir_all(rc_file.parent().expect("rc parent")).expect("create rc parent");
    std::fs::write(
        &rc_file,
        "# manual line\n# >>> git-outpost shell integration >>>\nmanual init\n# <<< git-outpost shell integration <<<\n",
    )
    .expect("write rc");
    common::assert_success(
        &common::run(
            fixture
                .gop()
                .args(["shell", "install", "bash", "--rc-file"])
                .arg(&rc_file)
                .arg("--script-file")
                .arg(&script_file),
        ),
        "gop shell install bash",
    );

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "uninstall", "bash", "--rc-file"])
            .arg(&rc_file)
            .arg("--script-file")
            .arg(&script_file),
    );

    common::assert_success(&output, "gop shell uninstall bash");
    let stdout = common::stdout(&output);
    let rc = std::fs::read_to_string(&rc_file).expect("read rc");

    assert!(stdout.contains("uninstalled bash shell integration"), "{stdout}");
    assert!(!script_file.exists());
    assert!(!rc.contains("# >>> git-outpost shell install >>>"), "{rc}");
    assert!(rc.contains("# manual line"), "{rc}");
    assert!(rc.contains("# >>> git-outpost shell integration >>>"), "{rc}");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_is_idempotent() {
    let fixture = common::CliFixture::new();
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "uninstall", "bash", "--rc-file"])
            .arg(&rc_file)
            .arg("--script-file")
            .arg(&script_file),
    );

    common::assert_success(&output, "gop shell uninstall bash absent");
    assert!(common::stdout(&output).contains("not installed"), "{}", common::stdout(&output));
}

#[cfg(unix)]
#[test]
fn shell_install_rejects_malformed_managed_block_without_editing() {
    let fixture = common::CliFixture::new();
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");
    std::fs::create_dir_all(rc_file.parent().expect("rc parent")).expect("create rc parent");
    let original = "# before\n# >>> git-outpost shell install >>>\nmissing end\n";
    std::fs::write(&rc_file, original).expect("write rc");

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "install", "bash", "--rc-file"])
            .arg(&rc_file)
            .arg("--script-file")
            .arg(&script_file),
    );

    assert!(!output.status.success(), "malformed install unexpectedly succeeded");
    assert_eq!(common::stdout(&output), "");
    assert!(common::stderr(&output).contains("missing git-outpost shell install end marker"));
    assert_eq!(std::fs::read_to_string(&rc_file).expect("read rc"), original);
    assert!(!script_file.exists());
}
```

- [ ] **Step 2: Add e2e test for relative paths and `-C`**

Append:

```rust
#[cfg(unix)]
#[test]
fn shell_install_relative_paths_use_effective_cwd() {
    let fixture = common::CliFixture::new();
    let work = fixture.root.join("shell-work");
    std::fs::create_dir_all(&work).expect("create work");

    let output = common::run(
        fixture
            .gop()
            .arg("-C")
            .arg(&work)
            .args([
                "shell",
                "install",
                "zsh",
                "--rc-file",
                "home/.zshrc",
                "--script-file",
                "config/git-outpost/shell.zsh",
            ]),
    );

    common::assert_success(&output, "gop shell install zsh relative");
    assert!(work.join("home/.zshrc").exists());
    assert!(work.join("config/git-outpost/shell.zsh").exists());
}
```

- [ ] **Step 3: Add default path behavior tests**

Append:

```rust
#[cfg(unix)]
#[test]
fn shell_install_uses_home_and_xdg_config_home_defaults() {
    let fixture = common::CliFixture::new();
    let home = fixture.root.join("home");
    let config_home = fixture.root.join("xdg-config");

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "install", "bash"])
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &config_home),
    );

    common::assert_success(&output, "gop shell install bash defaults");
    assert!(home.join(".bashrc").exists());
    assert!(config_home.join("git-outpost/shell.bash").exists());
}

#[cfg(unix)]
#[test]
fn shell_install_requires_home_for_default_paths() {
    let fixture = common::CliFixture::new();

    let output = common::run(
        fixture
            .gop()
            .args(["shell", "install", "bash"])
            .env_remove("HOME")
            .env_remove("XDG_CONFIG_HOME"),
    );

    assert!(!output.status.success(), "install without HOME unexpectedly succeeded");
    assert_eq!(common::stdout(&output), "");
    assert!(
        common::stderr(&output).contains("HOME is not set"),
        "{}",
        common::stderr(&output)
    );
}
```

- [ ] **Step 4: Add shell smoke test through installed rc file**

Append:

```rust
#[cfg(unix)]
#[test]
fn shell_installed_rc_enables_gop_cd() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let rc_file = fixture.root.join("home/.bashrc");
    let script_file = fixture.root.join("config/git-outpost/shell.bash");
    let source_display = common::displayed_path(&fixture.source);

    common::assert_success(
        &common::run(
            fixture
                .gop()
                .args(["shell", "install", "bash", "--rc-file"])
                .arg(&rc_file)
                .arg("--script-file")
                .arg(&script_file),
        ),
        "gop shell install bash",
    );

    let script = format!(
        r#"
set -eu
. "{}"
cd "{}"
gop cd
pwd
"#,
        rc_file.display(),
        outpost.display()
    );

    let output = bash_script(&script, &fixture);

    common::assert_success(&output, "installed bash rc gop cd");
    assert_eq!(common::stdout(&output), format!("{source_display}\n"));
}
```

- [ ] **Step 5: Run e2e tests and verify they fail**

Run:

```bash
cargo test -p git-outpost --test e2e shell_install --locked
```

Expected: fail because dispatch is not wired yet.

- [ ] **Step 6: Wire dispatch**

Modify `crates/cli/src/main.rs`.

Add a helper near `resolve_path_arg`:

```rust
fn shell_install_options(
    cwd: &Path,
    args: cli::ShellManageArgs,
) -> CliResult<shell::InstallOptions> {
    let rc_file = match args.rc_file {
        Some(path) => resolve_path_arg(cwd, path),
        None => shell::default_rc_file(args.shell)?,
    };
    let script_file = match args.script_file {
        Some(path) => resolve_path_arg(cwd, path),
        None => shell::default_script_file(args.shell)?,
    };
    Ok(shell::InstallOptions {
        shell: args.shell,
        rc_file,
        script_file,
    })
}
```

Update the `Command::Shell` dispatch arm:

```rust
        Command::Shell(args) => match args.command {
            ShellCommand::Init { shell: shell_kind } => {
                print!("{}", shell::init_script(shell_kind));
            }
            ShellCommand::Install(args) => {
                let report = shell::install(shell_install_options(&cwd, args)?)?;
                if report.changed {
                    println!("installed {} shell integration", report.shell.as_str());
                } else {
                    println!("{} shell integration already installed", report.shell.as_str());
                }
                println!("rc: {}", report.rc_file.display());
                println!("script: {}", report.script_file.display());
            }
            ShellCommand::Uninstall(args) => {
                let report = shell::uninstall(shell_install_options(&cwd, args)?)?;
                if report.changed {
                    println!("uninstalled {} shell integration", report.shell.as_str());
                } else {
                    println!("{} shell integration was not installed", report.shell.as_str());
                }
                println!("rc: {}", report.rc_file.display());
                println!("script: {}", report.script_file.display());
            }
        },
```

- [ ] **Step 7: Run e2e tests**

Run:

```bash
cargo test -p git-outpost --test e2e shell_install --locked
```

Expected: all `shell_install_` tests pass.

- [ ] **Step 8: Run uninstall e2e tests**

Run:

```bash
cargo test -p git-outpost --test e2e shell_uninstall --locked
```

Expected: all `shell_uninstall_` tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/cli/src/main.rs crates/cli/tests/e2e.rs
git commit -m "feat: wire shell install commands"
```

---

### Task 5: Document Persistent Shell Management

**Files:**
- Modify: `README.md`
- Modify: `docs/src/product.md`
- Modify: `docs/src/roadmap.md`

**Interfaces:**
- Consumes:
  - dummy binary `gop cd [<outpost>]`
  - `gop shell init [bash|zsh]`
  - `gop shell install <bash|zsh>`
  - `gop shell uninstall <bash|zsh>`
- Produces:
  - user-facing setup, update, and removal documentation.

- [ ] **Step 1: Update README shell setup**

In `README.md`, replace the current one-time setup paragraph that says `gop shell install` is not part of the milestone with:

````markdown
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
````

- [ ] **Step 2: Update product Story**

In `docs/src/product.md`, update the shell navigation paragraph to say:

```markdown
Because a child process cannot change its parent shell's current directory,
`gop cd` is provided by shell integration rather than by the binary itself.
`gop shell init bash` and `gop shell init zsh` print integration code for the
current shell. `gop shell install bash` and `gop shell install zsh` persist that
integration by writing a generated script and a small managed source block in
the shell startup file. The generated function shadows `gop` in that shell,
intercepts only invocations whose first argument is exactly `cd`, and delegates
every other `gop ...` invocation to the installed binary with `command gop "$@"`.
The binary also exposes a dummy `gop cd [<outpost>]` command so root help can
show the feature and so users without shell integration get setup guidance.
```

- [ ] **Step 3: Update product Synopsis**

In `docs/src/product.md`, add install and uninstall near `gop shell init [bash|zsh]`:

```text
gop shell init [bash|zsh]
gop shell install <bash|zsh>
gop shell uninstall <bash|zsh>
gop cd [<outpost>]   # shell function after setup; binary fallback prints setup guidance
```

- [ ] **Step 4: Update product Working Directory Matrix**

Replace the existing `shell init` row with:

```markdown
| `shell init [bash\|zsh]` | Prints shell integration; does not inspect repo state | Prints shell integration; does not inspect repo state |
| `shell install <bash\|zsh>` | Installs shell integration; does not inspect repo state | Installs shell integration; does not inspect repo state |
| `shell uninstall <bash\|zsh>` | Uninstalls shell integration; does not inspect repo state | Uninstalls shell integration; does not inspect repo state |
| `cd [<outpost>]` | Binary fallback prints shell setup guidance; shell function changes directory after setup | Binary fallback prints shell setup guidance; shell function changes directory after setup |
```

- [ ] **Step 5: Update product command reference**

Replace the current statement that install/uninstall are not part of the milestone with:

````markdown
For persistent setup, use `install`:

```bash
gop shell install bash
gop shell install zsh
```

`install` writes the generated integration script under the Git Outpost config
directory and adds a marker-wrapped source block to `~/.bashrc` or `~/.zshrc`.
Run `install` again after upgrading Git Outpost to refresh the generated script.

Remove the managed source block and generated script with:

```bash
gop shell uninstall bash
gop shell uninstall zsh
```

`uninstall` removes only Git Outpost's managed source block and generated
script. It does not remove manually pasted `gop shell init` snippets.
````

Add a `cd` command reference section near `shell init`:

````markdown
### `cd [<outpost>]`

After shell integration is active, `gop cd` is a shell function that changes the
current shell directory. The binary still exposes `gop cd [<outpost>]` for help
and setup guidance.

If the binary receives `gop cd`, shell integration is not active in the current
shell. It prints setup instructions and exits without changing directories:

```bash
gop shell install bash
gop shell install zsh
```
````

- [ ] **Step 6: Update roadmap deployment scope**

In `docs/src/roadmap.md`, replace the existing shell init row with:

```markdown
| `gop shell init [bash\|zsh]` | Present | Prints marker-wrapped Bash/Zsh shell integration that shadows `gop` only to implement `gop cd`; calls whose first argument is not exactly `cd` delegate to the binary. |
| `gop cd [<outpost>]` | Present | Binary fallback listed in help; prints shell setup guidance when shell integration is not active. The shell function handles real directory changes after setup. |
| `gop shell install <bash\|zsh>` | Present | Writes the generated integration script and a managed source block in the selected shell startup file. Re-running updates the generated integration. |
| `gop shell uninstall <bash\|zsh>` | Present | Removes only Git Outpost's managed source block and generated integration script. |
```

- [ ] **Step 7: Build docs**

Run:

```bash
mdbook build docs
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add README.md docs/src/product.md docs/src/roadmap.md
git commit -m "docs: document shell install commands"
```

---

### Task 6: Final Verification

**Files:**
- No additional edits.

**Interfaces:**
- Consumes all prior tasks.
- Produces final implementation evidence.

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

- [ ] **Step 5: Manual smoke test with disposable files**

Run from the repository root, replacing the outpost path with a disposable managed outpost:

```bash
tmp="$(mktemp -d)"
gop shell install bash --rc-file "$tmp/.bashrc" --script-file "$tmp/shell.bash"
bash --noprofile --norc -c ". \"$tmp/.bashrc\"; type gop; cd /path/to/an/outpost; gop cd; pwd"
gop shell uninstall bash --rc-file "$tmp/.bashrc" --script-file "$tmp/shell.bash"
test ! -f "$tmp/shell.bash"
```

Expected:

- `type gop` reports a shell function.
- `gop cd` changes to the associated source repository.
- `uninstall` removes the generated script file.

- [ ] **Step 6: Commit if verification required a correction**

```bash
git add crates/cli/Cargo.toml crates/cli/src/cli.rs crates/cli/src/main.rs crates/cli/src/shell.rs crates/cli/src/shell/install.rs crates/cli/tests/e2e.rs crates/cli/tests/help.rs README.md docs/src/product.md docs/src/roadmap.md
git commit -m "fix: align shell install verification"
```
