# `gop status` Orientation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `gop status` provide one local orientation report from either a source repository or a managed outpost, then simplify and install the agent skill around that interface.

**Architecture:** Keep one public Core seam, `ops::status::run`, returning `StatusReport::{Source, Outpost}`. Use private `status::source` and `status::routes` modules for source registry inventory and local upstream-route probing; keep CLI dispatch shallow and render the two report variants in `output.rs`.

**Tech Stack:** Rust 2024 workspace, Git CLI, Clap, serde/serde_json, mdBook, Markdown agent skills, Vercel Skills CLI.

**Spec:** `docs/superpowers/specs/2026-08-16-gop-status-orientation-design.md`

## Global Constraints

- `gop status` is text-only; continue rejecting `--json`.
- Status is strictly local and read-only: no fetch, pull, push, `ls-remote`, ref update, registry/config write, or remote contact.
- The first successful output line is exactly `context: source` or `context: outpost`.
- The source registry authoritatively records source-to-outpost ownership; a missing path is stale, while an existing contradictory checkout is an integrity error.
- `outpost-container` is optional. `<unset>` is normal, and status never chooses or writes a value.
- Preserve existing managed-outpost degraded diagnostics and cached-ref ahead/behind behavior.
- Effective fetch and push URL sequences collapse only when their complete typed route states are equal.
- Do not call `ops::list::run` from status because it fetches.
- Do not add dependencies or change `gop pull`/`gop push` literal-`origin` behavior.
- Do not stage, edit, or commit the six unrelated July files already untracked under `docs/superpowers/`.
- Every production behavior change follows an observed failing test.

---

### Task 1: Implement the Context-Aware Core Status Module

**Files:**
- Modify: `crates/core/src/ops/status.rs`
- Create: `crates/core/src/ops/status/routes.rs`
- Create: `crates/core/src/ops/status/source.rs`
- Modify: `crates/core/src/error.rs`
- Test: `crates/core/tests/status.rs`

**Interfaces:**
- Produces: `run(&Path) -> OutpostResult<StatusReport>` and the existing `run_with(&Path, &BTreeMap<OsString, OsString>)` test seam.
- Produces: `StatusReport::{Source(SourceStatus), Outpost(OutpostStatus)}` and the refined source/head/upstream/route/registry-row types in the approved spec.
- Produces: `OutpostError::RegisteredOutpostIntegrity { source, outpost }`, mapped to exit code 6.
- Preserves: all existing outpost path, remote, dirty, cached ahead/behind, and health information.

- [ ] **Step 1: Write failing context and refined-state tests**

Replace direct field access in existing outpost tests with one explicit variant helper and add source tests:

```rust
fn expect_outpost(report: StatusReport) -> OutpostStatus {
    match report {
        StatusReport::Outpost(report) => report,
        StatusReport::Source(report) => {
            panic!("expected outpost report, got source {}", report.source_path.display())
        }
    }
}

#[test]
fn source_context_succeeds_from_root_and_nested_directory() {
    let fixture = AbcFixture::new();
    let nested = fixture.source.join("nested");
    fs::create_dir(&nested).expect("create nested");

    let report = run_with(&nested, &fixture.git_env).expect("source status");

    let StatusReport::Source(report) = report else {
        panic!("expected source report");
    };
    assert_eq!(report.source_path, canonical(&fixture.source));
}

#[test]
fn explicit_false_marker_is_source_and_invalid_marker_is_an_error() {
    let fixture = AbcFixture::new();
    set_local_config(&fixture, &fixture.source, "outpost.managed", "false");
    assert!(matches!(
        run_with(&fixture.source, &fixture.git_env).expect("source report"),
        StatusReport::Source(_)
    ));

    set_local_config(&fixture, &fixture.source, "outpost.managed", "maybe");
    assert!(matches!(
        expect_error(run_with(&fixture.source, &fixture.git_env), "invalid marker"),
        OutpostError::BadMetadata { .. }
    ));
}
```

