# Task 48 - blocking CI workflow

Date: 2026-11-06
Workdir: `/Users/lhs/Desktop/mcr-worktrees/exec-w8-ci` (branch `codex/mcr-exec-w8-ci`)
Base: `ef02302d41ec303054965c540de700861000d315` (`origin/master`)
Deliverable: `.github/workflows/mcr-ci.yml`, created from the task 6 inventory in
`docs/mcr/execution-plan.md` ("CI gate inventory (task 6, write-only)") and
`.omo/evidence/mcr-finish-project/task-6-ci-inventory.md`.

The workflow has four jobs, all triggered on `push` and `pull_request`, all
blocking. No job or step uses `continue-on-error`; the only non-failing path is
the release job's documented `INCOMPLETE` notice, explained below.

## Job table

| Job | Trigger | Steps (inventory row) | Blocking condition |
| --- | --- | --- | --- |
| `guards` | push, pull_request | `python3 scripts/mcr/check-plan.py` (1); `bash scripts/mcr/check.sh guards` (2, boundaries/architecture); `generate-api-client.py --check` (3); `generate-domain-transport.py --check` (4); `contracts/c07/test_inventory.py` (5); `python3 scripts/mcr/contracts/route-parity.py`; `git diff --check` | Any non-zero exit of any step |
| `frontend` | push, pull_request | `corepack pnpm@11.7.0 fetch --frozen-lockfile` then `corepack pnpm@11.7.0 install --frozen-lockfile --offline` (7); `run typecheck` (8); `run lint` (10); `run build` (9); `vitest run` with the three named exclusions (11); collected-file floor 408 (row 11 rule); `git status --porcelain --untracked-files=no` empty (15) | Any non-zero exit, fewer than 408 collected test files, or a tracked-file rewrite |
| `backend` | push, pull_request, `postgres:16` service | `cargo +1.88.0 check --workspace --all-targets --locked` (12); `cargo test --locked` for the 16 core domain/application/policy crates (13); tracked-tree check (15) | Any non-zero exit or failed test target |
| `release` | push, pull_request | `bash scripts/mcr/release-check.sh --candidate "$GITHUB_SHA"` (14) | Exit 1 (FAIL) always fails; exit 0 passes; exit 2 with `"status": "INCOMPLETE"` emits a notice and passes until the QA10 manifest is wired |

Pinned versions: Node `24.22.2` (`.node-version`; the vitest count depends on it),
corepack `0.35.0`, pnpm `11.7.0` (`frontend/package.json` `packageManager`), Rust
`1.88.0` (`backend/rust-toolchain.toml`), PostgreSQL `16`. The pnpm store is
cached by `actions/cache` keyed on `frontend/pnpm-lock.yaml`; the Rust target is
cached by `Swatinem/rust-cache` (which hashes `backend/Cargo.lock`).

## Exclusions, each with its reason (also in the workflow comments)

| Exclusion | Reason |
| --- | --- |
| C07 pin: `frontend/src/shared/api/__tests__/c07-social-client/compatibility.test.ts` | Compares `docs/mcr/evidence/c07-social-client-in03/inputs.json` against the generated client hash; the recorded pin predates the current `domain-operations.generated.ts`, so it fails on a stale record, not a regression. Observed red locally (see below). |
| `src/app/operations-composition/__tests__/composition.test.tsx` and `src/app/operations-composition/__tests__/reports-production/composition.test.tsx` abort tests | Pre-existing timing flakes; the suite measured 53/54 then 54/54 across repeated runs (`task-26-invitations.md`). Excluded instead of retried so a flake cannot mask a real failure. |
| `services/concert-service` lib tests needing `NB01_OWNER_ROOT` / `CONCERT_CONSUMER_TEST_URL` | `notification_binding` tests and the consumer probe panic before reaching product code without runner-owned roots (`task-blocked-feed.md`). The backend job therefore runs the core crates only. |
| `scripts/mcr/contracts/qa01-coverage/run.py` (inventory row 6) | Not wired yet: `pins.json` pins canonical candidate `9185c9922` and the checker reports `pinned input drift: ['frontend/src/shared/api/domain-operations.generated.ts']` at the current candidate. Red today for the same stale record as the C07 pin; becomes blocking when the pin is reconciled. |

No exclusion is expressed as a warning, `continue-on-error`, or a skipped assert;
excluded tests are not selected at all, and the excluded reason is written next
to the command.

## Local validation (Node `v24.22.2`, Rust `+1.88.0`, PostgreSQL not used)

