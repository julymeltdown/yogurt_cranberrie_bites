# Task 10 — BE10/BE11 admission and room leases

Date: 2026-11-06
Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w1-admission`
Branch: `codex/mcr-exec-w1-admission` (base `501710d29`, commit `1980fb0c7`)
Target dir: `/Users/lhs/Desktop/mcr-build/exec-w1-adm` (`CARGO_BUILD_JOBS=4`)

## Outcome

One-use world tickets and room leases were already implemented; the production wiring and
runtime were present, but the native qualification harnesses had drifted from the schema:

1. `bridge_native.py` and `world_admission_smoke.py` applied only migrations `0001`-`0005`.
   The current repository calls `concert_world_account_deletion_fences` (migration `0052`)
   from ticket consumption and membership mutation, so every native test failed with
   `world authority is unavailable`. Both harnesses now apply the full production migration
   set in order, matching the real service schema.
2. `owner_lease_fencing_and_ghost_prune_are_persisted` assumed an empty shared test database
   and pruned at `now + 120s`, deleting live tickets issued by earlier tests. The pre-check
   now prunes at the current instant, so the assertion tests stale-row collection without
   depending on cross-test ordering.
3. Added `native_database_bridge_two_tickets_one_room_and_replay_refusal`: two distinct users
   obtain distinct tickets, both are admitted into the same room as active canonical views,
   two membership rows exist, and replaying the first secret returns `AUTH_REQUIRED` (4401).
   The existing full-target test covers capacity refusal (`ROOM_FULL`, switchRejected) while
   preserving the current membership.

No second admission path was added; the WebSocket `AdmissionPort` and `PgWorldRepository`
remain the only paths.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo +1.88.0 test -p concert-service --test world_ws` | exit 0 — `20 passed; 0 failed; 5 ignored` |
| `bridge_native.py --binary <world_ws test binary>` | exit 0 — `5 passed; 0 failed` on PostgreSQL 16.14 marker-owned socket-only cluster |
| `world_admission_smoke.py --binary <world_admission test binary>` | exit 0 — `{"result":"PASS","cleanup":"PASS","native_test_exit":0,"restart_persistence":"PASS"}` |
| `python3 -m unittest test_world_admission_smoke` | exit 0 — `Ran 5 tests ... OK` |

Native bridge tests (real PostgreSQL, actual `AdmissionBridge`):

```
native_database_bridge_cancelled_postcommit_admission_retains_cleanup ... ok
native_database_bridge_consumption_atomic_resync_owner_fences_and_cleanup ... ok
native_database_bridge_full_target_rejection_preserves_current_membership ... ok
native_database_bridge_successful_socket_switch_spans_owner_ticks ... ok
native_database_bridge_two_tickets_one_room_and_replay_refusal ... ok
test result: ok. 5 passed; 0 failed
```

Admission smoke (real PostgreSQL): `direct_sql_migrations PASS`, `schema_constraints PASS`,
native 6/6 tests, `restart_persistence PASS`, `direct_sql_reapply EXPECTED_REFUSED`.

## QA mapping

- Happy: two clients obtain distinct tickets and both appear in one room —
  `native_database_bridge_two_tickets_one_room_and_replay_refusal` (two canonical membership
  rows, both views current) plus the loopback `two_independent_sockets_receive_snapshots_and_actual_authoritative_movement`.
- Failure: same ticket twice — second `bridge.admit` returns 4401 `AUTH_REQUIRED`; the socket
  suite's `unknown_ticket_and_replay_never_receive_a_world_snapshot` covers the wire path.
- Capacity: `native_database_bridge_full_target_rejection_preserves_current_membership`
  rejects the switch with `SwitchRejection::RoomFull` and keeps the old membership.
- Disconnect grace: `membership_leave_and_disconnect_grace_expire_canonical_rows` verifies the
  grace lease, expiry to `Gone`, prune and row removal on real PostgreSQL.

## Artifacts

- `task-10-admission-report.json` — admission smoke receipt (source hashes, PG version, restart).
- `task-10-bridge-report.json`, `task-10-bridge-report.log` — native bridge receipt and test log.

## Limitations

- Native bridge tests use synthetic external Auth/owner authority (`DatabaseFixture`); they do
  not claim IN01 two-account Auth/Profile composition.
- Publication to `master` and QA08 performance qualification are out of this branch's scope.
- The production HTTP ticket route and gateway composition are wired but are not exercised by
  these reports; their own suites remain separate.
- `scripts/mcr/check-plan.py` fails in this worktree before validation because
  `docs/source/mcr_v2_en/SHA256SUMS_EN.json` is not present (pre-existing worktree condition).

## Cleanup

Both marker-owned PostgreSQL clusters reported `cleanup: PASS` and removed their private
`/tmp/mcr-pg-*` roots. `/tmp` report copies were retained only as sources for the artifacts
above; the worktree has no target or fixture output.
