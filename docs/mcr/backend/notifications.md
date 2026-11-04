# BE33 durable concert notifications

Status: IN_PROGRESS. Source/persistence qualification is separate from canonical
Auth/target authority, authenticated RPC/HTTP composition and IN03 acceptance.
No public route is enabled by this patch. Goal remains unfinished through validation,
committed handoff and concrete fixes. Latest qualified source `9bb4a98b` compiles
all five requested artifacts and passes 215 current tests (204 platform, 3 Auth,
3 Friend, 5 Concert/PostgreSQL), plus the production six-migration CLI and SQL
witness/WAL probes. Real canonical owner RPC, Root target fencing and public
inbox vertical-flow qualification remain NOT_RUN. See the final checkpoint below;
earlier uncompiled-increment statements are historical source snapshots.

Base: `a25b85592b906209f4a9d20f9c7279ced8abab02`, branch
`codex/mcr-be-BE33`, isolated checkout
`$MCR_WORKTREE_ROOT`. Delivery alone integrates and
pushes `julymeltdown/mcr/master`. Four shared module registrations are the exact
Delivery grant, committed separately as `dd9776e`; manifests, locks, proto,
production authority and root composition remain Delivery-owned.

## Ownership and provenance

The inherited Notification owner remains authoritative. Its `notifications`,
subject-state serialization, actor/recipient suppressions, deletion saga/audit,
retention and purge are retained. Notification migration directory is
`backend/services/notification-service/migrations/`; the new identity is
`0005_concert_inbox.sql`. No Concert 0017 migration is required.

The imported foundation is port-rust
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, recorded in
`docs/mcr/backend/import-manifest.json`. Inspected inherited source includes
`services/notification-service/src/adapters/out/persistence/notification_repository.rs`
and migrations 0001–0004 under mapped `backend/`. Existing `event_id` there means
the source domain event/outbox UUID, **not** the concert event UUID. The added
metadata table stores the latter separately and references the inherited row.
BE15 outbox and receipt seams are inspected at this task's base. New domain,
application, adapters, migration and tests are authored for MCR. The test-only
socket/token/inode guard is adapted from
`backend/services/concert-service/tests/support/relationship/mod.rs` at the base;
destination is `backend/services/notification-service/tests/concert_inbox_support/mod.rs`.
The smoke harness imports the existing `migration_smoke.Cluster` guard rather
than introducing an arbitrary `DATABASE_URL` fallback. No frontend imports or
new visual dependencies are involved. `docs/source/` is untouched.

## Durable delivery and lifecycle

`PgProjectionOwner` reads only Concert's own committed BE15 receipt and outbox.
It verifies the entire envelope (outbox, receipt, event, resource, revision),
then invokes an explicit aggregate/policy routing port. Missing routing fails
closed. It never forwards arbitrary request bodies, handle values, meeting
descriptions, source titles or inherited `payload_json`.

`NotificationSink` is an `IdempotentInvalidationSink`. Success requires the
Notification owner to commit before BE15 acknowledges its lease. Retryable
authority/owner failures remain retryable; invalid envelopes or conflicting
immutable delivery identities are rejected. BE15 retains its attempt/deadline,
lease-fencing and terminal diagnostics behavior.

`PgConcertInbox::ingest` atomically inserts an immutable-envelope delivery
tombstone and the first resolved fanout into inherited notifications plus typed
Concert metadata. Concurrent/repeated outbox delivery creates one row per
recipient. Later projection changes do not expand a committed fanout. Payload
text is empty; kind-based neutral labels are fixed constants. Deletion locks use
inherited permanent subject rows in UUID order, inserting **all** missing rows
before locking any row, exactly as inherited creation does. Suppressed recipients/actors
receive no late row. A purged inherited row cascades its metadata, while its
delivery tombstone prevents recreation. There is no second inbox or Redis-only
durability. Empty authorized fanout is also durably recorded.

## Authorized inbox and read state

`FencedInboxAuthority` must verify the server-bound canonical actor, current Auth
family, event membership/moderation, blocks/deletion and each owning target. It
returns exact candidate-bound decisions and a retained `AuthorityFence` valid
through commit/response. A timestamp alone does not implement this fence.
`UnavailableAuthority`, `UnavailableRouting`, `UnavailableProjectionOwner` and
`UnavailableNotificationOwner` are strict defaults; no fixture authority is
registered in production. The plain `Context` is a port value and is never proof
of browser authentication on its own.

Reads use one repeatable-read owner snapshot, reauthorize the complete retained
inbox and retain local deletion locks through commit. Hidden targets disclose no
row or unread contribution. An explicitly authorized neutral stale record has
`target:null`; stale does not itself grant permission. Target navigation must
still perform the owning endpoint's current authorization.

Pagination uses random server-stored UUID tokens, scoped to actor/event/unread
filter and an upper cutoff, with a 15-minute expiry. Clients cannot supply raw
tuple positions. Limit defaults to 20 and is bounded 1–100. Every page gets fresh
authorization; unavailable authority returns unavailable, never partial rows.

Read state is monotone: revision 1 is unread, revision 2 is read. Deriving it from
the inherited `read_at` keeps legacy owner mark-read operations coherent. A table
guard prohibits changing an established Concert read timestamp. Single reads
check expected revision. Durable actor/event/operation/key receipts bind exact
notification/revision inputs; conflicting reuse fails. Read-all captures a server
cutoff and touches only authorized own-event unread rows in that snapshot. Exact
replay returns the original count/cutoff, leaving later ingress unread. The
24-hour result window expires but its receipt tombstone remains. Saved results
are never returned before current authorization. Serialization/lock/dependency
failures return unavailable; callers may reauthenticate and retry an identical
key without resetting read state. Deferred SQL receipt/cursor guards and bounded
statements reject commits after the original authority deadline.

## Public contract and composition request

Pinned C04 amendment: `a25b855`, read from
`contracts/domain/v1/{operations.json,feed.schema.json,policy.json}`.
All four operations are `no-store`; `concert::wire::{execute,response}` produces
the existing closed DTO shapes and safe errors.

| Operation | Method and event-relative suffix | Result |
|---|---|---|
| listConcertNotifications | GET `/me/notifications` | `{items,next_cursor}` |
| getConcertNotificationUnreadCount | GET `/me/notifications/unread-count` | `{actor_id,event_id,unread_total,as_of}` |
| markConcertNotification | PATCH `/me/notifications/{notificationId}/read` | Notification |
| markAllConcertNotifications | PATCH `/me/notifications/read-all` | `{read_count,read_at}` |

Full prefix is `/api/concert/events/{eventId}`. Unread count has no list
query/filter and counts the **entire authorized unread owner snapshot**,
independent of pagination. Policy failure remains 503; loaded rows are never a
substitute count and unavailable is never zero. Single mark-read takes the
canonical `expected_revision` body and UUID idempotency header; read-all takes
the canonical empty body and idempotency header.

The adapter maps invalid input/cursor to 400, actor/event denial to 403, hidden
or missing item to 404, revision/key conflict or expired key to 409 and unavailable
authority/persistence to 503. Gateway owns canonical authentication (including
401), strict parsing, Origin plus browser-intent enforcement, deadlines and
route registration. No pending 202 is fabricated for these synchronous local
transactions. Durable delivery requires a separately authenticated workload and
source-envelope verification; existing Gateway-only actor delegation must not
be relaxed silently for Concert-to-Notification calls.

Delivery must provide path dependencies (Notification -> Concert domain/app;
Concert test -> Notification), authenticated transport/identity policy, real
aggregate routing and current Auth/target fencing, and root route/RPC/service
composition. The new ports do not assert those are already qualified. FE35 has
confirmed compatibility with the a25b855 count shape and unavailable semantics.

## Verification record

