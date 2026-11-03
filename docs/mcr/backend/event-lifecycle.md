# BE41 event lifecycle — implementation and qualification record

## READ_ONLY increment — native-qualified, full BE41 still IN_PROGRESS

The earlier `3d65e251` run passed 21 native/23 SQL/6 pure checks, then developer-owned
risk verification identified an authorization race while waiting on delivery/native
mutexes. That source was not published as final. The fix retains actual authority
SQL locks after the delivery wait, checks conservative monotonic expiry under both
World native/views mutexes immediately before any mutation, then releases SQL
before admission/drain. Two deterministic regression tests now pass. The current
reconciled candidate passes **23 native, 23 SQL and 6 pure checks**, exit 0.

The bounded READ_ONLY increment implements actual World/community closure in one
transaction and qualifies the native command driver. It is **INTEGRATED** within
the candidate described below; full BE41 remains **IN_PROGRESS**, with acceptance
**BLOCKED** on actual operator/retention authority, later owner composition and
transport/root wiring. The broader persistent goal still reports **BLOCKED**;
Main authorized manual continuation without reset or false completion.

### Ownership and provenance

Authoring base: `1e5022885525ab56f056aa2c24100e18c4bc67ed`; isolated branch
`codex/mcr-be-read-only-barrier`, checkout `be-lifecycle-resume`. Former
`c2a09dcc` source and evidence are preserved as historical records below.
The exact eleven-path grant covers service
`src/event_lifecycle/{read_only,mod}.rs`, `src/community/{closure,mod,giveaway}.rs`,
`migrations/0024_event_read_only_barrier.sql`, `tests/event_lifecycle_read_only.rs`,
`tests/event_lifecycle_support/{read_only,migrations}.rs`, the backend lifecycle
Python runner, and this record. Abbreviated service paths are under
`backend/services/concert-service/`. Giveaway changes only expose existing
sibling `load`/`save` functions. The separately granted ledger test adaptation
retains exact receipt/checksum, forward upgrade, existing-row and reapply checks.

All lifecycle/community code is newly authored; no legacy rendering is reused.
Existing pinned Rust/SQLx/Tokio/Axum dependencies and lock resolutions remain.
World owns its actual component, 0023 schema, admission/runtime/native publication
implementation and test fixture. Source dependencies were consumed without
editing World-owned files:

| Owner source | Integration | Purpose |
| --- | --- | --- |
| `06874327af5252c65ee2a665f387f641520db241` | `892b68ac6671d0f4222f4f7a6597d80a25bf3b39` merge | Real World closure API and schema |
| `dd002448` | `682fd533` merge | Initial-effect/replay guard and actual four-owner/native fixture |
| `1f6b0c7a` | `6ec81cc7` merge | Narrow cfg(test) fixture access; includes published peer ancestry through `8430cd57` |
| `3fa3f53c` | `83f0814f` cherry-pick | Narrow cfg(test) socket trace/geometry access |

The first native source candidate was `83f0814f` plus `3d65e251`.
The current qualification consumes World-owned guarded invalidation source
`e4685896` + `98235424` (cherry-picked as `cf64caa6`/`e56e2f01`) and the driver's
retained-authority fix `3a6957e2`. Merge `9d247403` reconciles published master,
including canonical Avatar `b13d79e6`, actual World compatibility `d9e9a00a` and
fixture adaptation `e28549a3`. The owned forward-ledger test now includes published
0025. Frozen current source includes that exact ledger adaptation. Avatar metadata
remains synthetic test data; this run does not certify approved character art.
Peer frontend/Friend changes inherited through
merges are preserved, not authored by this assignment. `docs/source/`, contracts,
central manifests/locks, previous migrations and root composition are unchanged
by the lifecycle patch.

### Actual component and native ordering

