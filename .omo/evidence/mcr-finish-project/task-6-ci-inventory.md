# Task 6 - CI gate inventory (write-only)

Date: 2026-11-06
Workdir: `/Users/lhs/Desktop/mcr` (master at `501710d293e5309e862997a353714e5753a76da0`)
Deliverable: appended section "CI gate inventory (task 6, write-only)" in `docs/mcr/execution-plan.md`.

## Constraint proof

- `ls .github/workflows` -> `No such file or directory` (exit 1). No workflow file exists and none was created or modified by this task.
- The contract-only workflow deleted at task 3 (`501710d29 ci: remove contract-only workflow pending full gate`) is not reintroduced.
- Task 48 owns creating `.github/workflows/mcr-ci.yml` from this inventory.

## Inventory contents

15 rows, each naming the job, the exact command, the working directory, the blocking condition and the state at `501710d29`:

| # | Job | Command | State at candidate |
| --- | --- | --- | --- |
| 1 | plan | `python3 scripts/mcr/check-plan.py` | exit 0 |
| 2 | guards | `bash scripts/mcr/check.sh guards` | exit 0, PASS, 5863 files |
| 3 | generated drift | `python3 scripts/mcr/generate-api-client.py --check` | exit 0, PASS |
| 4 | generated drift | `python3 scripts/mcr/generate-domain-transport.py --check` | exit 0 |
| 5 | contracts | `python3 scripts/mcr/contracts/c07/test_inventory.py` | exit 0, 4 tests OK |
| 6 | contracts | `python3 scripts/mcr/contracts/qa01-coverage/run.py` | exit 0, PASS |
| 7 | frontend install | `corepack pnpm@11.7.0 install --frozen-lockfile` | NOT_RUN (task 48 must prove before blocking) |
| 8 | frontend typecheck | `corepack pnpm@11.7.0 run typecheck` | exit 0 |
| 9 | frontend build | `corepack pnpm@11.7.0 run build` | exit 0 |
| 10 | frontend lint | `corepack pnpm@11.7.0 run lint` | exit 0 (generated-file warning only) |
| 11 | frontend test | `corepack pnpm@11.7.0 run test` | exit 0, 411 files / 5683 passed |
| 12 | backend check | `cargo +1.88.0 check --locked --workspace --all-targets` | exit 0 |
| 13 | backend test | `cargo +1.88.0 test --locked --workspace` | NOT_RUN (task 48 captures first run) |
| 14 | release checks | `python3 scripts/mcr/release_checks/check.py --candidate <sha>` | exit 2, INCOMPLETE by design; non-blocking until task 47 |
| 15 | dirty tree | `git status --porcelain --untracked-files=no` | empty |

Source for the recorded states: `.omo/evidence/mcr-finish-project/task-4-candidate.md` (same-candidate command run at `501710d29`).

Non-blocking rows with reasons (plan QA scenario rule):

- Row 7 frontend install: not executed at task 4 because dependencies were preinstalled; blocking from task 48 once the first frozen install is green.
- Row 13 backend test: task 4 ran `cargo check` only; blocking from task 48 once the first full workspace test run is captured.
- Row 14 release checks: requires the QA10 receipt bundle that task 47 publishes; `INCOMPLETE`/exit 2 before then is the documented outcome, not a candidate defect.

Future gates are named in the same section and remain owned by their task: route parity (task 12), venue suites (22-23), IN02/IN04 real-stack gates (29, 31), QA suites (38-42). Task 6 does not create them.

## Verification

| Command | Exit | Result |
| --- | --- | --- |
| `git status --porcelain --untracked-files=no` (before commit) | 0 | only `docs/mcr/execution-plan.md` modified |
| `python3 scripts/mcr/check-plan.py` | 0 | plan table JSON |
| `ls .github/workflows` | 1 | `No such file or directory` (expected: no workflow added) |
| `git show --stat HEAD` (after commit 2) | 0 | exactly `docs/mcr/execution-plan.md` |

## Adversarial probes

- dirty_worktree: PASS. Commit 2 contains only `docs/mcr/execution-plan.md`; commit 1 contains only `docs/mcr/salvage-decisions.md`. `.omo/` evidence and the pre-existing untracked `docs/mcr/evidence/delivery-2026-11-04/*` files are not staged.
- misleading_success_output: PASS. Rows 14 and 1 are explicitly recorded as "exit 2 by design / NOT_RUN sub-obligations" so a reader cannot mistake structured JSON for a pass.
- stale_state: PASS. `git fetch origin` immediately before each push re-checked `origin/master`; the inventory's candidate SHA is the pushed `501710d29`.

## Delivery

- Commit: `0dfbf3d8bd7c53cc43273a9f73d48a3ab502175e` `docs(mcr): inventory blocking CI gates`
- `git show --stat HEAD` at delivery: 1 file changed, `docs/mcr/execution-plan.md` (33 insertions).
- Push: `49ac04156..0dfbf3d8b master -> master` (fast-forward, `origin/master` re-read `0dfbf3d8b`).
- Pre-push check: `origin/master` was `49ac04156` (commit 1), i.e. still descended from the required base `501710d29`; no foreign commits appeared.