| Check | Actual evidence |
|---|---|
| Direct `rustfmt +1.88.0 --edition 2021` on owned Rust paths | Exit 0 |
| `python3 -m unittest discover -s backend/scripts/mcr -p test_notification_smoke.py -v` | Exit 0, 3 harness refusal checks |
| Owned PostgreSQL 16.14, apply Notification migrations 0001–0005 transactionally | Exit 0; 5 migrations, 10 public tables; guarded cleanup PASS |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning/source provenance only |
| `git diff --check` | Exit 0 |
| Cargo targeted test compilation | PASS at 08e76dbd after two-URL fixture fix |
| Rust repository/outbox tests on real PostgreSQL | PASS: 22 inbox plus five Concert source/outbox tests at 08e76dbd |
| Runtime migration ledger and restart/replay | PASS: migration CLI, collision rollback/recovery, drift refusal, WAL crash recovery and exact receipt replay |
| Canonical Auth/target mutation fencing, public HTTP/RPC, two-account flow | NOT_RUN, Delivery-owned composition required |
| IN03/release acceptance | NOT_RUN |

The native harness is `backend/scripts/mcr/notification_smoke.py`. It requires
both exact test executables and SHA-256 pins, copies them into its private owned
fixture before use, optionally qualifies the Notification service migration CLI,
runs all discovered owned tests with zero skipped cases, stops its owned
postmaster with `pg_ctl -m immediate`, verifies WAL recovery/persistent rows and
exact delivery/read-receipt replay after restart, then
cleans up only its marker/inode-validated store. Its report records source,
migration and executable hashes, command exits and explicit authority/integration
limits. Rust tests require the guarded socket, owner token and inode identity;
they never fall back to inherited or environment-default databases.

Current repository tests use real SQL/persistence/transactions and explicitly
test-only authority rows. They cannot certify real canonical authentication or
cross-service target revocation. Full-snapshot authorization scans all retained
inbox rows; production scale/latency and an owner snapshot protocol remain
unqualified. No partial count is returned if the deadline cannot cover a scan.

Committed handoff, exact changed paths, native results and master ancestry will
be appended after qualification. Only Delivery publishes master.

## Functional resume and review correction

At functional resume the original unbudgeted goal was ACTIVE. Backend staffing reconfirmed
exclusive ownership of the original BE33 paths; there is no new application,
composition, manifest or SDK ownership. The four preserved validation changes
were retained. No build target or compile lease is held.

The focused read-only review found a real inherited-writer lock cycle: with
subject A present and B absent, Concert could lock A before inserting B while
inherited creation inserted B before trying to lock A. The helper now matches
the inherited two-phase insertion/locking order. The added real-PG regression
uses an advisory-gated subject insertion and observed database wait states to
force that interleaving; both actual repository operations must finish. It is
authored, not yet executed. Follow-ups also add transaction-wide fanout rollback,
ingress/deletion racing, expired receipt tombstones and authorized foreign cursor
rejection. Error timestamps now use canonical UTC `Z`.

Current authored native scope: **22 inbox tests and five Concert source/outbox
tests**, all NOT_RUN until exact binaries are compiled. Harness refusal tests
were rerun: three passed, exit 0. Rustfmt and diff whitespace checks pass. The
native harness now additionally checks migration 0005 collision rollback,
forward recovery and checksum-drift refusal when supplied the service binary.

Read-only inspection of qualified Participant `6767ee8e` and owner `dc70d620`
confirms reusable canonical owner guards/settlement, but the inspected SDK's
producer types admit only Concert/Post/Profile. Notification-owned transactions
cannot be mislabeled Concert. Delivery has been asked for the exact Notification
producer/child settlement and authenticated transport extension, plus the four
public operation registrations. Three missing path-dependency/lock changes were
requested separately so original persistence qualification can proceed without
waiting for the entire integration wave. No accepting replacement authority,
invented production interface or source-only acceptance is introduced.

The reviewable central-file dependency diff is
[`central-dependencies.patch`](../../../backend/services/notification-service/tests/concert_inbox_support/central-dependencies.patch).
It adds the two Notification path dependencies, the Concert test dependency and
their existing-package lock entries. It introduces no new package resolution.
The original proposal passed `git apply --check` before application. Delivery's
exact temporary three-file grant was recovered from its trusted coordination
record after direct steering failed. Published master
`59de1ac9547ccd22a94bea8e8f224fe16a8c4eeb` was merged into this isolated branch
as `eb0afcc41e87d2b11d39194f95a293a8c3b1047c`; three export conflicts were
resolved by retaining every published export and the already-granted Notification
registration. The dependency diff was then adapted and applied against that
merge. TOML parsing and existing-package resolution equality checks passed; only
the three granted dependency edges changed. The artifact now records the applied
diff. `git apply --unidiff-zero --reverse --check` validates that exact correspondence.
The zero-context form keeps literal blank patch-context lines out of whitespace
checks. No other
shared-file scope or build lease is implied, and native tests remain NOT_RUN.

## Production protocol inventory for composition owner

The public operation identifiers and closed shapes are already represented by
`notification::wire::OPERATIONS`, `Operation` and the domain DTOs:

| Operation ID | Method and suffix after `/api/concert/events/{eventId}/me/notifications` | Input | Success |
|---|---|---|---|
| `listConcertNotifications` | GET, empty suffix | `limit` 1–100 (default 20), `unread_only` (default false), optional opaque `cursor` | `{items, next_cursor}` |
| `getConcertNotificationUnreadCount` | GET `/unread-count` | No list query/filter | `{actor_id, event_id, unread_total, as_of}` |
| `markConcertNotification` | PATCH `/{notificationId}/read` | `{expected_revision}` and UUID idempotency key | The exact seven-field Notification |
| `markAllConcertNotifications` | PATCH `/read-all` | Empty body and UUID idempotency key | `{read_count, read_at}` |

Notification fields are `notification_id`, `event_id`, `kind`, `target`,
`created_at`, `read_at`, `revision`; a target contains only `target_type` and
`target_id`. These are existing C04 shapes, not authority credentials. Gateway
must derive actor/family/request identity from canonical authentication, enforce
browser Origin/intent rules, and map the concrete Notification producer through
dedicated authenticated transport. A browser-supplied actor, forwarded digest,
or signed timestamp alone is insufficient.

The present owner port accepts `Context` (actor/event/family/request UUIDs,
32-byte authority digest, observed/expiry timestamps bounded to five seconds),
then supplies the complete ordered retained snapshot as `Candidate`
(notification UUID, optional source actor, optional typed target). Even an empty
snapshot requires current own-inbox authorization. `Authorization` must bind the
exact context and candidate sequence, every Visible/Stale/Hidden decision and
its validity interval. Production Auth, participation/moderation, relationship,
deletion and target-owner mutation fences must remain held through the actual
Notification database commit and response projection.

This local transaction is Notification-owned, never Concert-owned. The SDK
extension needs a genuine Notification child producer and local transaction
registration/commit settlement before the repository can qualify production
authority. `authorize_in` receives its actual `PgConnection`; `Snapshot::finish`
currently validates the retained fence immediately before and after `commit`.
That timestamp/fence hook does not implement the missing canonical child
transaction settlement. The implementation owners must agree the concrete SDK
adapter and any necessary finish hook before enabling transport. No accepting
substitute is installed.

Durable ingress has a separate workload identity. Its dedupe/recovery envelope
is exactly `{outbox_id, receipt_id, event_id, resource_id, resource_revision}`;
attempt/lease identifiers are not dedupe identity. Concert verifies its actual
committed receipt/outbox and resolves policy-approved typed recipients. The
Notification owner verifies the authenticated source and durably commits the
immutable envelope plus first fanout before acknowledging. Recovery uses the
same envelope and read idempotency key; retained tombstones prohibit recreation
after purge or expired-key reuse. The authored native restart test is
`durable_restart_replays_exact_delivery_and_read_all_receipt`; lost-ack replay is
`lost_ack_redelivery_deduplicates_before_outbox_ack`. These tests prove local
persistence only once executed, not production workload authentication.

Post-merge checks: the three Python harness refusal tests passed (exit 0),
`python3 scripts/mcr/check-plan.py` passed (exit 0; planning only), and owned-path
whitespace plus unchanged `docs/source/` and frontend comparisons against the
merged master passed. Native compilation/execution still requires the named
build offer requested from backend resource coordination.

The merged BE15 `Delivery` now carries an `InvalidationKind` and optional command
receipt. The command sink and test constructors were aligned with that actual
API. A genuine maintenance invalidation has no command receipt and cannot be
projected using this command-only source verifier. It remains retryable and
unacknowledged until its explicit Notification policy is implemented; malformed
kind/receipt pairs are rejected. No fake command UUID or successful empty fanout
is substituted. The new native regression uses the actual `PgOutbox` with
published migration `0011_outbox_maintenance.sql`, now included in the focused
Concert fixture alongside 0001–0002. It checks retry state, absence of an inbox
delivery tombstone and rejection of mismatched kinds. This brings the authored
native scope to **27 tests**, still NOT_RUN. Rustfmt, the three harness safety
tests and whitespace checks passed after this correction.

