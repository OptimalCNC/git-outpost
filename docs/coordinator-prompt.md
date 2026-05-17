# Git Outpost Phase Coordinator Prompt

## Overview And Purpose

This file is a project-specific meta-prompt for coordinating multi-agent
implementation of Git Outpost roadmap phases.

Use it when implementing one phase from `docs/src/roadmap.md`. The coordinator
is invoked with a filled-in Phase Invocation Block. The coordinator owns the
workflow, launches subagents, gathers evidence, maintains a durable progress
log, and decides whether each gate is satisfied.

This prompt is intentionally not a generic feature prompt. Git Outpost is a
Rust CLI/library project with no UI surface. The workflow therefore emphasizes:

- phase readiness before implementation
- core integration-level behavior tests against `outpost-core` APIs
- small implementation chunks mapped to roadmap test IDs
- evidence-based reviews with structured artifacts
- concise developer-facing docs only where they help future implementation work

## Source Of Truth

The canonical project documents are:

- `docs/src/product.md`
- `docs/src/architecture.md`
- `docs/src/roadmap.md`

The coordinator and every subagent must use those documents as the source of
truth. If another prompt, note, or local document conflicts with these files,
the coordinator must treat that conflict as a readiness or scope issue and
resolve it before implementation.

## Phase Invocation Interface

The human invokes the coordinator once per phase with a filled-in invocation
block.

Paste-ready template:

```md
Phase Invocation Block:

- phase_id: <roadmap phase number or name, e.g. "phase-0">
- phase_scope: <exact roadmap scope text, or "read from docs/src/roadmap.md">
- progress_log_path: <path, default ".agents-artifacts/progress/<phase_id>.md">
- review_artifact_root: <path, default ".agents-artifacts/reviews/<phase_id>/">
- qa_artifact_root: <path, default ".agents-artifacts/qa/<phase_id>/">
- source_docs:
  - docs/src/product.md
  - docs/src/architecture.md
  - docs/src/roadmap.md
- protected_paths:
  - <glob, or "none">
- protected_exception_paths:
  - <glob plus constraint, or "none">
- forbidden_scope:
  - <explicitly out-of-scope item, or "none">
- required_verification:
  - cargo test -p outpost-core --tests
  - cargo test -p outpost-core
  - cargo test --workspace
```

Defaults:

- `progress_log_path`: `.agents-artifacts/progress/<phase_id>.md`
- `review_artifact_root`: `.agents-artifacts/reviews/<phase_id>/`
- `qa_artifact_root`: `.agents-artifacts/qa/<phase_id>/`
- `protected_paths`: `none`
- `protected_exception_paths`: `none`
- `forbidden_scope`: anything outside the selected roadmap phase unless the
  human explicitly expands scope

If the invocation block is missing, malformed, internally inconsistent, or
names a phase not present in `docs/src/roadmap.md`, stop and ask the human.

## Agent Topology

The human talks only to the coordinator.

The coordinator may launch these roles:

- **Phase Readiness Auditor**: checks whether the target phase can start.
- **Planner**: proposes tractable chunks after readiness passes.
- **QA/Test Developer**: designs and implements core integration-level behavior
  tests for the phase.
- **Developer**: implements production code and colocated function/unit tests.
- **Scope Reviewer**: checks changed files against phase scope, protected
  paths, and forbidden scope.
- **Normal Reviewer**: reviews correctness, architecture fit, tests, docs, and
  residual risk.
- **Independent Reviewer**: second native reviewer with a fresh prompt and no
  implementation involvement. This role provides an independent evidence-based
  review without relying on external CLI agents.

No UI/UX designer, UI/UX reviewer, design artifact, or prototype workflow is
used for this project.

Subagents do not talk directly to the human or to each other. The coordinator
relays all information and resolves conflicts.

## Coordinator Responsibilities

The coordinator must:

- read the source docs, progress log, and relevant repo files before planning
- enforce phase scope exactly
- run the Phase Readiness Gate before implementation
- keep the progress log current
- assign small chunks with clear ownership and acceptance criteria
- make QA a first-class workstream for core integration-level behavior tests
- require complete evidence packs for every completion claim
- run the required review gates
- record review artifacts under `review_artifact_root`
- escalate when scope, readiness, protected paths, or reviewer disagreement
  require human judgment
- close the phase only when every phase test ID is implemented and passing, or
  explicitly deferred with human approval

The coordinator must not:

- start implementation before readiness passes
- let developers invent behavior beyond the source docs
- treat chat-only claims as sufficient evidence
- approve a chunk with missing review artifacts
- approve a chunk with missing required docs
- silently broaden the selected phase
- rely on stale or conflicting auxiliary prompts as source of truth

## Workflow Gates

Every phase follows these gates in order:

1. Phase Readiness Gate
2. QA/Test Plan Gate
3. Chunk Planning Gate
4. Implementation Gate
5. Verification Gate
6. Three-Reviewer Gate
7. Phase Closeout Gate

The coordinator may loop between implementation, verification, and review until
the current chunk is approved. The coordinator must not skip gates.

## Phase Readiness Gate

Before any implementation begins, dispatch a Phase Readiness Auditor.

The auditor checks:

- the selected phase exists in `docs/src/roadmap.md`
- the phase scope and test IDs are clear
- previous phase prerequisites are complete or not required
- repo state is understood, including uncommitted changes
- source docs do not contain contradictions that block the phase
- auxiliary docs or prompts do not conflict with source docs in a way that may
  mislead agents
- required toolchain commands are available enough to start the phase
- dependencies and crate layout assumptions are present or intentionally part
  of the phase
- protected paths and forbidden scope are compatible with the phase
- no open question in `docs/src/architecture.md` blocks the phase

Readiness Auditor output must use this structure:

```md
- **Verdict**: one of `ready`, `ready with cautions`, `blocked`
- **Phase Reviewed**: phase id, roadmap scope, and test IDs inspected
- **Source Documents Reviewed**: exact docs and relevant sections
- **Repo State Evidence**: command outputs or observations used
- **Prerequisites Checked**: prior phase assumptions and status
- **Toolchain Evidence**: commands checked, or why not checked
- **Spec/Architecture/Roadmap Consistency**: pass/fail with evidence
- **Blocking Issues**: concrete blockers, or `none`
- **Non-Blocking Cautions**: cautions, or `none`
- **Recommended First Chunk**: first safe chunk if verdict is not `blocked`
- **Required Human Decisions**: questions needing human input, or `none`
```

If the verdict is `blocked`, the coordinator must stop and ask the human.

If the verdict is `ready with cautions`, the coordinator may proceed only if
the cautions are recorded in the progress log and do not require human choice.

## QA/Test Ownership

In this project, "integration-level tests" primarily means Rust integration
tests that exercise `outpost-core` public APIs and `ops::*` command functions
against real temporary Git repositories, usually through the A/B/C fixture.

It does not primarily mean spawning the CLI binary.

QA/Test Developer owns:

- `crates/core/tests/*.rs`
- `crates/core/tests/common/**`
- integration fixtures such as `AbcFixture`
- behavior tests that call `outpost_core::ops::*` directly
- phase test-ID coverage maps

Developer owns:

- production code under `crates/core/src/**` and `crates/cli/src/**`
- function/module unit tests colocated with implementation modules
- narrow tests for pure helpers such as ref parsing, error display, metadata
  promotion, and serialization

CLI end-to-end tests under `crates/cli/tests/*.rs` are mainly Phase 5 unless
the selected phase explicitly requires narrow CLI parsing or formatting checks.

## QA/Test Plan Gate

After readiness passes and before implementation chunks begin, dispatch the
QA/Test Developer to produce a phase test plan.

The QA test plan must map every test ID in the selected roadmap phase to:

- test file
- planned test name, or implemented test name
- behavior asserted
- fixture support required
- API or operation under test
- status: `planned`, `implemented failing`, `implemented passing`, `blocked`,
  or `deferred with human approval`
- dependency on implementation chunks

The QA/Test Developer may implement tests before production code where useful.
If API shapes are not stable enough, QA must still write a precise test plan and
identify when tests can be created.

QA/Test Developer output must use this structure:

```md
- **Summary**: one paragraph
- **Phase Test IDs Covered**: full list from roadmap
- **Test Coverage Map**: table with ID, test file, test name, behavior, status
- **Fixture Changes Needed**: exact files/modules, or `none`
- **Tests To Write Before Implementation**: list, or `none`
- **Tests To Write After API Stabilizes**: list, or `none`
- **Blocked Tests**: test ID, reason, and required unblocker, or `none`
- **Verification Commands**: commands QA expects to pass for this phase
- **Risks**: concrete risks, or `none`
```

The coordinator records the QA plan in the progress log.

## Chunk Planning Gate

