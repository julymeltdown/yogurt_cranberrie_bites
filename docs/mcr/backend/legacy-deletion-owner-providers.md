# BE42-LEGACY-OWNER-PROVIDERS-01

Status: `IN_PROGRESS`. Source and meaningful qualification fixtures are authored.
Cargo, PostgreSQL, Redis, real RPCs, durable Auth worker and physical restart
qualification are `NOT_RUN` until a fresh bounded resource offer is accepted.
This is the Profile/Friend/Post provider slice, not whole BE42 or release acceptance.
QA08/load/performance remains `DEFERRED_BY_USER`; functional privacy, persistence,
recovery and developer-owned fixes remain required.

## Ownership and provenance

Base: `cf1ca629a33ece977f8b5b1b6faf6152b35e5dbd`. Existing exclusive retained
`be-notification-product` worktree, branch `codex/mcr-be-legacy-deletion-owners`.
Earlier branches, qualified Notification evidence and unique artifacts are preserved.
Root is the named publishing developer. No king, SL or separate QA inspection,
acknowledgment or approval is a dependency of the committed handoff/publication.

The actual source transactions were extracted from these base paths:

| Owner | Existing transaction | Destination |
|---|---|---|
| Profile | `backend/services/profile-service/src/adapters/out/persistence/profile_repository.rs`, `apply_user_deletion` | `backend/services/profile-service/src/deletion_receipt/mod.rs` |
| Friend | `backend/services/friend-service/src/adapters/out/persistence/follow_repository.rs`, lifecycle implementation | `backend/services/friend-service/src/deletion_receipt/mod.rs` |
| Post | `backend/services/post-service/src/adapters/out/persistence/user_lifecycle_repository.rs` | `backend/services/post-service/src/deletion_receipt/mod.rs` and `concert_cleanup.sql` |

Those retained headless repositories originate in the port-rust foundation
imported by `909e2bf4`; the exact MCR revision copied here is the base above.
The Post affinity helper's same installed-function probe/call is retained inside
the new leaf without changing that separately owned helper's visibility.
There is no rendering reuse, new identity, new saga or cross-database write.

Root granted only the corresponding three library exports and RPC deletion
methods; three application deletion DTO/default-fail-closed ports/Arc forwards/new
receipt entry points; existing repository deletion methods; Auth's three client
methods and necessary receipt verifiers; and these exact additive proto paths:
`backend/proto/profile/v1/profile.proto`, `backend/proto/friend/v1/friend.proto`,
`backend/proto/post/v1/post.proto`.
Existing `applied=1` and all request fields keep their wire numbers.

New reserved migrations are Profile `0010_committed_deletion_receipts.sql`, Friend
`0019_committed_deletion_receipts.sql`, Post `0033_committed_deletion_receipts.sql`.
Each maximum was checked against the source before authoring; these are not edits
to historical migrations. New tests are each service's `tests/committed_deletion_rpc.rs`
and Auth `tests/legacy_deletion_receipts.rs`. Shared test-only fixtures and the finite
qualification controller live under `backend/scripts/mcr/legacy_deletion_owners/`.
The private Cluster helper is read from the pinned `migration_smoke.py`; no copy
or mutation of its owned implementation is required. Controller/guardian scaffolding
was adapted from the prior qualified BE33 c9f8a848 finite controller, without its
old offers, archives or Notification workload.

Dependencies are retained sqlx, Tokio, tonic/prost, chrono, uuid, anyhow, existing
platform identity/deadline/TLS/method-policy helpers and migration-runtime.
No manifest, lockfile, Main, config, pool or Auth inventory changes are made.
The test processes use actual service composition. Post consequently needs a
private Redis manager even though deletion cache eviction remains best effort.
The existing historical BE33 persistent goal is incompatible and still reports
`blocked`; it was not reset, falsely completed or replaced. This does not turn
the newly authorized source work into a manager approval wait.

## Actual transaction and policy boundary

Each owner starts its own real database transaction, takes its original subject
locks, performs retained deletion effects and writes audit, a positive identity
fence and immutable v1 receipt on that same connection. Post preserves the original
complete affected-author discovery, sorted hashed subject locking, rediscovery
and retry before row locks; its cohort now includes current Concert comment and
reaction parents. Original legacy counter repair and affinity forgetting remain.
The old application entry/port signatures remain available; the RPC uses the new
receipt-preserving entry. Post retains postcommit best-effort cache eviction.