## First native qualification, 2026-11-04

The fresh BACKEND lease was explicitly accepted for the three exact commands
below, with Rust 1.88.0, locked/offline dependencies, two jobs, dev/test debug 0
and incremental 0. Preflight found more than 8 GiB free and no competing Cargo
or target handles. Source and all compiler JSON/diagnostics/executables were
frozen outside the target before unconditional release; no PostgreSQL fixture
ran while the target was held.

| Command under `backend/` | Exit and evidence |
|---|---|
| `cargo +1.88.0 test --locked --offline -j 2 -p notification-service --test concert_inbox --no-run --message-format=json` | Initial 101: restart UUID offset inferred i32; explicit u128 fix `9f81e2de`; retry 0 |
| `cargo +1.88.0 test --locked --offline -j 2 -p concert-service --test notification --no-run --message-format=json` | Initial 101: stale worktree proto path/ABI; scoped cache cleanup; retry 0 |
| `cargo +1.88.0 build --locked --offline -j 2 -p notification-service --bin notification-service --message-format=json` | 0 |

The evidenced stale packages were contracts-build, platform, concert-domain,
concert-application and Auth; only those were cleaned. No source authority or
policy was relaxed to repair the cache. The qualified source was
`9f81e2de90b4babc99ab02fb7a1f535cde0dcef0`, based on `77fbb186`.

Frozen artifact directory: `${BE33_NATIVE_77FBB186}`. It contains
compiler `*.jsonl`, `*.stderr`, `executables.json`, 25 source-file pins in
`source-pins-qualified.json` and the copied binaries. SHA-256:

| Executable | SHA-256 |
|---|---|
| `notification-inbox-test` | `83c37daae27f7cc7e47f35f6330ceb066e619ff2724406f859ea93d51d88101e` |
| `concert-notification-test` | `63803d57f7d2736d1b61b3b91ea24a89f9fb45605044b908d88dca60c35998cd` |
| `notification-bin-notification-service` | `cd6bc3b88ed3c778da184b2af8a51515ca7962a9d95091d33d6d5b8904f4001d` |

The copied-executable native harness ran on PostgreSQL 16.14, with a clean
worktree. All **22 inbox tests passed**, including whole authorized unread
counts, concurrent ingress/read operations, rollback, inherited deletion races,
two-phase lock-order regression, purge tombstones and expired read receipts.
The actual production migration CLI passed migration/verification/reapply,
0005 collision refusal and rollback, forward recovery, and checksum-drift
refusal. Five ledger rows were verified. Expected negative CLI exits were 1;
successful migration operations and inbox tests exited 0.

The overall harness exited 1: all five Concert tests refused the fixture URL
before their assertions because `seed` requires both Concert and Notification
pools but the original harness supplied one database URL. Owned cleanup passed;
crash/restart qualification was therefore not reached. Report
`native-report-1.json` SHA-256 is
`cc9e6b577ac37bc07e5ada6e8be9a362ce3c214ed0f261ee3746f7f29379d3a5`.

Fix `3b961c446312d0dc44b905d2f94219a9740c6bd9` supplies the Notification URL
separately as `CONCERT_TEST_NOTIFICATION_DATABASE_URL`; each pool still requires
its exact allowed database, socket, root/token/inode, private marker and actual
data directory. No URL is rewritten and no ambient fallback is introduced.
The three Python refusal checks, rustfmt and whitespace checks passed. A fresh
two-test compile offer is requested; the prior target was already released and
is not reused without a new offer/acceptance. The corrected Concert tests and
crash restart remain NOT_RUN. Canonical production authorization, transport,
public routes and release acceptance remain NOT_RUN independently.


## Completed local persistence qualification, 2026-11-04

The second fresh BACKEND offer was accepted at clean source
`08e76dbde86be08197ed7cbbad71450242c6625b` (test-support fix `3b961c44`).
Preflight verified 8.47 GiB free, no competing Cargo process and no target
handles. Only the two named test targets were rebuilt, with the same Rust 1.88,
locked/offline, two-job, debug-0, incremental-0 settings. Inbox compilation
exited 0. Concert initially encountered evidenced stale proto/ABI artifacts;
contracts-build, concert-domain, concert-application and Auth only were cleaned,
and its retry exited 0 in 21.16 seconds. No source changes were needed.

Both exact test executables, any automatically emitted binaries, compiler JSON,
diagnostics and 25 current source hashes were frozen under
`${BE33_NATIVE_08E76DBD}` before unconditional target release.
Fresh `lsof +D` found no handles. The unchanged production Notification binary
from source `9f81e2de` was hash-verified and copied there; its implementation and
migration inputs did not change in the later test-support/doc commits.

The native harness then exited **0** on PostgreSQL 16.14: **22 inbox tests plus
five Concert delivery tests passed, with none ignored**. An additional exact
restart receipt test passed after an owned `pg_ctl -m immediate` crash, a new
postmaster and WAL recovery. Persisted notification rows matched their pre-crash
hash. Delivery/outbox and read-all receipts replayed without duplicate effects.
Production CLI verification and the exact five-row migration ledger passed
again after restart. Owned postmaster/directory cleanup passed. There was no
shared-target access during the fixture run.

This qualifies the actual repository, migration CLI and injected-port BE15
adapter. Native authorization policy rows remain explicitly test-only; genuine
canonical Auth/target mutation fencing, workload transport, public routes and
IN03/release acceptance remain NOT_RUN. Maintenance without a command receipt
is intentionally unacknowledged until its explicit Notification policy is bound.
This is no full BE33 product-acceptance or production-enable claim.

Portable native report (the original local JSON SHA-256 is `05b4bde2e0324c50d4d8ee0d507f694780d6b4bfde29e96be55a208dec0c6d10`):

