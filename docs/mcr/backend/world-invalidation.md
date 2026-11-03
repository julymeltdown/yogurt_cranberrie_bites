# C05 World invalidation checkpoint

Task: `C05-WORLD-INVALIDATION`. Worker base: `18d4af0284a556cc98394e3622dcc3710e1f1a8f`. Branch: `codex/mcr-be-world-invalidation`. Delivery repository: `julymeltdown/mcr`, integration target `master`.

## Scope and status

The local Concert block/unblock consumer is an opt-in implementation candidate (`CONTRACT_READY`); it does not enable the production worker, startup, or a general fanout consumer. Root production qualification, authenticated Auth/Friend replay consumption, moderation producer integration, and the real event-closure vertical flow remain unfinished. The broader persistent World goal is still `BLOCKED`; this bounded manual continuation does not complete or reset it.

The subsequent World READ_ONLY barrier is a separately assigned increment. This checkpoint does not credit that work or its migrations, owners, or tests. At this base, `closeConcertEvent` requires a qualified `LifecycleOwners.apply_in(ReadOnly)` effect before START commits. No qualified production implementation was available in this checkout. Closure and restriction native validation are `NOT_RUN`; synthetic owner success is not acceptance evidence. Main owns allocation of the disjoint lifecycle/composite work.

## Implemented behavior

`WorldInvalidationConsumer` accepts removal-only signals resolved from the real `concert_outbox` joined to its atomic command receipt. It verifies the delivery UUID, event, kind, receipt UUID, resource, revision, lease token, attempt, live lease, succeeded receipt status, operation, actor/target pair, current local relationship revision, database source epoch, and bound World owner. Transport `Delivery` fields alone grant no authority. The native test found and fixed the actual `SMALLINT` receipt-status decoding mismatch.

Block and unblock invalidate both members of the event-local pair. Unblock removes cached authority too; it never installs an allow decision. The consumer does not translate virtual position into physical arrival or change access expiry. SQL and view operations retain exact event/owner/epoch scope. A foreign live World owner is an unresolved fanout dependency and returns retryable failure.

The order is delivery mutex, short synchronous native-handoff fence, admission mutex, publication-lease pause, then SQL transaction. No native mutex is held across an await or SQL call. Before application success the bridge cancels affected sealed parents, removes active and pending authority, masks recipient caches, and rejects late admission/publication while the removal fence is pending. Recipient generation changes force safe resync/close; the implementation does not drop numbered frames and continue a broken sequence. Unrelated rooms/events retain their own authority. Bytes already handed to the native socket before the fence cannot be retracted.

`0022_world_invalidation_receipts.sql` adds an immutable database source epoch, immutable consumer receipts, and independent per-consumer/event/source/scope revision cursors. These are local application tombstones, not a second queue. Scoped SQL disconnection effects and the exact victim identities commit with the receipt. A duplicate only repeats removal for those exact user/room/family/connection/owner/epoch victims, so a fresh connection is preserved. Superseded pair revisions record a durable no-op. Revision ordering is local to that source and pair; it does not infer Auth/Friend cursor coverage.

Failed or cancelled application retains the in-memory removal fence and returns failure. SQL failure rolls back persistent effects and the receipt. A retry must use a live current outbox claim; stale token/attempt acknowledgements remain rejected. The consumer validates live source/owner binding again at the commit boundary. Stable configured World process identity is required to resume this consumer's durable history after restart; a new owner epoch does not replace the database source epoch.

`WorldCompositeSink` requires a World consumer and an explicit remaining fanout leg. World alone does not implement `IdempotentInvalidationSink`. A remaining-leg failure prevents global acknowledgement even after local World commit. Existing `run_batch`, claim/ack/fail, 30-second leases, ten-second worker deadline, eight-attempt exhaustion, and retry behavior are retained. Unsupported/unbound local operations and all unqualified remote hints fail closed. Do not enable this composite until its real remaining legs are qualified.

## Provenance and paths

