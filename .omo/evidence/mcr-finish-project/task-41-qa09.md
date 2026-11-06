# QA09 recovery and lifecycle rehearsal

Status: PASS (QA09-DELETION-SAGA: NOT_RUN)
Run: 2026-11-06T17:43:47.827212+00:00 to 2026-11-06T17:44:48.827533+00:00
Scope: owned PG16 cluster and the seven-service development composition; not a deployment or capacity claim.

| Scenario | State | Restored invariant |
|---|---|---|
| QA09-OUTBOX-REPLAY - Outbox replay after SQL reset and cluster restart | PASS | the native test reset the delivered outbox row to READY with SQL and the harness stopped and started the owned postmaster; a fresh process and connection replayed the row; the consumer receipt for the replayed signal remained exactly one; the outbox row returned to DELIVERED after the new process with one new attempt |
| QA09-DELETION - PrivateData owner deletion mount | PASS (isolated owner mount) | the production Auth deletion source and mounted Concert PrivateData factory sanitized the seeded participant row and removed the avatar and age-owner rows over mTLS (the private-row predicate is 0); the control user's rows were unchanged; a fresh composed factory replayed the sealed acknowledgement after a service restart without rewriting its terminal receipt; the same case ran again after a physical PostgreSQL restart against the same durable stores; the canonical Auth saga's eight steps never left PENDING in this mount, so it is not deletion convergence |
| QA09-BASELINE-PICKUP - Real pickup baseline | PASS | real giveaway, meeting, reservation and pickup code created; claim consumed exactly one unit; same idempotency key replayed the original claim receipt |
| QA09-SERVICE-RESTART - Gateway restart | PASS | the restarted gateway returned readiness; committed event, giveaway, reservation, outbox identity and receipt rows unchanged; product read recovered through the gateway |
| QA09-PENDING-PICKUP - Interrupted pickup claim | PASS | a claim blocked in its database transaction completed exactly once after the lock backend was released; inventory ended consumed exactly once; gateway restart and the same idempotency key replayed the original receipt; no double consumption |
| QA09-DB-RESTART - Postmaster crash restart | PASS | the in-flight transaction resolved to a rollback after crash recovery; no half-committed marker survived; committed pickup rows and receipts remained present; the product read recovered through the gateway |
| QA09-DELETION-SAGA - Live seven-service deletion saga | NOT_RUN | not claimed: the live seven-service saga ran but did not converge; measured step rows below |
| QA09-BACKUP-RESTORE - Logical backup and restore | PASS | logical backup restored the exact committed rows; the post-backup mutation was discarded; outbox and receipt identities were neither lost nor duplicated |
| QA09-PARTIAL-CLOSURE - Interrupted event closure | PASS | a closure blocked mid-transaction completed atomically to READ_ONLY after the lock backend was released; a durable closure job and READ_ONLY effect were persisted; product writes were refused with 409 after closure; product reads continued |

## QA09-OUTBOX-REPLAY - Outbox replay after SQL reset and cluster restart

State: PASS

Actions:

```json
[
 {
  "owned_cluster": "/private/tmp/mcr-pg-wr_78sv5",
  "postgres": "16.14 (Homebrew)",
  "binary_sha256": "6182f440066d45810d2730633f0cb6c04552be23cd2e95e3da85906ebcd73fe1"
 },
 {
  "postmaster": [
   61894,
   62124
  ]
 },
 {
  "replay_fault": "SQL reset of the delivered outbox row to READY plus a full owned PostgreSQL cluster stop/start, not a process killed mid-publish"
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
 "outbox": "44b1ecf2-873d-4844-8cba-67a719c065a1|READY|1",
 "receipts": "1",
 "replayed_signal_receipts": "1",
 "persisted_signal": "44b1ecf2-873d-4844-8cba-67a719c065a1|ae4b27f6-e0a4-456e-82cd-6b710b7c7fe4"
}
```

After:

```text
{
 "outbox": "44b1ecf2-873d-4844-8cba-67a719c065a1|DELIVERED|2",
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

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.89s



running 1 test
test native::native_restart_durable_receipt_preserves_fresh_connection ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 3.83s


```

## QA09-DELETION - PrivateData owner deletion mount

State: PASS (isolated owner mount)

Actions:

```json
[
 {
  "status": "PASS",
  "source": 0,
  "after_pg_restart": 0,
  "restart": {
   "after_pid": 62518,
   "before_pid": 62376
  },
  "migration_receipts": {
   "auth": 19,
   "concert": 63
  },
  "migration_receipts_after": {
   "auth": 19,
   "concert": 63
  },
  "persisted_after_pg_restart": {
   "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
   "private_rows": "1|0|0",
   "saga": "IN_PROGRESS|",
   "steps": "CONCERT_PRIVATE_DATA|PENDING|0\nCONCERT_SESSIONS|PENDING|0\nCONCERT_SHARING|PENDING|0\nCONCERT_TICKETS|PENDING|0\nFRIEND|PENDING|0\nNOTIFICATION|PENDING|0\nPOST|PENDING|0\nPROFILE|PENDING|0"
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
 "control": {
  "age_owners": 0,
  "avatar_settings": 0,
  "participants": 1
 },
 "control_user": "4666014d-6999-42b5-a65f-08a0295fead5",
 "deleted": {
  "age_owners": 1,
  "avatar_settings": 1,
  "participants": 1
 },
 "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
 "phase": "before",
 "private_data_progress_completed": 0,
 "private_rows_remaining": 3,
 "saga_completed_at": null,
 "saga_status": "IN_PROGRESS",
 "steps": [
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_PRIVATE_DATA"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_SESSIONS"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_SHARING"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_TICKETS"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "FRIEND"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "NOTIFICATION"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "POST"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "PROFILE"
  }
 ]
}
```

