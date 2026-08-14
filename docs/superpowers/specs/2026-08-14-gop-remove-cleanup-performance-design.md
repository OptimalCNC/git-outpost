# `gop remove` Cleanup Performance Design

## Purpose

Make interactive `gop remove` reach its branch-cleanup prompts with at most one
remote evidence request on the normal path, while preserving the existing
proof requirements and exact-OID deletion guards. Reuse that same evidence in
`gop analyze` so remote identities, safe-delete proof, and GitHub pull-request
metadata do not independently repeat equivalent observations.

This change optimizes evidence collection. It does not weaken eligibility,
change prompt order, make cleanup failures fatal to outpost removal, or add the
separate policy that the upstream branch OID must equal the source branch OID.

## Current Cost

After local safety checks, cleanup currently performs these serial operations:

1. discover and fetch the upstream default branch;
2. observe the candidate upstream branch with `ls-remote`;
3. run one or two `gh pr list` requests;
4. after deleting the source branch, observe the upstream branch again;
5. finally run the leased deletion push if the user approves it.

`gop analyze` performs its own upstream/default observations and fetches, then
calls branch cleanup analysis, which repeats them, and finally requests GitHub
pull-request metadata again. The network round trips, rather than local Git
work, dominate the delay.

## Chosen Approach

Batch provider evidence behind one semantic seam. Replace the PR-only provider
interface with a single snapshot operation:

```rust
pub trait CleanupEvidenceProvider {
    fn snapshot(
        &self,
        request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>>;
}
```

`CleanupEvidenceRequest` binds every observation to the selected upstream
remote name and URL, candidate branch, and already-proven source OID.
`CleanupEvidenceSnapshot` owns the observed default branch tip, optional
candidate branch tip, and optional exact merged-pull-request proof. `None`
means that an adapter does not handle the requested remote; errors are
diagnostic and permit the generic Git adapter to try.

This is a deeper module than the current callback: callers ask one question,
while provider-specific batching, output parsing, exact proof selection, and
caching stay behind the interface. There are three real adapters at the seam:
the GitHub GraphQL adapter, the generic Git adapter, and deterministic test
adapters.

## Evidence Adapters

### GitHub GraphQL

For a recognized GitHub remote, the CLI adapter issues one `gh api graphql`
command. The document requests:

- `defaultBranchRef { name target { oid } }`;
- `ref(qualifiedName: $candidateRef) { target { oid } }`;
- merged pull requests selected by head name;
- a repository-scoped SHA search as a second alias in the same document;
- the pull-request fields already rendered by `gop analyze`, including the
  last commit's status/check rollup.

The adapter accepts merged proof only when both `headRefName` and `headRefOid`
equal the request. It caches the parsed operation result inside the probe.
`GhStatus::analyze` reads the cached summaries, so it does not issue a second
GitHub request after cleanup analysis.

`gh --version` is removed from the eager path. Missing executables,
authentication errors, and GraphQL failures are learned from the useful API
command itself and retained for the existing fallback diagnostic.

### Generic Git

If no provider is present, the provider declines the URL, or the provider
fails, core runs one batched observation:

```text
git ls-remote --symref <remote> HEAD refs/heads/<candidate>
```

The parser returns the default branch name and HEAD OID together with the
optional candidate OID. Malformed or incomplete output remains unavailable;
cleanup fails closed and never prompts without a default-branch identity and a
valid proof.

This path works for local bare repositories, SSH remotes, and non-GitHub
hosting without depending on `gh`.

## Cleanup Data Flow

The ordering is:

```text
resolve and validate managed outpost
-> run dirty/unpushed safety gates
-> capture all outpost-local cleanup facts
-> collect one evidence snapshot
-> evaluate exact PR proof or default ancestry
-> save registry and remove outpost directory
-> prompt for exact-OID source deletion
-> optionally prompt for snapshot-OID upstream deletion
-> perform the exact --force-with-lease push
```

Disabled, non-interactive, missing-outpost, and locally ineligible paths stop
before provider or generic remote evidence calls. Evidence collection stays
before filesystem removal; no thread or background mutation is introduced.
The latency reduction comes from batching and removing duplicate work, rather
than overlapping safety-sensitive stages.

The default branch is provider-observed metadata. A merged PR proof needs no
default fetch. For ancestry proof, core first checks whether the observed
default OID is already present locally. Only a missing object triggers an
exact default-branch fetch. Ancestry is still tested against the snapshot OID;
if a fetch does not make that OID available, cleanup records a warning and
does not produce a candidate.

## Deletion Races

Source deletion remains:

```text
git update-ref -d refs/heads/<branch> <snapshot-source-oid>
```

The second pre-prompt upstream `ls-remote` is removed. If the snapshot contains
an upstream OID and the source deletion succeeds, the user may approve remote
deletion. The only mutation is:

```text
git push --force-with-lease=refs/heads/<branch>:<snapshot-upstream-oid> \
  <remote> :refs/heads/<branch>
```

The lease is the atomic final observation-and-delete guard. If the remote
branch moves or disappears after the snapshot, Git rejects the push and the
operation records the existing warning outcome; it never deletes the moved
branch.

## `gop analyze` Reuse

Branch cleanup analysis exposes its operation-scoped evidence observation.
`ops::analyze` uses it for:

- upstream remote name and URL;
- upstream candidate branch identity;
- upstream default branch identity;
- source-to-upstream comparisons;
- safe-delete proof;
- the CLI adapter's cached GitHub summaries.

Ahead/behind calculations require commit objects, not another identity
observation. Analyze first uses the observed OIDs directly. If one or both are
missing, it fetches only the required named refs, batching both into one fetch
when possible, and still compares against the original snapshot OIDs. Existing
read-only semantics are preserved: remote-tracking refs may change, but local
branches, outpost branches, the registry, and GitHub state do not.

When local cleanup eligibility prevents creation of an evidence request,
`analyze` retains its current best-effort probes. The eligible/common path uses
one snapshot throughout.

## Failure Behavior

- Provider decline: silently use generic Git evidence.
- Provider execution or parse failure: record the provider warning and try
  generic Git evidence.
- Generic observation failure: report unavailable evidence and skip cleanup.
- Unknown default branch: report `DefaultBranchUnknown` and never prompt.
- Mismatched merged PR fields: warn and try ancestry proof.
- Missing default object: fetch only that named branch, then retry against the
  observed OID.
- Remote movement before deletion: the leased push fails and leaves the remote
  branch intact.

Outpost removal continues once its preflight succeeds. Branch cleanup errors
remain report outcomes rather than rollback triggers.

## Verification Strategy

Performance tests assert operation counts and arguments, not elapsed time:

- no external evidence calls on disabled, non-interactive, and locally
  ineligible removal paths;
- exactly one provider snapshot call for a matching PR proof;
- one GraphQL process invocation, with no preceding `--version` or following
  PR-list invocation;
- one batched generic `ls-remote` command and correct parsing;
- no default fetch when exact PR proof succeeds or the default OID is local;
- a default fetch only when ancestry needs a missing object;
- no upstream observation after source deletion;
- the final deletion push uses the snapshot OID in its lease;
- a moved upstream branch makes the push fail and remains present;
- eligible `analyze` uses one snapshot for report identities and cleanup proof.

The full workspace format, lint, unit/integration, and documentation build
checks run after the focused red-green cycles.
