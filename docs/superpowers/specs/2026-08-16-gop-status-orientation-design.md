# `gop status` Orientation Design

## Purpose

Make `gop status` the single local orientation command for Git Outpost. The
command succeeds from either a source repository or a managed outpost, names
that context on its first line, and reports the locally knowable facts useful
to a person or agent in that context.

The `using-git-outpost` skill will use this one command for normal orientation
instead of combining `git rev-parse`, an outpost-only `gop status`, exit-code
interpretation, and registry inference. The skill still treats status as
orientation, not as proof that a later mutation is ready.

## Scope And Non-Goals

This change:

- adds a source-repository form of `gop status`;
- adds an explicit `context: source|outpost` first line;
- reports the checked-out branch, working-tree state, applicable source-branch
  upstream, effective local fetch/push URLs, and optional `outpost-container`;
- reports existing registered outposts and stale registrations from a source;
- preserves the current degraded managed-outpost diagnostics and locally
  cached ahead/behind comparisons;
- simplifies the skill and its README description around the one-call
  orientation interface.

This change does not:

- add JSON or another machine-oriented output mode;
- fetch, contact a remote, test authentication, or prove URL reachability;
- make `status` a readiness check for a mutating `gop` command;
- change `gop list`, whose ahead/behind calculation currently fetches;
- discover unregistered clone directories by scanning an
  `outpost-container`;
- require, infer, or automatically configure `outpost-container`;
- change the literal-`origin` requirements of `gop pull` or `gop push`.

## Ownership Model

The source registry is authoritative for the source-to-outpost relationship.
An entry means that the path is registered as an outpost of that source.
Outpost-local `outpost.managed`, `outpost.sourceRepo`, and
`outpost.remoteName` metadata provide the reverse relationship and the data
needed to operate on the checkout.

The source status report therefore has only these successful registry
classifications:

- an existing registration whose checkout and reverse metadata agree is an
  `outposts` row;
- a registered path proven not to exist is a `stale-registrations` row.

An existing registered path with missing, invalid, or contradictory checkout
state is not a legitimate third kind of outpost. It is an integrity error. A
successful row requires all of these facts:

- `outpost.managed` is `true`;
- canonical `outpost.sourceRepo` equals the reporting source;
- `outpost.remoteName` equals the registry entry's recorded remote name;
- that named Git remote exists and every effective fetch and push URL resolves
  to the reporting source repository.

For example, replacing a registered directory with an ordinary clone or with
an outpost copied from another source contradicts the source-owned
registration. Hand-editing its remote name or redirecting that remote away
from the source does too. Status fails and names the registered path instead
of printing "registered outpost is not managed by this source."

Status uses `fs::metadata` as a fallible existence check for both registered
paths and an outpost's configured source path. `NotFound` proves absence; any
other I/O failure is an error rather than evidence of absence.

## Core Interface

Context classification and report construction stay behind the existing
`ops::status` seam:

```rust
pub enum StatusReport {
    Source(SourceStatus),
    Outpost(OutpostStatus),
}

pub fn run(target_path: &Path) -> OutpostResult<StatusReport>;

pub fn run_with(
    target_path: &Path,
    env: &BTreeMap<OsString, OsString>,
) -> OutpostResult<StatusReport>;
```

The report uses refined types so renderers cannot combine incompatible
states. The exact private field layout may follow the implementation, but it
must preserve these shapes and invariants:

```rust
pub struct SourceStatus {
    pub source_path: PathBuf,
    pub head: SourceHead,
    pub source_dirty: bool,
    pub outpost_container: Option<PathBuf>,
    pub outposts: Vec<RegisteredOutpostStatus>,
    pub stale_registrations: Vec<StaleRegistration>,
}

pub enum SourceHead {
    Attached {
        branch: BranchName,
        upstream: Option<TrackedUpstream>,
    },
    Detached,
}

pub enum TrackedUpstream {
    Remote {
        remote: RemoteName,
        branch: BranchName,
        routes: RemoteRoutes,
    },
    LocalRepository {
        branch: BranchName,
    },
}

pub struct RemoteRoutes {
    pub fetch: RouteAvailability,
    pub push: RouteAvailability,
}

pub enum RouteAvailability {
    Known(RemoteUrlList),
    Unavailable,
}

pub struct RemoteUrlList(Vec<String>);

pub struct RegisteredOutpostStatus {
    pub display_id: OutpostIdPrefix,
    pub path: PathBuf,
    pub head: RegisteredOutpostHead,
    pub dirty: bool,
    pub locked: bool,
}

pub enum RegisteredOutpostHead {
    Attached(BranchName),
    Detached,
}

pub struct StaleRegistration {
    pub display_id: OutpostIdPrefix,
    pub path: PathBuf,
}
```

`OutpostStatus` is the current status report renamed and wrapped by
`StatusReport::Outpost`, with the following refinements:

```rust
pub enum SourceLocation {
    Unconfigured,
    Missing(PathBuf),
    Present(PathBuf),
}

pub enum OutpostHeadStatus {
    Attached {
        branch: BranchName,
        source_upstream: SourceUpstreamStatus,
    },
    Detached,
}

pub enum SourceUpstreamStatus {
    Configured(TrackedUpstream),
    Unset,
    Unavailable,
}
```

`SourceLocation` replaces the current optional source path plus independent
presence boolean so combinations such as no path plus `source_present: true`
cannot be constructed. `OutpostHeadStatus` makes source-upstream information
structurally inapplicable to detached `HEAD`. The current remote name, dirty
state, two ahead/behind values, and `ConfigProblem` collection retain their
meanings and output.

`RemoteUrlList` is a nonempty, first-seen de-duplicated sequence. A route
lookup that successfully establishes that a named remote or URL is absent is
`Unavailable`; process launch, termination, I/O, or invalid-data failures
remain errors. Git's valid `branch.<name>.remote=.` form constructs
`TrackedUpstream::LocalRepository` directly; it never invokes
`git remote get-url .`. The enum cannot combine a local-repository target with
remote URL routes.

The recorded outpost-to-source remote is a local relationship. Every effective
fetch and push URL for it must resolve, after the same local path handling used
by current outpost status, to the canonical source path. A network URL or a
local path resolving elsewhere is an integrity error. This route check is
separate from the source branch's upstream routes reported to the user.

The checked-out local branch and the branch inside an upstream target are
separate facts. A valid report can say:

```text
branch: release-prep
upstream: origin/main  git@github.com:acme/widget.git
```

In source context, `branch` is the checked-out source branch and `upstream` is
that branch's configured tracking target. In outpost context, `branch` is the
checked-out outpost branch and `source-upstream` is the same-named source
branch's configured tracking target. This follows the same-named branch model
used by `gop pull` and `gop push`; status does not reinterpret a manually
retargeted outpost branch as a different publication branch.

Known `RemoteRoutes` values come from `git remote get-url --all <remote>` and
`git remote get-url --push --all <remote>`. Fetch and push route states are
collapsed in text output only when the complete states and URL sequences are
equal; status does not infer repository identity from URL spelling.

The private route probe maps command success with at least one nonempty URL to
`Known`, and `GitFailed` with exit code 2 to `Unavailable`; Git uses code 2 for
a missing named remote or a remote without an effective URL. Every other
`GitFailed`, process termination, I/O error, or empty/malformed successful
output remains an error. This mapping is made by the probe helper, not by
matching human-readable stderr.

`Unavailable` is a reportable state for a source branch's upstream route. The
same result for the recorded outpost-to-source remote fails the ownership
predicate and is an integrity error.

## Context Classification And Data Flow

`run_with` performs these steps:

1. Discover and canonicalize the Git work tree for `target_path` once.
2. Read `RawMetadata` from that work tree once.
3. Select `StatusReport::Outpost` only when `outpost.managed` parses as
   `true`; select `StatusReport::Source` when the marker is absent or parses as
   `false`. An invalid marker is a metadata error, not a guessed context.
