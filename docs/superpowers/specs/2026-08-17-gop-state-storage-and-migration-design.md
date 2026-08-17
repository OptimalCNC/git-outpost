# Git Outpost State Storage and Migration Design

## Purpose

Move Git Outpost's private source and outpost state from its current storage
locations into each repository's exact, per-worktree Git directory. Put a
role-specific read/write Interface in front of that storage, and isolate
legacy-format migration in removable Adapters. A caller should continue to
ask for source or outpost information in domain terms without knowing whether
the information came from the new files or from a legacy representation.

## Scope and non-goals

This design covers:

- source-owned configuration and registration state;
- outpost reverse-link metadata;
- the storage-neutral Interfaces used to read and write those documents;
- one-time migration from the released layout and Git configuration;
- the observable behavior of migration during `gop status` and other commands;
- atomicity, precedence, errors, tests, and documentation.

This design does not:

- preserve readability of the new format by older `gop` binaries;
- move standard Git configuration such as remotes, branch tracking, or
  `receive.denyCurrentBranch`;
- use the Git common directory as a shared state authority;
- add a global state database;
- automatically delete legacy files or Git configuration;
- make migration a long-term fallback path after the migration period.

The source registry remains authoritative for source-to-outpost registration.
Outpost metadata remains a reverse link and an integrity check. A contradictory
reverse link is inconsistency or corruption, not a second ownership category.

## Storage decision

All private gop state is rooted at the repository's exact Git directory:

```text
<exact-git-dir>/outpost/config.json       # source configuration
<exact-git-dir>/outpost/registry.json     # source registration
<exact-git-dir>/outpost/metadata.json     # outpost reverse link
```

The files are role-specific; a repository need not contain all three.

The exact Git directory is resolved from Git, not inferred by appending
`.git` to the worktree and not obtained from `git-common-dir`:

- an ordinary repository uses `<repository>/.git`;
- a linked worktree uses its private directory below
  `<common-git-dir>/worktrees/<name>`;
- a repository using a separate Git directory follows the path Git reports.

The path resolver returns a proof-bearing `RepositoryLocation` containing the
canonical worktree and exact Git directory. Storage Modules receive that value
and compute their private paths internally. No caller constructs a JSON path.

Each exact Git directory owns an independent copy of source state. In
particular, linked source worktrees do not share `config.json` or
`registry.json` through their common Git directory; this is intentional
per-worktree ownership.

Because the files are under Git administrative storage, they are not shown by
Git's ignored-file listing and do not need an ignore rule. They survive
`git clean -fdx`. Removing the repository or its administrative directory can
still remove them, as expected for repository-local state.

## On-disk schemas

### Outpost metadata

`metadata.json` is strict, versioned JSON:

```json
{
  "version": 1,
  "source_repo": "/canonical/path/to/source",
  "remote_name": "local"
}
```

There is no `managed` boolean. Absence of the document means that the
repository is not a managed outpost. A present document with invalid JSON,
an unsupported version, an unknown field, a missing field, an invalid recorded
source path, or an invalid remote name is damaged metadata.

`source_repo` is canonical and absolute when written. A valid recorded path is
still valid metadata if the source directory is later moved or deleted;
current source existence is a separate health fact.

### Source configuration and registry

The existing versioned schemas remain the source of truth, with only their
locations changing:

```json
// config.json
{
  "version": 1,
  "outpost_container": "/absolute/path/to/container"
}
```

```json
// registry.json
{
  "version": 1,
  "outposts": [
    {
      "path": "/canonical/path/to/outpost",
      "created_at": "...",
      "remote_name": "local",
      "locked": false,
      "lock_reason": null,
      "locked_at": null
    }
  ]
}
```

The source configuration keeps its current strict decoding: unknown fields and
unsupported versions are errors. The registry keeps its current decoding
compatibility: unknown fields are accepted and discarded on load/save, while
unsupported versions remain errors. Moving storage must not silently broaden
or narrow either schema's existing behavior. A missing source config or
registry is a valid empty source state; a present malformed document is an
error. The distinction between absent and present is retained internally so
the migration Adapter can detect legacy state.

## Role-specific state Interfaces

“Typed state Interface” means the complete domain-facing read/write surface for
one repository role. It is not one aggregate `State` object and it is not a
string-key configuration map.

### Source state Interface

The source has two independent documents with different lifecycles:

```rust
trait SourceStateStore {
    fn read_config(&self) -> OutpostResult<Stored<SourceConfig>>;
    fn write_config(&self, config: &SourceConfig) -> OutpostResult<()>;

    fn read_registry(&self) -> OutpostResult<Stored<Registry>>;
    fn write_registry(&self, registry: &Registry) -> OutpostResult<()>;
}
```

`Stored<T>` is an explicit document-presence type:

```rust
enum Stored<T> {
    Absent,
    Present(T),
}
```

The source-facing `ConfigStore` and `Registry`/`RegistryMut` operations remain
domain conveniences over this Interface. They may interpret `Absent` as an
empty config or registry after migration has had an opportunity to run, but
the storage Interface must not erase that distinction.

Config values, registry entries, remote names, paths, timestamps, and lock
state are typed values. Callers do not pass raw JSON or arbitrary key strings.
Configuration and registry saves are separate atomic document writes; there
is no false promise of a transaction spanning both files.

### Outpost state Interface

An outpost has one private reverse-link document:

```rust
trait OutpostStateStore {
    fn read_metadata(&self) -> OutpostResult<MetadataState>;
    fn initialize_metadata(&self, metadata: &OutpostMetadata)
        -> OutpostResult<()>;
}
```

The read result preserves ownership and corruption distinctions:

```rust
enum MetadataState {
    Absent,
    Valid(OutpostMetadata),
    Invalid(MetadataProblems),
}
```

`OutpostMetadata` contains a validated `RecordedSourcePath` and a parsed
`RemoteName`; it cannot contain partially missing fields. `MetadataProblems`
contains structured reasons without exposing the JSON representation.

`initialize_metadata` creates a new document atomically and does not silently
overwrite an existing one. There is no general metadata update operation in
this design because no normal command edits the reverse link. A future update
operation must be explicit if one becomes necessary.

### Standard Git configuration

Remote URLs, branch upstreams, and `receive.denyCurrentBranch` remain ordinary
local Git configuration accessed through the existing Git operations. They are
not folded into either gop-owned state Interface.

## Implementations and Adapters

The normal implementations are concrete Git-directory stores:

```text
GitDirSourceStore   -> config.json and registry.json
GitDirOutpostStore  -> metadata.json
```

Their implementations own path construction, strict JSON decoding, schema
versions, canonicalization, and atomic persistence. Normal callers depend on
the role-specific Interfaces, not on these implementation names.

There is no public generic filesystem backend solely for hypothetical future
storage. The seam is real because the temporary migration Adapters are a
second implementation, and because the Interface is the test surface.

## Migration Adapter

Migration is isolated in a separate module and is composed around the normal
Interfaces:

```text
Source callers
    -> MigratingSourceStore
         -> GitDirSourceStore       (current state)
         -> LegacySourceReader      (old .outpost files)

Outpost callers
    -> MigratingOutpostStore
         -> GitDirOutpostStore      (current state)
         -> LegacyOutpostReader     (old local outpost.* config)
```

The legacy readers are read-only. They produce the same validated domain
values consumed by the current writers; they do not expose legacy keys or file
formats to normal callers.

### Read precedence

Each migrating Adapter follows this order:

1. Read the new document.
2. If it is present and valid, return it.
3. If it is present but invalid, return the invalid state/error and do not
   consult legacy storage.
4. Only if it is absent, read the corresponding legacy representation.
5. If legacy state is absent, return `Absent`.
6. If legacy state is valid, write it through the current store, re-read the
   new document, verify equivalence, and return the new value.
7. If legacy state is damaged, report the damage; do not create a partial new
   document.

The new document is authoritative as soon as it exists. Legacy state is left
untouched, so migration does not perform destructive cleanup and can be
retried safely. A later explicit cleanup can remove legacy artifacts after
the migration period.

Source config and registry migrate independently. If one succeeds and the
other fails, the next invocation retries only the missing or failed document;
there is no cross-document transaction.

### Outpost legacy conversion

The legacy outpost reader reads only local `outpost.managed`,
`outpost.sourceRepo`, and `outpost.remoteName` values. Global and system Git
configuration must not classify a repository.

`outpost.managed=true` with both required fields valid converts to
`metadata.json`. A true marker with missing or invalid fields remains an
invalid outpost state for diagnostic status and is not persisted as a partial
new document. An absent or false marker means there is no legacy outpost
state.

