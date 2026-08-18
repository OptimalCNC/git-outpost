# Git Outpost State Storage and Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move private source and outpost state to each repository's exact Git directory while preserving caller behavior through typed stores and a removable legacy migration Adapter.

**Architecture:** `SourceStateStore` and `OutpostStateStore` are the external seams. Git-directory stores own parsing, validation, path construction, and atomic writes; migrating stores wrap them, parse legacy `.outpost` files or local `outpost.*` Git configuration only when the new document is absent, and remove only known legacy artifacts after current state is verified. `SourceRepo`, `Outpost`, status, and lifecycle operations consume the role-specific stores, so removing migration later changes only composition wiring and migration modules.

**Tech Stack:** Rust 2024, serde/serde_json, chrono, tempfile, existing `GitInvoker`, cargo test, Markdown architecture/product/skill documentation.

**Spec:** `docs/superpowers/specs/2026-08-17-gop-state-storage-and-migration-design.md`

## Global Constraints

- Store private state only at `<exact-git-dir>/outpost/{config,registry,metadata}.json`; never use `git-common-dir` as an authority.
- Keep source config and registry as independent typed documents and preserve their existing schema behavior (strict config; registry unknown fields accepted and dropped).
- New outpost metadata is strict versioned JSON with no `managed` field; absence means unmanaged.
- Legacy parsing is local-only for Git config and never runs after a present invalid new document. After verified migration, Adapters delete only the matching legacy JSON file or the three known local Git keys; cleanup is retried from valid current state.
- `gop status` may write equivalent new gop-owned state and remove its known legacy input during migration; its report and context classification remain unchanged for valid legacy state.
- Preserve existing dirty files and unrelated historical docs; stage only task-specific files when committing.
- Every production change is preceded by a focused failing test and followed by the smallest passing implementation.

---

### Task 1: Establish the typed state seams and exact repository location

**Files:**
- Create: `crates/core/src/state.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/source_repo.rs`
- Modify: `crates/core/src/config.rs`
- Modify: `crates/core/src/registry.rs`
- Modify: `crates/core/src/metadata.rs`
- Test: `crates/core/src/state.rs` and focused existing unit tests

**Interfaces:**
- Produces `Stored<T>`, `SourceStateStore`, `OutpostStateStore`, `MetadataState`, and `MetadataProblems` with domain types rather than raw keys or JSON values.
- Produces `RepositoryLocation { work_tree, git_dir }` and a single exact-Git-directory path helper used by both roles.
- Keeps existing public convenience names (`ConfigStore`, `Registry`, `RegistryMut`, `Metadata`) as callers' façade.

- [ ] **Step 1: Write failing seam and path tests.** Add tests that construct an ordinary repository and a linked worktree, assert `git_dir/outpost/config.json`, `registry.json`, and `metadata.json`, and assert `Stored::Absent` is distinguishable from `Stored::Present`. Add a compile/use test for the two traits through a small in-memory test Adapter.

- [ ] **Step 2: Run the focused tests and verify the intended failure.**

  Run: `cargo test -p outpost-core state -- --nocapture`

  Expected: compile/test failure because the new types and exact-directory paths do not exist.

- [ ] **Step 3: Add the domain seam.** Define:

  ```rust
  pub enum Stored<T> { Absent, Present(T) }

  pub trait SourceStateStore {
      fn read_config(&self) -> OutpostResult<Stored<SourceConfig>>;
      fn write_config(&self, config: &SourceConfig) -> OutpostResult<()>;
      fn read_registry(&self) -> OutpostResult<Stored<Registry>>;
      fn write_registry(&self, registry: &Registry) -> OutpostResult<()>;
  }

  pub trait OutpostStateStore {
      fn read_metadata(&self) -> OutpostResult<MetadataState>;
      fn initialize_metadata(&self, metadata: &Metadata) -> OutpostResult<()>;
  }
  ```

  Add `MetadataState::{Absent, Valid(Metadata), Invalid(MetadataProblems)}` and a structured `MetadataProblems` carrying the repository path, reason, and optional legacy raw fields for diagnostic status. Make `SourceConfig` and its domain accessors visible to the store implementation without exposing JSON structs.

- [ ] **Step 4: Add `RepositoryLocation` and path construction.** Have `SourceRepo::at_with` and `Outpost::at_with` retain canonical worktree and exact `rev-parse --git-dir` values. Implement `state_dir()` as `git_dir.join("outpost")`; expose current paths through `SourceRepo::config_path` and `registry_path`, and an outpost metadata path helper. Do not derive paths from the worktree or common directory.

- [ ] **Step 5: Run the focused tests and refactor-only checks.**

  Run: `cargo test -p outpost-core state -- --nocapture`

  Expected: PASS, with no production caller yet changing observable storage behavior beyond path helpers.

