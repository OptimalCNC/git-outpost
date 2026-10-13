# Core Coverage And Test Policy

This document is the normative policy for permanent tests and coverage work in
`crates/core`. It separates behavioral obligations, coverage measurement, and
safe ways to resolve gaps. The accepted target is 100% production executable
line coverage within the scope below; the final section describes enforcement
and adoption.

## Purpose And Behavioral Obligations

Tests protect supported behavior: correct results, data preservation, safety
checks, meaningful failures, and documented no-mutation and partial-state
guarantees. The [product specification](product.md) owns product behavior;
the [architecture](architecture.md#10-test-strategy) owns the test layers.
This policy owns coverage scope and permanent-test admission.

A coverage percentage does not establish those guarantees. Important outcomes
still need semantic assertions even when their lines have already executed.
Conversely, an uncovered line does not automatically justify another test.
Decide whether behavior needs protection, how to test it safely, and whether
the source belongs in the measured scope as separate questions.

## Coverage Measurement

The metric is production executable line coverage for `outpost-core`, initially
for one explicitly declared Linux target in the debug profile. Include public
and private production code in `crates/core/src`: operations, state handling,
configuration, metadata, registry, Git interactions, filesystem effects, safety
checks, and success, degraded, and error outcomes. Private helpers and thin
wrappers remain in scope; they need not each have a standalone test.

Exclude non-production source from the measured denominator:

- `#[cfg(test)]` test implementations and fixtures;
- test-only instrumentation enabled by `test-helpers`, including observation
  logs, test constructors, and test-only accessors;
- integration-test source under `crates/core/tests`;
- generated code, dependencies, and the `git-outpost` CLI package.

Declarations without executable code contribute no executable lines. Missing
instrumentation for executable production code is a measurement defect, not an
exemption. Source-selection rules must be auditable and narrow enough that new
production code cannot silently disappear from the report.

Keep running unit and integration tests. Their execution of production code
counts; their own implementations do not. Do not drop library tests to avoid
classifying inline test code, or replace production behavior with test-only
behavior to improve coverage.

Production exemptions are separate from non-production source. They require
the evidence and approval described below and must remain visible in reports.
Rarity, difficulty, or a trivial implementation is not an exemption reason.

Each canonical result must record:

- the commit and any tested uncommitted changes;
- operating system, Rust target, profile, enabled features, and test targets;
- Rust and coverage-tool versions, exact commands, and report-processing rules;
- the production executable-line total before approved exemptions, exempted
  lines with reasons, and covered and uncovered eligible lines.

Eligible lines are production executable lines remaining after approved
exemptions. The eventual gate requires zero uncovered eligible lines, successful
tests, and a complete report. A rounded percentage, an empty denominator, or an
incomplete run is not evidence of 100%. LLVM line, function, region, and
source-map counts are different units; do not add or subtract them as if they
were interchangeable.

Function and branch coverage remain informational. Line coverage does not
require every branch of every executed line. Windows, macOS, and release-only
behavior remain separate compilation and behavioral checks; do not merge
mutually exclusive configurations into one percentage or claim that Linux/debug
proves them. Doctests run separately with `cargo test --doc`, and CLI contracts
remain owned by CLI integration tests.

## Test Admission And Construction

A permanent test must satisfy all four criteria:

1. **Independent wrongness.** The observed behavior would still be wrong if
   the test did not exist.
2. **Semantic evidence.** Assertions observe a module's result, effect,
   invariant, or documented representation contract rather than mirror its
   implementation.
3. **Marginal value.** The test protects a distinct behavior or failure mode
   not already established by a stronger test.
4. **Proportionality.** Setup, coupling, runtime, and maintenance cost are
   proportionate to the risk and value of the contract.

Coverage gain alone never justifies a test. Choose the owning test layer from
the architecture and the smallest fixture that establishes the behavior. Pure
parsers and transformations can use unit tests through their module interfaces;
repository workflows should use public core operations and real, hermetic Git
repositories. Use the shared `TempDir`-owned A/B/C fixture when needed: A is the
local bare upstream, B the source repository, and C the managed outpost. Not
every test needs all three repositories.

Prefer typed errors with meaningful stable payloads, refs and OIDs, worktree
files, persisted state, and documented reporter events. For failure and
read-only contracts, check the promised preserved or partial state as well as
the returned result. Avoid broad `is_err()` assertions when a specific outcome
matters, and avoid brittle operating-system diagnostic strings.

Text, ordering, representation, command, or call-count assertions are useful
when the observed property is itself a documented contract, including security
or compatibility requirements. Otherwise prefer observable outcomes over
private logs and implementation sequencing. For concurrent operations such as
`ops::list`, assert result semantics and documented output order, not worker
counts, scheduling, or elapsed time.

Invalid application-owned input is legitimate test material. Malformed Outpost
JSON remains such input even when stored inside `.git`. This does not justify
corrupting Git internals or fabricating private Git output merely to execute a
defensive parser branch. Do not expose a private parser solely for coverage.

Use small adapters at existing production seams according to their meaning:
`CleanupEvidenceProvider` acquires branch-cleanup evidence;
`MissingBranchAuthorizer` authorizes fetching a specified missing branch from
`origin`; `BranchCleanupPrompt` confirms branch deletion; and `Reporter` exposes
step and warning events. Adapters must represent credible inputs or decisions.
Recordings should establish the interface's requests, choices, or events;
operation outcomes and real repository effects remain the primary evidence.

A deterministic change to real Git state during a user-confirmation interface
can model a genuine concurrent change. It does not need a timing race. Do not
mutate state inside an observational provider callback just to force a later
branch, or invent impossible provider evidence.

Bounded fake executables are appropriate for direct `GitInvoker` process
mechanics: spawn failure, lossy stdout/stderr decoding, exit status, and signal
termination. Sentinel wrappers may detect prohibited commands for an explicit
no-network or routing contract while delegating allowed operations to real
Git. They must make violations observable to the test; merely returning an
error that production code can swallow is insufficient. Neither technique
may fabricate repository or remote-provider behavior.

Do not invent a generic Git executor, filesystem abstraction, or writer mock
merely to execute residual lines. A new seam requires the approval process
below, including its product guarantee and maintenance cost.

## Uncovered Code And Exceptions

Resolve gaps by behavior, not by the number of LLVM records:

1. Identify the behavior and why users or callers depend on it.
2. Look for an existing, safe way to exercise it. Add or improve a test only
   when it meets the admission criteria.
3. If testing requires source changes, present the guarantee, why current
   fixtures or interfaces are insufficient, the smallest proposed interface,
   its production role and maintenance cost, and the outcome to assert.
   Obtain approval before implementing it.
4. If a path is allegedly impossible, provide the invariant or construction
   proof. If the report is misleading, provide evidence of the specific
   measurement limitation. Propose a source simplification or narrow exemption
   for approval; do not treat an unsuccessful fixture search as proof.
5. Keep unresolved supported behavior visibly uncovered. Stop and report the
   remaining gaps when worthwhile safe tests are exhausted.

Difficulty, rarity, or triviality does not remove code from scope. Neither
`panic!` nor `expect` has a blanket exemption: deliberate misuse guards can be
meaningful contracts, while an invariant-protected failure requires a different
analysis. Inspect the executable-line mapping before assuming an untested
failure branch prevents line coverage.

Current core examples illustrate these distinctions:

| Behavior | Treatment |
|---|---|
| `shortest_unique_prefixes` rejects duplicate IDs. | An ordinary public rejection contract; no cryptographic collision is needed. |
| Its private helper expects distinct full IDs to have a unique prefix. | Analyze the fixed-length ID and duplicate-rejection invariants; do not bypass them to manufacture a panic. |
| A remote branch moves during removal confirmation. | Exercise real Git state at the prompt interface and verify the final deletion lease preserves the moved branch. |
| Metadata initialization encounters an existing document. | Test the no-clobber contract using ordinary files and verify the existing document is preserved. |
| `RegistryMut` is dropped with unsaved changes. | Its debug panic and release warning are deliberate, profile-specific behavior. |
| A late write or persist failure lacks a safe fixture. | Keep the supported failure in scope; request a justified source change rather than use a hazardous device. |

Every proposed production exemption must identify the exact source and affected
configuration, supply its rationale and evidence, record explicit approval,
and state what changes require reconsideration. Revisit it when the relevant
source, invariant, contract, or coverage mapping changes. Do not use broad
file, module, panic, or error-handler exclusions. Exempting a failure path must
not silently hide reachable behavior on the same source line. This policy
does not itself approve any production exemption.

## Safe Execution

Do not use the following to force coverage:

- special devices, including `/dev/full`;
- FIFOs, sockets, infinite streams, or other reads without a bounded EOF;
- permission-bit tricks to manufacture filesystem errors;
- sleeps, timing-dependent races, TOCTOU replacements, or concurrent directory
  swaps;
- corruption of Git internals or fabricated private Git protocol output;
- impossible provider evidence or mutation during an observational callback.

An unsafe fixture says nothing about whether the underlying failure matters.
Reading `/dev/full` does not reach EOF and can exhaust memory before an intended
write-failure assertion. It also does not model the crate's atomic temporary-file
write path.

Keep files and repositories in test-owned temporary directories. Thread Git
environment overrides through `_with` constructors; do not mutate process-global
current directories, environment variables, or `PATH`. Scrub inherited Git-routing
variables and use a trusted `PATH` in runners. Guard platform-specific behavior
with the appropriate `cfg`; avoid hard-coded executable paths, nonportable
snapshots, and unnecessary UTF-8 conversion of paths.

Before coverage or long-running test runs, set explicit wall-time, build/test
concurrency, and aggregate memory limits for the process tree. Select and record
limits appropriate to the host. A timeout or low worker count alone does not
bound rapid memory growth. Retain ownership of each running command and its
session identifier until completion; on cancellation or timeout, terminate its
unfinished descendants rather than abandon them.

Do not overlap coverage runs in shared artifact directories. Reports must use
the intended tree and configuration without stale or partial artifacts.
Interrupted runs, failed tests, and incomplete reports cannot satisfy the gate.

## Workflow And Enforcement Status

The current `cargo coverage-core` alias, ignoring its threshold, selects:

```text
cargo llvm-cov --package outpost-core --all-features --locked
```

This does not yet isolate the production denominator. The default selection
includes library tests, so inline test code is attributed to production source
files. `--all-features` enables `test-helpers`, which is also enabled by the
crate's self dev-dependency. `--tests` still includes library tests, and selecting
only integration targets would lose unit-test execution without removing the
self dev-dependency's instrumentation.

Changing only the numeric threshold does not implement this policy. Do not
claim 100% production coverage until an auditable denominator mechanism has
been validated. This document does not select a filtering mechanism, coverage
attribute, toolchain change, or test-source reorganization.

Proceed in this order:

1. Propose and obtain approval for the production-denominator mechanism, then
   validate that it includes production code and excludes only classified
   non-production source and approved exemptions.
2. Obtain a canonical bounded baseline with the required run metadata and
   line counts.
3. Add worthwhile behavioral tests using existing interfaces, reviewing their
   value independently of percentage gains. Run focused checks first.
4. Resolve remaining gaps through explicit source-change or exemption decisions,
   preserving unresolved gaps in reports. Re-run relevant platform/profile
   checks and the bounded coverage report after approved changes.
5. Enable the hard gate only when its measurement satisfies this contract and
   zero uncovered eligible lines is established by a complete successful run.

Explicit approval is required before modifying `crates/core/src`, adding or
changing a production seam, changing the denominator or exclusions, or changing
target, profile, feature, or metric requirements. Making function or branch
coverage a hard gate also requires approval. Approval of this policy does not
grant blanket approval for those implementation changes.