The receipt binds version 1, uppercase owner, saga/user/actor UUIDs, original
retention seconds and nanoseconds, positive fence revision, native `xid8` and
write observation time. PostgreSQL stores microseconds; explicit original parts
make even a one-nanosecond rebound fail. The observation is not a fabricated
commit timestamp. Nil IDs and leap-second nanoseconds fail before effects.
Receipt/fence insertion has deferred same-current-XID, tombstone, audit, command
and cleanup checks. An isolated receipt or later proof transaction cannot certify
the effect. Nothing becomes returnable until COMMIT succeeds.

Exact retries return the original immutable receipt without rerunning counters,
versions or audit. Saga/user/actor/deadline rebound fails closed. Historical mutable
tombstones/audits without v1 evidence are not silently upgraded into proof; they
remain an explicit reconciliation case. Permanent linked legacy tombstones and
audit, new fences and receipts reject mutation/removal/truncation. Existing retention
metadata and purge timing remain; no new retention duration or blanket physical
erasure policy is inferred.

Profile scrubs username to an opaque deletion identifier, nickname, bio, avatar
URL/bytes/filename and counters; expires/nulls replay outcomes while retaining
operation metadata. Friend keeps the original soft-deleted relationship retention
records, cancels pending requests and removes recommendation traces, including
deleted candidates inside another viewer's snapshot. Peer relationships survive.
Post scrubs legacy post/comment bodies and replay outcomes, preserves original
soft-delete metadata, removes recommendations/affinity/timeline effects, and handles
current Concert posts/comments/reactions in the same transaction. Deleted Concert
content becomes `[deleted]` with empty media/links; state/version/counters advance
and surviving peer counters are repaired exactly once.

The actual legacy event producers in `post-application/src/use_cases/mod.rs`
embed raw personal text in `payload.content`. That field is removed from the
deleted actor's outbox and matching local publication copies. Event identity,
order, attempts, genuine publication timestamps and pending status are preserved;
cleanup never fabricates successful delivery. A delayed local publication copies
the current scrubbed canonical source under its row lock instead of restoring a
stale claim body. Existing current Concert receipts/outbox contain only operation
IDs, revisions, counts and timestamps, as traced in `concert_feed/persistence.rs`;
they remain replay metadata, not copies of post text/media. This local proof does
not claim to recall external messages already published to another system.

Database guards close raw-body/replay restoration and current Concert late-write
paths through the existing permanent subject deny. Feed owner 21a owns the
`execute_in`/`receipt_in` captured-transaction admission changes; actor/parent
cohort ordering is coordinated directly with that owner. No wrapper-only check is
claimed as actual-kernel validation and this slice does not qualify its native
reader/authority transport.

The existing authenticated Auth-only RPC policy remains mandatory. A missing
original finite call clock is rejected; original-clock timeout surrounds cleanup
and COMMIT, with a final expiry check before receipt exposure. Cancellation can
leave an outcome unknown to a caller; it never produces an ACK, and an actually
committed receipt is recoverable by an exact retry. Auth creates one bounded
edge scope without extending an outer deadline and accepts only the exact typed
owner receipt. Boolean-only/partial/mismatched replies fail closed.

The protected Notification client/verifier span is byte-exact against the base:
SHA256 `97e8d0b237afa563b98901c22f031fa9aba8abe5031971a29af0771bb326ef9c`.
Tickets, Notification, Sessions, Sharing and PrivateData qualification remains
with their responsible owners. No target outside this slice is acknowledged here.

## Validation and retained evidence

Completed lightweight checks, exit 0:

- `git diff --check`.
- Rust 1.88 rustfmt parsing/formatting of new Rust sources and only authorized
  shared hunks; unrelated source bytes remain outside formatting scope.
- Python `compile()` syntax checks of the controller and independent guardian,
  without launching either workload.
- `python3 scripts/mcr/check-plan.py`: planning-only PASS; application build/tests
  explicitly `NOT_RUN`, all 13 original archive checksums preserved.
- Byte comparison of the protected Notification span; no `docs/source/` diff.

The ambient stable rustfmt executable failed to load its absent driver dylib
(exit -6). The retained explicit Rust 1.88 formatter succeeded; no toolchain
installation or unrelated repair was performed.

