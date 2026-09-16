# Working in an Outpost

Run these commands in the selected outpost checkout. Use `gop -C <outpost-path> ...` when running from another directory.

## Read Outpost Status

Reuse this outpost's orientation report. When selecting another checkout, run `gop --no-color -C <outpost-path> status` to orient there.

Read these fields from the report:

- `outpost:` identifies the current checkout and `source:` identifies its source.
- `remote:` names the outpost's source remote. Use that name instead of assuming `local`.
- `branch:` identifies the current branch or detached `HEAD`; `outpost-state:` reports whether the checkout is clean or dirty.
- `source-upstream:` describes the same-named source branch's tracked upstream. If fetch and push routes differ, read `source-upstream-fetch:` and `source-upstream-push:`.

Preserve `none`, `-`, `<unset>`, `<not-applicable>`, and `<unavailable>` as explicit results. Status comparisons use existing local refs. Summarize the paths, roles, and problems relevant to the user's task.

Read any `health: problems` before choosing an operation. Stop synchronization, publication, or destructive lifecycle work if status reports a remote/source mismatch: the configured remote and reported source can name different repositories.

## Inspect and Navigate

| Purpose | Command | Effect |
| --- | --- | --- |
| File-level changes | `git status` | Ordinary working-tree status |
| Source path | `gop path src` | Prints the associated source path |
| Another registered outpost's path | `gop path <outpost>` | Prints a live managed path |
| Registered outposts | `gop list` | Lists the source's outposts using local information |
| Remote branch state, matching PRs, and source-branch cleanup eligibility | `gop --no-color analyze` | Analyzes the current outpost; may fetch refs and contact GitHub |

An `<outpost>` selector accepts a registered path or the unique ID prefix shown by `gop list`. Use `gop path` plus the execution tool's working-directory option for agent navigation. For `gop path`, `src` is reserved for the source; use an explicit path such as `./src` or `../src` for an outpost named `src`.

`gop list` reports paths, `HEAD` identities, branches, locks, and missing or not-managed annotations. It does not scan file changes, fetch, or update refs or state. Use `gop status` in the relevant checkout for local cleanliness and ahead/behind diagnostics.

Use `gop analyze` when the task needs upstream branch comparisons, push hazards, matching GitHub PRs, or source-branch cleanup eligibility. It may fetch remote-tracking refs and query GitHub while leaving working files and local branches unchanged.

In `pull-requests:`, a `- none` result means the lookup completed with no matching PR. An `unknown` or `unavailable` result is inconclusive; preserve the reported reason. `safe-delete: yes` describes source-branch deletion eligibility; deletion still requires the user's authorization.

## Synchronize and Publish

Choose the intended branch or source ref, affected checkouts, and destination repositories. Publication requires authorization to write to the upstream repository.

| Purpose | Command | Repository hops |
| --- | --- | --- |
| Fast-forward the current branch `B` | `gop pull` | `origin/B` -> source `B` -> outpost `B` |
| Refresh another source branch `B` | `gop source pull B` | `origin/B` -> source `B` |
| Linear integration | `gop rebase <source-remote>/<branch>` | source -> outpost |
| Merge integration | `gop merge <source-remote>/<branch>` | source -> outpost |
| Push only to the source | `git push <verified-source-remote> B:B` | outpost `B` -> source `B` |
| Publish attached branch `B` | `gop push` | outpost `B` -> source `B` -> `origin/B` |

For current work rebased onto upstream `main`:

```bash
gop source pull main
gop rebase <source-remote>/main
```

For source-only Git pushes, use an explicitly verified source remote and branch refspec.

`pull` and `source pull` use the source repository's remote `origin`. They can fast-forward files in whichever source worktree has the branch checked out, so include that worktree update in the intended scope. `gop pull` then integrates the full source branch, including source-only commits. `merge` and `rebase` do not refresh the source branch from upstream. These commands rely on Git rather than autostash.

`gop push` requires the matching source branch and allows a missing `origin/B`. Successful execution confirms that both fast-forward pushes completed, that `origin/B` matches the published source commit, and that the source branch tracks `origin/B`. A dirty outpost does not block publication of committed history; a dirty checked-out source branch can make the first push fail.

A later step can fail after an earlier repository or checked-out worktree changed. After a failed synchronization, inspect affected worktrees for conflict or rebase state.

## Protect the Current Outpost

```bash
gop lock
gop unlock
```

A lock protects the outpost against `gop` cleanup. Run locking and unlocking one at a time with other creation or lifecycle operations for the same source repository.

For creation, configuration, moving, removal, or pruning, use the reported `source:` path and follow [source workflows](source-workflows.md).