```json
{
  "scope": "BE33 durable Notification owner and BE15 sink",
  "result": "PASS",
  "commands": {
    "empty-ledger-refused": 1,
    "runtime-migrate": 0,
    "runtime-verify": 0,
    "runtime-reapply": 0,
    "0005-collision-refused": 1,
    "0005-forward-recovery": 0,
    "0005-recovered-verify": 0,
    "checksum-drift-refused": 1,
    "notification_tests": 0,
    "concert_tests": 0,
    "restart_receipt_replay": 0,
    "runtime-verify-after-restart": 0
  },
  "binary_sha256": {
    "notification_test": "8dc3f87552e986bb570a23db8859231ce897bb7dba510a5712f745c78b563a20",
    "concert_test": "f5adee06217f957c77fd0b639786b07b3c21769ce8d7b0e7c1096320d9ded62b",
    "service_binary": "cd6bc3b88ed3c778da184b2af8a51515ca7962a9d95091d33d6d5b8904f4001d"
  },
  "canonical_auth_target_authority": "NOT_RUN",
  "public_http_rpc": "NOT_RUN",
  "release_acceptance": "NOT_RUN",
  "source_revision": "08e76dbde86be08197ed7cbbad71450242c6625b",
  "dirty_files": [],
  "source_sha256": {
    "backend/crates/concert-domain/src/notification/mod.rs": "24df36fa3380af8e47a48ca420bf789757eb2b39fd56013d49f902e72e06c0e7",
    "backend/crates/concert-application/src/notification/mod.rs": "07b0e37d6a503cda623660bab63143c75bb5cf48ab811f1d49ecd417276a1b98",
    "backend/services/notification-service/src/concert/mod.rs": "d9794051398c1f21945a48c30827c841b5b700829b5634408db332bd9f4109b3",
    "backend/services/notification-service/src/concert/repository.rs": "a0abbdcd6620e9b8b9df63e4014c64949dd52035e98d61926a44540456b8cfca",
    "backend/services/notification-service/src/concert/wire.rs": "a7c1f38be425d32690e2d2234880ee86b8baf7893b83ea18a57f1fca8b1f1b33",
    "backend/services/concert-service/src/adapters/out/notification/source.rs": "92fb4d603f9be9b58339e727e0c85e253fb3fa8c43128889ba379961e448154e",
    "backend/services/concert-service/src/adapters/out/notification/mod.rs": "87be6d77bbf3edafd481bbd780f0b77b2d1862e575a3c95e2a0398c3884e89b8",
    "backend/services/notification-service/tests/concert_inbox_support/mod.rs": "b1bef1879dfc4ed9169fe490bcb22947257102235da0a81d417a1649442b3bb4",
    "backend/services/notification-service/tests/concert_inbox.rs": "4d4a90f281828f96e4ce9107f0498e2d7b8cc1b0ebc8ea6452cd3e70ccb6a9a2",
    "backend/services/concert-service/tests/notification.rs": "f24b9a1f9e3422bbd393481dd550eafc96af3812b4dabd8b3daab1f53e146fcb",
    "backend/scripts/mcr/notification_smoke.py": "71843399ea687a72c3b2d0ac3278377d181df854d1e1d423f1425ae6a0f5d13a",
    "backend/scripts/mcr/migration_smoke.py": "a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8",
    "backend/Cargo.lock": "dad6e0bd3194942ac1bb5bf14cd95a53c3cb2b75bedd7c69c9f9f072300973eb"
  },
  "postgres_version": "16.14 (Homebrew)",
  "migration_sha256": {
    "backend/services/notification-service/migrations/0001_init_notification.sql": "3a74c58e0366823d732de64e2d3e3ba0c1ec1f1fa15bde379296e37ab8c0124c",
    "backend/services/notification-service/migrations/0002_governance.sql": "d645b9fb9d23bf9295d63c90fdb0ba0df2c9435072d2bbef45e8b77e46f52087",
    "backend/services/notification-service/migrations/0003_recipient_suppression.sql": "70b2cd29a581c838569d6e1cbd0f11920b3bceb2112b1e525c7190c05297854e",
    "backend/services/notification-service/migrations/0004_notification_consistency.sql": "94efdaf34a7cc130274ba399b9c6299d9c2d19b9a2f546013f05f4a5a387d19e",
    "backend/services/notification-service/migrations/0005_concert_inbox.sql": "c5b228ad5b502c2c182b76797b9c1c3f76189b51f7020a2226e6114ea09aa40a",
    "backend/services/concert-service/migrations/0001_concert_core.sql": "9978e75e569c1ba25cb191ddf5b02bbdffcd5e64f3385173f05a885370d9215a",
    "backend/services/concert-service/migrations/0002_command_receipts_outbox.sql": "5f5760dfe6384cb2ab5162bcc27fbc0ed75bdca6bb85229012f6389269ee442f",
    "backend/services/concert-service/migrations/0011_outbox_maintenance.sql": "8971093dee1293dfd160920c773d663866dcb5cb688fb7cabf6746eaca2eb61b"
  },
  "migration_ledger_rows": 5,
  "runtime_migrations": "PASS",
  "notification_tests": {
    "output": "ing_envelope ... ok\ntest durable_restart_replays_exact_delivery_and_read_all_receipt ... ok\ntest expired_read_receipt_is_retained_and_cannot_mark_new_ingress ... ok\ntest full_unread_count_is_independent_of_pages_filter_and_read_state ... ok\ntest ingress_failure_rolls_back_entire_fanout_and_tombstone ... ok\ntest ingress_racing_inherited_deletion_never_leaves_a_live_notification ... ok\ntest inherited_creation_and_concert_ingress_share_two_phase_subject_lock_order ... ok\ntest inherited_deletion_suppresses_actor_recipient_late_ingress_and_replay ... ok\ntest inherited_mark_read_is_reflected_in_concert_revision ... ok\ntest multi_recipient_fanout_and_no_secret_fields ... ok\ntest opaque_cursor_paging_binding_tamper_and_expiry ... ok\ntest purge_tombstone_prevents_recreation ... ok\ntest read_all_cutoff_receipt_replay_and_event_scope ... ok\ntest single_read_is_monotone_idempotent_and_revision_guarded ... ok\ntest stale_target_is_neutral_and_current_auth_denial_wins ... ok\ntest typed_wire_contract_and_unavailable_no_store ... ok\ntest unknown_authority_corrupt_binding_and_deadline_fail_closed ... ok\n\ntest result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.41s\n\n",
    "result": "PASS",
    "test_count": 22
  },
  "concert_tests": {
    "output": "\nrunning 5 tests\ntest expired_lease_fences_old_ack_after_durable_owner_commit ... ok\ntest lost_ack_redelivery_deduplicates_before_outbox_ack ... ok\ntest maintenance_without_command_receipt_is_not_acknowledged ... ok\ntest missing_owner_never_acknowledges_and_invalid_envelope_rejects ... ok\ntest source_receipt_verifies_full_envelope_and_unavailable_routing ... ok\n\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s\n\n",
    "result": "PASS",
    "test_count": 5
  },
  "restart_receipt_replay": {
    "output": "\nrunning 1 test\ntest durable_restart_replays_exact_delivery_and_read_all_receipt ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.05s\n\n",
    "result": "PASS",
    "test_count": 1
  },
  "restart_persistence": "PASS",
  "restart_mode": "owned-pg-ctl-immediate-WAL-recovery",
  "cleanup": "PASS"
}
```

The implementation handoff is the BE33 branch rooted at original `a25b8559`,
refreshed from published master `59de1ac9` by merge `eb0afcc4`. The validated
Rust source is `08e76dbd`; later evidence-only commits do not alter that source.
Four module registrations were explicitly granted in `dd9776e`; the exact three
manifest/lock changes were separately granted and committed in `0b04fb25`.
Only the designated integrator publishes master. Remaining implementation work
requires the concrete Notification producer/child-transaction settlement and
transport slice from the composition owner, then the canonical authority and
public vertical-flow qualification. No goal is marked complete by these local
persistence results.


## Genuine Notification SDK increment

The composition owner explicitly transferred the genuine producer-kind slice,
then the six additional identity/policy/build boundaries, to this isolated
writer. Published master was refreshed without source conflict as merge
`729f7b390cc03492ecc76a3fb69bbfccb80c95e8` before these changes. Auth and Friend
migration identity 0018 was verified absent and created exclusively. Their
published 0017 files remain unchanged.

The additive wire identity is `NOTIFICATION = 4`; existing 0/1/2/3 values are
preserved. The platform identity fingerprint, Concert producer mapping and
Auth/Friend native-identity validators now recognize that exact producer name.
Each owner requires the matching authenticated Notification workload for Track
and an explicitly configured `AUTHORITY_NOTIFICATION_SETTLEMENT_ENDPOINT`.
Its absence does not create a positive resolver or guess a Concert endpoint:
Notification tracking remains unavailable. Existing resolver TLS, required
Concert configuration, root binding, expiry and recovery paths remain intact.

The owner RPC/registry allowlists admit Notification only to Check, Enroll and
Track. Acquire and Complete remain Concert-only. Settlement responses are still
restricted to authenticated Auth/Friend callers. Forward-only owner migration
0018 adds the Notification producer to the existing participant constraint;
no source mutation or authority identity/transition guards are replaced.
Notification's build now compiles the local lease contract alongside both
inherited contracts, preserving their generated messages.

Pure regressions cover published producer wire numbers, genuine producer
fingerprint separation, database/XID/effect binding and the child/root RPC
capability boundary. They are authored and rustfmt-clean, not yet compiled or
executed. A fresh named build offer is still required. The earlier 27-test
persistence qualification applies to source `08e76dbd`, not this unqualified SDK
increment. The SDK is committed separately from the owning witness/adapter.
New Notification same-transaction witness/recovery and the real Auth/Friend
adapter remain implementation work; typed child transport and closed recovery
intent require the separately requested exact shared ABI before editing it.


The SDK's **SQL-only** qualification at `37c6f1a8` applied all 18 actual Auth
migrations and all 18 actual Friend migrations in private PostgreSQL databases.
Both retained CONCERT/POST/PROFILE and accepted NOTIFICATION. Unknown producer,
identity rewrite, evidence delete and truncate attempts were refused by the
retained constraints/triggers. An owned immediate crash and WAL recovery
preserved exact participant-row hashes; guarded cleanup passed. This proves
schema forward compatibility and retention only, not Rust owner Track/RPC
admission. Source copies and report are at
`${BE33_SDK_37C6F1A8}`; report SHA-256 is `910d84606cec54b058861698464e423d2c5c4a23cb03fe96eee9049c15672685`.