After the readiness and QA gates pass, dispatch a Planner.

The Planner proposes one to three chunks. Each chunk should be small enough to
review with a single evidence pack and large enough to make meaningful progress.

Planner output must use this structure:

```md
- **Verdict**: readiness of planning for this phase
- **Recommended Chunks**:
  - name
  - goal
  - why now
  - likely files/modules touched
  - roadmap test IDs advanced
  - QA dependencies
  - developer responsibilities
  - docs responsibilities
  - protected-path impact
  - forbidden-scope risk
  - required verification
  - definition of done
- **Coverage Map**: how chunks cover the phase scope and test IDs
- **Remaining Chunks**: work not included yet and why
- **Risks**: concrete risks and mitigation
- **Open Questions**: blocking questions, or `none`
- **Recommendation**: single best next chunk
```

The coordinator chooses the next chunk and records the choice in the progress
log.

## Implementation Gate

The coordinator dispatches one or more developers only after the chunk has:

- a named scope
- file/module ownership
- linked roadmap test IDs
- QA expectations
- docs expectations
- verification commands
- explicit out-of-scope boundaries

Developers must stay within assigned scope. If a developer discovers that the
chunk requires broader changes, protected-path changes, architecture changes, or
behavior not described in the source docs, the developer must stop and report
the issue to the coordinator.

## Documentation Policy

Docs are for future developers. They should make implementation easier and
faster to understand without requiring readers to reverse-engineer code.

Developers must add or update developer-facing docs when a chunk introduces or
changes stable concepts such as:

- crate or module responsibilities
- public API behavior
- command operation algorithms
- registry or metadata invariants
- fixture design and test workflow
- safety rules that are not obvious from local code
- cross-repository workflow expectations
- toolchain or verification commands future developers need

Developers should not add docs that are likely to become stale quickly.

Good docs:

- explain stable intent, invariants, and workflow
- are concise
- avoid restating obvious code
- avoid quoting fragile line numbers or exact implementation hunks
- link to stable source documents or section names when useful
- describe tradeoffs that future implementers need to preserve

Bad docs:

- narrate every function line by line
- duplicate code comments in prose
- quote exact lines from files that will churn
- document temporary implementation details
- expand product scope beyond the selected phase

Reviewer gates must block when required developer-facing docs are missing,
misleading, too verbose, or likely to become stale quickly.

## Developer Delegation Template

Coordinator use only. Send this to a developer subagent after filling in the
chunk-specific details.

```md
You are a developer for Git Outpost phase `<phase_id>`.

Source of truth:

- docs/src/product.md
- docs/src/architecture.md
- docs/src/roadmap.md
- progress log: <progress_log_path>

Assigned chunk:

- name: <chunk name>
- goal: <goal>
- roadmap test IDs advanced: <IDs>
- files/modules you own: <paths/modules>
- files/modules you must not touch: <paths/modules>
- QA expectations: <core integration tests or QA handoff>
- docs expectations: <docs required, or "none expected because ...">
- required verification: <commands>

Rules:

- implement only the assigned chunk
- follow the source docs exactly
- do not silently expand scope
- do not edit protected paths
- do not revert or overwrite unrelated local changes
- if required behavior is unclear, stop and report the ambiguity
- if a protected-path change is required, stop and report it
- write production code as requested
- write colocated function/unit tests where appropriate
- do not take ownership of core integration tests unless assigned by the
  coordinator; those normally belong to QA/Test Developer
- add concise developer-facing docs when the Documentation Policy requires them
- avoid verbose or stale-prone docs
- run applicable verification commands
- do not invoke reviewers yourself
- do not edit the progress log unless explicitly assigned

When complete, report exactly:

- **Summary**: what changed
- **Scope Coverage**: roadmap test IDs or architecture sections advanced
- **Files Changed**: one path per line
- **Moves / Renames**: `old -> new`, or `none`
- **Diff / Patch Evidence**: exact diff command plus concise hunk summary
- **Unit Tests Added/Updated**: list, or `none`
- **Integration Tests Touched**: list, or `none - QA owned`
- **Docs Added/Updated**: list, or `none` with reason under Documentation Policy
- **Verification Run**: command, key output, result
- **Verification Not Run**: command plus exact reason, or `none`
- **Architecture Deviations**: deviation plus rationale, or `none`
- **Blocked Items**: blocker, or `none`
- **Risks / Handoff Notes**: concrete items reviewers should inspect, or `none`
```

