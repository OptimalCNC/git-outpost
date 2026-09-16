# Git Outpost Workflows

- [Command Locations and Selectors](#command-locations-and-selectors)
- [Create an Outpost for Worktree Intent](#create-an-outpost-for-worktree-intent)
- [Inspect and Navigate](#inspect-and-navigate)
- [Synchronize and Publish](#synchronize-and-publish)
- [Lifecycle](#lifecycle)

## Command Locations and Selectors

| Run from | Commands |
| --- | --- |
| Source checkout | `add`, `config`, `move`, `remove`, `prune` |
| Outpost checkout | `pull`, `source pull`, `merge`, `rebase`, `push` |
| Source or outpost | `status`, `list`, `path`, `lock`, `unlock`, `analyze` |

When a command requires the source checkout, use the reported `source:` path with `gop -C <source> ...`.

An `<outpost>` selector accepts a registered path or the unique ID prefix shown by `gop list`. From the source checkout, `lock`, `unlock`, and `analyze` require a selector; from an outpost, omission selects the current outpost.

Run `add`, `lock`, `unlock`, `move`, `remove`, and `prune` one at a time for each source repository.

## Create an Outpost for Worktree Intent

A request for a worktree or parallel checkout maps to `gop add`. Use `git worktree add` only when the user explicitly requires linked-worktree semantics. The commands are not flag-compatible: an outpost is a self-contained clone.

### Choose `outpost-container` When Needed

An explicit destination path (absolute, `./name`, `../name`, or `group/name`) bypasses `outpost-container`. For a one-off explicit path, use it and leave `outpost-container` unchanged. The agent may still configure a container when recurring named creation would benefit from one.

When choosing a container, use `outpost-container` and `outposts` from `gop --no-color -C <source> status`, reusing the report if available. `<unset>` is normal. The container setting belongs to that source checkout; linked source worktrees have independent settings and outpost registrations.

Bare names and branch-derived omitted destinations require a configured container. When it is unset, use an explicit destination or configure a safe container if recurring named creation justifies it.

For a new container:

1. Use existing `outposts` rows to establish the recurring layout; ignore `stale-registrations`.
2. Choose a durable container:
   - prefer the clear common parent of existing outposts;
   - otherwise prefer an obvious repository-specific sibling container;
   - otherwise use a dedicated sibling such as `<source-parent>/<source-name>-outposts`.
3. Reject filesystem roots and broad home or workspace ancestors.
4. Choose a writable, unambiguous absolute path outside the source work tree. State the path and reason, then run:

   ```bash
   mkdir -p <absolute-container>
   gop -C <source> config set outpost-container <absolute-container>
   ```

Ask only when candidates conflict or the safe location is genuinely ambiguous.

### Create the Checkout

Choose the source checkout, branch to check out or create, base branch, and destination.

```bash
# Existing source branch
gop -C <source> add [--fetch-missing] [--remote-name <name>] <path-or-name> [<target-branch>]

# New source branch
gop -C <source> add -b <new-branch> [--fetch-missing] [--remote-name <name>] [<path-or-name> [<target-branch>]]
```

Omitting `<target-branch>` uses the source checkout's current branch, including when this workflow starts in an outpost. Pass the target explicitly when another branch is intended.

With `-b`, one positional is the destination, not the target branch. Omitting the destination derives a bare name from the branch's final component and therefore requires `outpost-container`. An explicit path bypasses the container.

`--remote-name <name>` sets the source remote's name inside the new outpost; the default is `local`.

If an explicit target branch is missing locally, an interactive `gop add` asks before fetching `origin/<target-branch>` and defaults to no. Agent executions are normally non-interactive: pass `--fetch-missing` only when the user's request authorizes fetching that missing branch. Without that authorization, keep the command local-only and report the missing branch rather than adding the flag silently.

An authorized fetch retrieves only the requested branch without tags, creates the local source branch tracking `origin/<target-branch>`, and adds that branch to the source's fetch configuration when needed. When the branch already exists locally, `--fetch-missing` does not contact `origin`.

Example for recurring checkouts when the source is already on the desired base branch:

```bash
mkdir -p /work/project-outposts
gop -C /work/project config set outpost-container /work/project-outposts
gop -C /work/project add -b feature/catalog
```

The source checkout does not switch branches. Uncommitted source changes are not copied. The destination must be absent or empty. `gop add` checks the containing Git work tree and requires an in-tree destination to be explicitly ignored by that repository.

A successful `add` registers the outpost, may create a source branch, and sets source-local `receive.denyCurrentBranch=updateInstead`. That setting remains after removal or prune and lets a later push update a clean source branch even while it is checked out.

| Worktree intent | Git Outpost command |
| --- | --- |
| New branch `B` at path/name `P` from `S` | `gop add -b B P S` |
| Existing branch `B` at path/name `P` | `gop add P B` |
| Branch `B` missing locally, with fetch authorization | `gop add --fetch-missing P B` |

For moving or removing the checkout, read [Lifecycle](#lifecycle).

## Inspect and Navigate

| Purpose | Command | Effect |
| --- | --- | --- |
| Relationship summary and detection | `gop status` | Local read-only diagnostic; does not fetch, update refs, or write state |
| File-level changes | `git status` | Ordinary working-tree status |
| Source path | `gop path src` | Prints the associated source path |
| Registered outpost path | `gop path <path-or-id>` | Prints a live managed path |
| Registered outposts | `gop list` | Local read-only checkout identity; does not scan changes, fetch, or update refs or state |
| Remote branch state, matching PRs, and source-branch cleanup eligibility | `gop analyze [<outpost>]` | May fetch refs and contact GitHub |

Use `gop path` plus the execution tool's working-directory option for agent navigation.

For `gop path`, the exact token `src` is reserved for the source. Use an explicit path such as `./src` or `../src` to navigate to an outpost named `src`; lifecycle selectors do not reserve it.

Source status supplies the local outpost layout for orientation and container
choice. `gop list` supplies registered paths, `HEAD` identities, branches,
locks, and missing or not-managed annotations. It does not establish
working-tree cleanliness or ahead/behind relationships; use `gop status` in
the relevant checkout for those local diagnostics.

Use `gop analyze` when the task needs upstream branch comparisons, push hazards, matching GitHub PRs, or source-branch cleanup eligibility. It may fetch remote-tracking refs and query GitHub while leaving working files and local branches unchanged.

```bash
# Current outpost
gop --no-color analyze

# From the source checkout
gop --no-color -C <source> analyze <outpost>
```

In `pull-requests:`, a `- none` result means the lookup completed with no matching PR. An `unknown` or `unavailable` result is inconclusive; preserve the reported reason. `safe-delete: yes` describes source-branch deletion eligibility; deletion still requires the user's authorization.

## Synchronize and Publish

Choose the intended branch or source ref, affected checkouts, and destination repositories. Publication requires authorization to write to the upstream repository.

| Purpose | Command | Repository hops |
| --- | --- | --- |
| Fast-forward the current branch `B` | `gop pull` | `origin/B` -> source `B` -> outpost `B` |
| Refresh another source branch `B` | `gop source pull B` | `origin/B` -> source `B` |
| Linear integration | `gop rebase <source-remote>/<branch>` | source -> outpost |
| Merge integration | `gop merge <source-remote>/<branch>` | source -> outpost |
| Push only to the source | `git push <verified-source-remote> B:B` from the outpost | outpost `B` -> source `B` |
| Publish attached branch `B` | `gop push` | outpost `B` -> source `B` -> `origin/B` |

For current work rebased onto upstream `main`:

```bash
gop source pull main
gop rebase <source-remote>/main
```

For source-only Git pushes, use an explicitly verified source remote and branch refspec.

`pull` and `source pull` use the source repository's remote `origin`. They can fast-forward files in whichever source worktree has the branch checked out, so include that worktree update in the intended scope. `gop pull` then integrates the full source branch, including source-only commits. `merge` and `rebase` do not refresh the source branch from upstream. These commands rely on Git rather than autostash.

`gop push` requires the matching source branch and allows a missing `origin/B`. Successful execution confirms that both fast-forward pushes completed, that `origin/B` matches the published source commit, and that the source branch tracks `origin/B`. A dirty outpost does not block publication of committed history; a dirty checked-out source branch can make the first push fail.

A later step can fail after an earlier repository or checked-out worktree changed.

After a failed synchronization, inspect affected worktrees for conflict or rebase state.

## Lifecycle

Before moving or removing an outpost, identify the selected checkout, destination for a move, requested deletion scope, and any explicit force authorization.

| Worktree lifecycle intent | Git Outpost command |
| --- | --- |
| List checkouts | `gop list` |
| Protect or unprotect a checkout | `gop lock [<outpost>]`, `gop unlock [<outpost>]` |
| Move a checkout | `gop move <outpost> <new-path>` |
| Remove a checkout | `gop remove <outpost>` |
| Prune missing registrations | `gop prune --dry-run --verbose`, then `gop prune --verbose` |

`lock` protects an outpost against `gop` cleanup. Capture exact cleanup targets with `gop prune --dry-run --verbose`.

Successful `prune` removes missing unlocked registrations and leaves directories and branches unchanged.

`move --force` bypasses only dirty and lock guards. Before `remove`, inspect ignored files or other local data explicitly: the clean guard omits ignored content, but `remove` deletes it. Use `--no-branch-cleanup` for checkout-only deletion.

Interactive `remove` can analyze GitHub/fetch state and, after deleting the outpost, separately prompt to delete the source and upstream branches; treat each deletion as a distinct authorization scope. Guarded removal is the default; apply `--force` only with explicit authorization to bypass dirty, commits-not-pushed-to-source, and lock guards. Read the reported branch-cleanup outcomes: a warning can indicate an incomplete cleanup step even when removal exits 0.