Add assertions that `SourceLocation::{Unconfigured, Missing, Present}` replaces the impossible `Option<PathBuf> + bool` combinations and that detached outposts cannot carry source-upstream state.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p outpost-core --test status --locked`

Expected: compilation fails because `StatusReport` is still the old outpost-only struct and source context still returns `NotAnOutpost`.

- [ ] **Step 3: Introduce the refined report types and context dispatch**

Define the public shape in `status.rs`:

```rust
pub enum StatusReport {
    Source(SourceStatus),
    Outpost(OutpostStatus),
}

pub struct OutpostStatus {
    pub outpost_path: PathBuf,
    pub source: SourceLocation,
    pub remote_name: Option<RemoteName>,
    pub head: OutpostHeadStatus,
    pub outpost_dirty: bool,
    pub source_ahead_behind_upstream: Option<AheadBehind>,
    pub outpost_ahead_behind_source: Option<AheadBehind>,
    pub problems: Vec<ConfigProblem>,
}

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

Discover and canonicalize the work tree once, read `RawMetadata` once, and dispatch `managed == Some(true)` to the existing degraded outpost builder; dispatch absent/false to `source::build`. Keep malformed boolean parsing as `BadMetadata`.

- [ ] **Step 4: Write failing upstream-route tests**

Add Core tests for local and target branch names that differ, equal and different fetch/push routes, multiple URLs with first-seen de-duplication, an exit-2 missing remote as `Unavailable`, a configured remote without URL entries using Git's remote-name fallback, non-2 Git failure as an error, detached and unset tracking, and `branch.<name>.remote=.` as `TrackedUpstream::LocalRepository` without a `remote get-url .` invocation. The central remote-target assertion is:

```rust
assert_eq!(
    report.head,
    SourceHead::Attached {
        branch: branch("release-prep"),
        upstream: Some(TrackedUpstream::Remote {
            remote: remote("origin"),
            branch: branch("main"),
            routes: RemoteRoutes {
                fetch: RouteAvailability::Known(urls(["https://example.test/widget.git"])),
                push: RouteAvailability::Known(urls(["ssh://git@example.test/widget.git"])),
            },
        }),
    }
);
```

- [ ] **Step 5: Run the route cases and verify RED**

Run: `cargo test -p outpost-core --test status source_ --locked`

Expected: the route and source-head assertions fail because the route module and typed tracking states do not exist.

- [ ] **Step 6: Implement local tracking and route probes**

In `status/routes.rs`, implement:

```rust
pub(super) fn read_upstream(
    git: &GitInvoker,
    branch: &BranchName,
) -> OutpostResult<Option<TrackedUpstream>>;

fn probe_urls(
    git: &GitInvoker,
    remote: &RemoteName,
    direction: RouteDirection,
) -> OutpostResult<RouteAvailability>;
```

Read both `branch.<name>.remote` and `.merge`; only `refs/heads/<branch>` is complete branch tracking. For named remotes run `remote get-url --all` and `remote get-url --push --all`. Parse nonempty lines, including Git's remote-name fallback for a configured remote without URL entries, into a private-constructor `RemoteUrlList`; de-duplicate without sorting, map only `GitFailed { code: 2 }` to `Unavailable`, and propagate every other error. Special-case remote `.` before any remote lookup.

- [ ] **Step 7: Write failing source inventory and integrity tests**

Add named tests that prove: absent registry gives no rows; live rows preserve registry order and include shortest unique ID, path, attached/detached branch, dirty state, and lock; ID prefixes include stale entries; `fs::metadata` `NotFound` creates `StaleRegistration`; another I/O error is not stale; duplicate paths are `BadRegistry`; and unset/configured/malformed `outpost-container` has the approved behavior.

Add integrity cases for missing/false managed metadata, wrong `sourceRepo`, mismatched `remoteName`, a missing recorded remote, and a remote redirected away from the source. Manufacture contradictions only by editing local metadata, replacing a registered directory, or editing the test registry.

- [ ] **Step 8: Run the inventory cases and verify RED**

Run: `cargo test -p outpost-core --test status source_registry --locked`

