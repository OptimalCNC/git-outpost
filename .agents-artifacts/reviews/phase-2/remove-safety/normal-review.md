**Verdict**: approved

**Evidence Reviewed**: Diff `06e5f77..HEAD`; current artifact workspace status; evidence pack; QA note; scope review; progress log; `docs/src/product.md`; `docs/src/architecture.md`; `docs/src/roadmap.md`; changed source/tests: `ops/remove.rs`, `ops/mod.rs`, `outpost.rs`, `safety.rs`, `tests/remove.rs`.

**Requirement Reasoning**: Roadmap Phase 2 includes `ops::remove` and R-01..R-11. Product docs require `remove` to refuse dirty, unpushed, and locked outposts by default, allow `--force`, remove registry entries, delete only managed outpost directories, and preserve source branches.

Architecture section 5.9.12 ordering is met: registry lookup first, registry lock check before missing-path cleanup, missing unlocked paths deregister without filesystem deletion, existing paths pass `check_path_is_managed_outpost_of`, non-force paths run dirty and unpushed guards, then registry removal/save precedes `remove_dir_all`.

R-01..R-11 are each represented by the named integration tests in `crates/core/tests/remove.rs`. The unpushed support is confined to `Outpost::unpushed_commits` and `safety::check_no_unpushed`, which are documented support points in architecture sections 5.5 and 5.8 and justified in the progress log.

**Test Reasoning**: The remove integration suite proves clean removal, dirty refusal, unpushed refusal, forced dirty/unpushed removal, unregistered-path refusal, missing-path deregistration, unrelated-directory protection, wrong-source protection, locked refusal/force behavior, and locked-missing behavior.

The tests also check registry state after success/failure and include sentinel/source-branch assertions for destructive cases. They do not prove CLI dispatch, contextual path omission, registry concurrency, or prune behavior; those are explicitly out of scope in the evidence pack and progress log.

**Docs Reasoning**: No docs changes were required. Existing product docs cover user-facing remove behavior; architecture sections 5.5, 5.8, 5.9.12, and 11.10 cover helper/API placement, safety behavior, command algorithm, and test inventory; roadmap Phase 2 covers scope. The no-docs rationale in the evidence pack and progress log satisfies the Documentation Policy without duplicating stable architecture text.

**Verification Reasoning**: Evidence records passing `cargo fmt --check`, `cargo test -p outpost-core --test remove`, `cargo test -p outpost-core`, `cargo test -p outpost-core --tests`, `cargo test --workspace`, `cargo test -p outpost-core --features test-helpers`, and `git diff --check`. QA note records the QA worker and coordinator reruns for the remove suite.

**Findings**: none

**Missing Evidence**: none

**Required Changes**: none

**Notes**: none
