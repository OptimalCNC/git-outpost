**Verdict**

Approved with nits.

**Scope Compliance**

The commit stays within `P5-C1-cli-surface`: it adds `crates/cli`, workspace
membership, `git-outpost` and `gop` binaries, the clap command tree, help
tests, deferred/removed flag rejection tests, and initial CLI test helpers.

No core command semantics, real dispatch, E2E behavior, output/color hardening,
or broader exit-code matrix work was added.

**Protected Paths**

None configured. No protected-path violation found.

**Forbidden Scope**

No forbidden scope found. Changed files are limited to workspace
metadata/lockfile, the new CLI crate/tests, and Phase 5 progress/QA/review
artifacts.

**Required Changes**

None.

**Nits**

- `.agents-artifacts/progress/phase-5.md` still says the
  implementation/evidence commit is `pending` even though this review targets
  commit `00f48c7`.
- H-03 is evidenced with `git outpost -h` because Git 2.43 intercepts
  `git outpost --help`. The deviation is recorded, but it is worth carrying
  forward so later reviewers do not treat it as exact `--help` coverage.

**Evidence Reviewed**

- Commit `00f48c7 phase-5: add cli surface`
- Evidence pack: `.agents-artifacts/reviews/phase-5/P5-C1-cli-surface/evidence-pack.md`
- Progress log: `.agents-artifacts/progress/phase-5.md`
- Source docs: `docs/src/product.md`, `docs/src/architecture.md`,
  `docs/src/roadmap.md`, `docs/coordinator-prompt.md`
- Changed paths: `Cargo.toml`, `Cargo.lock`, `crates/cli/**`,
  `.agents-artifacts/progress/phase-5.md`,
  `.agents-artifacts/qa/phase-5/P5-C1-cli-surface.md`, evidence pack
- Evidence claims reviewed: `cargo fmt --check`,
  `cargo build -p git-outpost`, `cargo test -p git-outpost --tests`,
  `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`,
  `cargo test --workspace`, `git diff --check` all recorded as passing.
