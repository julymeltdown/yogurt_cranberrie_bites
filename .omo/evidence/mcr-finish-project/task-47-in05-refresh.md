# Task 47 - IN05 snapshot refresh at the F5 candidate

- Original snapshot: `task-47-in05.json` / `task-47-in05.md`, candidate
  `7d14251bb`, generated 2026-11-06T18:17:43Z. Preserved byte for byte.
- Refresh: `task-47-in05-refresh.json`, candidate
  `dcac6b20d05c3aff8ab5bea6516d22a1c59fc972`, generated 2026-11-06T18:54:57Z.
- Generator: `task-47-in05-check.py` executed in memory with the candidate and
  output paths changed; no source file was edited.
- Consumer: `.omo/evidence/mcr-finish-project/f5-release.md` (F5 release audit).

## What changed

| Measure | Original (`7d14251bb`) | Refresh (`dcac6b20d`) |
| --- | --- | --- |
| Row 47 | OPEN (delivering commit landed later) | DONE, `7bbcba836` |
| Row 48 | OPEN; evidence path unresolved | DONE, `bbf9da478`; evidence exists |
| Plan rows DONE | 35 | 40 |
| Plan rows GAP | 14 | 15 (14 implementation rows plus F3) |
| Plan rows OPEN | 7 | 1 (F5) |
| Closed-row failures | 0 | 0 |
| Register mix | 65 DONE / 39 IN_PROGRESS / 3 OPEN / 3 WAITING_INPUT / 1 REOPENED / 1 DEFERRED_BY_USER | unchanged |
| Release decision | `NOT_RELEASE_READY` | `NOT_RELEASE_READY` |

## Checker-scope notes

- `gap_14` reports FAIL in the refresh because the checker asserts exactly 14
  gap rows across the 56-row plan and F3 is now a 15th (`[~]`) audit row. The
  implementation gap set is exactly rows 11, 14, 16, 28, 30, 32, 35, 36, 37,
  41, 42, 43, 45, 51.
- `qa08.status: PASS` in the refresh is the deferral predicate (the task is
  correctly `DEFERRED_BY_USER` with no evidence). QA08 itself remains
  `DEFERRED_BY_USER` / `NOT_RUN`; no capacity claim is made.
- The refresh does not rewrite the original snapshot's historical gap quotes;
  F5 carries the current gap text and owners in `f5-release.md`.
