# P5-C3 Exit Color Platform Hardening QA Note

## Coverage

- E-07: copies C to `C-copy` with Rust filesystem APIs, deletes B, verifies `git status`, `git log`, `git diff HEAD~1`, and `git checkout -b new-branch` succeed in the copy, then checks `gop status` reports `source-present: false`, `health: problems`, and `source missing:`.
- E-08: enumerates every `OutpostError` variant and expected documented exit code, including Git exit-code clamping; also runs focused black-box CLI broken states with stderr assertions for every variant the CLI can realistically surface.
- E-09: verifies `gop --no-color status` and `NO_COLOR=1 gop status` contain no ANSI escape bytes on stdout or stderr.

## Fixture Notes

- Recursive copy avoids shell-specific tools and preserves symlinks on Unix and Windows when possible.
- ANSI assertions deliberately reject any ESC byte, which is stricter than matching only CSI color sequences.
- `GitTerminatedBySignal` remains table-driven because reliably forcing a child `git` process to terminate by signal through the CLI would make the test brittle. Reachable variants use CLI broken-state fixtures.
