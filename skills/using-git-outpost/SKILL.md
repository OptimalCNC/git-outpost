---
name: using-git-outpost
description: Use when beginning work in a Git checkout, or when a Git task involves gop, Git Outpost, outposts, worktrees, parallel checkouts, checkout navigation or lifecycle, branch synchronization, or branch publication.
---

# Using Git Outpost

## Overview

Treat Git Outpost as a self-contained-clone realization of worktree intent. Orient once, then use ordinary Git for files and commits and `gop` for checkout topology or the two-hop path between outpost, source, and upstream.

## Orient First

1. Resolve the effective work tree:

   ```bash
   git -C <path> rev-parse --show-toplevel
   ```

   If this fails, return `NoDiscoverableWorkTree(error)`, preserve the error, and end checkout orientation. Obtain a valid checkout path before repository-bound workflows; repository-independent `shell` guidance remains available.
2. If `gop` is available, run against the resolved root:

   ```bash
   gop --no-color -C <root> status
   ```

3. Parse the result into exactly one named state:
   - `StatusManagedOutpost(report)`: exit 0. Preserve the full report; this proves diagnostic membership, not complete metadata, source reachability, registry membership, attached `HEAD`, or command readiness. `health: problems` is still an outpost.
   - `SourceContext(root)`: exit 2 and stderr says `not inside a managed outpost: <root>`. This is an unmanaged Git work tree that `gop` can treat as a source; it does not prove this is the intended source.
   - `Unknown(error)`: every other result, including a later `not inside a Git repository` diagnostic. Preserve the error because it can describe a broken associated source rather than `<root>`.

If `gop` is unavailable, use `git -C <root> config --local --type=bool --get outpost.managed`. Only `true` establishes `ManagedOutpostWithoutGop`; treat every other result as unknown and report that the CLI is unavailable. This fallback proves only the local marker and supports ordinary file and commit work, not `gop` workflows.

Orientation is complete only after naming the state.

## Choose the Workflow

Read [references/gop-workflows.md](references/gop-workflows.md) when the state is managed or the task concerns `gop`, an outpost, a worktree, or a parallel checkout. For mutations, always load **Context and Lifecycle**; also load **Two-Hop Model** for synchronization, publication, or removal. Then load the section matching the user's purpose.

Resolve `Unknown` and require a discoverable work tree before checkout-lifecycle, synchronization, or publication mutation. Before mutation, establish `Ready(command)`: verified live grammar and execution context; resolved selectors, paths, refs, and transport destinations; predicted writes and postconditions; applicable safety evidence and authorization. Any unresolved applicable field means the command is not ready. Status alone never establishes readiness. Check `gop --version` and live subcommand help when syntax may have changed:

```bash
gop <command> --help
```

After any failed multi-step mutation, re-read every affected repository, ref, config or registry file, and filesystem path before retrying. Treat rollback or partial-output deletion as a separate destructive action.

A mutation is complete only after every predicted `Ready(command)` postcondition is observed at its write target. Use the command-specific checks in the workflow reference; exit status alone is insufficient.