The next explicitly transferred SDK increment adds the closed internal
`notification.concert.v1.ConcertInboxService` contract (Execute, Check, Complete)
and its dedicated generated modules. Its policy admits only Concert, requiring
real parent owner/root capabilities inside the implementation; it does not
relax inherited Gateway actor forwarding. The closed request has exactly four
operation/input alternatives and typed safe DTO responses. Completion capability
bytes are private inputs and are never an output field.

`NotificationInbox` is a separate remote child kind (key 5), with own actor as
subject, a nonnil canonical UUID and exactly 32 private bytes encoded as 64
lowercase hexadecimal characters. Zero, malformed, foreign-subject, arbitrary
GID and extra-field variants are rejected. The actual root scope validates the
same actor binding and original disclosure window. Optional composite recovery
is unavailable unless its real Notification settler is provided; missing reply,
missing GID or expiry never attests a terminal outcome. Forward-only Concert
migration 0045 adds this child kind while retaining all prior kinds and old
0028/0037/0042 bytes. No public route or service is activated by these changes.
Root retains the scope ledger/child recovery runtime and main/operation union;
its coherent kind/settler composition remains required before enabling use.
The producer SDK and protocol/child SDK are separate commits from the actual
Notification witness and owner adapter, which are still in progress.

## Notification-owned witness and typed adapters (source increment)

Status: `IN_PROGRESS`. The new Rust adapters have **not** been compiled or
executed. The earlier 27 native tests and five-migration CLI evidence remain
qualified only at their original source pins. No public route is enabled here.

The explicitly granted `sha2.workspace = true` adds only Notification's existing
lockfile package dependency edge; it changes no version or resolution. New
Notification migration 0006 adds its persistent producer instance, immutable
private child binding/capability hash, native reference, and actual transaction
witness. It preserves migrations 0001–0005 byte for byte. The producer instance
belongs to Notification's database, never Concert's root registry.

`concert/authority/owners.rs` authenticates as Notification and inspects the
existing parent Auth/Friend view capabilities. It exposes Check/Enroll/Track,
not Acquire or root Complete. It validates the exact parent binding and root
view, original monotonic deadline, owner revisions, canonical actor/session and
source actors. Source blocks/inactive actors hide items; these checks do not
supply event membership or target authorization. No positive target provider is
installed. Even an empty inbox/count remains unavailable without that provider.

`concert/authority/durable.rs` captures system identifier, database OID, full
XID, PID/backend start, Notification producer instance and effect digest on the
actual repeatable-read inbox connection. It durably records that identity on a
separate Notification connection before either real owner Track. Both actual
Track replies are required before the actual inbox transaction inserts its
witness. Recovery metadata is kept in a separate table so its updates do not
invalidate or weaken the inbox's repeatable-read snapshot. The witness trigger
checks the real current transaction identity and holds the immutable intent
header row until commit; sealing takes a conflicting lock. A deferred witness
deadline guard also covers read-only/count snapshots.

Native resolution checks the configured real database and actual full-XID
status. COMMITTED requires the exact committed same-transaction witness;
ABORTED requires an actual aborted full XID and no witness. Missing/mismatching
metadata, committed-without-witness, old/unknown XIDs and pending outcomes do
not authorize release. Owner settlement accepts only independently verified
Auth/Friend callers with the exact binding/identity/registration. If cancellation
lost a Track reply before local acknowledgement, the authenticated actual owner
may resolve its own registration against the previously durable native
identity; a stored acknowledgement, when present, must match that owner's ID.
Notification never attests Root's `sealed_and_finished` response scope.

Completion before Execute creates a durable sealed tombstone using the exact
private child ID/capability and original binding. A delayed Execute cannot
reopen it. A sealed intent with no native assignment and no witness proves the
child never started, because sealing serializes against both assignment and
witness insertion. This is distinct from guessing an abort from missing native
metadata. Missing unsealed intent alone is never terminal proof. Other
completion outcomes use the actual native status/witness described above.

`concert/authority/inbox.rs` implements the dedicated typed Execute/Check/Complete
server for the four frozen inbox operations. It validates operation/input pairing,
canonical IDs, path/method/operation binding and private child capability; typed
request hashes bind the immutable child input. Safe output fields have no arbitrary
payload, source text or private capability. The operation-specific authority
retains the actual target fence and parent owner scope across the local commit.
Errors and success carry `no-store`. A timeout/lost Execute reply does not
manufacture a successful inbox result or native outcome.

Concert's `notification_live/client.rs` uses its real Concert workload to the
configured Notification endpoint. It persists the private root child intent and
attaches its live child before Execute; it retains the existing root response
gates, validates the typed reply's identity/cardinality/time/enums and clips the
root disclosure ceiling to the returned expiry. Restart recovery requires an
actual `NotificationRecoveryBinding` implementation loading the original
GuardBinding from Root's durable ledger. The child query cannot reconstruct
HTTP method/target/body digest or operation. The provider is optional only to
keep absence closed; no default supplies a binding. Complete recovery may send
only that exact binding (without expired owner/root view secrets); it proves
settlement and never renews disclosure authority. Root retains all ledger,
kind-dispatch, runtime/main and public operation composition ownership.

The adapter design references the existing Profile owner's headless authority
implementation at this branch's base `d3399ff0`:
`backend/services/profile-service/src/concert_self_profile/{guard,identity,settlement,read_reference}.rs`.
The new destination is `backend/services/notification-service/src/concert/authority/`.
No Profile source is changed; no renderer or visual dependency is reused. Native
witnesses and close-before-dispatch recovery are newly authored Notification
behavior, not Profile prepared-transaction or Concert identity aliases.

### Dedicated target callback ABI proposal (not a new shared contract)

Root explicitly retains the real provider and its canonical current-policy
composition. No qualified positive target provider exists yet. This proposal
records the requirements for Root's future exact shared-file delegation; it
does not add a service/method/policy or reuse Post's CheckSubject.

| Field | Closed requirement |
|---|---|
| Request parent | Exact private `ParentScope.root_view` (`RootReadView.scope`, view secret and GuardBinding), with the same parent GuardBinding/digest, actor, event, family, request, operation, original HTTP binding and expiry as Execute. No new root, lease, deadline or completion secret. |
| Candidate sequence | Entire ordered inbox snapshot, each `{notification_id, source_actor_id?, target?}`. IDs are nonnil canonical UUIDs; notification IDs are unique, repeated source/target IDs are allowed. No arbitrary resource metadata, summaries, locations, handles or actor-controlled grants. |
| Target | Absent or exactly `{target_type, target_id}`. Enum values are the frozen Notification contract: PARTY, APPLICATION, GIVEAWAY, RESERVATION, SOCIAL_REQUEST, POST, NOTICE, REPORT; unknown/zero values and half-present targets are rejected. |
| Response binding | Same root instance/scope and parent binding digest, exact sequence cardinality and candidate identities/order, original-or-shorter disclosure expiry, exact current policy revision evidence. No omitted, extra, reordered or duplicate notification decision. |
| Decision | Exactly VISIBLE, NEUTRAL (domain `Stale`, target omitted), or HIDDEN (item/count omitted). A missing target yields a policy-reviewed neutral decision; lookup failure/unavailable policy is not a positive or neutral fallback. |
| Required real owners | Root retains actual Auth/Friend Check/Enroll/current session/source block exclusions plus actual event membership/moderation and resource-owner target policy/exclusions. Virtual coordinates never prove physical arrival. Notification also independently checks its source actors. |
| Lifetime | Resolve the actual root capability, retain its in-flight permit and all required local/remote target guard handles as children of that same root. Exclusions survive callback return, local inbox commit, root serialization and before-byte checks until actual response completion or qualified native recovery. A timestamp/revision reply alone is insufficient. |
| Limits | No truncation or page-only count. Batch only under the same retained root and original deadline, aggregate every candidate once, and hold earlier batch exclusions. Transport/subject limits fail unavailable; never return a partial authorized unread count. The current canonical owner limit is 100 additional distinct subjects; larger whole-inbox snapshots need a real compatible protocol or stay unavailable. |
| Failure | Wrong capability/binding/actor, expired disclosure, missing owner/provider, unknown target enum/status, altered sequence, concurrent invalidation or unresolved completion all fail closed. No new Gateway forwarding allowance. |

