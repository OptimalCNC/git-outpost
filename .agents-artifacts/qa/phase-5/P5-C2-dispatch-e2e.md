# P5-C2 Dispatch E2E QA Note

## Coverage

- E-02: all three invocation forms produce identical status stdout.
- E-04: add/status/push/list/remove CLI lifecycle exits zero.
- E-05: `gop push` publishes the outpost commit to upstream A.
- E-06: two outposts sync through source via push/pull.
- E-10: Story flow with `add -b`, `source pull`, `rebase`, and `push` exits zero.
- E-11: `merge local/main` and `rebase local/main` accept the Story source-ref form.
- E-12: global `-C` changes status cwd and roots relative path args at the effective cwd.
- E-14: leading-dash target branch is rejected as `InvalidRefName` before Git subprocess execution.
- Additional matrix coverage: list from outpost matches list from source; lock/unlock work from source with explicit relative paths and from outpost with omitted path; move/prune dispatch from source succeeds; representative wrong-context commands fail with `OutpostError` exit code 2.

## Fixture Notes

- The CLI fixture creates A as a bare upstream, B as a normal source clone, and C/C1/C2 as sibling outposts outside B.
- Hermetic Git env uses empty global/system config files plus fixed author/committer values.
- Git dispatch tests set `PATH` so `git outpost` resolves the built `git-outpost` binary.

## Remaining Phase 5 QA

- E-07 copied outpost degradation and Git independence.
- E-08 exhaustive `OutpostError` exit-code behavior at the CLI edge.
- E-09 `--no-color` and `NO_COLOR=1` ANSI stripping assertions.