Auth has three adversarial typed-validator tests, including omitted optional parts,
wrong owner/version/UUID/revision/native XID/date, exact nanoseconds and SQL-epoch
boundaries. These are parser tests only, not positive commit evidence.

Each real owner RPC fixture populates private and peer data, exercises actual mTLS
method authorization, missing clock, two simultaneous exact retries, four-field
rebound, immutable evidence, stale recreation, forced receipt-insert rollback and
expiry while blocked on the actual subject lock. Post fixtures include legacy and
current content, peer counters, raw outbox/publication bodies and replay outcomes.
The Auth fixture calls the real DeleteUser use case and eight-step durable worker
against three separately running actual RPCs/stores. A controlled Profile rollback
keeps that step unacknowledged while Friend/Post commit; repair/retry then yields
exactly three successes with five incomplete targets. The controller physically
restarts PostgreSQL and all three RPC processes before checking identical receipts,
rebound rejection, scrub persistence and still-incomplete Auth inventory.

Those functional scenarios are `NOT_RUN` at this source checkpoint. A fresh
allocation must pin the committed HEAD, exact four Cargo argv, SQL/helper/tool
hashes, source manifest (hashes only, no full source archive), whole retained carry,
single ALLNEW cap, disk/RSS/process limits and absolute cleanup deadlines.
The independent guardian runs before workload children. Resource state writes use
atomic replacement; accounting tolerates temporary files disappearing during a
compiler step rather than falsely failing on a directory traversal race. No cold target is allowed;
copied native execution begins only after unconditional target release. All failed
attempts, old Notification artifacts, complete worktree/common Git charges and
nonshrinking historical peaks remain counted and preserved.

Resource offer `BE42-LEGACY-DELETION-MEDIA-NEXT-202611041815Z` is a queued
finite offer, not current target custody or acceptance. Its original source pin
is 8c17b923; same-scope pre-ACCEPT source/control pin updates are authorized.
The controller clears compiler flags/wrappers, bounds native execution to 360
seconds, and independently shortens FULL to target release plus 660 seconds with
300 seconds reserved for cleanup. Native admission also needs fresh 12 GiB and
all outstanding reservations accounted for. No target access occurs before the
actual direct predecessor release and fresh numeric admission.

Publication and functional acceptance remain pending developer-owned qualification,
fixes and the named publishing developer's normal committed handoff. One canonical
publication receipt is sufficient; no manager readback or acknowledgment is needed.

### First allocated build and declaration correction

The first actual warm-target turn accepted source `0deeeae9` at epoch
`1793817757.2146208`, after Media's literal target and full releases. Admission
measured 14,022,230,016 free bytes and conservatively reserved the still-reported
Intro/QA06 allocations plus this turn's entire 2 GiB ALLNEW; 10,532,569,088 bytes
remained, above the required 9 GiB. The independent guardian ran before the
controller. Byte-identical `build.rs` freshness updates touched only this owned
checkout and recorded before/after hashes and mtimes.

The exact Profile Cargo command exited **101** with `E0407`/`E0599`: the new
`apply_user_deletion_with_receipt` default had been placed on
`ProfileMutationReceiptMaintenancePort`, while its existing adapter, Arc forward
and use case implement or require `ProfileRepositoryPort`. This correction moves
the unchanged fail-closed default onto `ProfileRepositoryPort`; it does not add a
maintenance dependency or fall back to the legacy unit result. Friend and Post
already declare their defaults on their respective lifecycle repository traits.

The controller released the preserved target at epoch `1793817765.636936` on the
failure path with no handles. The guardian exited, and full release at epoch
`1793817787.433255` recorded compiler/children/PG/workflows/reservation all zero.
Report received the direct target handoff. Observed target growth was 20,480 bytes;
peak WHOLE was 4,828,655,616 bytes, maximum process count 6, and guardian peak RSS
556,679,168 bytes. The failure and all retained history remain under
`/private/tmp/mcr-be42-legacy-675134b1/`, including the build result, original Cargo
JSON, watchdog result and target/full release receipts. No executable was frozen.
The other three builds and all native RPC/SQL/restart scenarios remain `NOT_RUN`.
This source correction requires a fresh allocated build turn; the released turn
will not be reentered or its clocks reset.
