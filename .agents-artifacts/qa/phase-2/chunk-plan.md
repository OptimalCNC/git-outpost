# Chunk Plan: Phase 2

- Verdict: `ready_with_cautions`

## Recommended Chunks

### P2-C1: Lock / Unlock / Move

- Scope: implement `ops::lock`, `ops::unlock`, `ops::move`; wire modules through `ops::mod`.
- Files likely touched:
  - `crates/core/src/ops/lock.rs`
  - `crates/core/src/ops/unlock.rs`
  - `crates/core/src/ops/move.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/tests/lock_move_unlock.rs`
  - possible narrow fixture helpers in `crates/core/tests/common/**`
- Test IDs covered: LMU-01..LMU-08
- Dependencies: Phase 1 registry mutators, source/outpost discovery, `safety::check_clean`, `safety::check_path_is_managed_outpost_of`, `safety::check_destination_clean`.
- Review risks:
  - lock/unlock must reject unregistered or wrong-source paths before mutating registry
  - `move --force` bypasses dirty/lock guards but still validates managed outpost and destination safety
  - registry path update preserves lock fields and metadata
  - `std::fs::rename` failure leaves registry unchanged

### P2-C2: Remove + Unpushed Safety Support

- Scope: implement `ops::remove` and minimal unpushed-commit safety helpers.
- Files likely touched:
  - `crates/core/src/ops/remove.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/src/outpost.rs`
  - `crates/core/src/safety.rs`
  - `crates/core/tests/remove.rs`
  - possible narrow unit tests and fixture helpers
- Test IDs covered: R-01..R-11
- Dependencies: P2-C1 lock behavior; Phase 1 registry removal and managed-path gate.
- Review risks:
  - operation order: registry lookup and lock check before missing-path cleanup
  - force bypasses dirty/unpushed/lock only, not managed-outpost validation
  - tampered registry entries never reach `remove_dir_all`
  - unpushed detection stays scoped to remove safety and avoids Phase 4 sync behavior

### P2-C3: Prune

- Scope: implement `ops::prune` with structured `PruneReport`.
- Files likely touched:
  - `crates/core/src/ops/prune.rs`
  - `crates/core/src/ops/mod.rs`
  - `crates/core/tests/prune.rs`
  - possible fixture helpers
- Test IDs covered: Pr-01..Pr-09
- Dependencies: P2-C1 lock semantics; Phase 1 registry load/save.
- Review risks:
  - classification order: locked first, missing second, source-missing managed outpost third, otherwise keep
  - never delete real directories or source branches
  - existing unrelated directories and wrong-source outposts remain registered
  - dry-run reports without saving registry changes

## Docs Expectations

No product, architecture, roadmap, or README edits are expected. Source docs already define Phase 2 behavior.

## Remaining Risks / Cautions

- `Outpost::unpushed_commits` and `safety::check_no_unpushed` are required for R-03/R-05; keep them narrowly justified as remove safety support.
- No CLI binary tests, E2E tests, global `-C`, contextual CLI behavior, Phase 3+ status, Phase 4 sync commands, registry file locking, or concurrency work.
- Destructive tests must stay inside fixture temp roots and assert preservation of unrelated directories/source branches.

## Required Human Decisions

none

## Recommended First Chunk

Start with `P2-C1: Lock / Unlock / Move`.