### Source legacy conversion

The legacy source reader examines `<worktree>/.outpost/config.json` and
`<worktree>/.outpost/registry.json` independently. Valid documents are
converted to the new exact-Git-directory paths. Missing legacy documents are
normal empty state. Invalid legacy documents remain configuration or registry
errors rather than being silently replaced with empty state.

### Removal

The composition root is the only long-lived coupling to migration:

```text
current period:  callers -> Migrating*Store -> GitDir*Store
after removal:  callers -> GitDir*Store
```

Removing migration later therefore consists of deleting the migration module,
its legacy readers and tests, and changing this one composition choice. The
role-specific Interfaces and all normal callers remain unchanged.

## Status and observable behavior

`gop status` uses the same migrating Stores as every other command. It has a
one-time local state-format migration exemption from its otherwise read-only
contract.

On a successful first invocation against legacy state:

- status output and context classification are unchanged;
- no progress or migration message is added to stdout;
- no worktree files, index, refs, branches, remotes, standard Git config, or
  network state changes;
- only equivalent new gop-owned files may be created under exact Git dirs.

On later invocations, no migration write occurs. If a source status examines
registered outposts, each outpost's state is read through its migrating Store;
therefore the first source status may migrate the source documents and any
legacy outpost documents it actually inspects. This is still limited to
equivalent gop-owned state and does not alter topology.

Strict construction maps `MetadataState::Absent` to `NotAnOutpost` and
`MetadataState::Invalid` to `BadMetadata`. Diagnostic status does not
reclassify a present-but-invalid metadata file as a source; it reports an
outpost with a metadata health problem and retains link-independent facts such
as branch and dirtiness. A valid link whose source directory is missing
remains valid metadata and reports `SourceMissing`.

For a source registry, the registry remains authoritative. A live registered
path with absent, invalid, mismatching, or redirected reverse metadata is a
`RegisteredOutpostIntegrity` failure, not a normal outpost row.

Migration failures are explicit. A failed atomic write leaves the legacy
document intact; a partial source migration is retried per document. A
concurrent equivalent migration may observe an already-created equivalent
new document and succeed; a conflicting new value is an error.

## `gop add` ordering

`add` uses the new Interfaces and preserves source-authoritative ordering:

1. Resolve source configuration through `SourceStateStore` (including any
   needed migration).
2. Clone the destination, rename the source remote if requested, and apply
   the checkout.
3. Build validated `OutpostMetadata` from the canonical source path and
   selected remote name.
4. Initialize the destination's `metadata.json` through
   `OutpostStateStore`.
5. Set source-local `receive.denyCurrentBranch=updateInstead` through normal
   Git configuration.
6. Add and atomically save the source registry entry last.
7. Re-open the destination through the migrating/normal read Interface and
   verify the reverse link and registry relationship.

There is no cross-repository transaction. A failure after step 4 and before
step 6 may leave a valid but unregistered outpost; it must never leave a
source registry claim for a missing or invalid destination. Existing cleanup
and recovery rules remain responsible for that partial state.

## Testing and verification

Tests cross the role-specific Interfaces rather than private JSON helpers.

Storage and adapter tests cover:

- absent, valid, malformed, unknown-field, missing-field, and unsupported-
  version documents;
- canonical and missing recorded source paths;
- strict local-only legacy Git configuration reads;
- precedence when both legacy and new documents exist;
- invalid new state not falling back to legacy;
- atomic writes, existing-file protection, and interrupted writes;
- independent source config and registry migration;
- equivalent and conflicting concurrent migration attempts.

Real Git fixtures cover:

- ordinary repositories and linked worktrees;
- exact per-worktree Git-directory placement, never common-directory placement;
- `gop status` first-run output equivalence and second-run no-write behavior;
- source status migrating registered outposts it inspects;
- strict outpost construction, degraded invalid-metadata status, and source
  registry integrity failures;
- `gop add` partial-order recovery and final source/outpost agreement;
- survival under `git clean -fdx`.

## Documentation changes

The implementation must update the storage table and lifecycle descriptions in
the architecture and product documentation, remove claims that private state
lives in `<worktree>/.outpost` or `outpost.*` Git configuration, and document
the one-time status migration exemption. The `using-git-outpost` skill must
retain its strict context-classification rules while stating that the first
successful invocation may perform local gop-state migration.