`read_only.rs` calls the actual
`world::closure::fence_event_read_only_in(&mut PgConnection,event,job,operation)`.
No healthy/no-op owner substitutes for it. Its receipt must bind the exact
event/job/operation, nonnil identity and positive bounded revision. Community
cancellation and immutable community/composite receipt insertion use that same
caller connection. 0024 references World-owned 0023 receipt identity and checks
its complete binding with an insert trigger; it does not duplicate World schema.
The community receipt has an exact scoped composite FK.

`ReadOnlyCommandDriver` derives its event/pool from the actual `AdmissionBridge`.
After input/event validation it waits for the delivery gate FIRST, then checks
current authority and revision under event SHARE and the authority implementation's
retained identity/role/policy locks. SQL supplies its clock and authority deadline;
a monotonic anchor taken BEFORE that query conservatively excludes query/network
latency and local wall-clock skew. `begin_invalidation_if` obtains the actual World
native and views mutexes, checks capacity, then invokes the synchronous expiry
callback immediately before its first mutation. Expired/revoked preparation causes
zero view removal, pending marker insertion or parent cancellation. Authority locks
remain held until that guarded mutation completes. The read-only authority
transaction rolls back/releases before entering the admission gate or draining
publication leases. The existing BE15 producer reauthorizes in its subsequent
event UPDATE transaction. The exact order is delivery gate -> current retained
SQL authority -> guarded native/view fence -> release SQL -> admission gate ->
pause/drain -> existing BE15 transaction/event FOR UPDATE. No SQL lock spans entry
to either asynchronous gate; no native mutex spans an await. Exact-key replay,
conflicts and RESUME retain canonical BE15 behavior. Later six steps still require
explicitly supplied actual owning adapters.

Production publication permits retain event SHARE and OPEN/LIVE revision checks.
Their tasks disable native sends under the shared publication fence before SQL
rollback. Event UPDATE therefore also waits for actual qualifying permits; the
local early native fence prevents pending views reappearing while SQL waits.
A local bridge does not claim synchronous foreign-process cache removal.

Failure/cancellation rolls back event/job, World effects, community roots and
children, receipts and outbox together. Native removal is deliberately not undone:
private preparation permission drops, pending native removal remains, and old
cached grants cannot return. Recovery requires a committed-phase reread before clearing the pending marker,
then fresh qualified owner admission before publication can resume. Finishing a marker
after durable success restores no view; a finish error cannot rewrite success as
claimed SQL rollback. Process-kill qualification of this new driver is NOT_RUN.

Community closure locks all event roots in stable order, uses existing checked
Party Cancel/Giveaway Cancelled transitions and canonical serializers. Domain
owner parameters enforce internal invariants, not forged user authority. Pending
applications and held/ready reservations cancel, reserved counts release, pickup
secrets and meeting/code state disappear, and claimed/completed history remains.
Terminal party roots/audience revisions deny member access. Residual active
roots/children/counters/secrets abort the transaction. Both component helpers
refuse SQLx transaction depth zero. New effects require READ_ONLY, initial PENDING
revision 1; existing receipts may replay in later phases without repeated effects.
Published 0018/schema checksum remains
`8a93ed7e1b02345c3071cb0583384d61bb1e0b509e8939daf568f9ea352e2cb0`.

### Executed qualification

Exclusive BACKEND build lease was transferred directly by World. Rust 1.88.0,
locked/offline, jobs 2, debug 0 and incremental 0 were used with the existing
`/Users/lhs/Desktop/mcr-build/backend` target. The first build exited 101 because
cached contracts-build artifacts retained the former checkout's protobuf paths.
Rebuilding only that package's artifacts under the lease resolved it; source,
protobufs and manifests were not patched. Subsequent integration-only and combined
library/integration compilation exited 0. Four executables were copied to private
`/tmp/mcr-be41-frozen-race`; freeze manifest records source and binary hashes.
After the second exclusive fix-build lease, uploader-media received explicit
target transfer after copied hashes, empty `lsof +D` and no target Cargo/rustc
processes. Separate MEDIA builds were not affected. All later native execution used copied binaries only.