- [ ] **Step 6: Commit the seam as an isolated change.**

  ```bash
  git add crates/core/src/state.rs crates/core/src/lib.rs crates/core/src/source_repo.rs crates/core/src/config.rs crates/core/src/registry.rs crates/core/src/metadata.rs
  git commit -m "refactor: add typed gop state seams"
  ```

### Task 2: Implement Git-directory source stores and source migration

**Files:**
- Modify: `crates/core/src/config.rs`
- Modify: `crates/core/src/registry.rs`
- Modify: `crates/core/src/source_repo.rs`
- Create: `crates/core/src/source_state.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/tests/config.rs`, `crates/core/src/registry.rs`, and new `crates/core/tests/state_migration.rs`

**Interfaces:**
- Produces `GitDirSourceStore` and `MigratingSourceStore`, both satisfying `SourceStateStore`.
- `SourceRepo::config`, `registry`, and `registry_mut` use the migrating composition; callers still receive the existing `ConfigStore`/`Registry` façade.
- Legacy source reads use only `<worktree>/.outpost/config.json` and `<worktree>/.outpost/registry.json`; current writes use only exact Git-directory paths.

- [ ] **Step 1: Add failing source storage tests.** Cover: new path placement; absent versus present documents; valid legacy config migration; valid legacy registry migration; new state winning when both exist; malformed new state not falling back; malformed legacy state not replaced; independent config/registry retries; migrated files removed; cleanup of files left by an earlier migration; unrelated `.outpost/` files preserved; refusal to clean through a symlinked legacy directory; registry stale fields still accepted and dropped; linked worktrees receiving independent source files.

- [ ] **Step 2: Run the new tests to verify red.**

  Run: `cargo test -p outpost-core --test state_migration -- --nocapture`

  Expected: failures on old `.outpost` paths and missing migration behavior.

- [ ] **Step 3: Extract reusable document readers/writers.** In `config.rs` and `registry.rs`, split parsing and atomic persistence into path-parameterized functions. Preserve `ConfigFile`'s `deny_unknown_fields`; leave `RegistryFile` permissive for unknown fields. Make `Registry` carry the current storage path and keep `RegistryMut`'s source reference for Adapter-backed saves. Remove path construction from parsers.

- [ ] **Step 4: Implement the Git-directory store.** Create the `outpost` directory below the exact Git directory, write each document through `tempfile::NamedTempFile` and `persist`, and return `Stored::Absent` only for `NotFound`. Keep config and registry writes independent. Preserve the existing local-exclude compatibility behavior only where existing direct registry/config operations require it; never create or rewrite the legacy JSON files.

- [ ] **Step 5: Implement the migration Adapter.** On each document read, first call the Git-directory reader. Return invalid new state immediately without consulting or cleaning legacy storage. For valid current state, remove the matching legacy file if it remains, then return current state. Only on absence parse the matching legacy file, write the typed value to the new store, re-read it, compare domain values, remove that file, and return the verified value. A cleanup error leaves current state intact and is retried on the next read. A failure in one document must not prevent retrying the other document.

- [ ] **Step 6: Wire `SourceRepo` and convenience façades.** Add one composition method that constructs `MigratingSourceStore`. Make `ConfigStore::load/save`, `Registry::load`, and `RegistryMut::save` cross that seam. Update `storage_path()` and error paths to the new files while retaining domain-level empty-state behavior.

- [ ] **Step 7: Run focused and existing source tests.**

  Run: `cargo test -p outpost-core --test state_migration -- --nocapture && cargo test -p outpost-core --test config -- --nocapture && cargo test -p outpost-core --lib registry -- --nocapture`

  Expected: new migration tests and all config/registry unit tests pass; only successfully migrated legacy JSON files are deleted.

- [ ] **Step 8: Commit source storage and migration.**

  ```bash
  git add crates/core/src/source_state.rs crates/core/src/config.rs crates/core/src/registry.rs crates/core/src/source_repo.rs crates/core/src/lib.rs crates/core/tests/config.rs crates/core/tests/state_migration.rs
  git commit -m "feat: migrate source state into git directories"
  ```

### Task 3: Implement strict Git-directory outpost metadata and migration

**Files:**
- Modify: `crates/core/src/metadata.rs`
- Modify: `crates/core/src/outpost.rs`
- Modify: `crates/core/src/error.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/src/metadata.rs`, `crates/core/src/outpost.rs`, and `crates/core/tests/state_migration.rs`

