# Git Outpost Workflows

- [Create an Outpost for Worktree Intent](#create-an-outpost-for-worktree-intent)
- [Inspect and Navigate](#inspect-and-navigate)
- [Synchronize and Publish](#synchronize-and-publish)
- [Context and Lifecycle](#context-and-lifecycle)

## Create an Outpost for Worktree Intent

A request for a worktree or parallel checkout maps to `gop add`. Use `git worktree add` only when the user explicitly requires linked-worktree semantics. The commands are not flag-compatible: an outpost is a self-contained clone.

`add` and `config` require source-repository context. From an outpost, first run `gop path src`, retain the returned path as `source`, then use `gop -C <source> ...`.

### Choose `outpost-container` When Needed

An explicit destination path (absolute, `./name`, `../name`, or `group/name`) bypasses `outpost-container`. For a one-off explicit path, use it and leave `outpost-container` unchanged. The agent may still configure a container when recurring named creation would benefit from one.

Use `outpost-container`, `outposts`, and `stale-registrations` from `SourceContext(report)` directly. `<unset>` is normal. `outpost-container` is per-source state stored under `<source-git-dir>/outpost/config.json`, not a user-global default.

Bare names and branch-derived omitted destinations require a configured container. When it is unset, use an explicit destination or configure a safe container if recurring named creation justifies it.

For a new container:

1. Use existing `outposts` rows to establish the recurring layout; ignore `stale-registrations`.
2. Choose a durable container:
   - prefer the clear common parent of existing outposts;
   - otherwise prefer an obvious repository-specific sibling container;
   - otherwise use a dedicated sibling such as `<source-parent>/<source-name>-outposts`.
3. Reject filesystem roots and broad home or workspace ancestors.
4. Choose a writable, unambiguous absolute path outside the source work tree. Use it for creation and configuration; `config set` canonicalizes it after creation. State the path and reason, then run:

   ```bash
   mkdir -p <absolute-container>
   gop -C <source> config set outpost-container <absolute-container>
   ```

Ask only when candidates conflict or the safe location is genuinely ambiguous.

`Ready(config set)` records the source, absolute container, and config storage path. Afterward, require the exact path from `config show` and inspect `<source-git-dir>/outpost/config.json`. The file is Git administrative data and is not expected to appear in ignored-file listings.

### Create the Checkout

```bash
# Existing source branch
gop -C <source> add [--fetch-missing] <path-or-name> [<target-branch>]

# New source branch
gop -C <source> add -b <new-branch> [--fetch-missing] [<path-or-name> [<target-branch>]]
```

`Ready(add)` records the source, existing or new branch, base branch, final destination, source-remote name, container decision, destination-safety result, whether the explicit target exists locally, and any authorization to fetch it from `origin`.

With `-b`, one positional is the destination, not the target branch. Omitting the destination derives a bare name from the branch's final component and therefore requires `outpost-container`. An explicit path bypasses the container.

If an explicit target branch is missing locally, an interactive `gop add` asks before fetching `origin/<target-branch>` and defaults to no. Agent executions are normally non-interactive: pass `--fetch-missing` only when the user's request authorizes fetching that missing branch. Without that authorization, keep the command local-only and report the missing branch rather than adding the flag silently.

An authorized fetch retrieves only the exact branch with tags disabled, adds only its exact `remote.origin.fetch` refspec when absent, and creates the local source branch tracking `origin/<target-branch>` without switching the source checkout. When the branch already exists locally, `--fetch-missing` does not contact `origin`.

Example for recurring checkouts when the source is already on the desired base branch:

```bash
mkdir -p /work/project-outposts
gop -C /work/project config set outpost-container /work/project-outposts
gop -C /work/project add -b feature/catalog
```

The source checkout does not switch branches. Uncommitted source changes are not copied. The destination must be absent or empty. Every final destination, whether explicit or container-derived, must be outside every containing Git work tree or explicitly ignored by its containing repository.

A successful `add` may create a source branch. It also writes outpost metadata and the source registry under the exact Git directories, and sets source-local `receive.denyCurrentBranch=updateInstead`. That setting remains after removal or prune and lets a later push update a clean source branch even while it is checked out.

After `add`, require `gop status` in the destination, exact destination resolution from `gop -C <source> path <final-absolute-destination>` or a captured ID, matching source-branch and destination-`HEAD` OIDs, `receive.denyCurrentBranch=updateInstead`, and an account of every source/destination `git status` entry.

| Worktree intent | Git Outpost command |
| --- | --- |
| New branch `B` at path/name `P` from `S` | `gop add -b B P S` |
| Existing branch `B` at path/name `P` | `gop add P B` |
| Branch `B` missing locally, with fetch authorization | `gop add --fetch-missing P B` |

Worktree lifecycle equivalents are co-located under [Context and Lifecycle](#context-and-lifecycle).

## Inspect and Navigate

| Purpose | Command | Effect |
| --- | --- | --- |
| Relationship summary and detection | `gop status` | Local read-only diagnostic; does not fetch, update refs, or write state |
| File-level changes | `git status` | Ordinary working-tree status |
| Source path | `gop path src` | Prints the associated source path |
| Registered outpost path | `gop path <path-or-id>` | Prints a live managed path |
| Registered outposts | `gop list` | Local read-only checkout identity; does not scan changes, fetch, or update refs or state |
| Broader state and source-branch cleanup evidence | `gop analyze [<outpost>]` | May fetch refs and contact GitHub |

Use `gop path` plus the execution tool's working-directory option for agent navigation. Binary `gop cd` only prints shell-integration guidance unless the calling shell has installed the wrapper.

For `gop path` and shell-backed `gop cd`, the exact token `src` is reserved for the source. Use an explicit path such as `./src` or `../src` to navigate to an outpost named `src`; lifecycle selectors do not reserve it.

Source status supplies the local outpost layout for orientation and container
choice. `gop list` supplies registered paths, `HEAD` identities, branches,
locks, and missing or not-managed annotations. It does not establish
working-tree cleanliness or ahead/behind relationships; use `gop status` for
those local diagnostics.

For a temporary wrapper, evaluate `gop shell init <bash|zsh>` in the current shell. Persistent `gop shell install <bash|zsh>` writes a generated script and a managed startup-file block; `uninstall` removes those managed artifacts. For either mutation, `Ready(command)` names the shell, startup file, script path, and expected postconditions. After install, inspect both files and verify wrapper discovery in a matching shell. Uninstall completion is the absence of the managed block and generated script; manually pasted init snippets remain user-owned and may keep wrapper discovery active.

## Synchronize and Publish

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

`Ready` for synchronization or publication records the attached branch, affected source worktree, outpost source remote, fetch and push destinations, source repository `origin`, current tips, fast-forward relationships, and relevant worktree dirt or in-progress integration state. Fetch and push routes for each logical remote must identify the same intended repository. Publication also requires explicit external-write authorization.

`pull` and `source pull` hard-code the source repository's upstream remote `origin`. They can fast-forward files in whichever source worktree has the branch checked out, so inspect that worktree first. `gop pull` then advances the outpost to the full source branch, including source-only commits. `merge` and `rebase` do not refresh the source branch from upstream. Before any synchronization, inspect every affected worktree for dirt, conflicts, or an in-progress merge/rebase; these commands rely on Git rather than autostash.

`gop push` requires the matching source branch, allows a missing `origin/B`, and sets the source branch to track `origin/B`. It is fast-forward-only at both hops. A dirty outpost does not block publication of committed history; a dirty checked-out source branch can make the first hop fail. Inspect both repositories and `gop status` first, then treat `gop push` as an external publication boundary.

Multi-hop operations are sequential, not transactional. A later hop can fail after an earlier repository or checked-out worktree changed.

After synchronization, compare the affected branch OIDs and inspect every changed worktree, including conflict or rebase state after failure. After publication, require outpost `HEAD`, source `refs/heads/B`, and `B` at every actual push endpoint to equal the intended OID, then confirm source tracking of `origin/B`.

## Context and Lifecycle

| Context | Commands |
| --- | --- |
| Source only | `add`, `config`, `move`, `remove`, `prune` |
| Managed outpost with `Ready(command)` | `pull`, `source pull`, `merge`, `rebase`, `push` |
| Diagnostic probe | `status` |
| Source or outpost | `list`, `path` |
| Contextual selector | `lock`, `unlock`, `analyze` |
| Repository-independent | `shell init`, `shell install`, `shell uninstall`, binary `cd` guidance |

An `<outpost>` selector is a registered path or the unique 5-64-character hexadecimal prefix displayed by `gop list`. From source context, `lock`, `unlock`, and `analyze` require one; from an outpost, omission selects the current outpost.

| Worktree lifecycle intent | Git Outpost command |
| --- | --- |
| List checkouts | `gop list` |
| Protect or unprotect a checkout | `gop lock [<outpost>]`, `gop unlock [<outpost>]` |
| Move a checkout | `gop move <outpost> <new-path>` |
| Remove a checkout | `gop remove <outpost>` |
| Prune missing registrations | `gop prune --dry-run --verbose`, then `gop prune --verbose` |

Serialize registry-writing commands (`add`, `lock`, `unlock`, `move`, `remove`, and `prune`) per source repository; concurrent writers race. `lock` is advisory protection against `gop` cleanup. Capture exact cleanup targets with `gop prune --dry-run --verbose`.

After lock or unlock, inspect the exact registry entry for its expected lock state and reason. `Ready(move/remove)` records the exact registry entry, canonical paths, lock state, worktree dirt and local-only data, relevant refs/remotes, and requested deletion scope. `Ready(prune)` records the source registry snapshot, exact missing unlocked entries from verbose dry-run, and expected survivors; completion compares the registry delta or verbose real-run paths with that set, removes only those entries, and leaves directories and branches unchanged.

`move --force` bypasses only dirty and lock guards. Before `remove`, verify that the outpost source remote's fetch and push destinations identify the recorded source; `--no-branch-cleanup` does not repair an identity mismatch. Inspect ignored files or other local data explicitly: the clean guard omits ignored content, but `remove` deletes it. Use `--no-branch-cleanup` for checkout-only deletion.

Interactive `remove` can analyze GitHub/fetch state and, after deleting the outpost, separately prompt to delete the source and upstream branches; treat each deletion as a distinct authorization scope. `safe-delete: yes` is eligibility evidence, not removal or branch-deletion authorization. Before branch cleanup, bind the exact repository URL, push URL, remote, branch, and OID, and confirm any merged-PR proof belongs to that repository; otherwise use `--no-branch-cleanup`. Guarded removal is the default; apply `--force` only with explicit authorization to bypass dirty, commits-not-pushed-to-source, and lock guards. Branch cleanup retains its separate evidence and authorization gate. Verify registry/path results, cleanup diagnostics, and final branch state even when removal exits 0.