| Executed check | Result and scope |
| --- | --- |
| Existing lifecycle/archive native suites | 9 PASS, including exact migration ledger/reapply and original restart pair |
| New READ_ONLY PostgreSQL suite | 10 PASS: actual World/community effects, replay/foreign isolation, concurrent START, scope/immutability/autocommit refusal, claimed-ticket/new-lease denial, SHARE serialization, after-component/between-owner rollback, commit deadline and new restart pair |
| Actual four-owner full driver rollback/recovery | 1 PASS: real Auth/Friend/Profile/Concert readers, active publication, failure at composite insertion rolls back all SQL; old grant stays invalid; pending marker denies new admission; explicit fresh qualified recovery, successful composite commit, replay and full receipt bindings |
| Actual native socket/driver drain | 1 PASS: real WorldRuntime and SocketTransport; an in-flight native handoff blocks START; after release driver closes, composite/community effects commit, owner leases disappear, no later snapshot/delta is published |
| Authorization-wait regression natives | 2 PASS: revocation while delivery gate is held and expiry while actual native fence is held; rejected commands leave OPEN and existing qualified World views/current authority intact |
| SQL constraint/restart probes | 23 PASS |
| Focused lifecycle library units | 6 PASS, including exact World receipt binding and preparation-token cancellation; 4 native tests ignored in this pure invocation and then run explicitly above |
| Planning/source provenance | PASS, exit 0; all 13 source archives unchanged, distinct from application/runtime evidence |
| Formatting, Python parse, diff whitespace and ordered pin parity | PASS, exit 0; exact 28-file Rust/Python registry |

Combined runner exit 0; driver cluster cleanup PASS; lifecycle runner stops its
socket-only owned PostgreSQL in finally and removes its temporary directory.
Both restart pairs run in separate processes around actual PostgreSQL stop/start.
The ledger requires exactly `1..12,16,18,19,20,21,22,23,24,25`, published 0018 SHA256,
exact fresh/forward receipts and unchanged reapplication. No live database is used.