Expected: inventory tests fail because source status does not yet partition and validate registry entries.

- [ ] **Step 9: Implement source inventory and typed integrity errors**

In `status/source.rs`, load config and registry once, reject duplicate path records, derive all `OutpostId`s before calculating prefixes, then process entries in registry order. Use `fs::metadata`: `NotFound` creates a stale row, success runs every reverse-link and recorded-remote predicate, and any other error becomes `IoAt`.

Add:

```rust
#[error(
    "registered outpost is inconsistent with source {}: {}",
    .source.display(),
    .outpost.display()
)]
RegisteredOutpostIntegrity { source: PathBuf, outpost: PathBuf },
```

Map it to exit 6. Require metadata remote name to equal the registry value and every effective fetch/push route of that local source remote to canonicalize to the reporting source.

- [ ] **Step 10: Refine outpost diagnostics and preserve degraded behavior**

Replace ambiguous status tracking problems with:

```rust
OutpostSourceTrackingUnavailable { branch: BranchName },
SourceBranchMissing { branch: BranchName },
SourceUpstreamTrackingUnset { branch: BranchName },
SourceUpstreamRouteUnavailable { remote: RemoteName },
```

Build source-upstream data whenever an attached outpost has a present source, even when the outpost remote name is missing. Preserve `LocalRemoteMismatch`, `NotInRegistry`, `PushWouldFail`, missing metadata, detached, and cached comparisons. Use fallible source-path metadata so access errors cannot become `source-present: false`.

- [ ] **Step 11: Prove status remains local and run all Core tests**

Record relevant refs before and after `run_with`, and use a PATH shim or the existing invocation hook to reject `fetch`, `pull`, `push`, `ls-remote`, and `update-ref` for source and outpost cases. Run:

```bash
cargo test -p outpost-core --test status --locked
cargo test -p outpost-core --tests --locked
```

Expected: all Core tests pass; no status case changes refs or invokes a forbidden command.

- [ ] **Step 12: Commit the Core deliverable**

```bash
git add crates/core/src/ops/status.rs crates/core/src/ops/status/routes.rs \
  crates/core/src/ops/status/source.rs crates/core/src/error.rs \
  crates/core/tests/status.rs
git commit -m "feat(status): orient from sources and outposts"
```

### Task 2: Render the Exact CLI Contract

**Files:**
- Modify: `crates/cli/src/output.rs`
- Modify: `crates/cli/src/cli.rs`
- Test: `crates/cli/tests/e2e.rs`
- Test: `crates/cli/tests/help.rs`

**Interfaces:**
- Consumes: `StatusReport::{Source, Outpost}` and refined Core types from Task 1.
- Produces: exact text from the design, with first-line context, tab-separated rows, route collapse, and stable health ordering.
- Preserves: the one-call `main.rs` dispatch and absence of status flags.

- [ ] **Step 1: Write exact failing source-output tests**

Add CLI fixtures for an empty source and for live/stale rows. Assert literal stdout with canonical fixture paths:

```text
context: source
source: <canonical-source>
branch: main
source-state: clean
upstream: origin/main  <canonical-upstream>
outpost-container: <unset>
outposts: none
stale-registrations: none
```

For populated sections assert `  <id>\t<path>\t<branch|detached>\t<clean|dirty>[\tlocked]` and `  <id>\t<stale-path>` exactly. Add detached, `<unset>`, `<not-applicable>`, `<local-repository>`, collapsed routes, split routes, and one-direction `<unavailable>` cases.

- [ ] **Step 2: Write exact failing outpost-output tests**

Assert `context: outpost` precedes all current fields, `source-upstream` follows `outpost-state`, and missing source prints `<unavailable>` for upstream identity while comparisons remain `-`. Assert these exact degraded rules:

```text
Unconfigured source -> source: -      / source-present: false
Missing source      -> source: <path> / source-present: false
Present source      -> source: <path> / source-present: true
Missing remote      -> remote: -
Detached HEAD       -> source-upstream: <not-applicable>
```

Assert the exact relationship-specific health strings in the spec.