4. Build the selected report using only local Git configuration, work-tree
   state, registry/config files, and existing refs.

The CLI dispatch remains shallow: it calls `ops::status::run(&cwd)` once and
passes the returned enum to the output renderer. Context detection, registry
classification, route lookup, and diagnostics do not move into `main.rs`.

### Source Construction

The source builder:

1. reads the detached-or-attached `HEAD` and dirty state;
2. reads the attached branch's configured upstream, if any, and its effective
   fetch and push URL lists;
3. reads `outpost-container`, accepting an absent config file or absent key as
   the normal unset state;
4. loads the source registry, where an absent registry file is a valid empty
   registry;
5. rejects duplicate registered paths as a malformed registry because they
   would have the same derived identity;
6. derives display IDs and shortest unique prefixes across every registry
   entry, including stale entries;
7. partitions entries in registry order into existing outposts and stale
   registrations without calling `ops::list`;
8. for each existing entry, checks every ownership predicate listed above,
   then reads only its local branch and dirty state.

The builder never computes per-outpost ahead/behind state. That would either
reuse the fetching `list` implementation or duplicate a broader inventory
operation that source orientation does not need.

### Outpost Construction

The outpost builder keeps the current `RawMetadata`-based degraded path. It
continues to report a managed outpost even when its source path or remote name
is missing, when the configured source no longer exists, or when the outpost
is absent from the source registry.

When an outpost has an attached branch and a present source, status also reads
the same-named source branch's configured upstream and effective local routes.
Detached `HEAD` makes `source-upstream` not applicable. A missing source or
another prerequisite already named under `health: problems` makes it
unavailable. Missing source-branch tracking is rendered as unset. The current
ambiguous `NoUpstreamTracking` problem is refined into
`OutpostSourceTrackingUnavailable { branch }` and
`SourceUpstreamTrackingUnset { branch }`, so health output always names which
relationship is incomplete. Source-path existence uses the same fallible
`fs::metadata` rule as registered paths, so an access error is never reported
as `source-present: false`.

Outpost-to-source tracking is available only when the outpost branch has a
complete branch tracking target on the configured outpost remote. The source
upstream is set only when the same-named, existing source branch has a complete
branch tracking target. A missing key, a non-branch merge ref, or the wrong
outpost remote produces the corresponding relationship-specific problem.

If the same-named source branch itself is absent, `source-upstream` is
unavailable and `SourceBranchMissing { branch }` is added to the health
problems. The command does not read stale `branch.<name>.*` configuration as
though the source branch still existed.

Ahead/behind values continue to compare existing local refs only. A dash means
the comparison is unavailable from the current context, configuration, or
locally cached refs; it does not trigger a fetch.

## Text Contract

Output is human- and agent-oriented text. It is not a promised serialization
format. Paths use the CLI's existing display convention. Each row starts with
two spaces and uses tabs between fields, so spaces in ordinary paths do not
change the field roles.

### Source

```text
context: source
source: /work/widget
branch: main
source-state: clean
upstream: origin/main  git@github.com:acme/widget.git
outpost-container: <unset>
outposts:
  a18f2  /work/widget-outposts/feature-api  feature/api  clean
  c04bd  /work/widget-outposts/fix-docs  detached  dirty  locked
stale-registrations:
  b72e1  /work/widget-outposts/old-experiment
```

The displayed column spacing above represents tabs. Actual rows have these
exact positional forms:

```text
  <id>\t<path>\t<branch|detached>\t<clean|dirty>[\tlocked]
  <id>\t<path>
```

Empty sections are explicit:

```text
outposts: none
stale-registrations: none
```

An attached branch without a complete configured tracking target prints:

```text
upstream: <unset>
```

A detached source prints:

```text
branch: detached
upstream: <not-applicable>
```

`outpost-container: <unset>` is normal. It does not imply a health problem or
that an agent should configure it. Whether to configure it, and which safe
directory to use, remains the agent's task-specific choice.