After:

```text
{
 "control": {
  "age_owners": 0,
  "avatar_settings": 0,
  "participants": 1
 },
 "control_user": "4666014d-6999-42b5-a65f-08a0295fead5",
 "deleted": {
  "age_owners": 0,
  "avatar_settings": 0,
  "participants": 1
 },
 "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
 "phase": "after",
 "private_data_progress_completed": 1,
 "private_rows_remaining": 0,
 "saga_completed_at": null,
 "saga_status": "IN_PROGRESS",
 "steps": [
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_PRIVATE_DATA"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_SESSIONS"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_SHARING"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "CONCERT_TICKETS"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "FRIEND"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "NOTIFICATION"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "POST"
  },
  {
   "attempt_count": 0,
   "status": "PENDING",
   "target": "PROFILE"
  }
 ]
}
```

Database rows:

```text
[deletion_mount]
{
 "after_pg_restart": [
  {
   "control": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "control_user": "850ccd5f-cd47-4e48-b81b-34a6e2abdb00",
   "deleted": {
    "age_owners": 1,
    "avatar_settings": 1,
    "participants": 1
   },
   "deleted_user": "425fe4d2-d03a-49c5-9bf1-6df2ada48905",
   "phase": "before",
   "private_data_progress_completed": 0,
   "private_rows_remaining": 3,
   "saga_completed_at": null,
   "saga_status": "IN_PROGRESS",
   "steps": [
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_PRIVATE_DATA"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SESSIONS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SHARING"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_TICKETS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "FRIEND"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "NOTIFICATION"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "POST"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "PROFILE"
    }
   ]
  },
  {
   "control": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "control_user": "850ccd5f-cd47-4e48-b81b-34a6e2abdb00",
   "deleted": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "deleted_user": "425fe4d2-d03a-49c5-9bf1-6df2ada48905",
   "phase": "after",
   "private_data_progress_completed": 1,
   "private_rows_remaining": 0,
   "saga_completed_at": null,
   "saga_status": "IN_PROGRESS",
   "steps": [
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_PRIVATE_DATA"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SESSIONS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SHARING"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_TICKETS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "FRIEND"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "NOTIFICATION"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "POST"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "PROFILE"
    }
   ]
  }
 ],
 "source": [
  {
   "control": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "control_user": "4666014d-6999-42b5-a65f-08a0295fead5",
   "deleted": {
    "age_owners": 1,
    "avatar_settings": 1,
    "participants": 1
   },
   "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
   "phase": "before",
   "private_data_progress_completed": 0,
   "private_rows_remaining": 3,
   "saga_completed_at": null,
   "saga_status": "IN_PROGRESS",
   "steps": [
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_PRIVATE_DATA"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SESSIONS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SHARING"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_TICKETS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "FRIEND"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "NOTIFICATION"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "POST"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "PROFILE"
    }
   ]
  },
  {
   "control": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "control_user": "4666014d-6999-42b5-a65f-08a0295fead5",
   "deleted": {
    "age_owners": 0,
    "avatar_settings": 0,
    "participants": 1
   },
   "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
   "phase": "after",
   "private_data_progress_completed": 1,
   "private_rows_remaining": 0,
   "saga_completed_at": null,
   "saga_status": "IN_PROGRESS",
   "steps": [
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_PRIVATE_DATA"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SESSIONS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_SHARING"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "CONCERT_TICKETS"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "FRIEND"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "NOTIFICATION"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "POST"
    },
    {
     "attempt_count": 0,
     "status": "PENDING",
     "target": "PROFILE"
    }
   ]
  }
 ]
}
[persisted_after_pg_restart]
{
 "deleted_user": "e0abc1ba-a07e-48dc-b122-7613c79e286d",
 "private_rows": "1|0|0",
 "saga": "IN_PROGRESS|",
 "steps": "CONCERT_PRIVATE_DATA|PENDING|0\nCONCERT_SESSIONS|PENDING|0\nCONCERT_SHARING|PENDING|0\nCONCERT_TICKETS|PENDING|0\nFRIEND|PENDING|0\nNOTIFICATION|PENDING|0\nPOST|PENDING|0\nPROFILE|PENDING|0"
}
```

Commands:

| Command | Exit |
|---|---:|
| `account-deletion-mount` | 0 |

Logs:

```text

running 1 test
test mounted_private_data_ack_restart_and_session_revocation_boundary ... QA09_DELETION_PROBE {"control":{"age_owners":0,"avatar_settings":0,"participants":1},"control_user":"4666014d-6999-42b5-a65f-08a0295fead5","deleted":{"age_owners":1,"avatar_settings":1,"participants":1},"deleted_user":"e0abc1ba-a07e-48dc-b122-7613c79e286d","phase":"before","private_data_progress_completed":0,"private_rows_remaining":3,"saga_completed_at":null,"saga_status":"IN_PROGRESS","steps":[{"attempt_count":0,"status":"PENDING","target":"CONCERT_PRIVATE_DATA"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SESSIONS"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SHARING"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_TICKETS"},{"attempt_count":0,"status":"PENDING","target":"FRIEND"},{"attempt_count":0,"status":"PENDING","target":"NOTIFICATION"},{"attempt_count":0,"status":"PENDING","target":"POST"},{"attempt_count":0,"status":"PENDING","target":"PROFILE"}]}
QA09_DELETION_PROBE {"control":{"age_owners":0,"avatar_settings":0,"participants":1},"control_user":"4666014d-6999-42b5-a65f-08a0295fead5","deleted":{"age_owners":0,"avatar_settings":0,"participants":1},"deleted_user":"e0abc1ba-a07e-48dc-b122-7613c79e286d","phase":"after","private_data_progress_completed":1,"private_rows_remaining":0,"saga_completed_at":null,"saga_status":"IN_PROGRESS","steps":[{"attempt_count":0,"status":"PENDING","target":"CONCERT_PRIVATE_DATA"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SESSIONS"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SHARING"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_TICKETS"},{"attempt_count":0,"status":"PENDING","target":"FRIEND"},{"attempt_count":0,"status":"PENDING","target":"NOTIFICATION"},{"attempt_count":0,"status":"PENDING","target":"POST"},{"attempt_count":0,"status":"PENDING","target":"PROFILE"}]}
MOUNTED DELETION PASS: sealed PrivateData ACK over mTLS, caller refusal before deleter, zero unrevoked sessions, tampered source refusal, restart replay without duplicate effects, zero private rows, control intact
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 6.41s



running 1 test
test mounted_private_data_ack_restart_and_session_revocation_boundary ... QA09_DELETION_PROBE {"control":{"age_owners":0,"avatar_settings":0,"participants":1},"control_user":"850ccd5f-cd47-4e48-b81b-34a6e2abdb00","deleted":{"age_owners":1,"avatar_settings":1,"participants":1},"deleted_user":"425fe4d2-d03a-49c5-9bf1-6df2ada48905","phase":"before","private_data_progress_completed":0,"private_rows_remaining":3,"saga_completed_at":null,"saga_status":"IN_PROGRESS","steps":[{"attempt_count":0,"status":"PENDING","target":"CONCERT_PRIVATE_DATA"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SESSIONS"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SHARING"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_TICKETS"},{"attempt_count":0,"status":"PENDING","target":"FRIEND"},{"attempt_count":0,"status":"PENDING","target":"NOTIFICATION"},{"attempt_count":0,"status":"PENDING","target":"POST"},{"attempt_count":0,"status":"PENDING","target":"PROFILE"}]}
QA09_DELETION_PROBE {"control":{"age_owners":0,"avatar_settings":0,"participants":1},"control_user":"850ccd5f-cd47-4e48-b81b-34a6e2abdb00","deleted":{"age_owners":0,"avatar_settings":0,"participants":1},"deleted_user":"425fe4d2-d03a-49c5-9bf1-6df2ada48905","phase":"after","private_data_progress_completed":1,"private_rows_remaining":0,"saga_completed_at":null,"saga_status":"IN_PROGRESS","steps":[{"attempt_count":0,"status":"PENDING","target":"CONCERT_PRIVATE_DATA"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SESSIONS"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_SHARING"},{"attempt_count":0,"status":"PENDING","target":"CONCERT_TICKETS"},{"attempt_count":0,"status":"PENDING","target":"FRIEND"},{"attempt_count":0,"status":"PENDING","target":"NOTIFICATION"},{"attempt_count":0,"status":"PENDING","target":"POST"},{"attempt_count":0,"status":"PENDING","target":"PROFILE"}]}
MOUNTED DELETION PASS: sealed PrivateData ACK over mTLS, caller refusal before deleter, zero unrevoked sessions, tampered source refusal, restart replay without duplicate effects, zero private rows, control intact
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 8.53s


```

## QA09-BASELINE-PICKUP - Real pickup baseline

State: PASS

Actions:

```json
[
 {
  "giveaway_create_attempts": 1
 },
 {
  "pickup_meeting": "baseline",
  "status": 200,
  "body": {
   "giveaway_id": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6",
   "meeting": {
    "expires_at": "2026-11-06T20:44:29.908073Z",
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
  "giveaway": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6",
  "reservation": "729fa094-ad2c-4ff2-92fa-f12251e38f75",
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
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de",
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
   "pid": 62890,
   "forced": false,
   "exit_code": 0
  }
 },
 {
  "start": "api-gateway",
  "pid": 63494
 },
 {
  "recovered_services": {
   "profile-service": 62881,
   "notification-service": 62882,
   "concert-service": 62883,
   "friend-service": 62884,
   "post-service": 62887,
   "auth-service": 62888,
   "api-gateway": 63494
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
    62890,
    63494
   ]
  },
  "unchanged_rows": [
   "event",
   "giveaways",
   "reservations",
   "outbox_ids",
   "receipts",
   "closure_jobs",
   "closure_effects"
  ]
 }
]
```

