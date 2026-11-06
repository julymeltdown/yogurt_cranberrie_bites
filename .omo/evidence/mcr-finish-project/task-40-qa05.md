# Task 40 — QA05 realtime failure suite

Status: **PASS** for the six requested failure modes over actual loopback
sockets and the production World router/runtime with marker-owned PostgreSQL 16
admission. Original QA05 remains open for the combined browser App/Auth producer
journey; the limits below are explicit.

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w7-qa05`
- Branch: `codex/mcr-exec-w7-qa05`, base `10e97e487e6ea20326c912eee6931db2aec00350`
- Target: `/Users/lhs/Desktop/mcr-build/exec-w7-qa05` (`CARGO_BUILD_JOBS=3`)
- New files: `backend/services/concert-service/tests/qa05_realtime_relay.rs`,
  `backend/scripts/mcr/qa05_realtime_suite.py`
- Runtime: PostgreSQL `16.14 (Homebrew)`, marker-owned socket-only cluster,
  production `AdmissionBridge`, `WorldRuntime`, `WorldSocketState` and axum
  `/world-ws` router, real `tokio-tungstenite` clients over `127.0.0.1`.

No product source changed. `git status --short` before commit:

```
?? backend/scripts/mcr/qa05_realtime_suite.py
?? backend/services/concert-service/tests/qa05_realtime_relay.rs
```

## What the suite proves

Every frame a client observes is produced by the production server over a real
TCP WebSocket and validated against the C05-shaped fields the server emits.
External Auth/Profile/owner observations are the existing synthetic
`LiveWorldOwners` fixture used by `tests/world_runtime_support/postgres.rs`
(`AdmissionBridge::new_fixture`); this is not IN01 two-account evidence.

| # | Case | Fault injection | Server close | Recovery path | Assertions |
| --- | --- | --- | --- | --- | --- |
| 1 | `qa05_delayed_and_gapped_frames_recover_to_a_fresh_snapshot` | Client stops reading 400 ms while a peer moves; then the client discards one delta and observes the next sequence number | none for the delay; `4409 RESYNC_REQUIRED` only for a deliberately stale stream in case 2 | Correlated `resync` from the last good `streamSeq` | Delayed frames stay monotonic; `snapshot cause=RESYNC`, `streamSeq=0`, new `streamId`, same `roomEpoch`, two unique players, two active memberships, one row per actor |
| 2 | `qa05_foreground_background_resync_is_fresh_and_uncorrupted` | Native client sends `FOREGROUND` and `PROTOCOL_RECOVERY` resyncs 1050 ms apart, then input on an archived stream | `4409 RESYNC_REQUIRED` on archived stream | Two correlated resyncs on the same socket | Each resync returns `cause=RESYNC`, `requestId` echo, `streamSeq=0`, new `streamId`, unchanged `accessExpiresAt`, `roomEpoch` equal to the observed stream and to the room epoch, two unique actors |
| 3 | `qa05_slow_consumer_backpressure_recovers_without_ghost` | Per-binding transport readiness gate emulates a saturated send buffer; peer keeps moving for the 5000 ms slow-consumer window | `4409 BACKPRESSURE` | Close, membership release, fresh-ticket reconnect | Close arrives inside the asserted 4.9–9.0 s window; only the peer remains active; reconnect `cause=INITIAL`, `streamSeq=0`, `roomEpoch` equal to the room epoch, `roomId` equal to the claimed room, two unique players, exactly one membership row for the recovered actor |
| 4 | `qa05_ticket_replay_is_rejected_without_a_second_membership` | Consumed ticket replayed on a second real socket | `4401 AUTH_REQUIRED` | Original socket keeps streaming; no admission for the replay | One active membership before and after, one row for the actor, replay inserts nothing, the surviving delta keeps the original `roomEpoch` and the room epoch |
| 5 | `qa05_server_restart_mid_session_recovers_with_a_new_epoch` | Stop the World socket server/runtime mid-session, then restart with new room epochs on the same owned PG | `4500 SERVER_UNAVAILABLE` to both clients | Fresh tickets, two reconnect snapshots | New `roomEpoch`, new `streamId`, `cause=INITIAL`, `streamSeq=0`, exactly two unique players after both rejoin, one membership row, two active members, pre-restart ticket still `4401 AUTH_REQUIRED` |
| 6 | `qa05_room_switch_is_atomic_and_replay_safe` | Successful `switchRoom` with a fresh ticket; then replay of the consumed switch ticket; then a full target room | Consumed switch ticket replay `4401 AUTH_REQUIRED`; capacity refusal is `switchRejected ROOM_FULL` | `ROOM_SWITCH` snapshot on the moving stream; old room publishes `removes` | `cause=ROOM_SWITCH`, request echo, new `streamId`, `streamSeq=0`, target `roomId`, `roomEpoch` equal to the target room epoch and different from the old room's, only the actor; old room drops the actor once; one membership row in the target; full-room rejection keeps the current stream and membership |

Epoch assertions are explicit in every case that recovers a snapshot. Cases 1, 2, 3, 5 and 6 compare the recovered snapshot's `roomEpoch` with `World::epoch(room)` for the room the snapshot claims to be in; case 1 also compares it with the gapped stream, case 2 with the previous snapshot on the same socket, and case 6 with the target epoch while asserting it differs from the old room's. Case 4 produces no recovery snapshot: the replay is rejected and the surviving stream continues, so it asserts that the continuing delta's `roomEpoch` equals both the original snapshot epoch and the room epoch. No case is left without an epoch check.

Committed assertion lines in
`backend/services/concert-service/tests/qa05_realtime_relay.rs`:

| Case | Lines | Assertion |
| --- | --- | --- |
| helper | 438 | `World::epoch(room)` resolves the configured epoch |
| 1 | 838–840 | recovered `roomEpoch` = gapped stream = room epoch; recovered `roomId` = room |
| 2 | 950–955 | each resync's `roomEpoch` = previous snapshot epoch = room epoch |
| 3 | 900–901 | reconnect `roomEpoch` = room epoch; `roomId` = room |
| 4 | 1012–1017 | surviving delta `roomEpoch` = original snapshot = room epoch (no recovery snapshot) |
| 5 | 1066, 1072 | both post-restart snapshots use the new room epoch |
| 6 | 1156–1157 | `ROOM_SWITCH` snapshot uses the target room epoch and differs from the old room |

## Commands and results

Build (exit 0):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa05 CARGO_BUILD_JOBS=3 \
CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline \
  -p concert-service --test qa05_realtime_relay --no-run --message-format=json
```

