# BE15/QA09 canonical durable owner backup/restore segment

Status: IN_PROGRESS. Source harness implemented; build/native validation NOT_RUN.
This is a supporting segment, not acceptance of full BE15 or QA09.

## Owned scope and preserved protocol

New worktree/branch: `be-canonical-owner-restore` /
`codex/mcr-be-BE15-QA09-owner-restore`, based on qualified CF01
`3afb8111d8ef206f3e73c63186699c98d9fd4997`. Only the new
`tests/canonical_owner_restore.rs`, new `tests/canonical_owner_restore_support/`
and this document are writable. Production composition, main, registry,
protocols, migrations, manifests, previous fixtures and QA09 central recovery
scripts are unchanged. Root retains production composition and /me ownership.

The existing persistent full canonical-owner goal is aligned and reused. Its
historical metadata was BLOCKED on deferred intake; this assignment explicitly
resumes source execution, without falsely claiming ACTIVE or resetting it.
Completion still requires the larger owner/protocol/runtime/master evidence.

## Physical boundary and cases

The runner creates a new marker/inode-owned socket-only PostgreSQL 16 cluster
with five isolated owner databases and actual mTLS owner transports. A source
Root test process constructs the real CanonicalOwnerComposition, acquires the
actual Auth/Friend scope, and retains actual Local/Profile prepared children.
It persists the original registered native/child state and a private operator
handoff. The controller SIGKILLs/reaps that exact process, stops/reaps the exact
source postmaster, copies the entire stopped data directory into a distinct
backup, verifies every regular file hash, then restores the backup into another
fresh marker/inode-owned directory. Only one postmaster runs at a time.

The restored process is a distinct PID. Every database OID and system identifier,
actual pg_xact state, original prepared GIDs/XIDs, encrypted child references,
producer UUID and independently retained original recovery keys must match. New
transport endpoints do not grant an expired request authority. Rotated recovery
keys must retain the exact original key; missing-key and wrong-instance attempts
cannot ACK a child or release a fence. The immutable backup must remain byte
identical while recovery runs against its restored copy.

| Case | Actual source state | Required restored outcome |
| --- | --- | --- |
| FINISHED | Actual native registration rolled back; real outer body EOF finishes Root; negative Profile Complete retains Profile after the earlier Local child has actually settled | Rotated keyring retaining the original lets the real composite worker settle the remaining actual prepared Profile child; repeated recovery adds no ACK/effect; all native prepared work reaches zero |
| ACTIVE | Actual owner scope and children; inner serialization only, no outer terminal proof | Original expiry and restart do not finish Root; owner recovery and child worker retain all four native fences and both pending children |
| MISSING_WITNESS | Actual registered transaction commits a test-owned effect once; actual outer EOF finishes Root; native resolver cannot certify a commit witness | Committed SQL remains committed/one effect; authenticated owner-to-root resolver calls return UNKNOWN, owner registrations remain PENDING/fenced; eligible children may settle, and repeated recovery creates no duplicate effect |

All cases reject restored in-memory authority and DeliveryCompleted when the
original registry was lost. Child settlement is not reconstructed delivery or
fresh content permission. No test manually rolls back prepared transactions or
writes lifecycle phases/ACKs; production recovery performs supported rollbacks.
Negative cases retain quarantined prepared work until exact owned-cluster
teardown, which is cleanup rather than native settlement proof.

## Backup rationale and production prerequisites

PostgreSQL requires a shutdown for a plain file copy and a whole cluster because
transaction status in pg_xact belongs with the table/catalog files. Its prepared
transaction state spans WAL/shared state and pg_twophase at checkpoints. The
harness chooses the explicit offline full-copy method; no pg_dump or live-copy
consistency is inferred. See [PostgreSQL 16 file backup](https://www.postgresql.org/docs/16/backup-file.html)
and [two-phase state](https://www.postgresql.org/docs/16/two-phase.html).

A production committed-effect resolver needs the actual producer-specific
same-transaction receipt/witness; PgNativeAbortResolver intentionally cannot
supply it. The missing-witness case tests that refusal, rather than fabricating
a receipt or widening factory ownership. The test-owned SQL effect counter is a
native durability/no-reexecution observation, not a domain outbox consumer or
business pickup receipt. Separate owner clusters need a coordinated operational
backup/recovery plan; this harness covers one exact five-database cluster.

## Planned finite validation

Only the exact integration target `canonical_owner_restore` is required:
`cargo +1.88.0 test --locked --offline -p concert-service --test canonical_owner_restore --no-run -j 2 --message-format=json`,
with debug 0, incremental off and the retained contracts-build opt-level 2.
Freeze the single executable/full source and live baked migration pins outside
the named target, explicitly release all target handles, then run
`canonical_owner_restore_support/qualify.py` with an exact source checkpoint,
explicit PG16 binary directory and new private report. The runner executes the
one exact ignored process host across three source/restore case pairs.

No Cargo/shared target/PostgreSQL/services/browser/heavy execution is authorized
until an exact finite READY request receives and accepts a named resource offer.
No new target, dependency installation or unrelated package cleanup is needed
by this source plan. Actual commands/exits/results will replace NOT_RUN only
when that boundary has executed. Provenance for adapted support is recorded in
`canonical_owner_restore_support/provenance.json`.