Before:

```text
{
 "event": "OPEN|1",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de",
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
  "giveaway_create_attempts": 1
 },
 {
  "pickup_meeting": "pending",
  "status": 200,
  "body": {
   "giveaway_id": "f5485121-1014-4c45-ae99-485269c0ff77",
   "meeting": {
    "expires_at": "2026-11-06T20:44:33.015436Z",
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
  "giveaway": "f5485121-1014-4c45-ae99-485269c0ff77",
  "reservation": "9d93b48a-a02e-4250-8617-3d0dba2801f4",
  "status": 200
 },
 {
  "pending_claim": "request blocked in its database transaction"
 },
 {
  "blocked_claim_outcome": {
   "status": 200,
   "body": {
    "claimed_at": "2026-11-06T17:44:34.398373Z",
    "giveaway_id": "f5485121-1014-4c45-ae99-485269c0ff77",
    "receipt_id": "880620f3-0f3d-4e6e-8c9e-c3714bbd9fa1",
    "reservation_id": "9d93b48a-a02e-4250-8617-3d0dba2801f4",
    "revision": 3,
    "state": "CLAIMED"
   }
  }
 },
 {
  "stop": "api-gateway",
  "result": {
   "pid": 63494,
   "forced": false,
   "exit_code": 0
  }
 },
 {
  "start": "api-gateway",
  "pid": 63605
 },
 {
  "recovered_services": {
   "profile-service": 62881,
   "notification-service": 62882,
   "concert-service": 62883,
   "friend-service": 62884,
   "post-service": 62887,
   "auth-service": 62888,
   "api-gateway": 63605
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
    "claimed_at": "2026-11-06T17:44:34.398373Z",
    "giveaway_id": "f5485121-1014-4c45-ae99-485269c0ff77",
    "receipt_id": "880620f3-0f3d-4e6e-8c9e-c3714bbd9fa1",
    "reservation_id": "9d93b48a-a02e-4250-8617-3d0dba2801f4",
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
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de",
 "receipts": "claimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
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
19e96785-a8fe-4270-bcb6-2e1c161fe699
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
   "marker": "6e530ad1-6415-49ca-8101-523a190fd18b",
   "postmaster": 62833
  }
 },
 {
  "postmaster": [
   62833,
   63683
  ],
  "in_flight_exit": 2
 },
 {
  "recovered_services": {
   "profile-service": 62881,
   "notification-service": 62882,
   "concert-service": 63835,
   "friend-service": 62884,
   "post-service": 62887,
   "auth-service": 62888,
   "api-gateway": 63605
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
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
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
.0 (clang-2100.0.123.102), 64-bit
2026-11-07 02:44:14.578 KST [62763] LOG:  listening on Unix socket "/private/tmp/mcr-pg-jz86r_zi/socket/.s.PGSQL.5432"
2026-11-07 02:44:14.583 KST [62766] LOG:  database system was shut down at 2026-11-07 02:44:14 KST
2026-11-07 02:44:14.587 KST [62763] LOG:  database system is ready to accept connections
2026-11-07 02:44:15.237 KST [62763] LOG:  received fast shutdown request
2026-11-07 02:44:15.237 KST [62763] LOG:  aborting any active transactions
2026-11-07 02:44:15.238 KST [62763] LOG:  background worker "logical replication launcher" (PID 62769) exited with exit code 1
2026-11-07 02:44:15.238 KST [62764] LOG:  shutting down
2026-11-07 02:44:15.238 KST [62764] LOG:  checkpoint starting: shutdown immediate
2026-11-07 02:44:15.241 KST [62764] LOG:  checkpoint complete: wrote 84 buffers (0.5%); 0 WAL file(s) added, 0 removed, 0 recycled; write=0.002 s, sync=0.002 s, total=0.004 s; sync files=39, longest=0.001 s, average=0.001 s; distance=52 kB, estimate=52 kB; lsn=0/3DADD60, redo lsn=0/3DADD60
2026-11-07 02:44:15.243 KST [62763] LOG:  database system is shut down
2026-11-07 02:44:15.384 KST [62833] LOG:  starting PostgreSQL 16.14 (Homebrew) on aarch64-apple-darwin25.4.0, compiled by Apple clang version 21.0.0 (clang-2100.0.123.102), 64-bit
2026-11-07 02:44:15.385 KST [62833] LOG:  listening on Unix socket "/private/tmp/mcr-pg-jz86r_zi/socket/.s.PGSQL.5432"
2026-11-07 02:44:15.393 KST [62837] LOG:  database system was shut down at 2026-11-07 02:44:15 KST
2026-11-07 02:44:15.397 KST [62833] LOG:  database system is ready to accept connections
2026-11-07 02:44:30.124 KST [62930] WARNING:  there is no transaction in progress
2026-11-07 02:44:30.139 KST [62899] WARNING:  there is no transaction in progress
2026-11-07 02:44:30.739 KST [62930] WARNING:  there is no transaction in progress
2026-11-07 02:44:30.747 KST [62899] WARNING:  there is no transaction in progress
2026-11-07 02:44:31.253 KST [62927] WARNING:  there is no transaction in progress
2026-11-07 02:44:32.293 KST [62927] WARNING:  there is no transaction in progress
2026-11-07 02:44:32.301 KST [62885] WARNING:  there is no transaction in progress
2026-11-07 02:44:32.720 KST [62931] WARNING:  there is no transaction in progress
2026-11-07 02:44:32.751 KST [62885] WARNING:  there is no transaction in progress
2026-11-07 02:44:33.204 KST [62927] WARNING:  there is no transaction in progress
2026-11-07 02:44:33.212 KST [62899] WARNING:  there is no transaction in progress
2026-11-07 02:44:33.568 KST [62930] WARNING:  there is no transaction in progress
2026-11-07 02:44:33.575 KST [62899] WARNING:  there is no transaction in progress
2026-11-07 02:44:34.018 KST [62930] WARNING:  there is no transaction in progress
2026-11-07 02:44:34.344 KST [63583] FATAL:  terminating connection due to administrator command
2026-11-07 02:44:34.344 KST [63583] STATEMENT:  BEGIN; select 1 from concert_community_giveaways where giveaway_id='f5485121-1014-4c45-ae99-485269c0ff77' for update; SELECT pg_sleep(300);
2026-11-07 02:44:35.323 KST [62833] LOG:  received immediate shutdown request
2026-11-07 02:44:35.328 KST [62833] LOG:  database system is shut down
2026-11-07 02:44:35.474 KST [63683] LOG:  starting PostgreSQL 16.14 (Homebrew) on aarch64-apple-darwin25.4.0, compiled by Apple clang version 21.0.0 (clang-2100.0.123.102), 64-bit
2026-11-07 02:44:35.474 KST [63683] LOG:  listening on Unix socket "/private/tmp/mcr-pg-jz86r_zi/socket/.s.PGSQL.5432"
2026-11-07 02:44:35.479 KST [63686] LOG:  database system was interrupted; last known up at 2026-11-07 02:44:15 KST
2026-11-07 02:44:35.479 KST [63687] FATAL:  the database system is starting up
2026-11-07 02:44:35.486 KST [63688] FATAL:  the database system is starting up
2026-11-07 02:44:35.488 KST [63689] FATAL:  the database system is starting up
2026-11-07 02:44:35.493 KST [63690] FATAL:  the database system is starting up
2026-11-07 02:44:35.500 KST [63691] FATAL:  the database system is starting up
2026-11-07 02:44:35.502 KST [63692] FATAL:  the database system is starting up
2026-11-07 02:44:35.518 KST [63694] FATAL:  the database system is starting up
2026-11-07 02:44:35.523 KST [63695] FATAL:  the database system is starting up
2026-11-07 02:44:35.525 KST [63696] FATAL:  the database system is starting up
2026-11-07 02:44:35.555 KST [63697] FATAL:  the database system is starting up
2026-11-07 02:44:35.562 KST [63698] FATAL:  the database system is starting up
2026-11-07 02:44:35.567 KST [63699] FATAL:  the database system is starting up
2026-11-07 02:44:35.568 KST [63700] FATAL:  the database system is starting up
2026-11-07 02:44:35.568 KST [63701] FATAL:  the database system is starting up
2026-11-07 02:44:35.591 KST [63702] FATAL:  the database system is starting up
2026-11-07 02:44:35.634 KST [63703] FATAL:  the database system is starting up
2026-11-07 02:44:35.645 KST [63704] FATAL:  the database system is starting up
2026-11-07 02:44:35.652 KST [63705] FATAL:  the database system is starting up
2026-11-07 02:44:35.652 KST [63706] FATAL:  the database system is starting up
2026-11-07 02:44:35.657 KST [63686] LOG:  database system was not properly shut down; automatic recovery in progress
2026-11-07 02:44:35.659 KST [63686] LOG:  redo starts at 0/3DADDD8
2026-11-07 02:44:35.668 KST [63686] LOG:  invalid record length at 0/4033320: expected at least 24, got 0
2026-11-07 02:44:35.668 KST [63686] LOG:  redo done at 0/4032E08 system usage: CPU: user: 0.00 s, system: 0.00 s, elapsed: 0.00 s
2026-11-07 02:44:35.672 KST [63684] LOG:  checkpoint starting: end-of-recovery immediate wait
2026-11-07 02:44:35.687 KST [63684] LOG:  checkpoint complete: wrote 755 buffers (4.6%); 0 WAL file(s) added, 0 removed, 1 recycled; write=0.008 s, sync=0.007 s, total=0.015 s; sync files=266, longest=0.001 s, average=0.001 s; distance=2581 kB, estimate=2581 kB; lsn=0/4033320, redo lsn=0/4033320
2026-11-07 02:44:35.690 KST [63683] LOG:  database system is ready to accept connections

```

