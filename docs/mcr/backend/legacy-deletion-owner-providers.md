# BE42-LEGACY-OWNER-PROVIDERS-01

Status: `IN_PROGRESS`, developer qualification complete for the three legacy
providers; committed publication through Root remains pending. Source `3594117e`
passed all four builds and all eight original test invocations using PostgreSQL
16.14, private Redis, real mTLS RPCs, the durable Auth worker and physical restart.
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

### Qualified source and actual runtime result

The qualified implementation is `3594117e4f5e51616717418e3ebc3a3062356d86`, on
the ancestry `cf1ca629` → `675134b1` → `8c17b923` → `0deeeae9` → `3594117e`.
The later documentation commit changes no backend source or SQL. The original
`1845` retry briefly accepted at epoch `1793822814.6071439`; a new capacity
sequencing instruction arrived during the first build. The developer stopped
immediately, released the target at `1793822823.7196312`, and recorded full
release at `1793822851.966291`. No executable or source-failure diagnosis resulted.
That spent turn, its 1,200,128 bytes of positive target growth, original clocks
and all failed evidence remain preserved; the successful turn did not reuse it.

Fresh offer `LEGACY-DELETION-RETRY-MEDIA-20261104T2015Z` accepted at epoch
`1793823184.513875`, after Age's actual copied-native full release. Fresh free
space was 19,391,115,264 bytes; after the entire proposed 2 GiB and all conservatively
reserved concurrent growth, 16,236,998,656 bytes remained. WHOLE was capped at
7 GiB with nonshrinking carry 4,854,657,024 bytes. Both private controls enforced
the offered 11 GiB start, 8 GiB live floor, target growth at most 1.25 GiB, outside
growth including source/Git at most 768 MiB, private PG at most 512 MiB, and four
copied executables at most 192 MiB together/128 MiB each. ALLNEW remained one
inclusive 2 GiB limit. Native execution retained the earlier binding 360-second
limit, target-release-to-FULL 660 seconds, and 300 seconds for cleanup.

The controls were repinned before acceptance, as allowed by the resource policy;
these exact private controls, rather than the historical tracked default floor,
supervised this qualification:

| Input | SHA-256 |
|---|---|
| private `controller.py` | `d3866dfe628d968020b0b23381fc47b66a29ff4468e110b76abe34e3b7b4ef5b` |
| private `guardian.py` | `1d6c79c0b4bccea7b5166bed181530da8a8a183af71781251a66411771bafd1f` |

Build commands used the existing retained warm target, Rust 1.88.0, jobs 2,
dev/test debug 0, incremental 0, and cleared compiler flags/wrappers. For each
row, the exact command was
`cargo +1.88.0 test --locked --offline -p PACKAGE --test TARGET --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2`:

| PACKAGE | TARGET | Exit | Frozen executable bytes |
|---|---|---:|---:|
| `profile-service` | `committed_deletion_rpc` | 0 | 29,952,144 |
| `friend-service` | `committed_deletion_rpc` | 0 | 28,272,720 |
| `post-service` | `committed_deletion_rpc` | 0 | 38,616,496 |
| `auth-service` | `legacy_deletion_receipts` | 0 | 20,002,816 |

The four immutable copies total 116,844,176 bytes. Their emitted-artifact paths,
SHA-256 hashes, complete backend source manifest and runtime tool hashes are in
`artifact-receipt.json`. Build execution took 63.77 seconds. The target was
unconditionally released at epoch `1793823248.571987`, with compiler/handles zero,
and handed directly to Media before native execution. No later target access,
separate main executable copy or hidden compilation occurred.

The native controller ran each requested case with `--exact`,
`--test-threads=1`, and `--nocapture`; the five database cases additionally used
`--ignored`. Every invocation reported exactly one passing test, zero failed and
zero ignored, with exit 0:

| Executable/cohort | Exact case |
|---|---|
| Auth | `profile_binding_rejects_boolean_partial_owner_and_every_rebound` |
| Auth | `friend_binding_rejects_boolean_partial_owner_and_every_rebound` |
| Auth | `post_binding_rejects_boolean_partial_owner_and_every_rebound` |
| Profile | `populated_actual_rpc_atomic_replay_rebound_fence_and_original_clock` |
| Friend | `populated_actual_rpc_atomic_replay_rebound_fence_and_original_clock` |
| Post | `populated_actual_rpc_atomic_replay_rebound_fence_and_original_clock` |
| Auth | `actual_eight_step_worker_commits_only_three_real_owners_and_recovers_rollback` |
| Auth, after restart | `actual_restart_preserves_all_three_receipts_and_five_incomplete_steps` |

The actual PostgreSQL server reported `160014` (16.14). Each owner exercised its
real service state, configured pool, mTLS server, deletion transaction and SQLx
migration runtime. Profile/Friend/Post RPC fixtures ran separately; the Auth
worker reached all three actual RPCs. Physical PostgreSQL restart changed the
postmaster PID from 66509 to 67787, and all three RPC processes restarted before
the final Auth case. Matching SQLx ledgers contained Profile 10, Friend 19, Post
33 and Auth 18 migrations. Prepared transactions were zero. Native execution
took 42.26 seconds; all private PostgreSQL and Redis processes and the owned
database root were removed. Peak WHOLE was 5,321,195,520 bytes; target growth
was 244,920,320 bytes. The native controller observed at most 23 processes,
328,761,344 bytes RSS, and no compiler.

Successful evidence is retained under
`/private/tmp/mcr-be42-legacy-2015-3594117e/`: `accepted-offer.json`,
`artifact-receipt.json`, `build-result-1793823184.json`,
`qualify-result-1793823301.json`, both watchdog results, original per-command
stdout/stderr, `target-release.json`, and `full-release.json`. Full release at
epoch `1793823399.336336` records compiler/target handles/controller/guardians/
native children/PG/Redis/workflows/reservations all zero. Earlier failed roots
`mcr-be42-legacy-675134b1` and `mcr-be42-legacy-3594117e` remain intact.

This qualifies the three actual legacy providers and their canonical Auth worker
receipt boundary. The five other saga targets deliberately remain incomplete;
their lack of acknowledgment is asserted before and after restart. No whole-BE42,
public Gateway journey, commercial-provider, live-data migration, load, or release
acceptance follows from these results. The protected Notification implementation
and frozen source archive remain unchanged. Root receives the committed scoped
handoff directly; publication does not wait for a king/SL review or acknowledgment.