When effective fetch and push routes are identical, each URL is printed once
with the collapsed label shown above. When they differ, the labels are split:

```text
upstream-fetch: origin/main  https://github.com/acme/widget.git
upstream-push: origin/main  git@github.com:acme/widget.git
```

Multiple effective URLs repeat the applicable line in stable order. A
configured tracking target whose named remote or effective URL is absent
retains the target and prints `<unavailable>` instead of inventing a URL. A
dot-repository route prints `<local-repository>`.

The dot-repository target itself uses Git's standard remote/branch notation:

```text
upstream: ./main  <local-repository>
```

When only one direction is available, the two directions stay separate. For
example:

```text
upstream-fetch: origin/main  https://github.com/acme/widget.git
upstream-push: origin/main  <unavailable>
```

Source output has no `health` section. A missing path is already named under
`stale-registrations`; contradictory registry state and unreadable or
malformed required local data are errors.

### Managed Outpost

```text
context: outpost
outpost: /work/widget-outposts/feature-api
source: /work/widget
source-present: true
remote: local
branch: feature/api
outpost-state: clean
source-upstream: origin/feature-api  git@github.com:acme/widget.git
outpost-vs-source: ahead 2, behind 0
source-vs-upstream: ahead 0, behind 1
health: ok
```

Differing routes use `source-upstream-fetch` and `source-upstream-push` in the
same manner as source output. Multiple URLs repeat the applicable line.

Detached `HEAD` prints:

```text
branch: detached
source-upstream: <not-applicable>
```

The degraded source and remote fields are exact:

```text
source: -
source-present: false
remote: -
source-upstream: <unavailable>
health: problems
  - missing source repo config
  - missing remote name config
```

`SourceLocation::Unconfigured` renders `source: -` and
`source-present: false`. `SourceLocation::Missing(path)` renders that path and
`false`; `SourceLocation::Present(path)` renders the path and `true`. A missing
remote name always renders `remote: -`. For an attached outpost,
`source-upstream` depends on the source location and same-named source branch,
not on the outpost's remote name, so it can still be configured when only
`remote: -` is degraded. Detached `HEAD` always uses `<not-applicable>`.

When the configured source is missing, the existing degraded information
remains available:

```text
context: outpost
outpost: /work/widget-outposts/copied
source: /work/widget
source-present: false
remote: local
branch: feature/api
outpost-state: clean
source-upstream: <unavailable>
outpost-vs-source: -
source-vs-upstream: -
health: problems
  - source missing: /work/widget
```

All current `ConfigProblem` categories remain, with the upstream-tracking
category split by relationship as described above. If a configured source
upstream target exists but its named remote or effective URLs are absent, the
target is shown with `<unavailable>` and
`SourceUpstreamRouteUnavailable { remote }` is added; status does not fail or
claim a route.

The new health text is stable:

```text
  - source branch missing: <branch>
  - outpost-to-source tracking unavailable for <branch>
  - source upstream tracking unset for <branch>
  - source upstream route unavailable: <remote>
```

Health problems render in this stable order: missing source path metadata,
missing remote-name metadata, missing source path, local/source remote
mismatch, missing source registration, missing same-named source branch, no
outpost-to-source tracking, unset source-to-upstream tracking, unavailable
source-upstream route, and checked-out-source push failure. Problems that
cannot apply are skipped.

## Error Behavior

A successful source or outpost report exits zero, including a degraded
managed-outpost report and a source report with stale registrations.

The command fails without a partial report when:

- no Git work tree can be discovered;
- `outpost.managed` is syntactically invalid;
- a source registry or source config file is unreadable, malformed, or has an
  unsupported version;
- a source registry contains duplicate registered paths;
- a configured `outpost-container` is invalid under the existing config
  contract;
- an existing registered path fails its reverse-link integrity check;
- required local Git state for the report or an outpost row cannot be read or
  parsed;
- a Git process cannot be launched, terminates abnormally, or returns invalid
  route data. A normal nonzero result proving an absent named remote or URL is
  the successful `<unavailable>` route state described above.

