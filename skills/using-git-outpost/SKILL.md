---
name: using-git-outpost
description: Use when beginning work in a Git checkout, or when a Git task involves gop, Git Outpost, outposts, worktrees, parallel checkouts, checkout navigation or lifecycle, branch synchronization, or branch publication.
disable-model-invocation: true
---

# Using Git Outpost

## Overview

Git Outpost provides a worktree-like parallel-checkout workflow using normal
local clones with their own `.git` directories. Use ordinary Git for files and
commits, and `gop` to manage checkouts and synchronize or publish branches
through their source repository.

## Core Model

Git Outpost links three repository roles:

- **Source:** the existing local repository from which `gop` creates and
  registers outposts.
- **Outpost:** a self-contained local clone of the source. Its configurable
  source remote points back to the source repository.
- **Upstream:** the repository behind the source branch's tracked upstream.

```text
outpost <-> source repository <-> upstream repository
```

## Orient First

Run one status command against the relevant checkout:

```bash
gop --no-color -C <path> status
```

On success, `context: source` identifies the source checkout and
`context: outpost` identifies an outpost. An outpost reporting
`health: problems` is still an outpost. Status is local and read-only; reuse
its report in the selected workflow without reconstructing it through Git or
filesystem probes.

Stop synchronization, publication, or destructive lifecycle work on a reported
remote/source mismatch.

If status fails or its output cannot be interpreted, preserve the error and
available output and resolve the uncertainty before making changes. If `gop`
is unavailable, report the command error and stop using this skill.

## Choose the Workflow

Use the current checkout as the default, and load only the guide needed for
the requested operation:

- [Outpost workflows](references/outpost-workflows.md): inspect, navigate,
  synchronize, integrate, publish, or protect work in an outpost.
- [Source workflows](references/source-workflows.md): inspect registered
  outposts, configure or create checkouts, and manage their lifecycle.

Creation, configuration, moving, removal, and pruning use the source guide
even when the task starts in an outpost. Working in a selected outpost uses
the outpost guide even when the task starts in the source. Follow the link to
the other guide only when the task needs its workflow.

Use the status report when choosing commands and targets. Resolve only the additional facts and authorization needed for the requested operation. Check `gop --version` and `gop <command> --help` when syntax may have changed.

When a `gop` command completes successfully without reporting a failed step, trust its guaranteed results. Additional checks serve command, target, or authorization choices, separate outcomes such as PR state or CI, or failure recovery. If the task requires a guarantee that `gop` does not provide, report that limitation.

After a failed multi-step operation, inspect affected repositories before retrying. Treat rolling back changes or deleting partially created files as separate destructive actions.