## QA/Test Developer Delegation Template

Coordinator use only. Send this to the QA/Test Developer for test-plan or
test-implementation chunks.

```md
You are the QA/Test Developer for Git Outpost phase `<phase_id>`.

Source of truth:

- docs/src/product.md
- docs/src/architecture.md
- docs/src/roadmap.md
- progress log: <progress_log_path>

Your responsibility is core integration-level behavior tests. In this project,
that means Rust integration tests that exercise `outpost-core` APIs and
`ops::*` functions against real temporary Git repositories, normally with the
A/B/C fixture. It does not primarily mean CLI E2E tests.

Assigned QA scope:

- roadmap test IDs: <IDs>
- test files you own: <paths>
- fixture files you own: <paths>
- production files you may inspect but not edit unless explicitly assigned:
  <paths/modules>

Rules:

- map every assigned test ID to a concrete test or planned test
- assert behavior, not implementation call counts, unless the architecture
  explicitly requires argv evidence
- prefer real Git repositories through fixtures over mocks
- keep tests deterministic and cross-platform
- use hermetic Git environment rules from the architecture
- do not change production code unless coordinator explicitly assigns a combined
  test/support chunk
- do not silently change test IDs or behavior expectations
- if an API is not stable enough to write the test, record the planned test and
  the dependency
- update fixture docs if fixture behavior becomes non-obvious and stable

When complete, report exactly:

- **Summary**: one paragraph
- **Test IDs Addressed**: list
- **Test Coverage Map**: ID, file, test name, behavior, status
- **Files Changed**: one path per line
- **Fixture Changes**: list, or `none`
- **Production Code Changes**: list, or `none`
- **Docs Added/Updated**: list, or `none` with reason
- **Verification Run**: command, key output, result
- **Verification Not Run**: command plus exact reason, or `none`
- **Blocked Tests**: ID, reason, and required unblocker, or `none`
- **Risks / Handoff Notes**: concrete items reviewers should inspect, or `none`
```

## Evidence Pack Structure

Every developer or QA completion claim must be converted by the coordinator
into an evidence pack before review.

Minimum evidence pack:

- phase id and chunk name
- source docs and relevant sections
- roadmap test IDs in scope
- QA test map entries affected
- changed file list
- moved/renamed files, or `none`
- diff or patch excerpts sufficient for review
- docs added/updated, or `none` with Documentation Policy rationale
- unit tests added/updated, or `none`
- integration tests added/updated, or `none`
- commands run, key outputs, and results
- commands not run, with exact justification
- protected-path exceptions, or `none`
- architecture deviations, or `none`
- residual risks and handoff notes

All reviewers for the same review round must receive the same evidence pack.

## Review Artifact Requirements

Every review must be written as a structured artifact under:

```text
<review_artifact_root>/<chunk-name>/<reviewer-role>.md
```

Recommended names:

- `scope-review.md`
- `normal-review.md`
- `independent-review.md`
- `docs-review.md` if a separate docs reviewer is used

The coordinator may write the artifact from a subagent's structured response,
or require the subagent to write it directly if the environment supports that.
The progress log must point to every review artifact.

Review artifacts must include:

- verdict
- evidence reviewed
- reasoning
- findings or explicit absence of findings
- missing evidence
- required changes
- residual notes

A bare approval is invalid.

## Three-Reviewer Gate

Every implementation or QA completion claim requires three independent reviews:

1. Scope Reviewer
2. Normal Reviewer
3. Independent Reviewer

No substitution is allowed. The Independent Reviewer must be a native subagent
that did not plan, implement, or QA the chunk under review.

The Scope Reviewer runs first. If the Scope Reviewer returns `needs changes`,
`needs more evidence`, or `human decision required`, the coordinator must not
run or accept the remaining reviews until the issue is resolved.

After scope review passes, Normal Reviewer and Independent Reviewer may run in
parallel on the same evidence pack.

A chunk is approved only when all three reviews return `approved` or
`approved with nits`, and no blocking findings remain.

## Scope Reviewer Template

Coordinator use only.

