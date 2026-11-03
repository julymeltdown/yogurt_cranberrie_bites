# BE41 event lifecycle — implementation and qualification record

## READ_ONLY owner increment — source authoring, native NOT_RUN

Current branch: `codex/mcr-be-read-only-barrier`, reused isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-lifecycle-resume`, base
`1e5022885525ab56f056aa2c24100e18c4bc67ed`. Former `c2a09dcc` branch/commits
remain intact. This increment is **IN_PROGRESS**, not native-qualified or accepted.
The aligned broader persistent goal still reports **BLOCKED**; Main explicitly
assigned manual continuation without reset or false completion.

The exact original ten-path grant covers service
`src/event_lifecycle/{read_only,mod}.rs`, `src/community/{closure,mod,giveaway}.rs`,
`migrations/0024_event_read_only_barrier.sql`, `tests/event_lifecycle_read_only.rs`,
`tests/event_lifecycle_support/read_only.rs`, the backend lifecycle Python runner,
and this record. Main separately granted the eleventh path
`tests/event_lifecycle_support/migrations.rs` for exact forward-ledger adaptation.
All abbreviated paths are under `backend/services/concert-service/`. World owns
0023 and its entire implementation/invalidation/runtime/outbox scope. No World,
frontend, contract, manifest, lockfile, root composition, previous migration or
source archive is changed here.

### Real owner and native ordering

`read_only.rs` calls the frozen actual owning API
`world::closure::fence_event_read_only_in(&mut PgConnection,event,job,operation)`;
no default, fixture, boolean result or separately committing consumer replaces it.
Its returned `WorldReadOnlyReceipt` must match exact event/job/operation, a nonnil
receipt identity and positive bounded revision. Community cancellation and both
community/composite receipt insertions use that same caller connection. The
World-owned 0023 schema stores its own lease/victim snapshots; 0024 does not
invent or duplicate that schema. The composite records only World receipt ID and
revision plus a same-scope community FK, not copied World identities or content.
A World receipt FK and binding trigger enforce exact event/job/operation/revision
against World-owned 0023, without changing that schema.

`ReadOnlyCommandDriver` obtains its pool/event from the actual `AdmissionBridge`.
It validates input and performs actual current operator/policy preflight before
any native invalidation; unauthorized/stale/new-event callers cannot install a
process-wide removal fence. Exact-key replay, conflicts and RESUME remain BE15
operations. Fresh START follows delivery gate -> short native fence via
`begin_invalidation(stable operation, Event scope)` -> admission gate ->
`pause_invalidation().await` -> existing BE15 transaction/event FOR UPDATE.
No SQL transaction is held while acquiring the owner gates, and no native mutex
spans an await. Current authority/revision is rechecked by the actual producer
inside SQL; preflight is not a durable permission grant.

The existing production publication lease retains event FOR SHARE and checks
OPEN/LIVE/revision. Its task disables native sends under the exact shared fence
before SQL rollback. Event FOR UPDATE therefore also drains qualifying permits
in other processes. The local early native fence prevents delayed pending views
from reappearing while SQL waits. A single local bridge does not claim synchronous
foreign-process cache removal; actual cross-process permit behavior remains a
native qualification obligation.

SQL failure/cancellation/process death rolls back the event/job, owning World
mutations, community root/children, receipts and outbox together. Removed native
views are **never restored** by rollback. The driver drops only its private
operation-bound preparation token; native pending removal remains on failure or
cancellation. Recovery requires fresh qualified World authority and a committed
phase reread before clearing that marker. Confirmed durable success may finish
the marker; finish failure cannot rewrite a committed admission as a rollback.
Later six steps still require an explicitly supplied actual owning adapter.

`community/closure.rs` locks all event roots in stable ID order, invokes existing
checked Party Cancel and Giveaway Cancelled transitions and persists through the
canonical root/child serializers. The aggregate owner parameter is used only for
internal domain invariant checks, never as a forged operator session or a user
command receipt. Pending party applications cancel; terminal party root/audience
fences deny membership access; held/ready reservations cancel and release reserved
inventory; pickup secrets and meeting/code state are removed. Claimed/completed
history remains. Residual active roots/children/counters/secrets abort the entire
transaction; no truncated successful batch exists. Ordinary concurrent producers
already hold event SHARE; READ_ONLY denies reopening while limited safety actions
remain subject to current authority.

0024 has immutable event/job/operation-bound community/composite receipts,
positive revisions, lifecycle commit-deadline guards and same-scope FK/unique
constraints. New mutations require the initial PENDING revision-1 job in
READ_ONLY; a stored same-operation receipt can be verified later without repeating
effects. Wrong bindings, non-READ_ONLY steps and missing/expired transaction
fences fail closed. Both component entry points refuse an SQLx connection with
transaction depth zero before any mutation. Existing published migrations remain unchanged.

### Authored validation and pending resource/API gates

Ten new ignored owned-PostgreSQL tests exercise actual World/community SQL:
root/child cleanup and foreign-event isolation, exact admission replay,
post-component and between-owner injected-failure rollback, expired-commit rollback, preconsumed-ticket/lease denial,
concurrent START, component scope and immutability, event SHARE serialization,
and two-process restart/replay. They use synthetic test-only Auth/role authority;
they do not certify production Auth, native send revocation or root mounting.
The SQL SHARE test specifically does not masquerade as a ProductionOwners native
permit test. Two focused units cover receipt-binding rejection and cancellation
of the private preparation token; they also remain NOT_RUN.

The runner now requires exactly three source-qualified binaries and all **19**
expected native tests (historical 9 plus new 10), validates no missing/duplicated
suite, and restarts both recovery pairs separately. Its compiled pin includes the
19 lifecycle/community/schema/test files and six read-only World dependencies,
so stale World binaries cannot qualify this composition. Raw evidence for this
increment must be written after actual execution, not copied from historical JSON.
The ledger test now requires the exact version sequence
`1..12,16,18,19,20,21,22,23,24`, pins published 0018 SHA256, and retains exact
receipt/checksum, existing-row upgrade and unchanged-reapply checks.

**Not run:** Cargo compilation, PostgreSQL install/reapply, all new tests, actual
native pending/active publication drain and gate-order/recovery tests, process
kill in the new component, current full migration runtime and production multiuser
flow. Main has not transferred a physical Cargo/PG slot; the World-owned API and
0023 must first be delivered/merged. No stub was added to make this compile early.
The existing HTTP helper accepts the older service type; the actual transport/root
must be explicitly wired through this new driver under separate ownership. Actual
Auth/operator/retention and later owner steps remain required. No unqualified
master publication or BE41 acceptance is claimed.

Executed non-native checks for this source checkpoint (all exit 0):

- `rustfmt +1.88.0 --edition 2021 --check` on the six assigned Rust roots (module,
  read-only component, community adapter, new suite, new support and ledger support).
- Python AST/`compile` of the runner, exact ordered 25-file Rust/Python pin parity,
  exact 19-test registry parity against all three Rust suites, no duplicate pins.
- Programmatic exact eleven-path ownership comparison; giveaway diff contains
  only two sibling `load`/`save` visibility changes.
- `git diff --check` and `python3 scripts/mcr/check-plan.py`: PASS. The planning
  checker separately verified all 13 original archive hashes and is not an
  application/native test. No Cargo, PostgreSQL or build-target command ran.

Before merging World publication, the explicitly absent compile dependencies were
`world/closure.rs` and `world/invalidation/{mod,store}.rs`. The latter two are in
World's published `55061be4`/master `a4ae8555`; the closure API/0023 is still pending.
Source commits may be shared for coordination; master publication requires the
real API merge plus relevant native qualification under a transferred slot.

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