| Command | Exit | Decisive output |
| --- | --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0 | plan table JSON with input hashes |
| `bash scripts/mcr/check.sh guards` | 0 | 46 unittest cases OK (1 skipped), 14 `node:test` cases pass, `{"task":"C08","result":"PASS","violations":[],"files_scanned":5909}` |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=245 commands=70 missing=0` |
| `python3 scripts/mcr/generate-api-client.py --check` | 0 | `{"operations":150,"outputs":18,"result":"PASS"}` |
| `python3 scripts/mcr/generate-domain-transport.py --check` | 0 | `Domain transport metadata current: 140 operations` |
| `python3 scripts/mcr/contracts/c07/test_inventory.py` | 0 | `Ran 4 tests ... OK` |
| `python3 scripts/mcr/contracts/qa01-coverage/run.py` | 1 | `"result": "FAIL"`, `"error": "ValueError: pinned input drift: ['frontend/src/shared/api/domain-operations.generated.ts']"` (pre-existing; row 6 not wired) |
| `git diff --check` | 0 | empty |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` (frontend) | 0 | `Already up to date; Done in 164ms using pnpm v11.7.0` |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b --pretty false`, no diagnostics |
| `corepack pnpm@11.7.0 run lint` | 0 | warning-level `unicorn(no-thenable)` on generated `src/shared/api/generated/document.ts` only |
| `corepack pnpm@11.7.0 run build` | 0 | `✓ built in 1.95s` |
| `corepack pnpm@11.7.0 exec vitest run src/app/__tests__/routes.test.ts` | 0 | 1 file, 27 tests passed |
| `vitest run` with the three exclusion flags + `src/app/__tests__/routes.test.ts` | 0 | 1 file, 27 tests passed (flags accepted) |
| `vitest run` on the three excluded files, for the record | 1 | `Test Files 1 failed \| 2 passed (3)`, `Tests 1 failed \| 63 passed (64)`; only C07 failed (hash pin), both composition files passed this run |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w8-ci cargo +1.88.0 check --workspace --all-targets --locked` | 0 | `Finished dev profile ... in 0.79s` (warnings in `concert-application` test targets only) |
| `CARGO_TARGET_DIR=... cargo +1.88.0 test --locked -p concert-domain --lib` | 0 | `test result: ok. 15 passed; 0 failed` |
| `bash scripts/mcr/release-check.sh --candidate ef02302d4...` | 2 | `"status": "INCOMPLETE"`, `"release_status": "INCOMPLETE"`, `"receipts": []`, `"source_checks": "NOT_RUN"` - the documented no-QA10-manifest state |
| `git status --porcelain --untracked-files=no` after build | 0 | empty |

## Broken-probe proof (blocking)

Probe applied in a throwaway detached worktree (`git worktree add --detach
/tmp/mcr-ci-probe ef02302d4`), never in the candidate checkout: one axum template
was renamed so its registry command no longer resolves.

```
# /tmp/mcr-ci-probe
.probe: .route("/api/concert/events/:event_id/mural/pixels" -> ".../pixels_broken"
$ python3 scripts/mcr/contracts/route-parity.py
route parity: registry=69 contract_commands=63 templates=245 commands=70 missing=1
MISSING POST   /api/concert/events/:event_id/mural/pixels  (placeConcertPixel)
route parity failed: every registered command needs an exact axum template
PROBE_ROUTE_PARITY_EXIT=1
```

The same command in the candidate worktree exits 0 with `missing=0` (table
above). The throwaway worktree was removed with `git worktree remove --force`;
the candidate checkout never held the probe.

## What could not run here

- **GitHub Actions runtime.** No hosted runner was available, so no job was
  executed as a workflow. Each workflow command was run locally instead (table
  above); `actionlint` and `act` are not installed, and Docker is not running on
  this host, so a containerized replay was impossible.
- **Full frontend vitest suite.** Not run locally by instruction ("do not run
  the entire workspace test suite"); the recipe was validated on one file plus
  the exclusion flags. The recorded suite is red today only on the excluded C07
  pin.
- **Backend `cargo test` for the 16 core crates.** Not run: the PG16 fixture path
  in this workflow relies on `MCR_TEST_POSTGRES_DATABASE_URL` against the service
  container, and this host has no Docker daemon. The only local PostgreSQL is a
  shared cluster other work uses, so it was not touched. The command was verified
  for one crate (`-p concert-domain --lib`, 15 passed) and the workspace test
  targets compile under `cargo check --all-targets`.
- **Release `source`/`composition` PASS path.** Requires the QA10 evidence
  manifest that has not been published; only the documented `INCOMPLETE` path was
  observed (exit 2).
- **PostgreSQL-dependent behavior.** The service container, health check and
  native-fixture database creation are unexecuted here.

## Findings left alone (pre-existing, out of scope)

- `scripts/mcr/contracts/qa01-coverage/pins.json` pins `9185c9922` and reports
  pinned input drift on `frontend/src/shared/api/domain-operations.generated.ts`.
- The same stale hash makes the C07 compatibility pin red.
  Both need their owning tasks to reconcile the recorded hash; this task does not
  edit generated artifacts or pins.