All application changes are newly authored against the worker base. No additional Bigyeting code, rendering, assets, visual libraries, dependencies, manifests, or lockfile resolutions were copied or changed. `docs/source/` and all preexisting migrations are unchanged.

| Origin at base `18d4af02` | Destination | Use and adaptation |
| --- | --- | --- |
| Existing `world/admission_bridge.rs`, `production_owners.rs` | Same files | Narrow invalidation/fence hooks; existing owner ports and native mutex retained |
| Existing `workers/outbox.rs` and `adapters/out/repository/outbox.rs` | `workers/outbox.rs` composite addition | Existing queue and lease/ack/retry implementation reused without replacement |
| Existing `tests/world_runtime_support/postgres.rs` | `tests/world_invalidation/fixture.rs` | Owned private PG checks and synthetic remote-owner fixture, adapted for event-local real pair visibility and multiple clients |
| Existing `tests/world_runtime_support/mod.rs` | `tests/world_invalidation/geometry.rs` | Synthetic metadata/map/identity helpers; no product art |
| Existing `tests/world_owner_composition/native_smoke.py` | `tests/world_invalidation/native_smoke.py` | Owned socket-only PG controller, extended for source-bound executable freeze, restart and outage |
| Existing `backend/scripts/mcr/migration_smoke.py` | Imported by the new controller | Marker/inode/PID checked init/start/stop/cleanup; direct migrations, not serving-ledger proof |
| New implementation | `src/world/invalidation/{mod.rs,store.rs}`, migration `0022` | Receipt-qualified removal, durable deduplication, scoped SQL effects |
| New tests | `tests/world_invalidation.rs`, `tests/world_invalidation/native.rs` | Real local command/receipt/outbox/consumer/native boundary with explicit missing remote-authority fixtures |

The real producer is `relationship/commands.rs::BlockCommand`, executed through `PgCommandRepository`, then claimed/acknowledged through `PgOutbox`. Missing Auth/Friend account/session authority and Profile metadata remain synthetic in this isolated consumer test. The required second fanout leg uses its own private durable fixture table; it is not proof of production feature fanout.

No new Cargo dependency is required. The retained Rust 1.88 lockfile is used offline. Migration `0022` depends on existing `0011`'s immutable trigger function and the existing Concert receipt/outbox/World schemas.

## Validation and evidence

Final commands, source-bound receipt hashes, results, and handoff ancestry are recorded below after execution. Native controller reports explicitly identify the executable SHA-256, base, every migration SHA-256, source-freeze receipt, per-test exit/log hashes, changed postmaster PIDs, and owned-root cleanup.

The controller applies migrations directly to a newly initialized private socket-only `mcr_world_invalidation` database. It does not certify a serving ledger, deployed startup, IN01, real remote service authentication, product art, or full multiuser acceptance. No live database, shared database, external volume, or physical device is used.

The six native cases cover:

1. Real `BlockCommand` → atomic receipt/outbox → claim → actual delayed SQL commit → local removal → mandatory remaining leg → ack. Presence is pending behind native readiness; no affected presence is handed off after ack. An unrelated room delivers native deltas with contiguous sequence numbers; an affected observer safely resyncs.
2. Actual PostgreSQL postmaster and test-process restart, durable duplicate, and preservation of a fresh authorized connection.
3. Failure before consumer commit, SQL rollback, retained removal fence, real retry/new lease token, stale-token rejection, duplicate after local commit, reordered pair revisions, forged delivery metadata, unrelated event preservation, and real `run_batch` integration.
4. Captured credential expiry closes the actual WebSocket without extension or later private presence.
5. In-flight native handoff overlaps application; application success must occur after the handoff releases the shared fence.
6. Controller stops the exact owned postmaster; source/ack failure never becomes success, and cached views expire within their existing bounded lease.

