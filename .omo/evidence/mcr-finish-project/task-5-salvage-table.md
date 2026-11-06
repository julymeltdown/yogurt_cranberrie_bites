# Task 5 - salvage recover-or-reimplement decision table

Date: 2026-11-06
Workdir: `/Users/lhs/Desktop/mcr` (master at `501710d293e5309e862997a353714e5753a76da0`)
Deliverable: `docs/mcr/salvage-decisions.md`
Mode: read-only inspection of salvage refs; no merge, cherry-pick, ref deletion or worktree removal of existing checkouts.

## Inputs

- `docs/mcr/task-progress-2026-11-04.json` - 112 tasks; remaining set = 48
  (`IN_PROGRESS` 40, `WAITING_INPUT` 3, `REOPENED` 2, `OPEN` 3; `DEFERRED_BY_USER` QA08 excluded).
- `/Users/lhs/Desktop/mcr-salvage-20261106/branches.tsv` - 86 branch/SHA rows.
- `/Users/lhs/Desktop/mcr-salvage-20261106/README.md`.
- `refs/salvage/20261106/*` - 86 local refs, present and resolved locally.
- `docs/mcr/progress.md` - current disposition text for the remaining rows.

## Commands and results

| # | Command | Exit | Output |
| --- | --- | --- | --- |
| 1 | `git for-each-ref --format='%(refname)' refs/salvage/20261106/ \| wc -l` | 0 | `86` |
| 2 | `git for-each-ref --format='%(refname) %(objectname)' refs/salvage/20261106/` | 0 | full 40-hex SHA per ref; recorded in the table as 12-hex prefixes, full SHAs remain in `branches.tsv` |
| 3 | `git cherry origin/master <ref> \| grep -c '^+'` (per cited ref) | 0 | unique patch counts used in the "Unique patches" column |
| 4 | `git diff --name-only origin/master...<ref>` grouped by top-level path | 0 | "Touched top-level areas" column |
| 5 | `git merge-tree --write-tree --name-only origin/master <ref>` (86 refs) | mixed | 20 refs clean (exit 0), 66 with conflicts; conflict-path counts recorded per row |
| 6 | `git rev-list --left-right --count origin/master...<ref>` (86 refs) | 0 | behind/ahead counts recorded per row |
| 7 | `python3 scripts/mcr/check-plan.py` | 0 | plan table JSON; exit 0 after the document was written |
| 8 | row counter (python, register vs table) | 0 | `rows 48 unique 48`, `missing []`, `extra []`, `RECOVER 0`, `REIMPLEMENT 48` |

## Isolated-worktree checks (RECOVER gate)

Only three refs could be checked inside the 30-minute bound. Each worktree was a detached checkout of the resolved ref tip.

### FE24 - `codex/mcr-fe-FE24-public-place-options-53d` @ `5de898eb348b` (full `5de898eb348be9199f7a0f114d831e2418d3eb41`)

```
git worktree add --detach /Users/lhs/Desktop/mcr-worktrees/salvage-check-fe24 refs/salvage/20261106/codex/mcr-fe-FE24-public-place-options-53d
cd frontend
corepack pnpm@11.7.0 install --frozen-lockfile --offline        -> exit 0 ("Done in 1.3s using pnpm v11.7.0")
corepack pnpm@11.7.0 exec vitest run src/features/gift-discovery/__tests__/public-place-options.test.ts
  -> exit 0
  Test Files  1 passed (1)
  Tests  28 passed (28)
```

Verdict REIMPLEMENT: the check passes and `git merge-tree` is clean, but the branch re-adds only the already-credited public-place caller slice. FE24's remaining scope (`docs/mcr/progress.md` line 291, register `remaining_work`) is authorized production media/motif inputs, which the branch does not touch.

### BE08 - `codex/mcr-be-BE08` @ `caee1a70f882` (full `caee1a70f882abfaf7a4cb7f635847d4b9bcb1ed`)

```
git worktree add --detach /Users/lhs/Desktop/mcr-worktrees/salvage-check-be08 refs/salvage/20261106/codex/mcr-be-BE08
cd backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/dev-target cargo +1.88.0 test --locked -p concert-domain --test place -p concert-application --test place
  -> exit 0
  concert-application tests/place.rs: 5 passed; 0 failed
  concert-domain     tests/place.rs: 4 passed; 0 failed
```

Verdict REIMPLEMENT: checks pass, but `git merge-tree` against `origin/master` fails with 5 conflicting paths (`concert-application/src/lib.rs`, `concert-domain/src/lib.rs`, `concert-service/Cargo.toml`, `concert-service/src/adapters/out/repository.rs`, `.../repository/place.rs`) and the tip is 2972 commits behind. The recorded spatial-release handoff needs re-implementation on the current descriptor.

### BE33 - `codex/mcr-be-BE33-notification-production` @ `5db23e373b5e` (full `5db23e373b5e35443aeb8c3ad0bb82d7b331fe24`)

```
git worktree add --detach /Users/lhs/Desktop/mcr-worktrees/salvage-check-be33 refs/salvage/20261106/codex/mcr-be-BE33-notification-production
cd backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/dev-target cargo +1.88.0 test --locked -p concert-service --test notification_projection_routing
  -> exit 101
  test result: FAILED. 0 passed; 5 failed
  Error: owned fixture root required    (all five tests)
```

Verdict REIMPLEMENT: the task's relevant check fails on the branch.

## Summary counts

- Remaining tasks: **48**.
- RECOVER: **0**.
- REIMPLEMENT: **48**.

## Cleanup receipts

```
git worktree remove --force /Users/lhs/Desktop/mcr-worktrees/salvage-check-fe24   -> exit 0
git worktree remove --force /Users/lhs/Desktop/mcr-worktrees/salvage-check-be08   -> exit 0
git worktree remove --force /Users/lhs/Desktop/mcr-worktrees/salvage-check-be33   -> exit 0
git worktree list
  /Users/lhs/Desktop/mcr                        501710d29 [master]
  /Users/lhs/Desktop/mcr-worktrees/dev-backend  9185c9922 [codex/mcr-dev-backend]
  /Users/lhs/Desktop/mcr-worktrees/dev-frontend 9185c9922 [codex/mcr-dev-frontend]
ls /Users/lhs/Desktop/mcr-worktrees
  dev-backend  dev-frontend
```

No temporary worktree survives; the two pre-existing developer checkouts were not touched.

## Adversarial probes

- stale_state: PASS. Every cited ref was resolved to a SHA with `git for-each-ref --format='%(refname) %(objectname)'`; no branch name is cited without its resolved SHA. Three tips were additionally checked out by SHA-detached worktree.
- misleading_success_output: PASS (3 reproductions above, all with captured test-level output, not only exit codes). Two cases where exit status and apparent success could mislead are named in the table: FE24 passes 28/28 but fails the remaining-scope test; BE08 passes 9/9 but fails the merge-applicability test.
- dirty_worktree: PASS. This task wrote only `docs/mcr/salvage-decisions.md` and `.omo/evidence/mcr-finish-project/task-5-salvage-table.md`; no salvage ref, branch or existing worktree was modified.
- Timebox: the 30-minute verification bound was reached after three checks; unverified rows are recorded as REIMPLEMENT as the plan row requires.

## Delivery

- Commit: `49ac04156f0832c67b5452fc6be83ed1235cc30d` `docs(mcr): record salvage recover-or-reimplement decisions`
- `git show --stat HEAD~1` at delivery: 1 file changed, `docs/mcr/salvage-decisions.md` (94 insertions).
- Push: `501710d29..49ac04156 master -> master` (fast-forward, `origin/master` re-read `49ac04156`).

