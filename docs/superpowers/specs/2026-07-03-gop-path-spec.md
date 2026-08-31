# `gop path` Spec

## Purpose

`gop path` gives users a scriptable way to ask Git Outpost for the concrete filesystem paths it already knows: the source repository path and managed outpost paths. The command is read-only and exists primarily to support navigation workflows without teaching shell snippets how to resolve Git Outpost state.

## Scope

This spec covers the binary command:

```bash
gop path src
gop path <outpost>
```

It does not cover current-shell directory changes. Shell-level `gop cd` behavior is specified separately in [2026-07-03-gop-shell-cd-spec.md](2026-07-03-gop-shell-cd-spec.md).

## User-Facing Behavior

`gop path src` prints the associated source repository path.

When run from the source repository, the associated source is the current source repository. When run from a managed outpost, the associated source is that outpost's registered source repository.

`gop path <outpost>` prints the path to a managed outpost associated with the current source context. The `<outpost>` argument follows the existing Git Outpost selector model used by lifecycle commands: explicit path syntax selects by path, and ID-prefix syntax selects by registered outpost ID where supported by the existing selector rules.

The literal target `src` is reserved for the source repository. If an outpost is named `src`, users must select it with an explicit path such as `./src`, `../src`, an absolute path, or an ID selector.

On success, `gop path` writes exactly one path followed by a newline to stdout. It does not add labels, color, quoting, shell syntax, or explanatory prose.

## Resolution Rules

`gop path` must use the existing source/outpost context discovery. It must not introduce a second navigation registry or shell-specific state.

`gop path src` must work from a source repository and from a managed outpost.

`gop path <outpost>` must resolve through the relevant source registry for the current context. It must print only paths for live managed outposts. Stale, missing, unregistered, or unmanaged outpost targets must fail through the existing selector and safety error paths instead of printing a path that a shell might try to `cd` into.

`gop path` must not mutate repository state, registry state, worktree files, refs, locks, or configuration.

## Errors

If Git Outpost cannot find an associated source repository from the effective current directory, `gop path` fails through the existing CLI error handling path.

If `<outpost>` is ambiguous, missing, stale, unregistered, or not managed by the associated source, `gop path <outpost>` fails through the existing selector and safety diagnostics.

Failures write diagnostics to stderr and return a non-zero exit code. Failed invocations must not print a path to stdout.

## Documentation Requirements

The product documentation must describe `gop path src` and `gop path <outpost>` as binary commands.

The README should include a short usage example for printing the source path and an outpost path.

The product command reference, synopsis, and working-directory matrix should include `path` with its source-context and outpost-context behavior.

## Test Requirements

Core tests must cover source-path resolution, live outpost-path resolution, and stale registered outpost rejection.

CLI tests must cover:

- `gop path src` from a source repository.
- `gop path src` from a managed outpost.
- `gop path <outpost>` by explicit path.
- `gop path <outpost>` by outpost ID or ID prefix where existing selector rules allow it.
- the reserved `src` behavior with an explicit path escape hatch.
- rejection of stale or unmanaged outpost targets without stdout path output.

Help tests must include the new `path` command.
