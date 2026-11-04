# BE41 event cleanup runtime

Status: **BLOCKED**. Original BE41 remains unfinished. This successor adds durable local batch progress; it does not turn missing production authority or owner implementations into successful lifecycle effects.

The binding requirement is [original BE41](../../source/mcr_v2_en/generated/DEVELOPER_TASKS_EN.md#be41--event-closure-and-archive-jobs): idempotent read-only transition, permitted archive projection, grant/beacon expiry, resumable retention jobs, recovery after partial failure, and exclusion of private locations, recipients and handles from the public archive. No scope reduction has been accepted.

## Source and ownership

Base: `3471190db5a5024f5376c92b282af797b7063c72`. Isolated branch: `codex/mcr-be-event-cleanup-BE41`. The prior Age and browser checkouts and frozen artifacts remain preserved and read-only.

Retained BE41 delivered record: `a93a70ec2b692e4837aa78b0350b5c6cebdec459`; authority race correction: `3a6957e294a9482d6b97d9bf7194a560de81e3b9`. Existing `event_lifecycle`, privacy, archive, World, community, cleanup worker and Gift files are reused without edits. The former BE41 writer formally stopped and returned that scope; this does not transfer permission to change those files.

New implementation is limited to application `event_cleanup`, service `event_cleanup_live`, `config/event_cleanup.rs`, their exact tests/support, migration `0046_event_cleanup_runtime.sql`, `scripts/mcr/event_cleanup`, this document and three additive module exports. Shared manifests, main/public dispatch, role acquisition, router and CI remain outside this assignment.

## Durable behavior

`BatchRequest` is immutable. Its SHA-256 binding includes actor, event, job, lifecycle step, closed owner enum, the exact pinned retention tuple, page ordinal, limit and cursor digest. A stable operation UUID correlates retries of that exact page. Neither a UUID nor an affected count grants authority or proves an owner effect.

`CleanupRuntime::run_owner` locks event, obtains current privileged lifecycle authority, locks the original closure job and checks current step, initiating actor, reviewed retention and the actual database deadline. No cached START grant authorizes background work. The limit is explicitly 1–100; cleanup cannot run before the pinned policy deadline.

Each successful page stores its actual verified owner receipt in `concert_event_cleanup_batches`. An insert trigger advances `concert_event_cleanup_progress` in the same native transaction. The immutable ledger and constant-size head restore the next cursor after worker/process/database restart. SQL rejects mismatched scope, stale page ordinals, repeated cursors, row counts above the limit, false direct completion, deleted/rewritten/truncated evidence, and expired authority at commit. Opaque cursors stay in private storage; public `Progress` carries only owner, state and processed count.

A failed effect or receipt verification rolls back its savepoint and speculative in-memory cursor, records partial failure at the same operation, and can retry. An unconfirmed `Pending` outcome rolls back speculative owner SQL. Missing owner bindings remain pending. Neither path advances the original seven-step job or emits its success audit.

`LocalOwner` is an explicit trusted owning-adapter boundary. `apply_in` must perform local effects on the supplied native connection; `verify_in` must verify the actual durable owner result and hold current source/revocation fences through commit. Register actual owner transactions with canonical `OwnerGuardScope` whenever that protocol is used. Fabricated scopes, detached transactions, deadline-only assertions and network deletion inside a local transaction are invalid. Remote owners need their real durable intent/observation protocol outside that transaction; absent implementations remain unavailable.

`ConvergedOwners` implements retained `LifecycleOwners` only for later steps. It requires every fixed required owner to be complete and verifies their final receipts again on the original caller connection. Root must use retained `ReadOnlyCommandDriver` for START/RESUME. `CleanupRuntime::tick` reads that driver, preserves completed and explicitly resumable parent states, processes bounded owner pages and calls retained `run_next` only after `CleanupRuntime::ready` succeeds. Root must bind the same actual event store, current authority and owner registry to both objects. The retained driver rechecks final receipts inside its own transaction. Calling the old step runner on incomplete pages would roll its savepoint back or incorrectly fail the parent; a partial page must never be reported as `Applied`.

The required owners cannot be configured to an empty successful list:

| Step | Required owners |
|---|---|
| READ_ONLY | Retained actual native/composite driver, outside batch certification |
| FREEZE_MURAL | Mural |
| SNAPSHOT | Permitted public snapshot |
| EXPIRE_GRANTS | Social grants |
| EXPIRE_BEACONS | Beacons, World, community |
| EXPORT | Permitted public export |
| CLEANUP | Private retention, Post retention, media retention, World, community |

No positive production owner is installed by this module. The new ledger is internal progress evidence; it is not the missing immutable operational audit append provider.

## Production dependencies still required

| Dependency | Actual counterpart result | Consequence |
|---|---|---|
| Current event/operator or system authority and reviewed retention | Ops205 has no published qualified provider; Root owns acquisition/configuration | Background execution fails closed |
| Privileged consent-data closure authority | Social1fe inspected published `54e1325fce20e3abc98da50c68de0bd72dfd1a33`: actor/session Auth/Friend restriction fences contain no operator/system event/job/step grant | Retained privacy helpers cannot be enabled by setting a GUC or using member/SafetySelf authority |
| Actual mural freeze with a verified closure receipt | Ops205 confirms no published or unpublished freeze provider; feature settings only toggle a boolean | FREEZE_MURAL cannot advance from metadata or settings alone |
| Permitted Post/media/archive source and retention convergence | Moderation230 explicitly has no public archive/Post adapter; AdminReview is metadata, not a publication projection | No public archive or Post/media cleanup acceptance |
| Reusable immutable BE41 operational audit append result | Ops205 reports missing provider; Moderation BE39 audit reader is not a cross-operation writer | No replacement with ledger UUIDs or success counters |
| Later World/community owner convergence | World20f inspected published `a67fc1342b351750ce766a783052c470dc9a9907`: no later-step port or qualification; existing closure API is crate-private and READ_ONLY-only | READ_ONLY barriers cannot certify full retention cleanup |

These prerequisites have been raised directly to Root for actual owner assignment/composition. No king/SL review gate is required. Any change to retained owner files requires a new exact ownership grant first.

The retained public archive already has encrypted event/source-revision-bound cursors and current owner fences. A field allowlist alone does not establish that Post text excludes private location, recipient or handle data. Full BE41 acceptance requires actual owner-reviewed publication/media sources and protected reads, not the synthetic archive fixtures or moderation review DTOs.

## Qualification and delivery

Current validation: Rust 1.88 formatting, Python parsing and DDL installation of all 30 current Concert migrations (including 0046) into an actual owned PostgreSQL 16 cluster passed. Initial DDL receipt: `${BE41_SCHEMA_ROOT}/schema.json`; every command exited zero, cluster cleanup passed.

`python3 backend/scripts/mcr/event_cleanup/run.py --sql-only` subsequently exited zero with **PASS_SQL_ONLY**. Receipt: `${BE41_SQL_ROOT}/native.json`, SHA-256 `250b4fb4ae563a43c445826e4fdc999ab8d651cf1bb426b64e60142b699f5880`. It records 13 explicit SQL assertions: direct completion/cursor/scope/limit changes rejected; atomic cursor advance; stale pages and rewritten/deleted/truncated ledger evidence rejected; savepoint rollback preserves the prior cursor; restart resumes that committed cursor; and parent lifecycle success is not fabricated. An additional delayed COMMIT was rejected after its actual authority deadline (expected nonzero SQL exit), with state rolled back. Actual PostgreSQL postmaster PID changed `3578 -> 3663`, start time changed, prepared transactions were zero and token/inode-owned cleanup passed. All recorded log hashes and the tested schema/script source hashes were independently checked against the current files. The earlier failed harness receipt `${BE41_FAILED_SQL_ROOT}/native.json` remains preserved with successful cleanup; pending deferred events had preempted the dedicated TRUNCATE guard, so the corrected test flushes them first.

This SQL-only mode uses explicit synthetic job/receipt metadata inside the marked disposable cluster. It qualifies database guards and persisted cursor restart, not actual operator authority or owner effects.

### First compiled-engine qualification

Named offer `BE41-FIRSTQUAL-BACKEND-202611040456Z` was explicitly accepted at clean source `1f71e915977a7970859124ec0108ffd9ceb3a836`, with fresh disk and zero-handle checks. Exactly the application `event_cleanup` and service `event_cleanup_runtime` test executables were compiled using `cargo +1.88.0 test --offline --locked -j 2 -p <package> --test <test> --no-run --message-format=json`, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, and both dev/test debug profiles set to zero. The first service attempt exited 101 because the shared cached `contracts-build` helper embedded the Notification checkout's Protobuf path; the current/Notification Auth proto hashes were identical. Only that helper package was cleaned (20 files, 816.3 KiB), and the retry compiled in 20.28 seconds. No source, shared manifest or dependency changes were needed. The failed Cargo JSON/stderr are preserved.

Both exact executables, complete pre-build source snapshot, Cargo JSON and Rust 1.88 toolchain metadata were frozen at `${BE41_FROZEN_ROOT}/frozen`. BACKEND was unconditionally released with zero open handles **before** executing any copied test or PostgreSQL work. Release receipt: `${BE41_FROZEN_ROOT}/release.json`. The whole observed additional disk peak was 118,067,200 bytes (about 112.6 MiB), within the 1 GiB compile/frozen/owned-PG allowance. No target was read after release.

`python3 backend/scripts/mcr/event_cleanup/run.py --frozen ${BE41_FROZEN_ROOT}/frozen` exited zero with **14 passing tests**: application 4; explicit configuration 1; SQLx fresh install/forward upgrade/reapply with receipt verification 1; actual PostgreSQL engine tests 6; restart preparation 1; restart verification 1. The engine tests cover missing authority/owners, concurrent workers and source-row revocation fencing, effect/verification/pending rollback with cursor resume, expired authority rollback, changed policy/wrong scope rejection and immutable evidence. Actual postmaster PID changed `87535 -> 87580` and its start time changed. The verifier resumed exactly the committed cursor and observed three final batch receipts, zero prepared transactions and successful marked-cluster removal. All frozen executable, Cargo JSON and native-log hashes were independently checked.

| Artifact | SHA-256 |
|---|---|
| Native receipt `frozen/native.json` | `37cc550e1ff7511334a523fbce18eaa16f5b406224cf34decc0f4c98c5a2ae93` |
| Freeze receipt `frozen/freeze.json` | `389bb6d9dcb028cc1f55814a6d5b3c16749e47b0f85024f4b33d95be7aa56e54` |
| Application executable, 1,143,584 bytes | `fbb832f70b8269d0692f2633dd591f64d8fe9808bb3463fe434088639919aa91` |
| Runtime executable, 14,757,488 bytes | `b35914cd69449773c437782fd5a4aa473e4f2a138b8296ae7232a560ecd9dfba` |
| Compiled additive source pin | `1a17fd5607a918a9fe663afdf3d6995ec384f765b8e2bd68d945ceccd68e3ba5` |

This is **PASS_ENGINE_ONLY**. Positive privilege, retention and mural-owner sources in these tests are explicitly synthetic owning fixtures. They are not production operator authority, actual mural freeze, consent-data expiry, Post/media retention or private-data-safe archive qualification. The original seven-step parent is asserted pending without fabricated owner success. No new target or dependencies were created; no master delivery or full BE41 acceptance is claimed. The existing full goal remains blocked by the production dependencies above; its source objective was not reset or reduced.

The authored tests distinguish engine qualification from product acceptance. Application tests cover immutable bindings, policy deadlines, stale/cross-owner receipts, partial failure, cursor restoration and invalid pages. The owned PostgreSQL suite uses explicitly synthetic privilege/owner sources to exercise migration fresh install/forward upgrade/reapply, missing bindings, effect/receipt rollback, pending rollback, concurrent workers, revocation fencing, deadline expiry, immutable SQL evidence and real postmaster restart. Its payload strings are private test fixtures; it is not an actual Post, mural, media, operator or archive adapter.

Build and freeze only under a fresh named SL offer and explicit acceptance. Use Rust 1.88, offline/locked, jobs at most two, debug information off and incremental off. Capture a pre-build snapshot with `python3 backend/scripts/mcr/event_cleanup/freeze.py --snapshot <before.json>`. Compile only the application `event_cleanup` and service `event_cleanup_runtime` test targets with Cargo JSON output; freeze their exact executables with `freeze.py --before <before.json> --cargo-json <application.jsonl> --cargo-json <runtime.jsonl> --out <fresh-artifact-directory>`. A source change invalidates that snapshot. Release all shared target access before invoking PostgreSQL.

Then run `python3 backend/scripts/mcr/event_cleanup/run.py --frozen <artifact-directory>`. It validates frozen executable hashes, uses a token/inode-owned socket-only PostgreSQL 16 cluster, applies actual migrations, records test exits and a changed postmaster PID/start time, verifies resumed cursor progress, checks prepared transactions and stops/removes only that marked cluster. Logs and failed receipts are preserved outside the disposable cluster. No ambient database URL or running imported service is used.

The final record must identify source and binary hashes, exact commands/exits, native receipt/cleanup, remaining real production gates, committed feature SHA and verified master ancestry. Passing the authored engine fixtures alone must not complete the full original BE41 goal.

## Blocked audit

The same production prerequisites remained unresolved across three consecutive goal turns. Independent implementation and SQL qualification are committed; further meaningful progress now requires an external state change. On the third audit, fresh `origin/master` was `0b8243299c9a8c3099351364d518bbd96fc38a7f`. Its Concert service, concert application, Post service and Profile service trees are unchanged from base `3471190db5a5024f5376c92b282af797b7063c72`; the missing production authority/archive/later-owner providers have not appeared. The current published lifecycle matches remain generic contracts, retained wrappers, unavailable defaults and test-only implementations.

At that audit, no fresh named target offer had been issued and no compiled-engine qualification had run. The later FIRSTQUAL offer and 14-test qualification above resolve that build wait. No target or owned PostgreSQL process is held. The full goal is still blocked, not complete or reduced to the engine evidence. Further progress requires actual owner assignment/composition for the production dependencies above. This is not a request for king/SL implementation review or approval; Root-owned acquisition/shared-file boundaries and the user's named target allocation policy remain binding. The engine candidate is qualified for an explicitly incomplete, fail-closed foundation handoff; full BE41 integration and acceptance remain unproven.

## Root intake provenance

The original qualified worker prose is retained as Git blob `8a82d864ff1e45e6003c3399fa3947b3d7e27740`, SHA256 `07d1a5881aafa465e7d013ed3af0f08cf5f449499545209c5ccaed9bf2345342`. Only machine-local artifact prefixes are substituted with portable placeholders in this delivery copy. Root retains every substantive gate and source result; coherent Root compilation and actual production owner composition remain open.