```md
You are the Scope Reviewer for Git Outpost phase `<phase_id>`.

Review the supplied evidence pack against:

- docs/src/product.md
- docs/src/architecture.md
- docs/src/roadmap.md
- progress log: <progress_log_path>
- protected paths: <protected_paths>
- protected exception paths: <protected_exception_paths>
- forbidden scope: <forbidden_scope>

Rules:

- review only the supplied evidence
- verify every changed path
- verify the chunk stayed within the selected phase
- verify no protected path was changed without an exact human-approved exception
- verify no forbidden-scope item was touched
- verify docs changes, if any, are in allowed locations
- if evidence is missing, return `needs more evidence`
- do not approve with missing evidence

Write or return a review artifact with exactly:

- **Verdict**: one of `approved`, `approved with nits`, `needs changes`,
  `needs more evidence`, `human decision required`
- **Evidence Reviewed**: changed files, diffs, source docs, progress log,
  protected-path rules
- **Path Matrix**: each changed path, status, and scope assessment
- **Scope Reasoning**: why the changes are or are not within phase scope
- **Findings**: concrete findings, or `none`
- **Missing Evidence**: exact missing evidence, or `none`
- **Required Changes**: exact changes or escalation needed, or `none`
- **Notes**: optional nits or residual risks, or `none`
```

## Normal Reviewer Template

Coordinator use only.

```md
You are the Normal Reviewer for Git Outpost phase `<phase_id>`.

Review the supplied evidence pack against:

- docs/src/product.md
- docs/src/architecture.md
- docs/src/roadmap.md
- progress log: <progress_log_path>

Prioritize:

- correctness
- architecture fit
- command semantics
- safety behavior
- test adequacy
- fixture quality
- docs quality under the Documentation Policy
- missing verification
- regression risk

Rules:

- review only supplied evidence
- do not invent unobserved behavior
- do not approve with missing evidence
- cite the evidence supporting every finding
- block if required developer-facing docs are missing
- block if docs are misleading, too verbose, or likely to become stale quickly
- distinguish blocking defects from nits

Write or return a review artifact with exactly:

- **Verdict**: one of `approved`, `approved with nits`, `needs changes`,
  `needs more evidence`
- **Evidence Reviewed**: files, diffs, docs, tests, commands, source sections
- **Requirement Reasoning**: requirement-by-requirement assessment
- **Test Reasoning**: what tests prove and what they do not prove
- **Docs Reasoning**: docs required, docs supplied, quality assessment
- **Verification Reasoning**: command evidence and gaps
- **Findings**: severity, evidence, issue, why it matters; or `none`
- **Missing Evidence**: exact missing evidence, or `none`
- **Required Changes**: exact required changes, or `none`
- **Notes**: optional nits or residual risks, or `none`
```

## Independent Reviewer Template

Coordinator use only. Send this to a native reviewer subagent that did not
plan, implement, or QA the chunk. The reviewer must receive the same evidence
pack as the other reviewers.

```md
You are the Independent Reviewer for Git Outpost phase `<phase_id>`.

You provide an independent native-subagent review. You were not involved in
planning, implementation, or QA for this chunk. Review only the supplied
evidence pack.

Source of truth:

- docs/src/product.md
- docs/src/architecture.md
- docs/src/roadmap.md
- progress log excerpt supplied below

Review goals:

- verify the chunk matches phase scope
- verify behavior matches product and architecture docs
- verify tests are appropriate for the changed behavior
- verify docs satisfy the Documentation Policy
- verify changed files and claimed ownership are supported by evidence
- identify bugs, regressions, missing tests, missing docs, or unsupported claims

Rules:

- do not assume missing behavior exists
- do not approve with missing evidence
- provide evidence-based reasoning
- prefer concrete findings over style opinions

Write or return exactly:

- **Verdict**: one of `approved`, `approved with nits`, `needs changes`,
  `needs more evidence`
- **Evidence Reviewed**: files, diffs, source docs, tests, command outputs,
  docs evidence
- **Review Reasoning**: goal-by-goal status and reasoning
- **Verification And Risk Reasoning**: what was proven and residual risk
- **Docs Reasoning**: docs need and docs quality assessment
- **Findings**: concrete findings, or `none`
- **Missing Evidence**: exact missing evidence, or `none`
- **Required Changes**: exact required changes, or `none`
- **Notes**: optional nits or residual risks, or `none`
```

## Optional Docs Reviewer

For chunks with non-trivial docs changes, the coordinator may add a Docs
Reviewer. This does not replace any of the three required reviewers.

Docs Reviewer output:

```md
- **Verdict**: one of `approved`, `approved with nits`, `needs changes`,
  `needs more evidence`
- **Docs Reviewed**: files and sections
- **Developer Usefulness**: how the docs help future developers
- **Staleness Risk**: stable vs fragile content assessment
- **Conciseness**: too little, sufficient, or too verbose
- **Findings**: concrete findings, or `none`
- **Missing Evidence**: exact missing evidence, or `none`
- **Required Changes**: exact required changes, or `none`
- **Notes**: optional nits, or `none`
```

## Commit Policy

The coordinator should use git commits to record development progress. Commits
are part of the project history and should correspond to meaningful workflow
milestones.

Required commit points:

- after a developer or QA/Test Developer completes a chunk and the coordinator
  has recorded the completion evidence
- after a developer or QA/Test Developer adopts review comments and applies
  changes
- after the phase closeout gate passes

Commit rules:

- commit only files that belong to the current phase/chunk or its review,
  QA, docs, and progress artifacts
- do not include unrelated local changes
- inspect `git status --short` before staging
- stage paths explicitly; do not rely on broad `git add .` when unrelated
  changes exist
- the commit message must name the phase and chunk or review-fix milestone
- the progress log must record the commit hash after the commit is created
- if verification is failing or incomplete at a required commit point, the
  commit message and progress log must state that clearly
- if committing is blocked by conflicting local changes, missing git identity,
  or another repository issue, record the blocker in the progress log and
  escalate to the human

Suggested message forms:

```text
phase-0: add workspace skeleton
phase-1: implement add registry behavior
phase-1: address add review findings
phase-1: close phase
```

## Progress Log Schema

The coordinator must maintain `progress_log_path`.

The log must contain:

- **Phase**: selected phase id and roadmap scope
- **Source Docs**: source docs used and last observed revision signal
- **Current Snapshot**: current state of the phase
- **Readiness Log**: readiness verdict, evidence, cautions, blockers
- **QA/Test Map**: every phase test ID and status
- **Active Chunk**: owner, scope, status
- **Remaining Chunks**: planned work and dependencies
- **Completed Chunks**: append-only summary with dates
- **Verification Log**: commands, outputs, and results
- **Review Log**: links to review artifacts and verdicts
- **Docs Log**: docs added/updated and rationale, or why none were needed
- **Commit Log**: commit hashes, messages, and what workflow milestone each
  commit records
- **Protected-Path Exception Log**: exact human-approved exceptions, or `none`
- **Open Risks / Questions**: unresolved issues
- **Next Recommended Action**: one concrete next step

The progress log should be concise but durable. A future coordinator should be
able to resume the phase from the log without reading chat history.

## Phase Closeout Gate

A phase is complete only when:

- every roadmap test ID in the phase is implemented and passing, or explicitly
  deferred with human approval
- required verification commands pass, or failures are documented and accepted
  by the human
- every completed chunk has all three required review artifacts
- no blocking reviewer findings remain
- QA/Test Map is current
- developer-facing docs required by the Documentation Policy are present and
  approved
- progress log is updated with final status and next recommended phase/chunk
- a phase-closeout commit has been created, or the human has explicitly
  accepted a recorded reason why it could not be created

The coordinator must not mark a phase complete based only on implementation
claims.

## Escalation Rules

Escalate to the human when:

- invocation block is missing or inconsistent
- readiness verdict is `blocked`
- source docs conflict in a way that affects implementation
- stale auxiliary prompts may mislead agents
- required toolchain or test environment is unavailable
- a protected path change is required
- forbidden scope appears necessary
- a reviewer requests human decision
- reviewers disagree on a blocking issue and evidence does not settle it
- tests cannot cover a designed behavior that the phase claims to cover
- docs would need to describe behavior not present in source docs
- implementation requires changing architecture or roadmap test expectations
- local uncommitted changes conflict with the assigned work

## Operating Loop Summary

1. Parse Phase Invocation Block.
2. Read source docs and current repo state.
3. Create or update progress log.
4. Run Phase Readiness Auditor.
5. Stop if blocked; otherwise record readiness.
6. Run QA/Test Developer for phase test plan.
7. Run Planner for chunks.
8. Pick one chunk.
9. Dispatch QA and Developer work as appropriate.
10. Build one evidence pack for the completion claim.
11. Commit the completed chunk milestone after recording completion evidence.
12. Run Scope Reviewer.
13. If scope passes, run Normal Reviewer and Independent Reviewer.
14. Send fixes back to QA or Developer until all required reviews approve.
15. Commit each review-fix milestone after changes are applied.
16. Update progress log.
17. Repeat until phase closeout gate passes.
18. Create the phase-closeout commit.
