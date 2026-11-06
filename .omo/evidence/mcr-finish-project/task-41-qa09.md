# QA09 recovery and lifecycle rehearsal

Status: PASS
Run: 2026-11-06T16:53:39.682085+00:00 to 2026-11-06T16:54:27.867027+00:00
Scope: owned PG16 cluster and the seven-service development composition; not a deployment or capacity claim.

| Scenario | State | Restored invariant |
|---|---|---|
| QA09-OUTBOX-REPLAY - Outbox redelivery and consumer dedupe | PASS | a crash after local application but before queue ack is replayed; the consumer receipt for the replayed signal remains exactly one; the outbox row returns to DELIVERED after the new process |
| QA09-DELETION - Interrupted private-data deletion saga | PASS | real Auth worker and production Concert factory over mTLS; physical PostgreSQL restart and service restart replay without duplicate effects; zero private rows for the deleted user and control user intact |
| QA09-BASELINE-PICKUP - Real pickup baseline | PASS | real giveaway, meeting, reservation and pickup code created; claim consumed exactly one unit; same idempotency key replayed the original claim receipt |
| QA09-SERVICE-RESTART - Gateway restart | PASS | the restarted gateway returned readiness; committed pickup rows unchanged; product read recovered through the gateway |
| QA09-PENDING-PICKUP - Interrupted pickup claim | PASS | a claim blocked in its database transaction completed exactly once after the lock backend was released; gateway restart and the same idempotency key replayed the original receipt; no double consumption |
| QA09-DB-RESTART - Postmaster crash restart | PASS | the in-flight transaction resolved to a rollback after crash recovery; no half-committed marker survived; committed pickup rows and receipts remained present; the product read recovered through the gateway |
| QA09-BACKUP-RESTORE - Logical backup and restore | PASS | logical backup restored the exact committed rows; the post-backup mutation was discarded; outbox and receipt identities were neither lost nor duplicated |
| QA09-PARTIAL-CLOSURE - Interrupted event closure | PASS | a closure blocked mid-transaction completed atomically to READ_ONLY after the lock backend was released; a durable closure job and READ_ONLY effect were persisted; product writes were refused with 409 after closure; product reads continued |

## QA09-OUTBOX-REPLAY - Outbox redelivery and consumer dedupe

State: PASS

Actions:

```json
[
 {
  "owned_cluster": "/private/tmp/mcr-pg-1yyxmv0n",
  "postgres": "16.14 (Homebrew)",
  "binary_sha256": "6182f440066d45810d2730633f0cb6c04552be23cd2e95e3da85906ebcd73fe1"
 },
 {
  "postmaster": [
   47825,
   48181
  ]
 },
 {
  "cleanup": "owned root absent",
  "absent": true
 }
]
```

Before:

```text
{
 "outbox": "f4e17889-c887-4371-a2b1-b08b67546cc6|READY|1",
 "receipts": "1",
 "replayed_signal_receipts": "1",
 "persisted_signal": "f4e17889-c887-4371-a2b1-b08b67546cc6|e4c00df1-686a-4a4a-9ee5-0acd8e75b15b"
}
```

After:

```text
{
 "outbox": "f4e17889-c887-4371-a2b1-b08b67546cc6|DELIVERED|2",
 "receipts": "1",
 "replayed_signal_receipts": "1"
}
```

Commands:

| Command | Exit |
|---|---:|
| `native_block_outbox_fence_commit_retry_and_native_send` | 0 |
| `native_restart_durable_receipt_preserves_fresh_connection` | 0 |

Logs:

```text

running 1 test
test native::native_block_outbox_fence_commit_retry_and_native_send ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.93s



running 1 test
test native::native_restart_durable_receipt_preserves_fresh_connection ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 3.82s


```

## QA09-DELETION - Interrupted private-data deletion saga

State: PASS

Actions:

```json
[
 {
  "status": "PASS",
  "source": 0,
  "after_pg_restart": 0,
  "restart": {
   "after_pid": 48371,
   "before_pid": 48240
  },
  "migration_receipts": {
   "auth": 19,
   "concert": 63
  },
  "cleanup": {
   "owned_root_absent": true
  },
  "limitations": [
   "Authenticated Apply uses the production factory/service on a test-owned mTLS loopback listener; the real main.rs process is covered by source-composition assertion, not a process launch.",
   "The Sharing terminal row is an exact durable fixture for the preceding saga step; the Social owner flow itself is not re-qualified here.",
   "No Gateway HTTP, browser, deployment or whole-task acceptance is claimed."
  ]
 }
]
```

Before:

```text
{
 "private_data_command": "fresh user with avatar/age/participant rows",
 "saga": "real Auth worker and managed Concert factory"
}
```

After:

```text
{
 "private_rows": "0 for deleted user; control user intact (test assertion)",
 "restart": {
  "after_pid": 48371,
  "before_pid": 48240
 },
 "migration_receipts": {
  "auth": 19,
  "concert": 63
 }
}
```

Commands:

| Command | Exit |
|---|---:|
| `account-deletion-mount` | 0 |

Logs:

```text

running 1 test
test mounted_private_data_ack_restart_and_session_revocation_boundary ... MOUNTED DELETION PASS: sealed PrivateData ACK over mTLS, caller refusal before deleter, zero unrevoked sessions, tampered source refusal, restart replay without duplicate effects, zero private rows, control intact
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 4.42s



running 1 test
test mounted_private_data_ack_restart_and_session_revocation_boundary ... MOUNTED DELETION PASS: sealed PrivateData ACK over mTLS, caller refusal before deleter, zero unrevoked sessions, tampered source refusal, restart replay without duplicate effects, zero private rows, control intact
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 3.38s


```

## QA09-BASELINE-PICKUP - Real pickup baseline

State: PASS

Actions:

```json
[
 {
  "pickup_meeting": "baseline",
  "status": 200,
  "body": {
   "giveaway_id": "834508ab-feda-4308-9279-b6ec61fcd3b7",
   "meeting": {
    "expires_at": "2026-11-06T19:54:14.806196Z",
    "identification": "Show the six digit code",
    "instructions": "Recovery collection point",
    "map_revision": 1,
    "place_id": "11111111-1111-4111-8111-111111111111",
    "place_revision": 1
   },
   "revision": 2
  }
 },
 {
  "provision": "baseline",
  "giveaway": "834508ab-feda-4308-9279-b6ec61fcd3b7",
  "reservation": "f82d33cd-ad7b-431d-891d-0afdfebfff9e",
  "status": 200
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "",
 "reservations": "",
 "outbox_ids": "",
 "receipts": "",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1",
 "reservations": "f82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,ec119e02-9aa1-4ba8-9a14-0fc63db33989",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Database rows:

```text
[baseline_giveaway]
0|1
[baseline_receipts]
1
```

## QA09-SERVICE-RESTART - Gateway restart

State: PASS

Actions:

```json
[
 {
  "stop": "api-gateway",
  "result": {
   "pid": 48726,
   "forced": false,
   "exit_code": 0
  }
 },
 {
  "start": "api-gateway",
  "pid": 49393
 },
 {
  "recovered_services": {
   "profile-service": 48717,
   "notification-service": 48718,
   "concert-service": 48719,
   "friend-service": 48720,
   "post-service": 48723,
   "auth-service": 48724,
   "api-gateway": 49393
  },
  "readiness": {
   "profile-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "notification-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "concert-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "friend-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "post-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "auth-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "api-gateway": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   }
  }
 },
 {
  "pids": {
   "gateway": [
    48726,
    49393
   ]
  }
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1",
 "reservations": "f82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,ec119e02-9aa1-4ba8-9a14-0fc63db33989",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1",
 "reservations": "f82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,ec119e02-9aa1-4ba8-9a14-0fc63db33989",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Commands:

| Command | Exit |
|---|---:|
| `restart-api-gateway` | 0 |

## QA09-PENDING-PICKUP - Interrupted pickup claim

State: PASS

Actions:

```json
[
 {
  "pickup_meeting": "pending",
  "status": 200,
  "body": {
   "giveaway_id": "ccc9d924-deba-4de8-9da0-74f6ba95f232",
   "meeting": {
    "expires_at": "2026-11-06T19:54:17.833597Z",
    "identification": "Show the six digit code",
    "instructions": "Recovery collection point",
    "map_revision": 1,
    "place_id": "11111111-1111-4111-8111-111111111111",
    "place_revision": 1
   },
   "revision": 2
  }
 },
 {
  "provision": "pending",
  "giveaway": "ccc9d924-deba-4de8-9da0-74f6ba95f232",
  "reservation": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa",
  "status": 200
 },
 {
  "pending_claim": "request blocked in its database transaction"
 },
 {
  "blocked_claim_outcome": {
   "status": 200,
   "body": {
    "claimed_at": "2026-11-06T16:54:19.203561Z",
    "giveaway_id": "ccc9d924-deba-4de8-9da0-74f6ba95f232",
    "receipt_id": "f89f7db6-73f7-4456-9e68-dfda35b439c7",
    "reservation_id": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa",
    "revision": 3,
    "state": "CLAIMED"
   }
  }
 },
 {
  "stop": "api-gateway",
  "result": {
   "pid": 49393,
   "forced": false,
   "exit_code": 0
  }
 },
 {
  "start": "api-gateway",
  "pid": 49501
 },
 {
  "recovered_services": {
   "profile-service": 48717,
   "notification-service": 48718,
   "concert-service": 48719,
   "friend-service": 48720,
   "post-service": 48723,
   "auth-service": 48724,
   "api-gateway": 49501
  },
  "readiness": {
   "profile-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "notification-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "concert-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "friend-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "post-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "auth-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "api-gateway": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   }
  }
 },
 {
  "replay_claim": {
   "status": 200,
   "body": {
    "claimed_at": "2026-11-06T16:54:19.203561Z",
    "giveaway_id": "ccc9d924-deba-4de8-9da0-74f6ba95f232",
    "receipt_id": "f89f7db6-73f7-4456-9e68-dfda35b439c7",
    "reservation_id": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa",
    "revision": 3,
    "state": "CLAIMED"
   },
   "attempts": 1
  }
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1",
 "reservations": "f82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,ec119e02-9aa1-4ba8-9a14-0fc63db33989",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Database rows:

```text
[giveaway_before]
OPEN|4|1|0
[reservation_after_interrupt]
CLAIMED|3
[giveaway_after_interrupt]
0|1
[original_receipt]
6880f5ae-ae2d-4cd7-8a17-2eb051eaa7aa
[giveaway_recovered]
0|1
[success_receipts_recovered]
1
```

Commands:

| Command | Exit |
|---|---:|
| `restart-api-gateway` | 0 |

## QA09-DB-RESTART - Postmaster crash restart

State: PASS

Actions:

```json
[
 {
  "in_flight_transaction": {
   "marker": "5fea9720-f1df-42ed-a7ab-ea44a5eb47d2",
   "postmaster": 48675
  }
 },
 {
  "postmaster": [
   48675,
   49619
  ],
  "in_flight_exit": 2
 },
 {
  "recovered_services": {
   "profile-service": 48717,
   "notification-service": 48718,
   "concert-service": 49959,
   "friend-service": 48720,
   "post-service": 48723,
   "auth-service": 48724,
   "api-gateway": 49501
  },
  "readiness": {
   "profile-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "notification-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "concert-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "friend-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "post-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "auth-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "api-gateway": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   }
  }
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Database rows:

```text
[inflight_after_restart]
0
```

Commands:

| Command | Exit |
|---|---:|
| `postmaster immediate stop` | 0 |

Logs:

```text
1-07 01:53:59.694 KST [48608] LOG:  listening on Unix socket "/private/tmp/mcr-pg-2f1ykhrh/socket/.s.PGSQL.5432"
2026-11-07 01:53:59.700 KST [48613] LOG:  database system was shut down at 2026-11-07 01:53:59 KST
2026-11-07 01:53:59.703 KST [48608] LOG:  database system is ready to accept connections
2026-11-07 01:54:00.356 KST [48608] LOG:  received fast shutdown request
2026-11-07 01:54:00.356 KST [48608] LOG:  aborting any active transactions
2026-11-07 01:54:00.357 KST [48608] LOG:  background worker "logical replication launcher" (PID 48616) exited with exit code 1
2026-11-07 01:54:00.357 KST [48611] LOG:  shutting down
2026-11-07 01:54:00.358 KST [48611] LOG:  checkpoint starting: shutdown immediate
2026-11-07 01:54:00.360 KST [48611] LOG:  checkpoint complete: wrote 84 buffers (0.5%); 0 WAL file(s) added, 0 removed, 0 recycled; write=0.002 s, sync=0.002 s, total=0.003 s; sync files=39, longest=0.001 s, average=0.001 s; distance=52 kB, estimate=52 kB; lsn=0/3DADD60, redo lsn=0/3DADD60
2026-11-07 01:54:00.362 KST [48608] LOG:  database system is shut down
2026-11-07 01:54:00.509 KST [48675] LOG:  starting PostgreSQL 16.14 (Homebrew) on aarch64-apple-darwin25.4.0, compiled by Apple clang version 21.0.0 (clang-2100.0.123.102), 64-bit
2026-11-07 01:54:00.510 KST [48675] LOG:  listening on Unix socket "/private/tmp/mcr-pg-2f1ykhrh/socket/.s.PGSQL.5432"
2026-11-07 01:54:00.515 KST [48678] LOG:  database system was shut down at 2026-11-07 01:54:00 KST
2026-11-07 01:54:00.518 KST [48675] LOG:  database system is ready to accept connections
2026-11-07 01:54:15.033 KST [48763] WARNING:  there is no transaction in progress
2026-11-07 01:54:15.047 KST [48734] WARNING:  there is no transaction in progress
2026-11-07 01:54:15.615 KST [48767] WARNING:  there is no transaction in progress
2026-11-07 01:54:15.647 KST [48734] WARNING:  there is no transaction in progress
2026-11-07 01:54:16.136 KST [48763] WARNING:  there is no transaction in progress
2026-11-07 01:54:17.105 KST [48767] WARNING:  there is no transaction in progress
2026-11-07 01:54:17.113 KST [48721] WARNING:  there is no transaction in progress
2026-11-07 01:54:17.569 KST [48767] WARNING:  there is no transaction in progress
2026-11-07 01:54:17.578 KST [48721] WARNING:  there is no transaction in progress
2026-11-07 01:54:17.999 KST [48768] WARNING:  there is no transaction in progress
2026-11-07 01:54:18.006 KST [48734] WARNING:  there is no transaction in progress
2026-11-07 01:54:18.406 KST [48766] WARNING:  there is no transaction in progress
2026-11-07 01:54:18.414 KST [48734] WARNING:  there is no transaction in progress
2026-11-07 01:54:18.866 KST [49318] WARNING:  there is no transaction in progress
2026-11-07 01:54:19.149 KST [49480] FATAL:  terminating connection due to administrator command
2026-11-07 01:54:19.149 KST [49480] STATEMENT:  BEGIN; select 1 from concert_community_giveaways where giveaway_id='ccc9d924-deba-4de8-9da0-74f6ba95f232' for update; SELECT pg_sleep(300);
2026-11-07 01:54:20.110 KST [48675] LOG:  received immediate shutdown request
2026-11-07 01:54:20.115 KST [49570] FATAL:  the database system is shutting down
2026-11-07 01:54:20.115 KST [49571] FATAL:  the database system is shutting down
2026-11-07 01:54:20.117 KST [48675] LOG:  database system is shut down
2026-11-07 01:54:20.255 KST [49619] LOG:  starting PostgreSQL 16.14 (Homebrew) on aarch64-apple-darwin25.4.0, compiled by Apple clang version 21.0.0 (clang-2100.0.123.102), 64-bit
2026-11-07 01:54:20.255 KST [49619] LOG:  listening on Unix socket "/private/tmp/mcr-pg-2f1ykhrh/socket/.s.PGSQL.5432"
2026-11-07 01:54:20.260 KST [49637] LOG:  database system was interrupted; last known up at 2026-11-07 01:54:00 KST
2026-11-07 01:54:20.372 KST [49640] FATAL:  the database system is starting up
2026-11-07 01:54:20.377 KST [49641] FATAL:  the database system is starting up
2026-11-07 01:54:20.378 KST [49642] FATAL:  the database system is starting up
2026-11-07 01:54:20.386 KST [49643] FATAL:  the database system is starting up
2026-11-07 01:54:20.386 KST [49644] FATAL:  the database system is starting up
2026-11-07 01:54:20.391 KST [49645] FATAL:  the database system is starting up
2026-11-07 01:54:20.392 KST [49646] FATAL:  the database system is starting up
2026-11-07 01:54:20.401 KST [49647] FATAL:  the database system is starting up
2026-11-07 01:54:20.409 KST [49648] FATAL:  the database system is starting up
2026-11-07 01:54:20.415 KST [49649] FATAL:  the database system is starting up
2026-11-07 01:54:20.415 KST [49650] FATAL:  the database system is starting up
2026-11-07 01:54:20.425 KST [49651] FATAL:  the database system is starting up
2026-11-07 01:54:20.453 KST [49652] FATAL:  the database system is starting up
2026-11-07 01:54:20.459 KST [49653] FATAL:  the database system is starting up
2026-11-07 01:54:20.459 KST [49654] FATAL:  the database system is starting up
2026-11-07 01:54:20.461 KST [49637] LOG:  database system was not properly shut down; automatic recovery in progress
2026-11-07 01:54:20.463 KST [49637] LOG:  redo starts at 0/3DADDD8
2026-11-07 01:54:20.467 KST [49655] FATAL:  the database system is not yet accepting connections
2026-11-07 01:54:20.467 KST [49655] DETAIL:  Consistent recovery state has not been yet reached.
2026-11-07 01:54:20.474 KST [49637] LOG:  invalid record length at 0/4036200: expected at least 24, got 0
2026-11-07 01:54:20.474 KST [49637] LOG:  redo done at 0/4035CD0 system usage: CPU: user: 0.00 s, system: 0.00 s, elapsed: 0.01 s
2026-11-07 01:54:20.477 KST [49635] LOG:  checkpoint starting: end-of-recovery immediate wait
2026-11-07 01:54:20.493 KST [49635] LOG:  checkpoint complete: wrote 759 buffers (4.6%); 0 WAL file(s) added, 0 removed, 1 recycled; write=0.008 s, sync=0.008 s, total=0.016 s; sync files=267, longest=0.001 s, average=0.001 s; distance=2593 kB, estimate=2593 kB; lsn=0/4036200, redo lsn=0/4036200
2026-11-07 01:54:20.496 KST [49619] LOG:  database system is ready to accept connections

```

## QA09-BACKUP-RESTORE - Logical backup and restore

State: PASS

Actions:

```json
[
 {
  "backup_bytes": 783446,
  "backup_sha256": "444f8b3c95e48d2a2b5bf96423fae1899bc857d0d33278926f819426012e56ed"
 },
 {
  "restored_database": "mcr_concert_restored",
  "dropped": true,
  "dump_removed": true
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Database rows:

```text
[mutation]
CREATE TABLE
INSERT 0 1
QA09 MUTATED
[restored_mutation_table]
ABSENT
```

Commands:

| Command | Exit |
|---|---:|
| `pg_dump` | 0 |
| `pg_restore` | 0 |

## QA09-PARTIAL-CLOSURE - Interrupted event closure

State: PASS

Actions:

```json
[
 {
  "recovered_services": {
   "profile-service": 48717,
   "notification-service": 48718,
   "concert-service": 49959,
   "friend-service": 48720,
   "post-service": 48723,
   "auth-service": 48724,
   "api-gateway": 49501
  },
  "readiness": {
   "profile-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "notification-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "concert-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "friend-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "post-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "auth-service": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   },
   "api-gateway": {
    "alive": true,
    "status": 200,
    "ready": true,
    "degraded": false
   }
  }
 },
 {
  "pending_closure": "request blocked on the event row"
 },
 {
  "blocked_closure_outcome": {
   "status": 200,
   "body": {
    "event_id": "f1fd7b85-d393-5410-8bc3-5ea6b71aeb77",
    "job_id": "e63f7fdb-ca88-51dc-b08f-a7af2f103c87",
    "revision": 1,
    "state": "PENDING",
    "steps": [
     {
      "name": "READ_ONLY",
      "state": "SUCCEEDED"
     },
     {
      "name": "FREEZE_MURAL",
      "state": "PENDING"
     },
     {
      "name": "SNAPSHOT",
      "state": "PENDING"
     },
     {
      "name": "EXPIRE_GRANTS",
      "state": "PENDING"
     },
     {
      "name": "EXPIRE_BEACONS",
      "state": "PENDING"
     },
     {
      "name": "EXPORT",
      "state": "PENDING"
     },
     {
      "name": "CLEANUP",
      "state": "PENDING"
     }
    ]
   }
  }
 },
 {
  "post_closure_write": {
   "status": 409,
   "body": {
    "code": "GATEWAY_409",
    "details": {
     "reason": "EVENT_READ_ONLY"
    },
    "message": "The requested action is unavailable.",
    "timestamp": "2026-11-06T16:54:27.462Z",
    "traceId": "3b4a06b9-6dc3-4051-95b8-719247288861"
   }
  }
 },
 {
  "post_closure_read": {
   "status": 200
  }
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|OPEN|5|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|OPEN|5|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "READ_ONLY|2",
 "giveaways": "834508ab-feda-4308-9279-b6ec61fcd3b7|CANCELLED|6|0|1\nccc9d924-deba-4de8-9da0-74f6ba95f232|CANCELLED|6|0|1",
 "reservations": "5bccac6c-8f77-4d87-8678-6ef97d5d06aa|CLAIMED|3\nf82d33cd-ad7b-431d-891d-0afdfebfff9e|CLAIMED|3",
 "outbox_ids": "0573c428-cd21-4782-94bf-8949c328034b,28a784f3-6732-4837-9aff-3b74245e9138,3e28628f-3e2b-4961-872e-ee9f2dce3dc0,5771f3e4-e2bb-4f84-b6d8-76a1d4624b60,6bb9e8af-f10a-4d1f-a757-80a2efc46e58,718e822f-3dcc-420e-aa0a-f6bf32ec814a,73afc3da-e545-4846-bb38-aef1e7e23650,a3779c47-a42c-4302-86fb-ff5bfebcf92a,baed443e-b244-49e7-b79c-816f6f684db6,d6db228e-b4fd-4e8b-8737-37e48c3e5109,ec119e02-9aa1-4ba8-9a14-0fc63db33989,f99c62df-74b8-477e-bdea-161db38b800b,fff47fa2-7538-435d-9f88-33e744272184",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncloseConcertEvent|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "e63f7fdb-ca88-51dc-b08f-a7af2f103c87|PENDING|1",
 "closure_effects": "READ_ONLY|1"
}
```

Database rows:

```text
[event_after_blocked_attempt]
READ_ONLY|2
[event_after_resume]
READ_ONLY|2
[closure_job_after_resume]
e63f7fdb-ca88-51dc-b08f-a7af2f103c87|PENDING|1
```

## Mutation proof

The outbox stale-lease fence was broken in a throwaway worktree
`/tmp/qa09-mutation` at base `80b03639d`. In
`backend/services/concert-service/src/adapters/out/repository/outbox.rs`,
`acknowledge` was reduced to `where outbox_id=$1 and state='LEASED'`, so a
holder whose lease was reclaimed could still acknowledge the delivery.

- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa09 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -j3 -p concert-service --test maintenance_outbox --no-run` exited 0.
- Mutated run (`maintenance_origin_transactions_and_leases` on a fresh owned PG16 cluster): exit 101, `test result: FAILED. 0 passed; 1 failed`, panic at `services/concert-service/tests/maintenance_outbox.rs:104: assertion failed: !box_.acknowledge(first).await.unwrap()`. The stale holder acknowledged after the lease rotated, which is exactly the recovery invariant the outbox replay depends on.
- Baseline run (copied unmodified binary): exit 0, `test result: ok. 1 passed; 0 failed`.
- Restore: the throwaway worktree was removed; the delivery worktree source is unchanged. The baseline binary is retained at `/tmp/qa09-trial/maintenance_outbox-baseline` and was the binary used for the passing run.

## Honest limits

- The stack is the development seven-service composition on an owned PostgreSQL 16.14 cluster with loopback TLS. It is not a deployment, capacity, multiuser race, browser/device, or production approval.
- The database restart is an immediate stop of the owned postmaster; it verifies crash recovery, in-flight rollback and committed-row survival, not disk failure or a standby.
- The backup is a logical `pg_dump`/`pg_restore` of the Concert owner database into a fresh owned database. It compares event, giveaway, reservation, outbox identity and receipt rows; it is not a physical restore of the live cluster and does not back up the Auth database.
- Outbox replay uses the native World invalidation consumer test with a real PostgreSQL cluster and real WebSocket handoff. It forces the documented crash-after-local-application-before-ack replay and verifies one consumer receipt. It is not the seven-service social worker's live pickup outbox path.
- Pending pickup blocks a real claim on the giveaway row, terminates the lock backend, lets the claim complete, restarts the gateway, and replays the same idempotency key. A validation retry was not needed because the fixed release let the claim complete; this does not claim a concert-process crash mid-claim.
- Deletion uses the committed production-worker/factory mount test: real Auth worker, managed Concert factory over mTLS, real PostgreSQL restart and service restart replay, zero private rows and an intact control user. The isolated composer does not mount Profile/Friend/Post/Notification/Session/Sharing owners for that command, so it does not claim full eight-target saga completion. The live seven-service Auth worker did not claim the saga steps in this composition (observed PENDING/attempt 0); that live-worker path is NOT_RUN, not relabeled as passing.
- Partial closure blocks the closure transaction, lets it complete atomically, and verifies READ_ONLY, a durable job, 409 writes and 200 reads. Later FREEZE_MURAL..CLEANUP owners and the public archive are not claimed; the live composition reports them unavailable. No concert-process restart after closure is claimed: a restarted concert fails World admission because the closed event has retired its room leases.
- Service restart is the gateway. Concert and post-process restart recovery are not claimed; the DB restart scenario covers database reconnection.
- The baseline uses the real giveaway, meeting, reservation, pickup-code and claim flow rather than the original party concurrency gate; QA03 owns race acceptance.
- No QA08 performance, 200-user, browser, or device acceptance is claimed.

## Cleanup

- Final stack report cleanup: PASS; the owned cluster root was removed and no owned service children remained.
- Native outbox and deletion mount runs removed their owned cluster roots; the mutation runs removed theirs.
- The throwaway mutation worktree was removed. No delivery source outside this harness and evidence was changed.