- [ ] **Step 3: Run the CLI tests and verify RED**

Run: `cargo test -p git-outpost --test e2e --locked`

Expected: source status fails and outpost status starts with `outpost:` instead of `context: outpost`.

- [ ] **Step 4: Implement context-specific rendering**

Make `print_status` match the enum and delegate to private `print_source_status` and `print_outpost_status`. Prefix section rows with two spaces and separate fields with tabs. Add one route renderer that repeats lines for multiple URLs and uses the collapsed label only when typed fetch and push values compare equal.

Update status help to:

```rust
/// Summarize the current source repository or managed outpost.
Status(StatusArgs),
```

Do not add a CLI flag or move classification into `main.rs`.

- [ ] **Step 5: Update invocation and shell-delegation coverage**

Run `gop`, `git-outpost`, and `git outpost` status in both contexts and assert identical stdout. Update the shell wrapper test's first line to `context: outpost`. Add direct-current-directory versus global `-C` equality for source and outpost, and retain the existing `--json` rejection test unchanged.

- [ ] **Step 6: Run CLI tests and commit**

Run:

```bash
cargo test -p git-outpost --test e2e --test help --test flags --locked
```

Expected: exact output, invocation equivalence, help, shell delegation, and `--json` rejection all pass.

```bash
git add crates/cli/src/output.rs crates/cli/src/cli.rs \
  crates/cli/tests/e2e.rs crates/cli/tests/help.rs
git commit -m "feat(cli): render source-aware status"
```

### Task 3: Update Product And Architecture Documentation

**Files:**
- Modify: `docs/src/product.md`
- Modify: `docs/src/architecture.md`

**Interfaces:**
- Consumes: the executable Core and CLI contract from Tasks 1-2.
- Produces: user and maintainer documentation that does not retain an outpost-only status claim.

- [ ] **Step 1: Replace the product status contract**

Rewrite only `### status`. State that the first line identifies source/outpost context; source output contains checked-out branch/state/tracking routes/optional container/live outposts/stale registrations; and outpost output retains health and cached comparisons plus source upstream. Keep the explicit no-fetch/no-remote-contact boundary and `gop -C <path> status` usage.

- [ ] **Step 2: Replace the architecture interface and invariants**

Update only the `ops/status.rs` architecture material. Show `StatusReport::{Source, Outpost}`, refined source/head/upstream/route types, registry authority, integrity errors, and the private source/routes implementations. State that CLI dispatch calls Core once and only `output.rs` renders text.

- [ ] **Step 3: Verify and commit documentation**

Run:

```bash
mdbook build docs
git diff --check -- docs/src/product.md docs/src/architecture.md
```

Expected: the book builds and both status sections match implemented behavior.

```bash
git add docs/src/product.md docs/src/architecture.md
git commit -m "docs: document status orientation"
```

### Task 4: Simplify, Validate, And Install The Agent Skill

**Files:**
- Modify: `skills/using-git-outpost/SKILL.md`
- Modify: `skills/using-git-outpost/references/gop-workflows.md`
- Modify if stale: `skills/using-git-outpost/agents/openai.yaml`
- Modify: `README.md`

**Interfaces:**
- Consumes: successful `gop --no-color -C <path> status` output from Tasks 1-2.
- Produces: one-call normal orientation with source/outpost named from the first line.
- Preserves: task-specific mutation readiness, two-hop safety, lifecycle postconditions, and the agent's choice whether and where to configure `outpost-container`.

- [ ] **Step 1: Invoke skill-writing guidance and establish RED scenarios**

Read `skill-creator` and `superpowers:writing-skills` fully. Before editing, run fresh-agent scenarios for:

1. orientation from a source checkout without manually interpreting exit 2;
2. orientation from healthy and missing-source outposts;
3. an unset `outpost-container` that must remain a normal agent choice rather than trigger an automatic write;
4. a worktree request requiring a choice between explicit `gop add` destination and safe container configuration.

Record the old-skill failures: it requires `git rev-parse` before status, treats source status as an error, and may reach for fetching `gop list` to infer layout.

