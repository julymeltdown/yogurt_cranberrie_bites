# Task 4 - clean-candidate verification

Candidate: `501710d293e5309e862997a353714e5753a76da0` (`origin/master` after the fast-forward push)
Base: `9185c99223bbf7437fff4346a7683dbb4187dfb0`
Executed: 2026-11-06, integration checkout `/Users/lhs/Desktop/mcr`, working tree at the pushed commit.

No product code was edited during this task. Every exit code below is the command's own exit status, captured with `$?`/`PIPESTATUS[0]`, not inferred from stdout.

## Checks

| # | Command | Exit | Decisive output |
| --- | --- | --- | --- |
| 1 | `python3 scripts/mcr/check-plan.py` | 0 | plan table JSON with input hashes; `application_build`/`application_tests`/`deployment` reported `NOT_RUN` by the checker (separate obligations) |
| 2 | `bash scripts/mcr/check.sh guards` | 0 | `{"task": "C08", "result": "PASS", "violations": [], "files_scanned": 5863, "duration_ms": 313.9}` |
| 3 | `python3 scripts/mcr/generate-api-client.py --check` | 0 | `{"operations": 150, "outputs": 18, "result": "PASS"}` |
| 4 | `python3 scripts/mcr/generate-domain-transport.py --check` | 0 | `Domain transport metadata current: 140 operations` |
| 5 | `python3 scripts/mcr/contracts/c07/test_inventory.py` | 0 | `Ran 4 tests ... OK` |
| 6 | `python3 scripts/mcr/contracts/qa01-coverage/run.py` | 0 | `{"task": "QA01", "result": "PASS", "candidate": "501710d293e5309e862997a353714e5753a76da0", "fixtures": 198, "responses": 1727, "operations": 150}` |
| 7a | `export PATH=/Users/lhs/.local/node-v24.22.2/bin:$PATH; cd frontend && corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b --pretty false`, no diagnostics |
| 7b | `corepack pnpm@11.7.0 run build` | 0 | `✓ built in 1.19s` |
| 7c | `corepack pnpm@11.7.0 run lint` | 0 | warning-level `unicorn(no-thenable)` on generated `src/generated/api-client/document.ts` only |
| 7d | `corepack pnpm@11.7.0 run test` | 0 | `Test Files 411 passed (411)`, `Tests 5683 passed | 14 skipped (5697)`, duration 31.88s |
| 8 | `cd backend && CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/dev-target CARGO_BUILD_JOBS=4 cargo +1.88.0 check --locked --workspace --all-targets` | 0 | `Finished dev profile ... ` with dead-code/unused warnings in `concert-application` test targets only |
| 9 | `git status --porcelain --untracked-files=no` | 0 | empty output (release-checks `dirty=false` equivalent) |

Node runtime for 7a-7d: `v24.22.2`.

## release_checks
`python3 scripts/mcr/release_checks/check.py --help` exits 0. The checker has no zero-input mode: `--candidate` (a full 40-hex SHA) is required and it evaluates an evidence manifest/QA10 receipts. Running it anyway:

```
python3 scripts/mcr/release_checks/check.py --candidate 501710d293e5309e862997a353714e5753a76da0
-> exit 2, status INCOMPLETE, release_status INCOMPLETE, receipts [], source_checks NOT_RUN
```

That INCOMPLETE is the expected "no QA10 evidence bundle yet" result, not a candidate defect; its dirty-tree rule (`candidate.worktree-dirty`) is covered by check 9, which is empty. The full source/composition mode requires the task 47 release evidence that does not exist yet.

## Adversarial probes
- dirty_worktree: PASS. `git show --name-only` per commit contains only allowlisted paths (107 files in 8e5cc757f, 2 in 8fc507fe9, 1 deletion in 501710d29). After the push `git status --porcelain` shows only `?? .omo/` and the 12 pre-existing `?? docs/mcr/evidence/delivery-2026-11-04/*` files, which were neither staged nor committed.
- stale_state: PASS. `git fetch origin` immediately before push read `origin/master = 9185c99223bbf7437fff4346a7683dbb4187dfb0`, `git merge-base --is-ancestor origin/master HEAD` succeeded, push output `9185c9922..501710d29 master -> master`, and `git rev-parse origin/master` read back `501710d29` afterwards.
- misleading_success_output: PASS. Each of the nine checks recorded its own exit status separately from stdout; two cases that could mislead are named: release_checks prints structured JSON but exits 2, and `check-plan.py` prints a full plan table while reporting three sub-obligations as `NOT_RUN`.

## Starting-state note
Expected 97 modified / 9 deleted / 17 untracked. Observed exactly that for the product tree; `git status --porcelain` additionally lists `?? .omo/` (the plan/evidence directory this task reads and writes), making the raw untracked count 18. No unexpected product path appeared, so the run continued under that recorded difference.

## Cleanup
None - no processes, services, databases or containers were started. Only `/tmp/fe-{typecheck,build,lint,test}.log` and the pre-existing cargo target dir were written.
