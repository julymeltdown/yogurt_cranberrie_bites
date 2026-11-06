# Task 16 — BE41 event closure, read-only barrier, archive export and cleanup

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-lifecycle`
Branch: `codex/mcr-exec-w2-lifecycle` (base `f75ec4238`)
Date: 2026-11-06

## Starting state

The BE41 engine (domain/application lifecycle, `ReadOnlyCommandDriver`, anonymous
archive projection, cleanup batch runtime and worker tick) was already present in
the base, but its own qualification suite failed: `event_lifecycle_full_migration_ledger_and_reapply`
asserted a hard-coded migration ledger ending at version 25 while later waves had
added migrations 26–72. The published gateway template `POST /api/concert/events/{eventId}/admin/closure`
(task 12, commit `4465e3f6f`) also had no handler-level route proof.

## Change

- `backend/services/concert-service/tests/event_lifecycle_support/migrations.rs`:
  the fresh/forward/reapply qualification now derives the expected ledger from the
  migration directory and requires every `.sql` version exactly once, strictly in
  version order. This keeps the invariant (no duplicates, no gaps in ordering) while
  covering newly assigned migrations instead of pinning a stale list.
- `backend/services/concert-service/tests/event_lifecycle.rs`: new real-PostgreSQL
  route test `event_lifecycle_admin_closure_route_admits_read_only_and_refuses_rewrite`
  that mounts the real `event_lifecycle::http` handlers on the published
  `admin/closure` and `admin/closure/{jobId}` templates and drives them through axum
  with an idempotency key:
  - `POST CLOSE_EVENT` -> 200 `Domain_admin_ClosureJob`, `cache-control: no-store`,
    event phase `READ_ONLY`, durable job `PENDING`;
  - identical same-key request -> byte-identical 200 replay (admission only, no
    second job);
  - any later closure write after `READ_ONLY` -> 409 (`INVALID_TRANSITION`);
  - `GET admin/closure/{jobId}` -> 200 `PENDING`; unknown job -> 404;
  - after every required step commits, phase becomes `ARCHIVED` and another
    closure write -> 409.
- `backend/scripts/mcr/event_lifecycle_pg.py`: owned-suite expectation list includes
  the new route test name.

No production composition was added: the concert-service producer for
`admin/closure` still requires Root-supplied operator-role and reviewed-retention
providers plus the World `AdmissionBridge` to be threaded into product composition.
Those inputs do not exist in this worktree, so the producer remains fail-closed
(503) rather than fabricating authority. This is the remaining integration gap for
the plan row and is recorded below.

## Commands and exit codes

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p concert-service --test event_lifecycle --test event_lifecycle_archive --test event_lifecycle_read_only --no-run` (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-life, JOBS=3) | 0 | three native test executables built |
| `python3 -B backend/scripts/mcr/event_lifecycle_pg.py --native-binary event_lifecycle --native-binary event_lifecycle_archive --native-binary event_lifecycle_read_only` (PATH=postgresql@16) | 0 | `"result": "PASS"`, 23 SQL checks, 3 native suites |
| `cargo check -p concert-service --all-targets` (same target dir) | 0 | all targets typecheck |

Owned PostgreSQL 16 cluster created by the runner (Unix socket only, owned marker
row), migrations applied/reapplied, then stopped and removed. Report:
`/Users/lhs/Desktop/mcr-build/exec-w2-life/be41-run.json`.
Owned source pin: `708c358068b95422c072867a2054c10e7be824114b7980947fbb72898b75056e`.

## Acceptance mapping

- Closure switches the event to `READ_ONLY`: SQL check `atomic_readonly_and_durable_job` + route test assertion.
- Archive contains only permitted data, no private locations/recipients/handles: `event_lifecycle_archive_deletion_invalidates_cursor_and_never_reintroduces_content` (current-source page, deletion invalidation, stale cursor conflict, strict `ArchivePost`/`ArchiveMural` DTO with `deny_unknown_fields`).
- Writes after closure 409/403: route test 409 on post-`READ_ONLY`/`ARCHIVED` closure writes; read-only barrier tests deny preconsumed claims and new leases.
- Restart resumes cleanup without duplicates: `event_lifecycle_restart_seed`/`restart_resume` and `event_lifecycle_read_only_restart_seed`/`restart_resume` run across an actual PostgreSQL restart within the run.
- Partial failure resumes: `event_lifecycle_partial_failure_rolls_back_and_explicit_resume_skips_success`.

## Remaining limitation (not claimed complete)

Producer-side composition of `admin/closure` inside the concert product dispatch
still needs the Root-owned operator-role provider, reviewed retention policy and the
World `AdmissionBridge`, exactly as the prior BE41 owner audit recorded. The gateway
route proof (task 12) and this handler-level route proof are in place; end-to-end
publication remains owned by that composition handoff.