## QA09-DELETION-SAGA - Live seven-service deletion saga

State: NOT_RUN

Blocker:

```text
the live seven-service saga ran but did not converge within the 30 s bound: CONCERT_PRIVATE_DATA=RETRY/attempts=3/error=Dependency unavailable; CONCERT_SESSIONS=RETRY/attempts=3/error=Dependency unavailable; CONCERT_SHARING=RETRY/attempts=3/error=Dependency unavailable; CONCERT_TICKETS=RETRY/attempts=3/error=Dependency unavailable
```

Actions:

```json
[
 {
  "auth_worker_env": {
   "AUTH_USER_DELETION_WORKER_ENABLED": "true",
   "AUTH_USER_DELETION_WORKER_INTERVAL_MS": "60000"
  },
  "worker_overrides": {
   "AUTH_USER_DELETION_WORKER_ENABLED": "true",
   "AUTH_USER_DELETION_WORKER_INTERVAL_MS": "250",
   "AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS": "8"
  }
 },
 {
  "delete": {
   "status": 202,
   "body": {
    "userId": "f385dd27-9272-4476-b104-f84ef841a123",
    "deleted": true,
    "sagaId": "0d862787-e531-4c47-ace3-2c881a196f60",
    "saga": {
     "id": "0d862787-e531-4c47-ace3-2c881a196f60",
     "userId": "f385dd27-9272-4476-b104-f84ef841a123",
     "status": "IN_PROGRESS",
     "retentionUntil": "2026-12-06T17:44:41.359577+00:00",
     "createdAt": "2026-11-06T17:44:41.359577+00:00",
     "updatedAt": "2026-11-06T17:44:41.359577+00:00",
     "completedAt": null,
     "steps": [
      {
       "target": "CONCERT_PRIVATE_DATA",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "CONCERT_SESSIONS",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "CONCERT_SHARING",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "CONCERT_TICKETS",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "FRIEND",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "NOTIFICATION",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "POST",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      },
      {
       "target": "PROFILE",
       "status": "PENDING",
       "attemptCount": 0,
       "nextAttemptAt": "2026-11-06T17:44:41.359577+00:00",
       "lastError": null,
       "completedAt": null
      }
     ]
    }
   }
  }
 },
 {
  "kill": "auth-service",
  "pid": 62888,
  "signal": "SIGKILL"
 },
 {
  "start": "auth-service",
  "pid": 63974
 },
 {
  "recovered_services": {
   "profile-service": 62881,
   "notification-service": 62882,
   "concert-service": 64070,
   "friend-service": 62884,
   "post-service": 62887,
   "auth-service": 63974,
   "api-gateway": 63605
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
  "auth_process_worker_env": [
   "AUTH_CONCERT_DELETION_MODE=managed",
   "AUTH_CONCERT_GRPC_ENDPOINT=https://127.0.0.1:60263",
   "AUTH_USER_DELETION_WORKER_ENABLED=true",
   "AUTH_USER_DELETION_WORKER_INTERVAL_MS=250",
   "AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS=8"
  ],
  "concert_process_deletion_env": [
   "CONCERT_ACCOUNT_DELETION_MODE=managed",
   "CONCERT_AUTH_GRPC_ENDPOINT=https://127.0.0.1:60273"
  ]
 }
]
```