- [ ] **Step 2: Reduce normal orientation to one command**

Replace the numbered multi-probe path with:

```bash
gop --no-color -C <path> status
```

Map exit-zero `context: source` to `SourceContext(report)` and exit-zero `context: outpost` to `ManagedOutpostContext(report)`. Preserve any nonzero result as `Unknown(error)`. Keep only a short unavailable-CLI fallback for an explicit local managed marker; do not recreate source status manually.

- [ ] **Step 3: Remove redundant workflow probes**

Use source status's `outpost-container`, `outposts`, and `stale-registrations` directly. Remove the instruction to run fetching `gop list` merely to infer layout or container choice. Keep `<unset>` normal and let the agent decide whether recurring named creation justifies configuration. Retain command-specific `Ready(command)` and postcondition checks only where mutation risk requires them.

- [ ] **Step 4: Update README and metadata narrowly**

Keep the existing Vercel Skills CLI installation block. Change only its explanatory paragraph to say the skill uses one context-aware `gop status`, reports optional `outpost-container`, maps worktree intent to outposts, and leaves configuration choice to the agent. Update `agents/openai.yaml` only if its description or prompt no longer matches.

- [ ] **Step 5: Validate and rerun the scenarios**

Run:

```bash
python3 /home/huwei/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  skills/using-git-outpost
```

Expected: `Skill is valid!`

Rerun the four fresh-agent scenarios. Require one `gop status` call on the normal path, correct first-line context interpretation, no automatic container write merely because the value is unset, and no `gop list` call for orientation.

- [ ] **Step 6: Commit repository skill changes**

```bash
git add skills/using-git-outpost/SKILL.md \
  skills/using-git-outpost/references/gop-workflows.md \
  skills/using-git-outpost/agents/openai.yaml README.md
git commit -m "docs(skill): use one-call gop orientation"
```

- [ ] **Step 7: Install through the Vercel Skills CLI and verify discovery**

Run with permission for the global installation target:

```bash
npx skills add . --skill using-git-outpost --agent codex --global --yes
```

Resolve the installed skill through the available-skills catalog, then re-read its `SKILL.md` and workflow reference from the installed location. Compare them with the committed repository files; command exit status alone is not installation proof.

### Task 5: Full Verification And Draft PR Update

**Files:**
- No new production files expected.
- Update: existing draft PR `OptimalCNC/git-outpost#4` through commits and PR metadata.

**Interfaces:**
- Consumes: all committed deliverables from Tasks 1-4.
- Produces: a pushed branch and draft PR describing and proving the implemented one-call orientation workflow.

- [ ] **Step 1: Run repository verification from a clean tracked state**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
mdbook build docs
python3 /home/huwei/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  skills/using-git-outpost
git diff --check origin/main...HEAD
git status --short
```

Expected: all commands pass. Status may list only the six pre-existing untracked July documents; no requested file remains modified or staged.

- [ ] **Step 2: Inspect complete branch scope**

Run:

```bash
git log --oneline origin/main..HEAD
git diff --stat origin/main...HEAD
git diff --name-status origin/main...HEAD
```

Confirm the branch contains the original skill/README commit, the approved design and plan, Core/CLI/docs/skill implementation commits, and no unrelated file.

- [ ] **Step 3: Request final code review and address actionable findings**

Use `superpowers:requesting-code-review` against `origin/main...HEAD`. Correct only contract, behavior, or test gaps; every behavior fix starts with a focused failing test. Do not absorb adjacent cleanup.

- [ ] **Step 4: Push and update the existing draft PR**

Push `codex/add-using-git-outpost-skill`, verify PR #4's head SHA, and update its title/body to cover both the skill and the `gop status` interface that simplifies orientation. Include the exact verification commands and keep the PR draft unless the user asks to mark it ready.

- [ ] **Step 5: Re-read hosted PR state**

Use `gh pr view 4 --repo OptimalCNC/git-outpost` and inspect hosted head SHA, draft state, changed files, and checks. Report pending or failing hosted checks; do not infer hosted success from local tests.
