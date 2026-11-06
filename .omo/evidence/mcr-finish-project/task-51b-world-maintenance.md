# Task 51b — World authority maintenance across a closure transition

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-worldmaint`
Branch: `codex/mcr-exec-w2-worldmaint` (base `f4f9875d0`)
Date: 2026-11-06

## Symptom reproduced before the fix

Driver: `backend/scripts/mcr/closure_vertical_stack.py`, current checkout binaries built at
`f4f9875d0` with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-worldmaint`,
`-j 3`.

```
python3 -B backend/scripts/mcr/closure_vertical_stack.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w2-worldmaint/debug \
  --report /tmp/worldmaint-prefix-1.json                              # exit 1
```

Result `FAIL`. `closure_rows.event = READ_ONLY|2`; the closure job and receipt were
persisted, and `event_read_only` was true. The very first request after the closure ACK
was refused by the dead dependency instead of the phase guard:

| Step | Observed | Expected |
| --- | --- | --- |
| `feed_post_write` (first post-closure write) | `503 CONCERT_503 DEPENDENCY_UNAVAILABLE`, rows unchanged | `409 EVENT_READ_ONLY` |
| `post_closure_write` (same closure command, fresh idempotency key) | `503 DEPENDENCY_UNAVAILABLE` | `409 INVALID_TRANSITION` |
| `archive` | `503 DEPENDENCY_UNAVAILABLE` | `404 RESOURCE_UNAVAILABLE` at `READ_ONLY` |

Failed checks: `feed_post_closure_409`, `post_closure_conflict`. Exact `concert-service`
stderr at the failure stage (`/tmp/worldmaint-prefix-1.json`, `logs.concert-service`):

```
World authority failure stage=maintenance-room-renewal error=Denied
World supervised failure before cleanup: World authority maintenance failed
CONCERT_COMPONENT_FAILURE=World World authority maintenance failed

Caused by:
    world authority denied admission
Error: World authority maintenance failed
```

The failure stage is `maintenance-room-renewal`; the error class is `WorldError::Denied`.
No host load was needed: one stack reproduced it, and the timing explanation below shows
why a fast run can pass.

## Mechanism

1. The closure START transaction (`event_lifecycle` READ_ONLY step +
   `world::closure::fence_event_read_only_in`) updates the event phase to `READ_ONLY`,
   revokes memberships, disables rooms and **deletes every `concert_world_owner_leases`
   row** for the event. Every authority-creating transaction takes
   `closure::lock_event_open_in` first, which returns `WorldError::Denied` when the phase
   is not `OPEN`/`LIVE`.
2. `AdmissionBridge` runs a supervised 1 s authority tick
   (`config/world_runtime.rs::authority`, `bridge.maintain()`). `maintain_once` paused
   publication, then renewed each configured room owner lease with
   `service.renew_owner(...)`.
3. After the closure commit, `renew_owner` takes the event `SHARE` lock (waits out an
   in-flight closure), observes `READ_ONLY`, and returns `Denied`. `maintain_once`
   mapped that to `authority_failure("maintenance-room-renewal")` and propagated it as a
   fatal error.
4. The authority future was one arm of the World `tokio::select!`; its `Err` ended the
   World task, `finish_world_run` returned `World authority maintenance failed`, and
   `component("World", ...)` printed `CONCERT_COMPONENT_FAILURE=World` and requested
   shared shutdown. The gRPC listener shut down, so the next gateway call mapped the dead
   dependency to `503 DEPENDENCY_UNAVAILABLE` instead of reaching the phase guard.

Intermittency: the defect needs one authority tick to land after the closure commit and
before stack teardown. The pre-fix driver issued all post-closure probes immediately, so a
run that finished and torn down inside the next 1 s interval could pass with 19 checks
true; a run whose probes crossed the tick failed. This is a service defect, not a
host-load artifact: the pre-fix run failed without load, and the failure is the
deterministic result of the tick observing the closed phase.

The earlier deferral fix `cf574eadf` addressed a different transient: a publication window
closing during `ProductionOwners::activate`. It correctly defers that tick, but it does not
classify the repository's phase-denial of room renewal.

## Fix

`backend/services/concert-service/src/world/admission_bridge.rs`:

- `maintenance_defers(error, event_open)` defers only a `Denied` whose independently read
  committed phase is not `OPEN`/`LIVE`. A `Denied` while the event is still open, and every
  other `WorldError` class, stays fatal.