The existing process-local `RootTargetSnapshotOwner` port accepts the exact
ParentScope/context/candidate slice and returns an exactly bound Authorization
with a retained AuthorityFence. A future remote implementation must additionally
retain exclusions at Root after its RPC reply. No implementation is supplied
by this increment. Root must also supply `NotificationRecoveryBinding`, register
NOTIFICATION_INBOX in its actual durable ledger/recovery union, and mount the
real child settler before enabling these public operations.

### SQL-only witness qualification

Command (exit 0):

```sh
python3 backend/services/notification-service/tests/concert_inbox_support/authority_witness_probe.py --artifact-dir ${BE33_WITNESS_0006} --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin
```

The committed probe creates a new marker/inode-guarded PostgreSQL 16 socket-only
cluster, copies the six exact migration sources into the external artifact,
and cleans up only its own postmaster/data. No Cargo target is touched. Its
registration IDs and digest bytes are explicitly SQL fixtures, **not real owner
Track receipts**. The probe validates schema/native transaction behavior only.

Artifact report: `${BE33_WITNESS_0006}/report.json`,
SHA-256 `b1449d6ca0a79b8f9ff039a5132b14b1747a4a991fdd5c5e64093aeb5a38a924`.
Probe source SHA-256 `9b01da911150ee4fff3b79a40e63d79a12ebd0467868975f7dafe9e871c452ed`.
Migration 0006 SHA-256 `9edbc2ccb49513e2b35684ea1b6d7ad8cfb916c204e989470cd5b46184227be0`.

All 13 recorded checks passed: six transactional migrations; immutable native
metadata; same-transaction witness under repeatable read after separate durable
registration; uncommitted witness invisibility; sealing waits for actual witness
transaction; committed native XID plus witness; seal/evidence immutability;
foreign native XID rejection; actual aborted XID; sealed intent rejects late
native assignment; new postmaster after immediate crash; exact witness after
WAL recovery; committed XID after recovery. Cleanup passed. PostgreSQL was
16.14 (Homebrew). The earlier local prototype report is superseded by this
portable committed probe; both used identical migration hashes.

Additional checks (exit 0): targeted Python harness safety tests (3/3), standalone
rustfmt for all newly owned Rust sources, `git diff --check`, and the planning
check. These are not Rust compilation, owner RPC, public HTTP or release proof.
A fresh named SL lease and explicit ACCEPT are still required before either
shared Cargo target can be touched. New Rust/source and six-migration production
CLI qualification, real owner Track/settlement, root target fencing and the four
public vertical flows remain `NOT_RUN`.

The authority reference/resolution store uses a distinct bounded two-connection
pool cloned from the actual inbox pool's configured database/role/TLS options.
It never accepts a peer URL. This avoids protected inbox transactions exhausting
the same pool needed for their mandatory durable metadata registration. All
metadata operations still share the original bounded authority deadline.

Standalone contract syntax validation also passed (exit 0): `libprotoc 33.4`
compiled local `authority/v1/lease.proto` and
`notification/v1/concert_inbox.proto` with imports into the external artifact's
`notification-contract.pb`. This validates protobuf syntax, not generated Rust
or runtime composition. An explicit parsed TOML comparison against `d3399ff0`
confirmed the lockfile changes only Notification's sha2 dependency edge, with
identical package set/resolutions and all peer package records unchanged.

## Qualified five-artifact checkpoint — 9bb4a98b

The final clean implementation source is
`9bb4a98b85ad9f9db91625f56495806f9ea27019`. This checkpoint is a compiled
implementation handoff, not acceptance of BE33's canonical/public flows. The
persistent objective remains unfinished. Documentation-only successors preserve
this exact backend source/dependency pin.

A fresh named lease `NOTIF-CANONICAL-BACKEND-20261104T0446Z` authorized only the
five requested artifacts in the existing BACKEND target. The owner explicitly
accepted after clean-pin, free-space and zero-handle checks. All builds used
Rust 1.88.0, `--locked --offline`, jobs 2, dev/test debug 0 and incremental 0.
Every exact final command exited 0:

```sh
cargo +1.88.0 test --locked --offline -p platform --lib --no-run --message-format=json
cargo +1.88.0 test --locked --offline -p auth-service --test notification_authority --no-run --message-format=json
cargo +1.88.0 test --locked --offline -p friend-service --test notification_authority --no-run --message-format=json
cargo +1.88.0 test --locked --offline -p concert-service --test notification --no-run --message-format=json
cargo +1.88.0 build --locked --offline -p notification-service --bin notification-service --message-format=json
```

The initial Auth test compile caught a call to a private platform validation
method. Commit `6fb19a9975567e0e88d62a5cfd8f719ac1ba2bd5` removes only that call
from the two owned owner tests, retaining the actual public method-permission
assertions. No platform API was exposed. Concrete stale-cache failures were
separately diagnosed: contracts-build retained another worktree's absolute proto
include path, and fresh=true Concert domain/application metadata lacked modules
present in the current source. Only the evidenced `contracts-build`,
`concert-domain` and `concert-application` package caches were cleaned. Removed
cache bytes were subtracted from the accounting baseline so repair did not hide
new output growth. Initial stderr/JSON/source archives and repair records remain
frozen; no peer process, serving binary or proof was modified.

The new remote child also exposed two non-exhaustive matches. Root explicitly
stopped the prior writer and transferred only those two arms. Separate commit
`9bb4a98b` adds exactly 10 lines in:

- `backend/services/concert-service/src/adapters/out/public_profile/mod.rs::recover_one`:
  explicit unsupported NotificationInbox `FailedPrecondition`, preserving
  public/Gift/Social behavior.
- `backend/services/concert-service/src/product_rpc/scope_ledger/children.rs::registration_binding`:
  remote Notification own-subject check, preserving original common binding,
  intent and capability checks; no fabricated Concert native identity.

Transfer base was `6fb19a99`. Before-edit SHA-256 values were respectively
`d99e638f7c86b6960799d6378cf157e7170cf64540338ce8080ca201acfe4a01` and
`583aec6a8296d9eaa8ac6be3a93bfa4b682a6a6bc3e2b636625ea8607066f3ed`.
Both slices were returned immediately after the separate commit; this owner
stopped editing them. Root still owns all other ledger/provider/composition
changes and integrates serially without a main-chat/SL review gate.

### Frozen executables and release

All five requested executables, exact Cargo JSON, failed-attempt diagnostics,
source archive and build/transfer/release records are outside the build target at
`$BE33_FROZEN_ROOT`. The directory name identifies the
original offered pin; the final manifest and source archive identify `9bb4a98b`.

| Frozen executable | Bytes | SHA-256 |
|---|---:|---|
| platform-lib | 24730800 | `f22b1397f01b84bb0e9d38a9e446348db38e9ed74c760be26800bc4628f97b3f` |
| auth-notification | 1225856 | `fc73f94b4f2e9cd127f5f0cb9077dd59eb27e03fdcfef9df93bfeeaee0bf6931` |
| friend-notification | 1226672 | `62cd5e9b0ca14f7d8b84a69fec9e9af57333f46f2a0e7e0e373730bec2f345ac` |
| concert-notification | 12943760 | `94de765c81a23ff5f8bf7d6499982b16d0c3e9101bfe0c69984415f8219eaf86` |
| notification-service | 31537168 | `9d4619eb9565bb0e035f95a0207150fd41c452e5e8601347befdca18088b790f` |

Unconditional BACKEND release preceded every copied executable test and owned
PostgreSQL/CLI run. Completed recursive lsof returned exit 1/no rows (zero target
handles); all owner Cargo sessions had completed. Release occurred 329.46 seconds
after the artifact-root creation. The compile/frozen accounting was 569311232
bytes; observed additional runtime peak was 91340800 bytes, for 660652032 bytes
combined (below the hard 1 GiB cap). No target access followed release. The
separate MEDIA writer, Root live stack and browser workflow were untouched.

### Frozen-only qualification