**Interfaces:**
- Produces `GitDirOutpostStore` and `MigratingOutpostStore` satisfying `OutpostStateStore`.
- `Outpost::at_with` consumes `MetadataState`: absent maps to `NotAnOutpost`, valid constructs an `Outpost`, invalid maps to `BadMetadata` without consulting legacy state.
- `Metadata::write` remains a compatibility façade but writes the new metadata document, never `outpost.*` Git config.

- [ ] **Step 1: Add failing metadata tests.** Cover strict versioned JSON, unknown/missing fields, invalid remote names, absolute recorded source paths whose directories are missing, atomic initialization without overwrite, local-only legacy reads, false/absent markers as unmanaged, valid legacy conversion, invalid legacy conversion, new-state precedence, and equivalent concurrent migration behavior.

- [ ] **Step 2: Run the metadata tests and verify red.**

  Run: `cargo test -p outpost-core metadata outpost -- --nocapture`

  Expected: failures because metadata is still stored in local Git config.

- [ ] **Step 3: Define and parse the new metadata document.** Add a strict serde document with `version`, `source_repo`, and `remote_name`; validate version, absolute source path, and `RemoteName`; canonicalize source paths on write but do not require recorded sources to exist on read. Represent malformed documents as `MetadataProblems`.

- [ ] **Step 4: Implement atomic current writes and existing-file protection.** `initialize_metadata` creates parent directories, writes a temporary JSON file, and refuses to overwrite an existing metadata document. During migration, if creation loses a race, re-read and accept an equivalent document or return a conflict error for a different value.

- [ ] **Step 5: Implement the legacy migration Adapter.** Read exactly `outpost.managed`, `outpost.sourceRepo`, and `outpost.remoteName` with `git config --local --get`. Ignore global/system values. Convert a true complete marker to typed metadata and migrate it; after verifying current metadata, unset all values for exactly those three local keys. Convert true incomplete/malformed values to invalid state without cleanup; treat false/absent markers as absent. Retry interrupted cleanup when valid current metadata already exists, and preserve unrelated local Git keys.

- [ ] **Step 6: Wire `Outpost` construction and public exports.** Replace `RawMetadata` as the normal construction path with the migrating store, retain a private/compatibility raw representation only for diagnostics and existing tests, and export the typed metadata/state interfaces needed by callers.

- [ ] **Step 7: Run focused tests and commit.**

  Run: `cargo test -p outpost-core metadata outpost -- --nocapture`

  ```bash
  git add crates/core/src/metadata.rs crates/core/src/outpost.rs crates/core/src/error.rs crates/core/src/lib.rs crates/core/tests/state_migration.rs
  git commit -m "feat: store outpost metadata in git directories"
  ```

### Task 4: Route status and `add` through the stores

**Files:**
- Modify: `crates/core/src/ops/status.rs`
- Modify: `crates/core/src/ops/status/source.rs`
- Modify: `crates/core/src/ops/add.rs`
- Modify: `crates/core/src/selector.rs` only if required by typed registry access
- Modify: `crates/cli/src/output.rs`
- Test: `crates/core/tests/status.rs`, `crates/core/tests/add.rs`, `crates/cli/tests/e2e.rs`, and new migration/status tests

**Interfaces:**
- Status classification reads one migrating outpost store and one migrating source store; it never reads raw paths or raw Git-config keys directly.
- `gop add` initializes outpost metadata through `OutpostStateStore` and writes the source registry through `SourceStateStore`, preserving the source-authoritative order.
- Diagnostic status retains branch/dirtiness facts for invalid metadata and adds a typed `InvalidMetadata` health problem without reclassifying the repository as a source.

- [ ] **Step 1: Add failing status/add tests.** Assert first-run legacy status output equals the current report, new files appear under exact Git dirs, corresponding legacy files/keys are removed, second status does not rewrite them, invalid new metadata remains outpost context, source status migrates only inspected rows, `add` writes no `outpost.*` keys, and source registry writes last after metadata initialization.

- [ ] **Step 2: Run focused tests to verify red.**

  Run: `cargo test -p outpost-core --test status --test add && cargo test -p outpost-cli --test e2e status`

  Expected: failures from old raw metadata/path reads and old add assertions.

- [ ] **Step 3: Refactor outpost status classification.** Discover exact Git dir once, read `MetadataState`, route absent to source status, valid to the existing healthy-report calculations, legacy incomplete state to the existing missing-field diagnostics, and new invalid state to a degraded outpost report. Do not call a legacy reader after invalid current metadata.

- [ ] **Step 4: Refactor source status.** Replace hardcoded `.outpost` parsers with `SourceRepo::config()` and `SourceRepo::registry()`. Use `Outpost::at_with`/typed metadata for each live registered row and map any invalid or mismatching reverse link to `RegisteredOutpostIntegrity`. Preserve duplicate-path checks, IDs, stale rows, and route checks.