- `event_open()` re-reads the phase with the same event `SHARE` lock the authority gate
  uses, so it observes the closure's committed phase and cannot misclassify an uncommitted
  update.
- The renewal loop defers that one tick (`return Ok(())`) after the denial is confirmed
  closed; it adds no retry loop. A phase-read failure is fatal and visible with its own
  stage (`maintenance-event-phase`), and all other renewal failures keep the existing
  `maintenance-room-renewal` stage.
- The World task stays alive and idle after closure, so the gRPC listener keeps serving
  phase-guarded responses; the phase gate itself is unchanged and still refuses every
  authority mutation.

Tests:

- `world::admission_bridge::maintenance_tests::a_closed_event_defers_the_maintenance_tick`
  proves a confirmed closed phase defers the tick.
- `world::admission_bridge::maintenance_tests::a_genuine_authority_failure_remains_fatal`
  proves `Unavailable`, `OwnerFenced`, `Invalid`, `MembershipMissing`, `Conflict`, and a
  `Denied` on an open event are never deferred.
- `config::world_runtime::shutdown_result_tests::a_genuine_maintenance_failure_stays_fatal_and_visible`
  proves a genuine maintenance error still reaches the supervisor with its stage and
  original `WorldError` class.
- `backend/scripts/mcr/closure_vertical_stack.py` now waits one full authority interval
  (2 s, bounded) after the closure ACK, asserts `concert-service` is alive and logged no
  `World authority maintenance failed`/`CONCERT_COMPONENT_FAILURE`, then issues the
  post-closure writes. The new check is `world_survives_closure`; the existing
  `feed_post_closure_409` and `post_closure_conflict` checks prove the 409 responses.

## Checks

```
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-worldmaint cargo build \
  -p concert-service -j 3                                             # exit 0
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-worldmaint cargo test \
  -p concert-service --bin concert-service -j 3                      # 9 passed, exit 0
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-worldmaint cargo test \
  -p concert-service --lib -j 3                                      # 130 passed, 4 failed, 60 ignored
```

The four library failures are environment-gated database fixtures, not this change:
`product_rpc::scope_ledger::notification_binding::tests::{constructor_rejects_nil_and_shares_only_the_existing_pool,
notification_client_can_own_reader_before_any_ledger_or_settler,
reader_and_ledger_share_exact_wire_and_fail_closed_results}` and
`relationship::consumer_probes::committed_block_and_suspension_remove_all_six_read_entries_and_reread_after_revision`
panic with `owned root required: NotPresent` / `owned consumer probe database required:
NotPresent` before touching the changed code. The new tests pass in the filtered lib run
(9/9).

## Three-run stability (post-fix)

| Run | Exit | Result | `world_survives_closure` | Post-closure feed write | Same closure command | 503 observed |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2 (`PARTIAL`) | 20 true, 0 false, 2 `null` | true | `409 EVENT_READ_ONLY` | `409 INVALID_TRANSITION` | no |
| 2 | 2 (`PARTIAL`) | 20 true, 0 false, 2 `null` | true | `409 EVENT_READ_ONLY` | `409 INVALID_TRANSITION` | no |
| 3 | 2 (`PARTIAL`) | 20 true, 0 false, 2 `null` | true | `409 EVENT_READ_ONLY` | `409 INVALID_TRANSITION` | no |

Settle wait recorded by the driver: 2.019 s, 2.048 s, 2.034 s; `concert_alive = true` each
run; the step log recorded `no new child log bytes` (no phase-guard or supervision line).
Reports: `/tmp/worldmaint-postfix-{1,2,3}.json`, sha256 prefixes
`35475f7be5103743`, `874728e3d098aa19`, `69977cc48ac77f1d`. Cleanup `PASS` in every run;
no owned cluster remained.

Exit 2 is the driver's truthful `PARTIAL`: `archive_public_200` and
`archive_public_fields` are `null` because the event is `READ_ONLY`, not `ARCHIVED`, and
the later closure owners are not composed. Every non-null check passed.

## Limitations

- Development vertical stack, not IN03 cohort admission, QA08 capacity or release
  qualification.
- The 2 s settle in the driver is deliberately time-based because the behavior under test
  is the 1 s supervised tick; it is bounded, not a retry.
- Cleanup of World membership rows during service shutdown after closure is outside this
  change and was not exercised as a measured assertion.