After release, the five frozen artifacts were hash-verified against their build
manifest. A frozen-only orchestration script `qualify.py` ran with no Cargo or
build-target access. All actual executable test runs used `--test-threads=1`:

| Qualification | Result |
|---|---|
| Full platform library suite, including identity/policy regressions | 204 passed, 0 failed/ignored |
| Actual Auth Notification SDK test artifact | 3 passed, 0 failed/ignored |
| Actual Friend Notification SDK test artifact | 3 passed, 0 failed/ignored |
| Current Concert Notification test artifact in owned PostgreSQL | 5 passed, 0 failed/ignored |
| New production Notification CLI empty-ledger verification | Refused, exit 1 as expected |
| Production CLI apply/reapply/verify | Six exact migrations/ledger checksums, exit 0 |
| 0005 collision | Refused without partial DDL/false ledger, then forward recovery to six, verified |
| 0006 collision | Refused without partial producer tables/false ledger; existing five retained, then forward recovery to six, verified |
| 0006 checksum drift | Refused, exit 1 as expected; original checksum restored |
| Immediate owned crash/WAL recovery | New postmaster, exact six-ledger row hash retained, production verify exit 0 |
| Current committed SQL/native witness probe | All 13 previously described checks pass, including its own WAL recovery |
| Owned PostgreSQL cleanup | PASS for both serial owned clusters |

The actual PostgreSQL version was 16.14 (Homebrew), with socket-only new private
clusters guarded by marker/token/inode and owned postmaster checks. CLI output
was checked for generated credential/URL disclosure. The current Concert tests
exercise durable delivery/lost-ack/lease fencing and strict unavailable routing;
they do not exercise the actual canonical owner RPC or target policy. The SQL
witness probe still uses explicitly synthetic registration IDs/digests, not real
Track acknowledgements. The old Notification 22-test artifact was **not rerun or
relabeled** at this new source pin. Its earlier qualification remains historical.

Portable evidence fingerprints:

| Artifact | SHA-256 |
|---|---|
| backend-source.tar.gz (all tracked backend at 9bb4a98b) | `b41e3ecb4ea91ff754513126b71781b3219b6432323b0e67d3f70dc9d86d4d99` |
| build-report.json | `7b5f217847a3210ab8990b3b3a3cd7d4bc7418694d882c00715854d7411d7939` |
| qualification-report.json | `56e86cff34f0ddc32118ed24d8ffb107b6c395e28db19d0994f3c075fc6342f8` |
| release.json | `222eb23170032b5ba7dc3726bf77ea31e3d607bd55c2ecfab960b73a47e664ac` |
| two-match-transfer.json | `045e492b34fd6bc7bf1703518ab1fef9349794507c7edf9ad04b1e2b5c10d92b` |
| qualify.py (exact frozen-only orchestration) | `bd6d75fbe4f482cce15542fff7108a9939fb518adf388535d05929537a3bab38` |

Remaining: Root's actual durable original-binding provider, real same-root
target snapshot/exclusion retention, coherent runtime/service/settler/public
operation composition, genuine owner Track/settlement/RPC execution, and all
four authenticated public inbox vertical flows. Missing real providers remain
unavailable. No public product activation, master push, goal reset or acceptance
claim is made by this checkpoint.

## Root intake hygiene

Root preserves the original qualified worker document as Git blob `0ca013839824bc6d55721c36945b1c8a08112fb9`, SHA256 `72dd944f3d88a03994ea467958e1e9f5530436edb106029c1a0649a805661beb`. Published prose substitutes portable checkout/artifact placeholders only; source and runtime evidence remain bound to the original worker pins. Coherent Root compilation and actual public producers remain NOT_RUN.


## NB01 — durable original root binding recovery

Status: `IN_PROGRESS` for BE33 overall. NB01 implementation and native lookup
qualification are complete; Root integration and canonical/public acceptance
remain `NOT_RUN`.

Source base: Root `73e89bceadc4b456522d0d979ccc768eea8e367e`, isolated branch
`codex/mcr-be-NB01`. Original BE33 handoff and worktree remain preserved.
This assignment owns only the new `scope_ledger/notification_binding.rs`, its
single `mod notification_binding;` registration in `scope_ledger.rs`, the new
`tests/notification_root_binding.rs` and `tests/notification_root_binding_support/`
files, and this appended section. Parent module bytes otherwise remain unchanged;
stopped lifecycle/guard/settler slices and CR08 implementation are untouched.

`PgScopeLedger` now implements `NotificationRecoveryBinding` directly. It loads
the stored `OwnerBinding` by its configured producer instance and exact query
fingerprint. `ChildQuery` carries no scope ID, so the provider fetches at most two
matching rows across **all phases** and requires exactly one before requiring
`ROOT_FINISHED`. Missing, foreign-instance, ambiguous, nonterminal, malformed or
mismatched rows return `Unavailable`. No new contract or migration is required;
ambiguity deliberately remains unavailable rather than choosing a scope.

The original actor/event/family/request, own-subject, expiry, complete fingerprint,
parent method and four Notification operation/method/path pairs must match. The
fingerprint validates all original parent/body/HTTP/operation fields. The exact
stored query bytes and digests are returned without reconstruction from the
child query. As in the original `OwnerGuardScope::wire_binding()` at this source
revision, wire subjects are empty. Recovery accepts an expired original binding
without renewing its expiry. A 750 ms mechanical lookup limit, shortened by the
caller deadline, bounds pool acquisition and SQL; it grants no content authority.

This provider neither dispatches owner RPCs nor settles a child/native transaction.
Existing encrypted durable intent, original child completion secret and actual
remote owner/native proof remain mandatory in their existing layers. Root runtime
composition and actual canonical-owner/public inbox journeys remain `NOT_RUN`.

Provenance: the implementation uses this base's `OwnerBinding::digest()`, original
`OwnerGuardScope::wire_binding()`, `NotificationRecoveryBinding` contract, and the
four Notification operation pairs in Notification's `concert/authority/inbox.rs`.
Owned fixture checks adapt `product_scope_ledger.rs` and
`owner_authority_support/environment.rs` at the same base. The runner reuses the
existing marker/token/inode/postmaster-guarded `backend/scripts/mcr/migration_smoke.py`
cluster and applies original migrations 0028 and 0048. No dependency, manifest,
lockfile, migration, source-pack or legacy-rendering changes are introduced.

Seven native tests cover all four exact expired wire bindings after a new ledger
and empty actual registry; actual register/seal/finish transitions; missing and
foreign instances; ambiguity with ACTIVE/SEALED/ROOT_FINISHED duplicates; every
query identity/expiry/digest/own-subject mismatch; malformed and altered original
fields; valid-fingerprint operation/path relabeling; and bounded pool-starvation,
caller-deadline and locked-table refusal. Synthetic historical rows are explicitly
lookup fixtures, not canonical owner admission or positive Track acknowledgements.

Before native qualification: targeted Rust 1.88 rustfmt and Python runner AST
parse exited 0; the unchanged-parent byte assertion passed;
`python3 scripts/mcr/check-plan.py` exited 0.


### NB01 qualified committed handoff

Compiled/native source pin: `41e81a3702c389fc30a9d0dc2c8a936d2f766382`.
Handoff ancestry is Root `73e89bce` → implementation `dc01de3b` → evidence-runner
hardening `52d6373f` → actor-mismatch regression improvement `41e81a37` → the
following documentation-only qualification commit. All tracked backend bytes
remain identical to the compiled pin. No manifest, lockfile or public contract
changed. Notification contract `backend/proto/notification/v1/concert_inbox.proto`
remains SHA-256 `a77385e06a567a59d91e93d8fe2e7acda9e1613705c5bfbf8d1dc4cffb554e76`.

Changed paths, relative to Root base:

- `backend/services/concert-service/src/product_rpc/scope_ledger/notification_binding.rs`
- `backend/services/concert-service/src/product_rpc/scope_ledger.rs` (one module declaration)
- `backend/services/concert-service/tests/notification_root_binding.rs`
- `backend/services/concert-service/tests/notification_root_binding_support/mod.rs`
- `backend/services/concert-service/tests/notification_root_binding_support/run.py`
- `docs/mcr/backend/notifications.md` (this appended NB01 section)