Before:

```text
{
 "rows": {
  "deleted_participants": 1,
  "control_participants": 1,
  "deleted_avatar_settings": 1,
  "control_avatar_settings": 1,
  "deleted_age_owners": 1,
  "control_age_owners": 0
 },
 "auth": "ACTIVE|"
}
```

After:

```text
{
 "rows": {
  "deleted_participants": 1,
  "control_participants": 1,
  "deleted_avatar_settings": 1,
  "control_avatar_settings": 1,
  "deleted_age_owners": 1,
  "control_age_owners": 0
 },
 "auth": "DELETED|2026-11-07 02:44:41.359577+09",
 "saga": "0d862787-e531-4c47-ace3-2c881a196f60|IN_PROGRESS|",
 "steps": [
  {
   "target": "CONCERT_PRIVATE_DATA",
   "status": "RETRY",
   "attempt_count": 3,
   "last_error": "Dependency unavailable"
  },
  {
   "target": "CONCERT_SESSIONS",
   "status": "RETRY",
   "attempt_count": 3,
   "last_error": "Dependency unavailable"
  },
  {
   "target": "CONCERT_SHARING",
   "status": "RETRY",
   "attempt_count": 3,
   "last_error": "Dependency unavailable"
  },
  {
   "target": "CONCERT_TICKETS",
   "status": "RETRY",
   "attempt_count": 3,
   "last_error": "Dependency unavailable"
  },
  {
   "target": "FRIEND",
   "status": "SUCCEEDED",
   "attempt_count": 1,
   "last_error": ""
  },
  {
   "target": "NOTIFICATION",
   "status": "SUCCEEDED",
   "attempt_count": 1,
   "last_error": ""
  },
  {
   "target": "POST",
   "status": "SUCCEEDED",
   "attempt_count": 1,
   "last_error": ""
  },
  {
   "target": "PROFILE",
   "status": "SUCCEEDED",
   "attempt_count": 1,
   "last_error": ""
  }
 ]
}
```

Database rows:

