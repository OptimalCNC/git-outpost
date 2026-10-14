
# GUIDELINES

## Think Before Acting

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before acting:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.
- Understand the user's intent, and ask clarifying questions if needed.

## Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing content,

- Don't "improve" adjacent content, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- Don't do things that are not asked to be done silently, mention them.

The test: Every changed line should trace directly to the user's request.

## Core Test Development

Before changing tests, coverage configuration, or exclusions for `crates/core`,
read and follow
[Core Coverage And Test Policy](docs/src/core-coverage-and-test-policy.md).

- Treat 100% as production executable line coverage under the approved target,
  profile, feature set, and source classification. Use the canonical
  `tools/core-coverage/run.py` measurement.
- Admit permanent tests only when they satisfy independent wrongness, semantic
  evidence, marginal value, and proportionality.
- Use the smallest safe fixture that establishes the behavior. Prefer real,
  hermetic repositories and observable outcomes; use existing seams according
  to their production meaning.
- Assess behavioral obligations, safe testability, and measurement scope
  separately. Keep unresolved supported behavior visibly uncovered; difficulty
  alone does not justify an exemption.
- Follow the policy's fixture restrictions and process-tree resource limits.
  Never force coverage with hazardous fixtures or abandon running commands.
- Obtain explicit approval before modifying `crates/core/src`, introducing a
  production seam, or changing the coverage denominator, exclusions, or
  target/profile/feature/metric requirements.
