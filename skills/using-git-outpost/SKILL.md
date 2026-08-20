---
name: using-git-outpost
description: Use when beginning work in a Git checkout, or when a Git task involves gop, Git Outpost, outposts, worktrees, parallel checkouts, checkout navigation or lifecycle, branch synchronization, or branch publication.
disable-model-invocation: true
---

# Using Git Outpost

## Overview

Git Outpost provides a worktree-like parallel-checkout workflow using normal
local clones with their own `.git` directories. Orient once, then use ordinary
Git for files and commits and `gop` for checkout topology and explicit two-hop
synchronization and publication.

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

The current worktree is the checkout against which `gop` runs; it can be the
source or a managed outpost. In an outpost, read the source remote from
`remote:` in `gop status` instead of assuming `local`. Read the upstream
remote, ref, and fetch/push routes from the report's `upstream` fields in
source context or `source-upstream` fields in outpost context. Ordinary Git in
an outpost covers the direct source hop. `gop` owns outpost lifecycle and
explicit two-hop workflows.

Treat a reported remote/source mismatch as a hard stop for synchronization,
publication, or destructive lifecycle work: the configured remote and
recorded source can otherwise name different repositories. A clean status does
not verify missing remotes, push URLs, or push-routing overrides. Before
transport or deletion, resolve the outpost source remote's fetch and push
destinations and source `origin` as applicable. Every fetch and push route for
a logical remote must identify the same intended repository; lookup failure or
mismatch is a hard stop. Use an explicit verified remote and refspec for
source-only pushes.

## Private State

Git Outpost's private state is stored below the exact per-worktree Git
directory reported by `git rev-parse --git-dir`:

```text
<git-dir>/outpost/config.json
<git-dir>/outpost/registry.json
<git-dir>/outpost/metadata.json
```

Linked worktrees have independent state directories; the shared Git common
directory is never the state authority. These files are Git administrative
data, so `git clean -fdx` and ignored-file listings do not remove or show
them. During the temporary migration period, a read with no current state may
import legacy `<worktree>/.outpost/*.json` or local `outpost.*` values into the
new files. After re-reading and verifying the new state, migration removes only
the imported legacy file or the three known legacy keys. If valid current state
already exists, it is authoritative: migration does not parse or compare the
legacy contents and only removes those known leftovers. A first `gop status`
may perform that local migration cleanup and must keep its report and context
classification unchanged.

## Orient First

Normal orientation has this exact recipe:

1. Run exactly one command directly against the relevant input path:

   ```bash
   gop --no-color -C <path> status
   ```

2. Parse the report and return exactly one state label verbatim:
   - Exit 0 with first line `context: source` and the required source fields: `SourceContext(report)`.
   - Exit 0 with first line `context: outpost` and the required outpost fields: `ManagedOutpostContext(report)` for both `health: ok` and `health: problems`.
   - Every nonzero result or malformed exit-0 report: `Unknown(error)`, preserving the error and any report output.

3. For a successful state, preserve the full report and identify all of the
   following from that same report:
   - **Current worktree:** its reported path and its `source` or `outpost` role.
   - **Source:** the reported `source:` path.
   - **Outpost:** the current `outpost:` path in outpost context; in source
     context, the complete registered set under `outposts:`, including `none`.
   - **Upstream:** the remote/ref and route reported by `upstream:`,
     `upstream-fetch:` and `upstream-push:` in source context, or by
     `source-upstream:`, `source-upstream-fetch:` and
     `source-upstream-push:` in outpost context.
   - **Outpost-to-source link:** the reported `remote:` in outpost context; it
     is not applicable to the source worktree itself.

   Preserve `none`, `-`, `<unset>`, `<not-applicable>`, and `<unavailable>` as
   explicit results. Do not infer replacements or run secondary Git probes
   during normal orientation. The report contains local facts and may include
   degraded health or stale registrations.

`gop` is required. If it is unavailable, preserve the command error as `Unknown(error)` and stop using this skill.

Orientation is complete only after naming the state and every applicable
identity above. `gop status` is the sole authority for normal orientation;
later `Ready(command)` checks may resolve additional command-specific facts.

## Choose the Workflow

Read [references/gop-workflows.md](references/gop-workflows.md) when the state is managed or the task concerns `gop`, an outpost, a worktree, or a parallel checkout. For mutations, always load **Context and Lifecycle**, then load the section matching the user's purpose.

For worktree, parallel-checkout, or outpost-creation tasks, read [Create an Outpost for Worktree Intent](references/gop-workflows.md#create-an-outpost-for-worktree-intent) before constructing the command and use its command forms.

`Ready(command)` extends the carried successful report without another orientation call by adding only missing command-specific facts (verified live grammar and execution context; resolved selectors, paths, refs, and transport destinations; predicted writes and postconditions; applicable safety evidence and authorization); a later `gop status` serves as a command-specific postcondition check after mutation. Resolve `Unknown` before mutation. Any unresolved applicable field means the command is not ready. Check `gop --version` and live subcommand help when syntax may have changed:

```bash
gop <command> --help
```

After any failed multi-step mutation, re-read every affected repository, ref, config or registry file, and filesystem path before retrying. Treat rollback or partial-output deletion as a separate destructive action.

A mutation is complete only after every predicted `Ready(command)` postcondition is observed at its write target. Use the command-specific checks in the workflow reference; exit status alone is insufficient.