A fresh explicitly accepted `NB01-BACKEND-202611040523Z` allocation provided one
existing BACKEND target and one serial copied-artifact owned PostgreSQL slot.
Fresh preflight had 36492264 KiB free and zero target handles. All Cargo work used
Rust 1.88.0, `--locked --offline`, jobs 2, dev/test debug 0 and incremental 0.
The same single test artifact was requested on both attempts:

```sh
cargo +1.88.0 test --locked --offline -p concert-service --test notification_root_binding --no-run --message-format=json
```

| Check/command | Exit/result |
|---|---|
| First compile | 101; retained stale-cache evidence, not a source regression |
| `cargo +1.88.0 clean -p contracts-build -p concert-domain -p concert-application` | 0; exactly three evidenced stale packages |
| Same single-artifact compile retry | 0; exact Cargo-emitted test frozen and hashed |
| Original migration 0028, then original migration 0048 | 0 each in a new socket-only PostgreSQL 16.14 cluster |
| Frozen `notification_root_binding --test-threads=1` | 0; **7 passed, 0 failed, 0 ignored**, 0.66 s |
| Committed owned-fixture runner | 0; PASS, 2.19 s, owned stop and deletion PASS |
| Final targeted rustfmt check, runner AST, unchanged parent/doc-prefix and complete backend pin assertions | 0/PASS |

The initial failure reported existing domain/application modules missing while
Cargo marked both libraries fresh, plus protoc shadowing this checkout with the
previous writer's checkout through a fresh `contracts-build` output. First Cargo
JSON/stderr/receipt are retained. The lease explicitly allowed evidenced
package-only repair; no broad clean, second target or source workaround occurred.

The exact test binary was frozen before unconditional target release. Incidental
`concert-service` main executable metadata is recorded but it was not executed.
After own Cargo process-group exit, the target handle scan returned no handles;
there was no target access after release. Native tests used only the frozen test
binary and strict marker/token/inode/system-identifier-owned PostgreSQL fixture.
The original encrypted intent and real completion capability/proof requirements
were neither forged nor bypassed. The actual registry constructed in the test is
empty, and the lookup itself never calls the rejecting child settler.

The conservative combined bound over both compile attempts, frozen evidence and
owned PostgreSQL was 199225344 bytes, below the hard 1 GiB limit. Whole compile,
freeze, native qualification and release completed in 190.68 s, below ten minutes;
final free space was 37210398720 bytes, above the 8 GiB floor. All owned resources
were released. Portable evidence under `<NB01_ARTIFACT_DIR>`:

| Evidence | SHA-256 |
|---|---|
| `notification_root_binding` (13757952 bytes) | `e33dcd37f50e6a69fe1f2a7e55be2ae5ffc8083a4efd893e9643e8b16f383a95` |
| `backend-source.tar.gz` | `655bc9629fa7f0fa5f8337846b9084c4cdb1107579a6df8c20c09e5af2cdd9b6` |
| `artifact-receipt.json` | `f020028d2968bf92ddcb7fc828010bcb54c1ca9f622f30d112cce6489606333e` |
| `qualification-report.json` | `a01d0ae4e126a0439f7db03f253be39f2b223b08e3c9a73abb0df6f8f97a784d` |
| `target-release.json` | `25cfd5ad466951e1346b95f1729642509d4acdc4ba1d23f441b7fff738496ff2` |
| `copied-release.json` | `091f92eb6b78eba3bf7f947ae5458f332f703505404c0dac08e7640634112cb1` |
| `first-artifact-receipt.json` | `ba565430375234f1a8f8716d10931ded13aa258f3563205af313b9a6255ce7f1` |
| `build.py` (bounded exact-artifact wrapper) | `7727b96ee9c5c6001d3fdcc97e3a9915f79e95ef8848f884dd94c9a38f1714a9` |

Reproduction uses the committed runner with the frozen binary/receipt, PG16 tool
directory and a report destination. It verifies the emitted test identity, complete
source manifest and artifact digest before creating its own database:

```sh
python3 backend/services/concert-service/tests/notification_root_binding_support/run.py --artifact <NB01_ARTIFACT_DIR>/notification_root_binding --receipt <NB01_ARTIFACT_DIR>/artifact-receipt.json --pg-bin <PG16_BIN> --report <NB01_ARTIFACT_DIR>/qualification-report.json
```

These results qualify the NB01 durable original-binding lookup mechanics only.
Root still owns runtime/client/settler/public operation composition and the actual
canonical owner/transaction authority and authenticated four-operation vertical
journeys. BE33 overall and its original persistent goal remain unfinished; this
handoff makes no public activation, product acceptance or master-push claim.


## NB02 — acyclic crate-private binding reader

Status: `IN_PROGRESS`; source refactor complete, fresh native validation `NOT_RUN`
pending a named backend-SL allocation and explicit acceptance. Original NB01
receipts and its documentation above remain immutable historical qualification.

Base: qualified NB01 `164ac70ffd6c996cca3b93737ee6f2d9677a8988`, same isolated
`codex/mcr-be-NB01` worktree, exclusive NB02 ownership. Root confirmed the actual
assembly comparison commit is `d0f063d18a0549935f80a536077c30962d89f9e9` (2,151
recorded input hashes); its `config/canonical_runtime.rs` constructor takes the
actual child settler before constructing the ledger and registry. A Notification
client whose recovery provider required that ledger created a constructor cycle.

The new crate-private `PgNotificationBindingReader` holds exactly the configured
`PgPool` clone and immutable instance UUID. Its constructor rejects nil instances
without database I/O; it holds no ledger, child keys, settler, registry, callback
or owner authority. Root can build this reader, then the actual Notification
client and composite settler, then its one canonical core using the same pool
and instance. `PgPool::clone` shares the existing pool. The narrow parent change
is only `pub(crate) use notification_binding::PgNotificationBindingReader;`.
Root's factory remains within the service library; visibility is not widened
for binaries or integration tests. Root core/factory/settler wiring is outside
this patch and remains separately unqualified here.

The original `PgScopeLedger` trait implementation now delegates to the reader.
Only the reader contains the lookup algorithm. Its complete NB01 lookup body is
byte-for-byte retained, SHA-256
`8d92c206064d2ca0842c04ce661356dd1b2408f7b70c42dc8fac863bfef6841c`.
The reader's private transaction bootstrap preserves pool acquisition and the
250 ms lock/750 ms statement timeouts within the existing caller-shortened
750 ms whole lookup limit. All-phase ambiguity refusal, unique ROOT_FINISHED,
exact stored full-binding validation, own-subject, original expired wire fields,
closed operations and unavailable refusals remain unchanged. No registration,
expiry renewal, transaction/remote proof or content authority is introduced.

All seven original ledger-facing native test bodies remain byte-identical;
they now exercise the sole reader implementation through delegation. The strict
owned fixture is shared using a parent crate alias so library tests can exercise
the actual private type without exporting it. Three additional private tests
cover: nil-instance refusal and sharing the existing pool; exact reader/ledger
wire and unavailable-result equivalence; and constructing/dropping the actual
lazy Notification client with a typed recovery reader before any ledger or
settler, checking that client ownership releases the reader. That transport test
constructs no server and invokes no RPC or authority proof. Synthetic historical
rows retain their original mechanics-only meaning and rejecting settler guard.

The committed runner now requires two separately identified, hash-bound Cargo
artifacts at the same complete source pin: the seven-test integration executable
and the library test executable filtered to the three private NB02 tests. It runs
them serially in one new socket-only owned PostgreSQL16 fixture using unchanged
migrations 0028/0048 and original strict ownership checks, then verifies owned
cleanup. Running only the old seven-test artifact cannot qualify NB02.

Changed paths are the same six assigned paths as NB01: the binding module, narrow
parent re-export, original integration test crate alias, strict fixture module,
owned runner and this appended document section. No public API/proto/DTO, caller
policy, main, settler, registry, CR08, manifest, lockfile or cross-lane path changes
are included. Fresh build/native receipts will be recorded separately below;
NB01's `f020...`/`a01d...` receipts do not certify the refactor.

The aligned persistent BE33 goal was inspected and execution explicitly resumed
under this follow-up grant. The goal API still reports its historical `BLOCKED`
state; it was not reset, budgeted or falsely completed. BE33 and the four real
public verticals remain unfinished; NB02 only unblocks Root's typed factory.