The integrity error names both the source and registered path, and says that
the registration and checkout are inconsistent. It must not describe the
path as a normal "registered outpost not managed by this source" state.

Missing upstream tracking, detached `HEAD`, an unset `outpost-container`, and
a missing registered path are expected report states rather than command
errors.

## Skill And Documentation Changes

The normal orientation section in `skills/using-git-outpost/SKILL.md` becomes
one call:

```bash
gop --no-color -C <path> status
```

When the relevant checkout is already the current directory, the skill may
omit `-C <path>`. Exit zero plus the first line maps directly to
`SourceContext(report)` or `ManagedOutpostContext(report)`. Any nonzero result
is preserved as `Unknown(error)`; the skill does not reinterpret a failed
status as source context.

If `gop` is unavailable, a short local-marker fallback may still distinguish
an explicitly managed outpost, but it cannot provide source orientation or
authorize `gop` workflows. The skill reports that limitation rather than
recreating status with a sequence of Git and filesystem probes.

The skill keeps only the status caveats that affect decisions: the report is
local, it may contain degraded health or stale registrations, and it does not
establish mutation readiness. The workflow reference retains command-specific
safety and postcondition guidance, but removes orientation logic and any need
to run `gop list` merely to infer layout or `outpost-container`.

The README installation command remains the Vercel Skills CLI workflow. Its
description is updated to say that the installed skill uses one context-aware
`gop status` call, reports optional `outpost-container`, and leaves the choice
of whether and how to configure that value to the agent.

The product and architecture documentation and CLI help are updated from
"current managed outpost" to the source-or-outpost orientation contract.

## Verification Strategy

Core tests cover:

- source status from the repository root and a nested directory;
- source attached and detached `HEAD`, including local and upstream branch
  names that differ;
- source clean/dirty state, where staged, unstaged, and ordinary untracked
  files are dirty and ignored files remain excluded by the existing porcelain
  query;
- unset and configured `outpost-container`, plus malformed config failure;
- absent registry as empty, existing clean/dirty/detached/locked rows, explicit
  empty sections, and stale registrations;
- display ID uniqueness across both existing and stale registrations;
- integrity errors created by removing reverse metadata or replacing a
  registered directory with another checkout, changing its recorded remote
  name, or redirecting that remote away from the source;
- duplicate registered paths and a non-`NotFound` existence error;
- identical fetch/push routes collapsed, differing routes split, multiple URLs
  retained in order, and unavailable configured routes not invented;
- dot-repository tracking without a remote URL probe;
- missing named remotes and remotes without URLs as exit-2 unavailable probes,
  plus injected non-2 Git failure and malformed successful output as errors;
- preservation of every current managed-outpost status diagnostic and
  ahead/behind behavior;
- missing-source and detached-outpost degraded reports;
- a present source with the same-named source branch removed;
- absent, false, true, and invalid `outpost.managed` classification;
- unchanged refs and an invocation log containing no `fetch`, `pull`, `push`,
  `ls-remote`, `update-ref`, or other mutation.

CLI tests cover:

- exact source and outpost text output, including the first-line context,
  tab-separated rows, empty sections, and route forms;
- exact degraded field output for unconfigured/missing sources and missing
  remote names;
- identical stdout for `gop`, `git-outpost`, and `git outpost` in both
  contexts;
- equivalence between direct invocation and global `-C`;
- continued rejection of `--json`;
- updated help and shell-delegation expectations.

Skill verification uses pressure scenarios before and after the edit. At
minimum they exercise source orientation, healthy and degraded outposts,
unset `outpost-container`, and a worktree-style request where the agent must
choose whether a new outpost and container configuration are appropriate.
The repository skill is validated, installed globally through the Vercel
Skills CLI, and re-read from the installed location to prove discovery and
content.

Final repository verification runs formatting, the full Cargo test suite,
Clippy with warnings denied, documentation checks, skill validation, and
`git diff --check` before the existing draft pull request is updated.
