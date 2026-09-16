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

The current checkout is the directory against which `gop` runs; it can be the
source or an outpost. Git operations using the outpost's source remote reach
the source repository. `gop` also provides workflows that reach upstream.

## Orient First

Run one status command against the relevant checkout:

```bash
gop --no-color -C <path> status
```

On success, `context: source` means the current checkout is the source
repository; `context: outpost` means it is an outpost. An outpost reporting
`health: problems` is still an outpost. Read its reported problems before
choosing an operation.

Use the same report to understand the repository relationships:

- `source:` identifies the source checkout. In an outpost, `outpost:` identifies
  the current checkout; in the source, `outposts:` lists registered outposts.
- In an outpost, `remote:` names its source remote. Use that name instead of
  assuming `local`.
- Source status reports the tracked upstream under `upstream:`; outpost status
  uses `source-upstream:`. If fetch and push routes differ, read the split
  `upstream-fetch:` and `upstream-push:` fields, or `source-upstream-fetch:` and
  `source-upstream-push:` in an outpost.

Preserve `none`, `-`, `<unset>`, `<not-applicable>`, and `<unavailable>` as
explicit results. Status is local and read-only: comparisons use existing
local refs, and the report may include stale registrations. Use this report
for orientation without reconstructing it through Git or filesystem probes.
Summarize the paths, roles, and problems relevant to the user's task.

If status fails or its output cannot be interpreted, preserve the error and
available output and resolve the uncertainty before making changes. If `gop`
is unavailable, report the command error and stop using this skill.

Stop synchronization, publication, or destructive lifecycle work if status
reports a remote/source mismatch: the configured remote and reported source
can name different repositories.

## Choose the Workflow

For `gop` operations, use [Command Locations and Selectors](references/gop-workflows.md#command-locations-and-selectors) to choose the working directory and outpost target, then read the workflow matching the user's purpose:

- [Create an Outpost for Worktree Intent](references/gop-workflows.md#create-an-outpost-for-worktree-intent) for a worktree, parallel checkout, or new outpost.
- [Inspect and Navigate](references/gop-workflows.md#inspect-and-navigate) for checkout paths, local status, or branch and PR analysis.
- [Synchronize and Publish](references/gop-workflows.md#synchronize-and-publish) for pulling, integrating, or publishing branch changes.
- [Lifecycle](references/gop-workflows.md#lifecycle) for locking, moving, removing, or pruning outposts.

Use the status report when choosing commands and targets. Resolve only the additional facts and authorization needed for the requested operation. Check `gop --version` and live subcommand help when syntax may have changed:

```bash
gop <command> --help
```

When a `gop` command completes successfully without reporting a failed step, trust its guaranteed results. Additional checks serve command, target, or authorization choices, separate outcomes such as PR state or CI, or failure recovery. If the task requires a guarantee that `gop` does not provide, report that limitation.

After a failed multi-step operation, inspect affected repositories before retrying. Treat rolling back changes or deleting partially created files as separate destructive actions.
