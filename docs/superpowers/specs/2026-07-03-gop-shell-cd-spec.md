# `gop cd` Shell Integration Spec

## Purpose

`gop cd` gives users a short current-shell navigation command while preserving `gop` as the single command name. Because a Rust binary cannot change its parent shell's current working directory, this feature is implemented as shell integration that delegates all path resolution to the binary behavior specified in [2026-07-03-gop-path-spec.md](2026-07-03-gop-path-spec.md).

## Scope

This spec covers:

```bash
gop shell init [bash|zsh]
gop cd
gop cd <outpost>
```

It assumes `gop path src` and `gop path <outpost>` already exist and satisfy the `gop path` spec.

This spec does not add standalone commands named `gcd`, `gsrc`, or `gout`. It does not add Fish, PowerShell, or Nushell support.

This milestone ships only `gop shell init [bash|zsh]`: a command that prints shell integration code to stdout. It does not add `gop shell install`, `gop shell uninstall`, shell startup file editing, or a Git Outpost-owned source file. Those belong to a later install/uninstall milestone.

## User-Facing Behavior

`gop shell init [SHELL]` prints shell code that users evaluate in their interactive shell. `SHELL` may be `bash` or `zsh`. If `SHELL` is omitted, the command prints the Bash/Zsh-compatible integration block used by the supported shells.

After evaluation, `gop` becomes a shell function in that shell. This function intentionally shadows the `gop` binary name only inside the evaluating shell.

The function intercepts only invocations whose first argument is exactly `cd`. Every other invocation, including `gop add`, `gop list`, `gop path`, and `gop shell init bash`, delegates to the installed `gop` binary.

`gop cd` changes the current shell's working directory to the associated source repository path. Operationally, it is equivalent to:

```bash
cd "$(gop path src)"
```

`gop cd <outpost>` changes the current shell's working directory to the resolved managed outpost path. Operationally, it is equivalent to:

```bash
cd "$(gop path <outpost>)"
```

`gop cd --help` and `gop cd -h` are handled by the shell function, because `cd` is not a binary subcommand. The help text should explain the two supported forms and the requirement to evaluate `gop shell init bash` or `gop shell init zsh`.

## Shell Contract

The generated shell code must support Bash and Zsh.

The generated shell code must be selected by a parsed shell kind, not by free-form string checks downstream. Unsupported shell names must fail through CLI argument parsing before any shell code is printed.

The generated shell code must define a function named `gop` in the evaluating shell. It must not write files, edit startup files, or change shell configuration automatically.

The generated shell code must be wrapped in clear marker comments:

```sh
# >>> git-outpost shell integration >>>
# <<< git-outpost shell integration <<<
```

The generated block must include a short comment saying that users can remove the marked block if they manually paste it into a shell startup file. This is manual removal guidance, not an uninstall command.

The generated shell code must remove an existing `gop` alias in the evaluating shell before defining the function, so aliases do not shadow the function.

The function must inspect only the first argument. It intercepts the invocation only when the first argument is exactly `cd`; invocations such as `gop --help`, `gop --no-color status`, and `gop shell init bash` must pass through unchanged.

The function must delegate non-`cd` invocations with `command gop "$@"` so it bypasses the function and runs the actual binary.

The function must quote paths safely. It should store `gop path ...` output in a variable and call `cd "$target"` rather than interpolating the path unquoted.

The later install/uninstall milestone may add commands that write a Git Outpost-owned shell file and manage a source line in the user's startup configuration. This spec deliberately does not implement that behavior.

## Errors

If `gop path src` or `gop path <outpost>` fails, `gop cd` must not change directory.

The function should preserve the binary diagnostic on stderr and return a non-zero status when path resolution fails.

If `cd "$target"` itself fails, the function should return the shell `cd` failure status.

If `gop shell init <shell>` receives an unsupported shell name, it must fail without printing shell code.

## Documentation Requirements

The README and product documentation must show the one-time setup explicitly:

```bash
eval "$(gop shell init bash)"
```

They should also show persistent setup examples for Bash and Zsh startup files without claiming those files are edited automatically. Persistent setup examples should use the marker-wrapped generated block or the same `eval "$(gop shell init <shell>)"` command, so users can remove the manually added integration by deleting one clearly identified block or line.

Documentation must state that `install` and `uninstall` are not part of this milestone.

The product command reference must document `gop shell init [bash|zsh]` as the binary command that prints integration code. It should document `gop cd` as shell-function behavior available only after setup.

The product synopsis should include `gop shell init [bash|zsh]` and `gop cd [<outpost>]` while making the setup dependency clear.

## Test Requirements

CLI help tests must cover `gop shell --help` and `gop shell init --help`, including the supported shell values.

CLI behavior tests must cover `gop shell init bash`, `gop shell init zsh`, and rejection of an unsupported shell value.

Generated-script tests must cover the marker comments and the manual-removal comment.

Shell integration tests must run the generated function inside a shell process and prove the current shell process changes directory after `gop cd`.

Bash tests must cover:

- `gop cd` changes to the source repository path.
- `gop cd <outpost>` changes to a managed outpost path.
- non-`cd` invocations delegate to the binary.
- invocations whose first argument is not exactly `cd` pass through unchanged.
- an existing `gop` alias is removed before the function is defined.
- paths containing spaces are handled safely.
- `gop cd --help` is handled by the function.

Zsh should have a smoke test when `zsh` is available in the test environment. The test must skip cleanly when `zsh` is not installed.