- [ ] **Step 5: Refactor `add`.** Build typed metadata from canonical source and selected remote, initialize the destination state file through the store, set standard `receive.denyCurrentBranch` configuration, then add/save the source registry last. Re-open through the normal migrating `Outpost` path and verify the relationship.

- [ ] **Step 6: Add CLI formatting for the new diagnostic problem without changing healthy/legacy output.** Keep status stdout unchanged for valid legacy migration and make the new error text deterministic.

- [ ] **Step 7: Run core/CLI focused tests and commit.**

  Run: `cargo test -p outpost-core --test status --test add && cargo test -p outpost-cli --test e2e -- --nocapture`

  ```bash
  git add crates/core/src/ops/status.rs crates/core/src/ops/status/source.rs crates/core/src/ops/add.rs crates/cli/src/output.rs crates/core/tests/status.rs crates/core/tests/add.rs crates/cli/tests/e2e.rs
  git commit -m "refactor: route status and add through state stores"
  ```

### Task 5: Update lifecycle tests and storage documentation

**Files:**
- Modify: `crates/core/tests/config.rs`
- Modify: `crates/core/tests/add.rs`
- Modify: `crates/core/tests/outpost_id.rs`
- Modify: affected lifecycle tests whose fixtures inspect `.outpost` paths
- Modify: `docs/src/architecture.md`
- Modify: `docs/src/product.md`
- Modify: `skills/using-git-outpost/SKILL.md`

**Interfaces:**
- Documentation names exact Git-directory ownership, independent linked-worktree state, legacy migration precedence, and the one-time status write exemption.
- Tests inspect state through public/domain stores where possible and assert successful migration removes only the known legacy files/keys while preserving invalid or unrelated state.

- [ ] **Step 1: Update failing path assertions.** Change fixture assertions from `<worktree>/.outpost/*` to `<git-dir>/outpost/*`, replace metadata Git-config assertions with JSON/domain assertions, and retain a regression that standard Git config (`remote.*`, branch tracking, `receive.denyCurrentBranch`) remains ordinary Git state.

- [ ] **Step 2: Run the affected tests and verify only expectation failures remain.**

  Run: `cargo test --workspace`

  Expected: all behavior tests pass except assertions intentionally tied to the old storage locations.

- [ ] **Step 3: Rewrite architecture storage/lifecycle sections top-down.** Update module tree, path/API tables, status/add sequencing, metadata schema, migration composition, and invariants. Remove claims that private state is authoritative in `.outpost` worktree files or `outpost.*` config, while documenting legacy readers as temporary.

- [ ] **Step 4: Rewrite product behavior and skill guidance.** Document new paths, survival under `git clean -fdx`, non-listing by ignored-file commands, exact per-worktree ownership, and that the first successful `gop status` may write equivalent local state without changing its report.

- [ ] **Step 5: Run documentation checks and commit.**

  Run: `rg -n "<worktree>/.outpost|outpost\\.(managed|sourceRepo|remoteName)" docs/src skills/using-git-outpost/SKILL.md` (expected only explicitly labeled legacy/migration references), then `git diff --check`.

  ```bash
  git add crates/core/tests docs/src/architecture.md docs/src/product.md skills/using-git-outpost/SKILL.md
  git commit -m "docs: describe git-dir state ownership and migration"
  ```

### Task 6: Full verification and migration invariants

**Files:**
- Modify only files already covered by Tasks 1–5 if verification exposes a concrete defect.
- Test: all workspace tests plus explicit real-Git migration probes.

- [ ] **Step 1: Run formatting and compile checks.**

  Run: `cargo fmt --all -- --check` and `cargo check --workspace`.

- [ ] **Step 2: Run the complete test suite.**

  Run: `cargo test --workspace`

  Expected: every unit, integration, CLI, and doctest target passes.

- [ ] **Step 3: Verify real storage and migration invariants.** Use a temporary ordinary repository and linked worktree to verify exact private paths, independent files, legacy precedence, invalid-new no-fallback, removal of only verified legacy artifacts, cleanup retry from current state, no `outpost.*` writes from `add`, and `git clean -fdx` survival. Capture `gop --no-color -C <path> status` before/after and compare stdout/context.

- [ ] **Step 4: Inspect scope and worktree boundaries.** Run `git status --short`, `git diff --stat HEAD~N`, and `git diff --check`; confirm `skills/using-git-outpost/agents/openai.yaml` and the six pre-existing historical docs are unchanged and unstaged.

- [ ] **Step 5: Report only claims supported by fresh command output.** Include test counts, exact migration behavior, committed spec/implementation changes, and any intentionally retained compatibility behavior.