Earlier failed diagnostic runs are retained outside the checkout. They are not successful evidence: initial setup refreshed generations before queuing presence; the native path exposed the INT2 decoder defect; a same-room observer correctly resynced; the expiry assertion initially required a JSON frame instead of the actual close; the race fixture initially relied on a Tokio timer while intentionally blocking a native worker. The final cases use the actual wire/SQL boundary and independent native-race coordination.

### Qualified isolated checkpoint

All commands below exited zero. Rust build used the exclusive BACKEND target, Rust 1.88, `--locked --offline -j 2`, dev/test debug level zero, and incremental compilation disabled. Exact test executables were selected from Cargo JSON and copied outside the target. All target process/file handles were checked closed before PostgreSQL startup. No third target or dependency install was used. The 8 GiB floor blocked two attempts before database creation; Main recovered host space, then the controller's fresh guard allowed execution.

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test world_invalidation --test world_runtime --test world_ws --lib --no-run --message-format=json
python3 backend/services/concert-service/tests/world_invalidation/native_smoke.py --binary "$MCR_WORLD_TEST_BINARY" --freeze "$MCR_WORLD_FREEZE_RECEIPT" --report "$MCR_WORLD_NATIVE_REPORT"
python3 scripts/mcr/check-plan.py
git diff --check
```

The four copied executables also ran with `--test-threads=1`: library **30 passed / 7 ignored**, World runtime **28 passed**, native WebSocket adapter **16 passed / 4 ignored**, invalidation **1 passed / 6 ignored**. Total **75 passed**; ignored owner/browser/database cases were not credited by these pure runs. The separate controller executed all **six** invalidation native cases explicitly with `--ignored --exact`, each exit zero. The planning check passed and preserved all 13 original archive checksums, 112 required tasks and 61 required IA entries; it does not certify implementation acceptance.

| Isolated evidence | Value |
| --- | --- |
| Source/executable freeze SHA-256 | `10e462fd9714ff48bb762803c733016a990b3a8da65ebea5e78e36b83c545fe2` |
| Frozen backend/contract source files | 1608 |
| Native test executable SHA-256 | `8cd655318bb9d2c546981f490ccf735dc3dae423c9af1cc42469fa0a3e5b1e3f` |
| Native report SHA-256 | `b5c7bc74c3a21e54d474b5009a1bee861ea383c65bfde2f6dc90ee14f325bb4e` |
| PostgreSQL | `16.14 (Homebrew)` |
| Actual restart postmasters | `[8841, 8954]` |
| Outage queue still unacknowledged | 1 leased row |
| Native cases / cleanup | 6 PASS / PASS |

| Native case | Log SHA-256 |
| --- | --- |
| `native_block_outbox_fence_commit_retry_and_native_send` | `48d79b43a714e941b9bc7f66cce08a995bcc8bc63eca84d04e50a4296098a457` |
| `native_restart_durable_receipt_preserves_fresh_connection` | `8da5ba529009a438f75a466d0220840c89f1540e8af1fa5f08b6a1d8b4d496ae` |
| `native_failed_commit_retry_duplicate_reorder_and_metadata_fences` | `eefc826eef713caee95c1df08424d9ada5021999cb9b4edda7b78e025fdae73e` |
| `native_captured_credential_expiry_closes_without_renewal` | `379d44e7a4550a986fbb3375eda13320be17ced0d2c8e47a46149254021b1b39` |
| `native_inflight_handoff_serializes_before_invalidation_ack` | `3a8e630a22477cc08dad5c9d541143a8fad26b2a54c4592e0a2c3cc245488891` |
| `native_database_outage_never_acknowledges_and_expires_live_views` | `41de493a7f966a2822073c8a070f9e46bbb03bee2ddd4d1afb805eec092da8fd` |

Logs, Cargo JSON, executable copies, source pins, native controller reports and the failed diagnostic reports are retained in the task-local evidence directory, outside the build target. The checkpoint's source ancestry is the base above plus the implementation commit containing this document; the publication integration ancestry is recorded separately after merging current master. This is a local consumer checkpoint, not overall C05/BE41 acceptance.