Reproduction (absolute copied executable paths, PostgreSQL 16.14 on PATH):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-service --lib --test event_lifecycle --test event_lifecycle_archive --test event_lifecycle_read_only --no-run
/tmp/mcr-be41-frozen-race/concert_service event_lifecycle:: --nocapture
PATH=/opt/homebrew/opt/postgresql@16/bin:$PATH python3 backend/scripts/mcr/event_lifecycle_pg.py --native-binary /tmp/mcr-be41-frozen-race/event_lifecycle --native-binary /tmp/mcr-be41-frozen-race/event_lifecycle_archive --native-binary /tmp/mcr-be41-frozen-race/event_lifecycle_read_only --native-driver-binary /tmp/mcr-be41-frozen-race/concert_service --output /tmp/mcr-be41-race-qualified.json
```

Build command requires a newly transferred exclusive target lease. Native runner
requires the frozen source candidate, including the cfg(test)-only World fixture;
source mismatch fails rather than certifying stale binaries. Copy executables
identified by Cargo to the private directory before relinquishing the lease.

| Artifact | SHA256 |
| --- | --- |
| Ordered 28-file compiled source pin | `9bbe31441bb178bef85edd890002fc20ceca8364acb4dc3ed331bb3b381f3f9a` |
| event_lifecycle executable | `6c3a9c16964719ad0cec8f86f993c82332c71754d7626435d5c7e69f97fe6783` |
| event_lifecycle_archive executable | `18df1373eac853449f6a697a7e677b58e148ed188708eb5508348de692d22c60` |
| event_lifecycle_read_only executable | `7d3b0f185295c39369bce20a35615c6e94171b9125c195011531f706d86692df` |
| concert_service library-test executable | `f694c4b2d467a25317a56659d42bcfc6d46183257acdba03863306bb2e836047` |
| `/tmp/mcr-be41-frozen-race/freeze.json` | `6a31004b3ed76f6758bf661fb5c1065ab933dd0890296e576299057f6802e0c3` |
| `/tmp/mcr-be41-race-qualified.json` raw combined evidence | `88e42252c7ee1f5a91c0c227c9618b25c86e9823c27c0a0ad541cc0a885ce6ac` |

The driver tests deliberately use synthetic operator-role/retention authority and
synthetic Avatar metadata. They prove the actual World four-source/native and
composite transaction path, not production operator authentication, approved
character art, multi-process/load qualification or the full BE41/root journey.
The existing HTTP helper accepts the older service type; the transport/root owner
must explicitly wire this driver and actual authorities/later owners. No mock or
passing component is promoted to BE41 ACCEPTED. Current owner-directed delivery
policy allows this validated exclusive patch to publish non-force to master after
peer reconciliation; no main-chat or scrum-lead approval gate is required.

### Delivery ancestry and final peer reconciliation

Owned implementation commits are `3265fb1a` and `d47479cb`; actual driver/native
qualification is `3d65e251`; the authorization race fix is `3a6957e2`; current
23-native evidence and exact migration-25 ledger adaptation are `bb8db428`.
Merge `129d0c31` reconciles the subsequently published participant producer.
The 28 qualified lifecycle/World/native-fixture inputs remain byte-identical to
the passing compiled source pin after that merge. Among the 1407 frozen backend
inputs, only `concert-service/build.rs` and `src/lib.rs` change: additive generation
and export of the participant RPC module. Its new protocol/implementation/tests
remain the participant owner's patch. Static dependency comparison finds no
lifecycle/World coupling or changed migration, manifest or lockfile. No fresh
Cargo compilation or participant-native qualification is claimed for this final
additive merge; the recorded native proof belongs to the exact frozen candidate.
Planning/provenance, ordered source-pin verification and diff checks after peer
reconciliation all exit 0. Backend build resources remain released to uploader.

## Historical native-qualified continuation at c2a09dcc

The following section describes the earlier isolated `f5af234` continuation only;
its 9/23/6 results and raw JSON do not qualify this READ_ONLY increment.


BE41 remains **IN_PROGRESS**, with full acceptance **BLOCKED** on production owner
composition and delivery wiring. The bounded native/migration continuation is
validated. The persistent goal tool still reports **BLOCKED**: Main authorized
manual continuation without replacing the aligned goal or falsely completing it.
This record supersedes the earlier source-only/native-NOT_RUN checkpoint.

Current continuation base: `f5af234c09fc75d9a91ba26e1c2a3766fc90f1f4`.
Worker branch: `codex/mcr-be-lifecycle-resume`. The former
`codex/mcr-be-BE41` checkout is read-only. Delivery alone integrates/pushes master.

## Provenance, scope and contract

All lifecycle logic is newly authored; no Bigyeting rendering/code was copied.
Dependencies are existing pinned workspace Rust crates (SQLx/PostgreSQL, Tokio,
Serde, Chrono, UUID, SHA2, Ring, Axum and JSON Schema), with no manifest/lock changes.
The source milestone sequence was imported with `cherry-pick -x`:

| Original commit | Continuation commit | Purpose |
| --- | --- | --- |
| `077f01cf0fe99a829eb9919243d9ae43813bb195` | `e90a62a` | Durable closure and anonymous archive boundaries |
| `2a8bfb469ed1f706330a595bb1a8014abfc8fbde` | `5221c2b` | Concert privacy expiry and retention components |
| `e2b150e92040c577bc1199bb74aaa26f641af97f` | `48c67c6` | Owned PostgreSQL and native test runner |
| `0136e90d1645633432cb0f9e07fa9d31a63326cb` | `410e146` | HTTP error separation and late-write retention regression |
| `c0bc87e49b5dbb6d01ba221046d2fba59a4a6ae4` | `468418a` | Historical blocked audit |

The imported original scope is the domain/application `event_lifecycle/mod.rs`,
service `src/event_lifecycle/{mod,archive,http,privacy}.rs` and `schema.sql`,
`src/workers/cleanup.rs`, `tests/event_lifecycle{,_archive}.rs`,
`tests/support/event_lifecycle/mod.rs`, `backend/scripts/mcr/event_lifecycle_pg.py`
and this record. Main additionally assigned exactly the three `event_lifecycle`
root exports, inline `workers::cleanup` export, canonical
`migrations/0018_event_lifecycle.sql`, and `tests/event_lifecycle_support/**`.
No routes, other-service implementations, central contracts, manifests, lockfiles
or historical migrations were changed. `docs/source/` remains byte-for-byte intact.

The frozen contract is `911f156`, already included in the continuation base:
[START/RESUME contract](../contracts/event-closure-resume.md). The existing
`closeConcertEvent` route accepts the closed START/RESUME union. START uses the
event revision; RESUME uses the job revision, original initiating actor, same
event/job, a fresh key, and current Auth/operator/event-policy authority. Exact
same-key/body replay returns prior admission and never retries failed effects.
GET and worker scheduling do not resume PARTIAL_FAILURE. A SUCCEEDED/200 command
means durable admission committed; it does not certify seven completed effects.
202 is pending/uncertain only, with null resource references. Anonymous archive
output is the exact C04 `admin.Archive` schema.

## Implemented behavior

The service producer locks the event, checks current fenced authority and commits
READ_ONLY, one durable bound job, real local admission-barrier owner evidence,
audit, and BE15 command receipt/outbox atomically. Phase alone cannot invalidate
already-admitted world/community flows: the owning adapter is mandatory.

Seven durable steps are READ_ONLY, FREEZE_MURAL, SNAPSHOT, EXPIRE_GRANTS,
EXPIRE_BEACONS, EXPORT and CLEANUP. Each owner step, its operation-bound receipt
and progress commit in one local PostgreSQL transaction. A failed effect rolls
back to a savepoint before recording FAILED. Completed steps cannot regress;
explicit resume changes only the failed boundary. Retention policy revision and
close/due timestamps are immutable. CLEANUP waits until due; ARCHIVED follows
actual final success. Remote destructive work cannot be treated as local SQL.

`schema.sql` and canonical migration `0018_event_lifecycle.sql` are identical.
Deferred phase/progress/authority guards reject false completion, missing owner
evidence, expired authority, changed bindings and mutable audit/effect receipts.
The migration is forward-only and leaves all published migrations unchanged.

The public archive stores no Post bodies. Every page requires authoritative
Post/deletion/moderation/privacy and independently public-media qualification,
plus a final source revision fence. Changed public projections advance archive
revision; stale cursors conflict. Pagination is strict descending time/UUID and
AEAD-bound to event, revision, upper/last tuple, limit and 15-minute expiry. Final
mural projection is frozen and immutable. Unknown/private output fields fail
closed. HTTP server projection errors return 500; dependency failures remain 503;
stale archive revision returns 409. Responses use no-store.

Actual Concert privacy SQL expires ACTIVE grants and PENDING/APPROVED requests,
revokes ACTIVE beacons, and performs due-only event-owned privacy retention.
It requires both lifecycle and independently pinned canonical relationship
fences. Retention deletes terminal request/binding/grant and revoked beacon rows,
scrubs handle ciphertext, preserves command/audit tombstones, and verifies no
owned private residuals. A late write cannot renew the event-close retention
period. Narrow beacon/retention component receipts cannot certify composite
world/community cleanup. There are no permissive production owner defaults.

## Executed qualification — 2026-11-04

All commands below exited 0 on the continuation base plus the committed worker
patch. Rust was pinned to 1.88.0, locked/offline, jobs=2, dev/test debug=0,
incremental=0. The only assigned target was
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`; no third target was
created and the 8 GiB floor was preserved (13 GiB free at inspection).

```sh
export CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target
export CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-service --test event_lifecycle --test event_lifecycle_archive --no-run
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-domain -p concert-application -p concert-service --lib event_lifecycle
python3 backend/scripts/mcr/event_lifecycle_pg.py --native-binary "$CARGO_TARGET_DIR/debug/deps/event_lifecycle-0a788f3afdfa0856" --native-binary "$CARGO_TARGET_DIR/debug/deps/event_lifecycle_archive-41a488a43c660038" --output backend/services/concert-service/tests/event_lifecycle_support/native-evidence.json
python3 scripts/mcr/check-plan.py
git diff --check
```

- Native integration: **9 passed, 0 failed, 0 ignored** across the two binaries.
  This includes admission replay, stale revision rejection, fresh-key resume,
  START replay leaving failure intact, START-key reuse conflict, rollback of
  partial/wrong owner effects, concurrent workers, delayed retention and current
  authority revocation. Actual Concert privacy SQL preserves a foreign event;
  archive deletion invalidates cursors without resurrecting deleted content.
- The seed and recovery tests ran in distinct native processes around an actual
  PostgreSQL stop/start. Completed effects remained once-only and the failed
  boundary required explicit resume after restart.
- Migration test used real SQLx/runtime startup on two extra databases on the
  verified fixture server: complete fresh install, full pre-0018 forward upgrade,
  unchanged preexisting event, exact checksum verification and reapply preserving
  every ledger field. All generated databases and fixture directories were removed.
- SQL fixture: **23 passed** for durable/atomic state, uniqueness, rollback and
  actual backend termination, restart, failure/resume, append-only evidence,
  authority deadlines, immutable bindings/mural and foreign-event privacy isolation.
- Focused units: **6 passed** (2 domain, 4 service). Application compiled but had
  **0 matching tests**; this is not an application test pass count. Service units
  include AEAD cursor failures, frozen mural projection, malformed response/500,
  and frozen START/RESUME closed-shape input qualification.
- Owned Rust roots passed `rustfmt +1.88.0 --edition 2021 --check`; Python source
  passed `compile(..., 'exec')`; planning/provenance check passed independently.
  Existing unrelated dead-code warnings remain; no native compile errors occurred.

Committed raw [native/SQL evidence](../../../backend/services/concert-service/tests/event_lifecycle_support/native-evidence.json)
contains exact migration, root export, generated contract and manifest/lock
hashes, native counts/output and binary SHA256. The runner rejects missing,
duplicate, unexpected or skipped native suites and mismatched compiled owned
source pins. Final evidence SHA256:
`de250f2e141a9bf5c8034346432a66b2298568ed890f2f67cb577e374d9d29ca`.

| Qualified artifact | SHA256 |
| --- | --- |
| Owned compiled-source pin (12 files) | `fd9c94c9139e7d722539520f26b9a4de6fa172b125056743126b8372b5ce3828` |
| Lifecycle native binary | `cbdcec0c641ad91fc0003ec1d1837406eee7d41049cc0fb29c5f67dde50a2367` |
| Archive native binary | `0f9d344949f9aee64dd8bcadfb56beeedfb7d3b3d2289a594b9e2bef8bdab26b` |
| Canonical 0018 / owned schema | `8a93ed7e1b02345c3071cb0583384d61bb1e0b509e8939daf568f9ea352e2cb0` |

## Delivery boundary and remaining acceptance

The target was explicitly released to Main after all Cargo/native/PG handles
finished. `lsof` found no target/fixture handles; process inspection found no
retaining process; the owned temporary-root glob was empty. Further compilation
requires a new explicit resource transfer. Delivery serially integrates the
worker commits and requalifies changed combined composition before master push.

Still **NOT_RUN / unresolved**: production canonical Auth/operator/event-policy
and reviewed retention composition; canonical BE37 relationship fence adapter;
real final mural freeze/snapshot/export; authoritative public Post/deletion/media
owner; community cancellation and world admission/lease invalidation; remaining
composite retention; Gateway route mounting and actual scheduled cleanup loop;
real multiuser/release acceptance. Synthetic authorities/healthy owners exist only
inside marker-owned native fixtures. Their receipts cannot certify those services.
This patch supplies validated lifecycle machinery and Concert privacy components,
not full BE41 acceptance or a scope cut. Required work remains on the original
persistent goal, whose actual tool status is BLOCKED.