Suite (exit 0):

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/qa05_realtime_suite.py \
  --binary /Users/lhs/Desktop/mcr-build/exec-w7-qa05/debug/deps/qa05_realtime_relay-<hash> \
  --report <new absolute report>
```

Final receipt:

```json
{"result":"PASS","cleanup":"PASS","test_exit":0,
 "failed_cases":[],
 "tickets":{"issued":18,"consumed":18},
 "post_suite_cleanup":{"active_memberships":0,"prepared":0},
 "restart_persistence":"PASS"}
```

All six cases reported `... ok`; the runner additionally restarts the owned
PostgreSQL cluster and confirms the 18 consumptions persist.

Existing suites (exit 0):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa05 CARGO_BUILD_JOBS=3 \
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline \
  -p concert-service --test world_ws --test world_runtime --test world_simulation \
  --test world_replication --test world_recovery
```

| Target | Result |
| --- | --- |
| `world_ws` | 20 passed, 0 failed, 5 ignored (marker-owned-PG bridge tests, unchanged) |
| `world_runtime` | 35 passed, 0 failed |
| `world_simulation` | 28 passed, 0 failed |
| `world_replication` | 12 passed, 0 failed |
| `world_recovery` | 15 passed, 0 failed |

Route parity (exit 0):

```
route parity: registry=69 contract_commands=63 templates=245 commands=70 missing=0
route parity ok: every registered command has an exact axum template
```

## Mutation

