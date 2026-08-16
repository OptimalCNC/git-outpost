---
name: using-git-outpost
description: Use when beginning work in a Git checkout, or when a Git task involves gop, Git Outpost, outposts, worktrees, parallel checkouts, checkout navigation or lifecycle, branch synchronization, or branch publication.
---

# Using Git Outpost

## Overview

Treat Git Outpost as a self-contained-clone realization of worktree intent. Orient once, then use ordinary Git for files and commits and `gop` for checkout topology or the two-hop path between outpost, source, and upstream.

## Orient First

Normal orientation has this exact recipe:

1. Run exactly one command directly against the relevant input path:

   ```bash
   gop --no-color -C <path> status
   ```

2. Return exactly one state label verbatim:
   - Exit 0 with first line `context: source`: `SourceContext(report)`.
   - Exit 0 with first line `context: outpost`: `ManagedOutpostContext(report)` for both `health: ok` and `health: problems`.
   - Every nonzero result: `Unknown(error)`, preserving the error.

3. For a successful state, preserve the full report, carry it forward, and end orientation. The report contains local facts and may include degraded health or stale registrations.

If `gop` is unavailable, use `git -C <path> config --local --type=bool --get outpost.managed`. Only `true` establishes `ManagedOutpostWithoutGop`; treat every other result as unknown and report that the CLI is unavailable. This fallback proves only the local marker and supports ordinary file and commit work, not source orientation or `gop` workflows.

Orientation is complete only after naming the state.

## Choose the Workflow

Read [references/gop-workflows.md](references/gop-workflows.md) when the state is managed or the task concerns `gop`, an outpost, a worktree, or a parallel checkout. For mutations, always load **Context and Lifecycle**; also load **Two-Hop Model** for synchronization, publication, or removal. Then load the section matching the user's purpose.

For worktree, parallel-checkout, or outpost-creation tasks, read [Create an Outpost for Worktree Intent](references/gop-workflows.md#create-an-outpost-for-worktree-intent) before constructing the command and use its command forms.

Build `Ready(command)` from the carried report facts plus only missing command-specific facts: verified live grammar and execution context; resolved selectors, paths, refs, and transport destinations; predicted writes and postconditions; applicable safety evidence and authorization. Resolve `Unknown` before mutation. Any unresolved applicable field means the command is not ready. Check `gop --version` and live subcommand help when syntax may have changed:

```bash
gop <command> --help
```

After any failed multi-step mutation, re-read every affected repository, ref, config or registry file, and filesystem path before retrying. Treat rollback or partial-output deletion as a separate destructive action.

A mutation is complete only after every predicted `Ready(command)` postcondition is observed at its write target. Use the command-specific checks in the workflow reference; exit status alone is insufficient.
