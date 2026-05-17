# Normal Review: list-ahead-behind

- Verdict: approved
- Review scope/range: `/home/huwei/projects/git-outpost`, `a2ec631..HEAD`; HEAD verified as `d5280eb phase-1: add list ahead behind`.

## Findings

none

## Test / Verification Notes

- Reviewed source docs, progress log, evidence pack, QA artifact, and diff.
- `cargo fmt --check`: pass
- `cargo test -p outpost-core --test list`: pass, 11 tests
- `cargo test -p outpost-core`: pass
- `git diff --check a2ec631..HEAD`: pass

## Scope Notes

- Implementation stays within chunk scope: `outpost.rs`, `ops/list.rs`, list tests, fixture helper, and review artifacts.
- Ahead/behind is computed from the outpost's current branch against its configured upstream branch, requires the upstream remote to match `outpost.remoteName`, fetches that source branch into `refs/remotes/<remote>/<branch>`, then compares `refs/heads/<branch>...refs/remotes/<remote>/<branch>`.
- `ops::list` maps unavailable ahead/behind computation to `None` via `.ok()`, so list does not crash on no upstream, detached/missing branch, or unavailable counts.

Approval conditions: none.