```text
[deleted_participants_before]
1
[control_participants_before]
1
[deleted_avatar_settings_before]
1
[control_avatar_settings_before]
1
[deleted_age_owners_before]
1
[control_age_owners_before]
0
[auth_before]
ACTIVE|
[saga_after_delete]
0d862787-e531-4c47-ace3-2c881a196f60|IN_PROGRESS|
[steps_after_delete]
CONCERT_PRIVATE_DATA|PENDING|0|
CONCERT_SESSIONS|PENDING|0|
CONCERT_SHARING|PENDING|0|
CONCERT_TICKETS|PENDING|0|
FRIEND|PENDING|0|
NOTIFICATION|PENDING|0|
POST|PENDING|0|
PROFILE|PENDING|0|
[steps_due_check]
2026-11-07 02:44:41.417993+09|true
[saga_after_worker_restart]
0d862787-e531-4c47-ace3-2c881a196f60|IN_PROGRESS|
[steps_after_worker_restart]
CONCERT_PRIVATE_DATA|RETRY|3|Dependency unavailable
CONCERT_SESSIONS|RETRY|3|Dependency unavailable
CONCERT_SHARING|RETRY|3|Dependency unavailable
CONCERT_TICKETS|RETRY|3|Dependency unavailable
FRIEND|SUCCEEDED|1|
NOTIFICATION|SUCCEEDED|1|
POST|SUCCEEDED|1|
PROFILE|SUCCEEDED|1|
[deleted_participants_after]
1
[control_participants_after]
1
[deleted_avatar_settings_after]
1
[control_avatar_settings_after]
1
[deleted_age_owners_after]
1
[control_age_owners_after]
0
[auth_after]
DELETED|2026-11-07 02:44:41.359577+09
```

Commands:

| Command | Exit |
|---|---:|
| `restart-auth-service` | 0 |

Logs:

```text

CONCERT_HEALTH_LISTENER=127.0.0.1:60264
CONCERT_WORLD_LISTENER=127.0.0.1:60265

```

## QA09-BACKUP-RESTORE - Logical backup and restore

State: PASS

Actions:

```json
[
 {
  "backup": {
   "bytes": 783491,
   "sha256": "bdcf7958db0a9e68fbae624028ea46c7772c71bb90ecd09c17f2dfd30c14ea37",
   "command": "/opt/homebrew/opt/postgresql@16/bin/pg_dump -h /private/tmp/mcr-pg-jz86r_zi/socket -p 5432 -U mcr_owner -Fc -f /private/tmp/mcr-pg-jz86r_zi/concert-backup.dump mcr_concert --no-password",
   "path": "/private/tmp/mcr-pg-jz86r_zi/concert-backup.dump"
  }
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
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "OPEN|1",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

Measured backup:

```json
{
 "bytes": 783491,
 "command": "/opt/homebrew/opt/postgresql@16/bin/pg_dump -h /private/tmp/mcr-pg-jz86r_zi/socket -p 5432 -U mcr_owner -Fc -f /private/tmp/mcr-pg-jz86r_zi/concert-backup.dump mcr_concert --no-password",
 "path": "/private/tmp/mcr-pg-jz86r_zi/concert-backup.dump",
 "sha256": "bdcf7958db0a9e68fbae624028ea46c7772c71bb90ecd09c17f2dfd30c14ea37"
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
   "profile-service": 62881,
   "notification-service": 62882,
   "concert-service": 64070,
   "friend-service": 62884,
   "post-service": 62887,
   "auth-service": 63974,
   "api-gateway": 63605
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
    "job_id": "f9cbab42-d795-5f7f-b0c3-d027e58f22ca",
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
    "timestamp": "2026-11-06T17:44:48.254Z",
    "traceId": "65ed42e0-e697-4a62-a3ff-da3a0450003c"
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
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|OPEN|5|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|OPEN|5|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "",
 "closure_effects": ""
}
```

After:

```text
{
 "event": "READ_ONLY|2",
 "giveaways": "ea14d5b2-8a59-4699-8d66-a7e36cb3fdd6|CANCELLED|6|0|1\nf5485121-1014-4c45-ae99-485269c0ff77|CANCELLED|6|0|1",
 "reservations": "729fa094-ad2c-4ff2-92fa-f12251e38f75|CLAIMED|3\n9d93b48a-a02e-4250-8617-3d0dba2801f4|CLAIMED|3",
 "outbox_ids": "0dcff66f-b6ae-4d53-afd4-c3dccbe8036a,277f7639-d14c-4265-bbe4-cba5466ed300,2885daa4-cd76-460b-94e4-e0564623290e,395062f9-d675-4b0a-888b-8573600c71c3,4c9a292a-ba1e-4263-b3e8-97c159085480,61067aaa-d294-4e39-8db2-4311e9cbb1d2,6ed09497-bf7d-4a26-af19-ffe33ba0b8e3,84f89f39-da7b-4616-b7af-06bc797b6405,89af7769-3f25-4c3f-8240-f2d96bfa994c,a40a5f04-15d3-4bc9-b7aa-6b0c01e212de,aa0fcb76-f1ab-4405-aaa4-65d440e72218,c4533723-beac-4192-8564-e3a9a66f635b,e9a06c84-ff1b-4e79-900a-0f9149be0bc9",
 "receipts": "claimConcertReservation|SUCCEEDED|\nclaimConcertReservation|SUCCEEDED|\ncloseConcertEvent|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\ncreateConcertGiveaway|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nissueConcertPickupCode|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nreserveConcertGiveaway|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nsetConcertPickupMeeting|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|\nupdateConcertGiveaway|SUCCEEDED|",
 "closure_jobs": "f9cbab42-d795-5f7f-b0c3-d027e58f22ca|PENDING|1",
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
f9cbab42-d795-5f7f-b0c3-d027e58f22ca|PENDING|1
```


## Method and limits

### Deletion

- `QA09-DELETION` is `PASS (isolated owner mount)`: the committed production Auth deletion source and the mounted production Concert PrivateData factory ran over a test-owned mTLS loopback listener. The after-state removes the avatar and age-owner rows and leaves one sanitized `concert_participants` row, so the owner's `private_rows_remaining` predicate is 0 while the raw participant count is 1. The control user is intact, the sealed acknowledgement replays after a service restart without rewriting its terminal receipt, and the same case re-runs after a physical PostgreSQL restart. It is not eight-target saga convergence.
- `QA09-DELETION-SAGA` records the live seven-service saga exactly as measured: per-step status, `attempt_count` and `last_error` are in that section. A failure or non-completion there is not deletion convergence; only a `PASS` on that row would be. No deletion-convergence claim is made for the live stack unless it is `PASS`.
- The isolated mount's own canonical saga likewise stays `PENDING`; it proves the PrivateData owner, not the saga.

### Outbox replay

- The replay is a SQL reset of one delivered outbox row to `READY` plus a full PostgreSQL cluster stop/start, followed by a fresh process and connection that claims the row again. It is not a process killed mid-publish; no mid-publish SIGKILL is claimed.
- A real mid-publish kill was not added: the native test's apply/ack boundary is in-process with no pause hook, and its restart fixture deliberately persists the replay identity before the SQL reset. The SQL reset plus cluster restart is the fault this harness exercises.
- The same native test also proves a failed second fanout leg leaves the row `LEASED` (no false ack) until the retry succeeds; the replay then returns the row to `DELIVERED` with exactly one consumer receipt and one new attempt.
- It is the native World invalidation consumer, not the seven-service social worker's live pickup outbox path.

### Database restart

- The database restart is an immediate stop of the owned postmaster; it verifies crash recovery, in-flight rollback and committed-row survival, not disk failure or a standby.

### Logical backup

- The backup is a logical `pg_dump -Fc` of the Concert owner database into a fresh owned database, restored with `pg_restore`. The exact command, measured byte count and SHA-256 are in the QA09-BACKUP-RESTORE section; the dump is removed after the restore.
- It is not a physical restore of the live cluster and does not back up the Auth database. `pg_dump -Fc` output is not byte-stable across runs; the published size is the measured size of this run.

### Pending pickup

- The scenario blocks a real claim on the giveaway row, terminates the lock backend, lets the claim complete, restarts the gateway, and replays the same idempotency key. A validation retry was not needed because the fixed release let the claim complete; this does not claim a concert-process crash mid-claim.

### Partial closure

- The closure transaction is blocked, released atomically, and verified to enter `READ_ONLY` with a durable job, 409 writes and 200 reads. Later FREEZE_MURAL..CLEANUP owners and the public archive are not claimed; the live composition reports them unavailable. No concert-process restart after closure is claimed: a restarted concert fails World admission because the closed event has retired its room leases.

### Gateway restart

- The gateway restart compares committed event, giveaway, reservation, outbox identity, command receipt, closure job and closure effect rows, not only giveaway and reservation rows.

### Other limits

- The stack is the development seven-service composition on an owned PostgreSQL 16.14 cluster with loopback TLS. It is not a deployment, capacity, multiuser race, browser/device, or production approval.
- The baseline uses the real giveaway, meeting, reservation, pickup-code and claim flow rather than the original party concurrency gate; QA03 owns race acceptance.
- No QA08 performance, 200-user, browser, or device acceptance is claimed.

## Mutation proof

The outbox stale-lease fence was broken in a throwaway worktree
`/tmp/qa09-mutation` at base `80b03639d`. In
`backend/services/concert-service/src/adapters/out/repository/outbox.rs`,
`acknowledge` was reduced to `where outbox_id=$1 and state='LEASED'`, so a
holder whose lease was reclaimed could still acknowledge the delivery.

- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa09 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -j3 -p concert-service --test maintenance_outbox --no-run` exited 0.
- Mutated run (`maintenance_origin_transactions_and_leases` on a fresh owned PG16 cluster): exit 101, `test result: FAILED. 0 passed; 1 failed`, panic at `services/concert-service/tests/maintenance_outbox.rs:104: assertion failed: !box_.acknowledge(first).await.unwrap()`. The stale holder acknowledged after the lease rotated, which is exactly the recovery invariant the outbox replay depends on.
- Baseline run (copied unmodified binary): exit 0, `test result: ok. 1 passed; 0 failed`.
- Restore: the throwaway worktree was removed; the delivery worktree source is unchanged. The baseline binary is retained at `/tmp/qa09-trial/maintenance_outbox-baseline`.
## Cleanup

- Final stack report cleanup: PASS; the owned cluster root and its service children were removed.
- Native outbox and deletion mount runs removed their owned cluster roots; the mutation runs removed theirs.
- The throwaway mutation worktree was removed. No delivery source outside this harness and evidence was changed.