A throwaway copy of the changed mechanism was kept at
`/tmp/qa05-mutation/admission_bridge.rs.orig` and `.mutant`. The mutation made
`AdmissionBridge::resync` return the current binding without publishing a fresh
stream (forcing resume without a fresh snapshot):

```diff
-        let result = self
-            .publish(m.membership, current.connection_id, Uuid::new_v4(), auth)
-            .await
-            .map_err(safe)?;
+        let _ = (m, auth);
         self.owners.activate().await.map_err(safe)?;
-        Ok(result)
+        Ok(current.clone())
```

The mutated binary run through the same runner:

```
test qa05_delayed_and_gapped_frames_recover_to_a_fresh_snapshot ... FAILED
test qa05_foreground_background_resync_is_fresh_and_uncorrupted ... FAILED
test qa05_room_switch_is_atomic_and_replay_safe ... ok
test qa05_server_restart_mid_session_recovers_with_a_new_epoch ... ok
test qa05_slow_consumer_backpressure_recovers_without_ghost ... ok
test qa05_ticket_replay_is_rejected_without_a_second_membership ... ok
test result: FAILED. 4 passed; 2 failed

panicked at ...: expected a json frame, got close 4500 "SERVER_UNAVAILABLE"
```

The two resync-dependent cases fail exactly as intended; the four independent
cases stay green. The source was then restored from the throwaway copy:

```
950585ff6652ef2bf20cdf2ed9678ddbcb758f8d0e815178e63688d4c30c9a4b  admission_bridge.rs
950585ff6652ef2bf20cdf2ed9678ddbcb758f8d0e815178e63688d4c30c9a4b  admission_bridge.rs.orig
```

After restore, `git status --short` shows only the two new files, the binary was
rebuilt, and the final suite run above returned `PASS` with `cleanup=PASS`.

## Cleanup receipts

- Runner-owned PG16 cluster: `cleanup=PASS`; 0 active memberships, 0 prepared
  transactions after the suite; owned root removed by `migration_smoke.Cluster`.
- 18/18 tickets consumed; a cluster restart preserves all 18 consumptions and a
  changed postmaster identity.
- Each case stops its socket server, owner task and maintenance task before
  returning; the runner verifies the database is quiescent afterwards.
- The suite runs `--test-threads=1` with a per-case event and rooms.

## Honest limits

- **No real Auth/Gateway/mTLS in this loop.** Tickets are issued and consumed by
  the production `PgWorldRepository`/`AdmissionBridge`; external owner/Auth
  observations are the committed synthetic `LiveWorldOwners` fixture
  (`new_fixture`). This is not IN01/IN03 evidence, not a real two-account Auth
  journey, and says nothing about the public world-ticket HTTP route.
- **Restart is an in-process World component restart.** The socket server,
  runtime and owner epoch are torn down and replaced over the same owned PG with
  new room epochs; it is not an OS-level Concert process restart. Process-level
  restart remains owned by the QA05 owner-control path and is not re-claimed here.
- **Slow consumer uses a transport readiness gate**, not a truly saturated TCP
  buffer. The gate only makes the runtime's `TransportPort::ready` false for one
  binding; every byte still travels through the production `SocketTransport`
  over a real socket, and the BACKPRESSURE decision is the production 5000 ms
  queue rule. Kernel-buffer saturation above the 256 KiB native write cap is not
  deterministically reproduced in this test.
- **Foreground/background is protocol-level, not browser visibility.** The
  native client sends the frozen `FOREGROUND` and `PROTOCOL_RECOVERY` resync
  reasons; no browser tab visibility, Phaser renderer or App decoder is involved.
- **Bounded deliberate delays.** The read pause is 400 ms, resyncs are 1050 ms
  apart to respect the production 1000 ms resync floor, and the slow-consumer
  case waits for the production 5000 ms window (asserted 4.9–9.0 s).
- **Single room pair / at most three actors.** This is not QA08 capacity,
  performance or device qualification, which remains `DEFERRED_BY_USER`.
- The owned PG cluster is initialized with `shared_memory_type=mmap` because this
  host's 32 SysV segments were all held by concurrent isolated clusters; cluster
  ownership, socket isolation and cleanup are unchanged.
