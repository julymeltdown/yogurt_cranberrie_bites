# Relationship policy and private social sharing

Status: BE37 IN_PROGRESS; BE28–BE30 IN_PROGRESS. Real owner composition and
HTTP/WS integration NOT_RUN. No task acceptance or release claim.

## Source and scope

Base: `f0e44bd1bba943b02319f9d9a42433e253e8de6f`, branch
`codex/mcr-be-social-policy`. New MCR code; no copied Bigyeting implementation,
rendering, or source archive changes. Uses existing canonical Auth/Event IDs,
Chrono/UUID/async-trait/SQLx and the BE15 receipt/outbox producer seam.
C04 is the bounded v0.1 candidate at the base, not a full freeze:

| Input | SHA256 |
| --- | --- |
| common.schema.json | 303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35 |
| social.schema.json | b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2 |
| operations.json | 42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223 |
| policy.json | 508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe |

## Shared typed interface

`concert_domain::relationship` owns `PolicyQuery {event, actor, subject, field}`,
`PolicyStamp {event_revision, relationship_revision, authority_revision}`,
`FreshAuthority`, `PolicySnapshot`, and the pure `decide` matrix.
`concert_application::relationship::RelationshipPolicy::authorize(query)` returns
`PolicyDecision` or coarse `RelationshipError::{Denied,Unavailable,Stale}`.

`PublicProfile` and `Presence` permit authorized public projections in READ_ONLY.
`Interaction` requires OPEN/LIVE. `SafetySelf` supports own safety operations in
READ_ONLY and requires actor=subject; producers independently check ownership and
command lifecycle. Self-pairs need current membership and real actor authority.
`PrivateHandle` always denies at this generic seam; only the dedicated reveal
transaction may reveal a platform/version-bound secret.

A decision is a point-in-time projection, not reusable permission for writes.
Consumers re-evaluate after invalidation and use `ensure_current` only with freshly
resolved revisions/authority. Check exact event/actor/subject/field. Never retain
an allow after policy failure. Neither a Profile row nor participant existence
establishes an authenticated actor, current suspension state, or absence of blocks.

## Required real authority binding

`FencedRelationshipAuthority::check_in(PgConnection, PolicyQuery, DB_now)` must
verify a request-bound authenticated actor, current owning Auth session/account
restrictions, subject restrictions, and inherited Friend blocks in both directions.
It returns the exact query, checked_at=DB_now, positive authority revision, bounded
valid_until, actor_allowed, subject_allowed and inherited_block. Unknown/outage
returns Unavailable. The owner revision must fence block/suspension/session
revocation through the protected transaction, or implement an independently
qualified equivalent revision protocol. An ordinary remote read without a fence
cannot supply mutation authority. Current imported Friend DTO has block booleans
but no such owner revision/fence. An adapter writer must add/qualify this owning
service capability; no permissive production implementation is supplied here.

`UnavailableAuthority` and `UnavailableRelationshipPolicy` are explicit production
defaults. Deterministic injected authority is test-only. Do not expose these
internal assertions as request-body fields or infer them from caller roles.

## Validation record

- `python3 scripts/mcr/check-plan.py`: exit 0, planning checks PASS; runtime NOT_RUN.
- Pinned Rust 1.88 rustfmt on owned Rust files: exit 0.
- Cargo and owned PostgreSQL checks: NOT_RUN at initial API checkpoint; queued
  behind existing shared-target lease. First checkpoint publishes the typed seam
  so community/admission/feed consumers can compile against its stable shape.

Deliver only assigned relationship directories/tests/scripts, three exact module
exports, reserved migration 0006 and this document. Integration, real authority
wiring, transports and remote master publication belong to delivery.

## Persisted implementation

`concert-service/src/relationship` provides `PgRelationshipPolicy::authorize_in`,
`FencedRelationshipAuthority`, a BE15 `BlockCommand`, managed-key crypto,
`SocialService`, and the dedicated reveal response adapter. `SafetyInteraction`
permits READ_ONLY cross-user privacy-restricting commands after the producer
checks its own resource ownership; ordinary Interaction still requires OPEN/LIVE.
The generic safety field retains block and restriction checks. Block/revoke/remove
producers additionally permit actor-owned safety actions against a now restricted
counterpart without restoring any authority to that counterpart.

Migration `0006_relationship_policy.sql` is exclusively owned by this lane. It
adds normalized directional event-local block pairs, versioned encrypted handles,
requests, unique platform consent bindings and reciprocal grants. SQL guards check
initial revisions, immutable scope, monotone tombstones, state transitions and
grant-to-request/version bindings. An inherited Friend block remains a separate
owning-service decision. Local unblock advances the pair revision and never
reactivates requests/grants. A genuinely fenced owner revision is also captured
at request creation so an inherited block/unblock cannot revive old consent.

Normal policy lock order is event/share, normalized pair/update, sorted
participants/share, then fenced authority. Social mutations and block mutations
first take event/update, then their pair/request/handle locks. This intentionally
serializes event social writes and closure for correctness. It is not measured
capacity or a claim that this coarse lock strategy meets the load target. Reveal
holds event/pair/request/grant/handle locks through secret materialization and
commit. External owners must independently fence their authority; Concert locks
cannot protect another service's database.

`SocialService` builds its own canonical BE15 request from typed command input;
callers cannot provide an unrelated hash/body. Creation derives a stable request
UUID from event, actor and idempotency key so uncertain identical retries refer to
one target. Explicit requester consent, selected unique platforms, current handle
versions, current intro/claim eligibility and reciprocal recipient approval are
checked before grants. Every mutation and safe outbox reference commits in the
same BE15 transaction; failed outbox insertion rolls back effects and receipts.

Handles normalize to lower-case under platform-specific frozen rules. AES-256-GCM
uses ring 0.17.14, random 96-bit nonces and framed associated data containing event,
owner, platform, version and key ID. `ManagedHandleKey`/`HandleKeys` accept trusted
managed key material; no default key, environment reader or plaintext logging is
provided. Retained keys permit rotation; missing keys and tampering fail closed.
Key IDs must never be reused for different material. Key provisioning, rotation
operations and production secret configuration require composition qualification.
Replacement/removal revoke pending requests and active grants, retain monotonically
increasing versions, and erase ciphertext on removal. Ordinary receipts and owner
metadata contain no handles or cipher/key material. `PrivateHandle` and
`PrivateReveal` do not implement ordinary serialization and Debug redacts secrets.

Reveal rechecks exact actor/event/request/platform, both current memberships and
restrictions, current intro/claim eligibility, current pair and fenced owner
revisions, request approval/expiry, two reciprocal ACTIVE grants, exact owner and
peer versions, configured handles and current event state. It returns only the
counterpart handle. The response adapter explicitly allowlists the C04 reveal
fields and sets `Cache-Control: no-store` for success and errors; errors disclose
no counterpart/request/handle metadata. A private reveal is not a public profile.
Revoke cannot erase information another person previously saw or copied.

`SocialEligibility::check_in` is a separate transaction-fenced current intro and
claim port; caller assertions never establish a sharing offer or verified age.
`UnavailableSocialEligibility` is the default. The upstream authority worker is
implementing bounded owner observations with a revision vector; those observations
are explicitly NOT the fenced mutation/reveal contract and are not converted to
its scalar revision. No distributed guarantee is invented here.

## Verification and delivery limits

Native preliminary SQL runs used a fresh marker/inode-owned PG16 cluster, applied
exact migrations 0001/2/3 and owned 0006 transactionally, restarted a real
postmaster, verified a retained block and cleaned up the owned root. They passed
with repository Rust tests explicitly NOT_RUN; records are
`/tmp/mcr-relationship-sql-first.json`, `-sql-social.json` and `-sql-guards.json`.
The first record predates the appended private-social tables; later records pin
individual migration/source hashes. These are preliminary evidence, not the final
producer qualification or an SQLx runtime migration-ledger claim.

The native runner reuses the existing ownership-safe `migration_smoke.Cluster`
and `concert_core_smoke` helpers without editing them. The Rust owned-pool fixture
is adapted from `concert-service/tests/command_receipts.rs` at base f0e44bd into
`tests/support/relationship/mod.rs`, changing only the fixture database name and
visibility. This test-helper reuse is the only copied implementation in this slice.
The ring manifest/lock dependency edge is delivery-owned source commit 0c161d7,
cherry-picked as 5eb6598, with no new resolved package/version.

At the source checkpoint, Cargo and the real PG Rust producer suite were pending.
The final qualification below resolves those local validation items. Gateway route,
RPC/startup composition, real Auth/Friend fencing, real intro/claim authority,
real users/browser privacy flows, PostgreSQL17, mobile devices and load gates are
NOT_RUN. Full BE37/BE28–30 and IN03 acceptance remain IN_PROGRESS.

The final authored migration also installs deferred transaction-end guards on
pair/handle/request/grant writes. The adapter pins the earliest trusted authority
and sharing-eligibility deadline in a transaction-local setting; deferred guards
reject expiry after apply but before receipt/outbox commit. The setting itself is
not authentication and is set only after qualified port checks. Reveal performs
a final database-time check after decryption. Direct administrative writes require
separately reviewed authority; test fixture writes explicitly inject test deadlines.
The additional preliminary SQL/deadline migration and restart run passed with
cleanup at `/tmp/mcr-relationship-sql-deadline.json`; Rust tests remain pending.


## Qualified local handoff — 2026-11-04

Implementation source is `b71ae36`, descended from initial typed API `bcd2cd2`
and delivery-owned dependency edge `5eb6598` (original `0c161d7`). Final evidence
and harness correction follow in a separate commit. The application Rust sources
and migration were unchanged between source commit and qualified binary copies.
Shared-target lease was transferred community → this lane → meeting; no Cargo
was run during the independent PostgreSQL fixtures.

| Check | Result |
| --- | --- |
| Rust 1.88.0, jobs=2, dev/test debug=0, locked/offline focused compile | exit 0, 3.83s, no warnings |
| Pure domain policy/normalization binary | exit 0, 6 passed, 0 failed |
| Filtered service crypto unit binary | exit 0, 2 passed, 0 failed; 1 unrelated test filtered |
| Owned PG16.14 Rust producer binary | exit 0, all 20 named tests passed, none skipped |
| Runtime SQLx migrate, verify, reapply | exit 0; exact versions 1,2,3,6 and 4 checksum receipts |
| Empty ledger, deliberate 0006 DDL collision and checksum damage | expected exit 1 for each refusal |
| 0006 atomic failure and forward recovery | no partial pair table/false receipt; recovery and verification exit 0 |
| Real postmaster restart, persisted block, runtime verify and exact ledger | PASS; verification exit 0 |
| Owned fixture cleanup | PASS; private marker/inode-owned root removed |
| Owned rustfmt check, Python compile, diff whitespace and assigned-path guard | exit 0; 20 paths including 2 delivery-owned dependency files |

Build command from the assigned checkout:

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 /Users/lhs/.cargo/bin/rustup run 1.88.0 cargo test --manifest-path backend/Cargo.toml --locked --offline -p concert-domain -p concert-service --lib --test relationship_policy --no-run --message-format=json
```

Executables were copied before lease release to
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-relationship-native-20261104-pgqearm7/`.
Its `manifest.json` pins every executable. Domain test SHA256:
`6afe7deb8ea7fb115a39552a03703add6aca92aa4446397f0d79efa9f78a9c2d`.
Service library-test SHA256:
`703cb390e90cf8c10e9fc8568ace10baef5ed7b58f90fadea1257edce63a89a7`.
Real producer-test SHA256:
`66e363e3dd325732aafac78b8e104b1d16b3989e451947ee0379429afff619f5`.
Service runtime SHA256:
`b60f63fa11cee938aefb860acaae61b7d19d07616648287037899583af8c48e2`.

Final native command:

```sh
python3 backend/scripts/mcr/relationship_smoke.py --pg-bin /opt/homebrew/opt/postgresql@16/bin --repository-test /var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-relationship-native-20261104-pgqearm7/concert-service-relationship_policy --binary /var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-relationship-native-20261104-pgqearm7/concert-service-concert-service --report /tmp/mcr-relationship-runtime-final.json
```

The final report SHA256 is
`1d18e7370f5f57546f9d5905ad86848651b17a65f848f9a7bca40b4d285e3f6a`.
It records source hashes, the dirty harness-only extension, exact executable
hashes, migration SHA256/SHA384, every CLI exit, all named producer tests,
restart and cleanup. Migration 0006 SHA256:
`0905ddf463325c5097a3a732e03a237f99d5f53060c1df255eed126e468b2fd3`.
The earlier runtime-harness extension run had a local variable-name collision in
its final ledger assertion after all producer tests passed; the runner correction
uses distinct expected-test and expected-migration names. Its FAIL record remains
at `/tmp/mcr-relationship-runtime-b71ae36.json`; the complete corrected run above
passed. No application or migration change was needed.

The 20 real producer tests cover both block directions, independent reverse block,
idempotent commands, READ_ONLY safety, self-pairs, unavailable/unknown/suspended
and inherited restrictions, cross-event and missing membership denial, stale
stamps, transaction locking, reciprocal grants/reveal, wrong actors/platforms,
private response allowlisting/no-store, expiry, changed handles, tombstones,
block/unblock non-restoration, concurrent approval/block and grant/revoke races,
outbox rollback, key rotation/AAD/tampering, terminal-state protection, and trusted
authority expiry between apply and commit. All authority/intro fixtures are
explicitly test-only rows in the owned database; these are real PostgreSQL
persistence tests with injected policy, not real Auth/Friend/intro integrations.

Remaining delivery dependencies are real fenced owner and intro/claim authority,
Gateway/root/RPC composition, current-snapshot stream invalidation consumption and
real two-account IN03 checks. The adapter is fail-closed until those are bound.
The dedicated reveal response adapter is qualified in process; no public route
or live provider configuration is enabled. Full task acceptance, real users,
browsers/devices, PostgreSQL17, load and release remain NOT_RUN/IN_PROGRESS.
Only the delivery integrator publishes master; this lane does not push master.

## Actor-only consumer lock-order correction

Meeting/community consumers identified a concrete cycle: preflight took an
actor self-pair before the party root, while a command took that root before the
self-pair. The follow-up adds `PgRelationshipPolicy::authorize_actor_in(c,key)`
returning `ActorDecision`, with exact key, current event/owner revisions and proof
window but no relationship revision, target, field permission or mutation grant.
It takes only event/share, own participant/share and fresh fenced actor authority;
it neither reads, inserts nor locks a relationship pair. Consumers may preflight
actor eligibility, lock roots in deterministic order, then acquire sorted canonical
user pairs and run full target authorization. Pair checks must remain after roots.
READ_ONLY actor checks do not become Interaction permission. A blocked counterpart
does not remove actor-only safety eligibility or give that counterpart any access.

The required follow-up includes a real PostgreSQL schedule where a root writer
waits for self-pair authorization while an actor-preflight reader waits for the
root, plus a regression proving the actor-only guard leaves no pair row. It also
adds pure actor binding/eligibility checks. The prior 20-test evidence remains pinned to its unchanged a5e84da/b71ae36
source. The actor-only follow-up qualification below supersedes its local
verification counts without changing the published migration.


## Qualified actor-only correction — 2026-11-04

Source commits `f5e5d44` and `49ce4647c901143cf86c230d5c6a7576da0cbd1d`
add the actor-only check and deny inconsistent self-eligibility assertions.
`authorize_actor_in` binds the exact canonical key, checks current event/member
and both self-bound owner eligibility assertions, pins the earliest trusted
expiry deadline, and never acquires a relationship pair. The change preserves
published migration 0006 byte for byte (SHA256 remains
`0905ddf463325c5097a3a732e03a237f99d5f53060c1df255eed126e468b2fd3`).

The resource blocker cleared on an explicit Delivery → this lane transfer.
The same focused Rust 1.88 / locked / offline / jobs=2 / debug=0 build command
above exited 0 in 4.28 seconds with no warnings. Cargo JSON selected all binary
paths; copies and source hashes were pinned before immediate transfer to meeting.
The copied-source hash snapshot matched the clean checkout before documentation
changed. No Cargo ran during the following native fixtures.

| Follow-up check | Result |
| --- | --- |
| Domain matrix, normalization and actor-only binding/eligibility | exit 0; 7 passed, none skipped |
| Focused AEAD crypto unit tests | exit 0; 2 passed; 1 unrelated unit filtered |
| Real owned PostgreSQL producer suite | exit 0; all 22 named cases passed, none skipped |
| Actor-only no-pair regression | PASS; no pair read/insert/lock, unavailable owner fails closed |
| Root-writer versus actor-preflight-reader contention schedule | PASS; writer self-pair check completed while reader waited on root, then reader completed |
| Runtime migrate/verify/reapply, 0006 rollback and forward recovery, checksum refusal | PASS; expected refusal exits 1, success exits 0 |
| Actual postmaster restart, stable four-row ledger and persisted block | PASS |
| Marker/inode-owned fixture cleanup | PASS |
| Owned rustfmt, Python compilation and whitespace checks | exit 0 |

The complete report is `/tmp/mcr-relationship-actor-runtime-final.json`, SHA256
`e5734e61fd10d1b038b220ee789e648955219cf63e46500f2130dfac4c682155`.
It records clean source revision `49ce464`, all source/migration hashes,
versions 1/2/3/6, exact command exits, all 22 tests and cleanup. The private copy
manifest is
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-relationship-actor-native-20261104-43luvd5c/manifest.json`.

| Binary | SHA256 |
| --- | --- |
| Domain relationship tests | 01c77cb03b6144c4c0fd05ee2ea9b0dca15601334ab562f7c4491e535b46f9f4 |
| Service crypto unit tests | 4b39e937f455a92d897b7b20c4606092d84ccfa984183899d1841db3811bad46 |
| Real PostgreSQL producer tests | 5ee99147450ec938eca78ca859cbeda4a746a12a81c7917be80c9f9f98fd4046 |
| Migration runtime | 180f5a385dd9e916520777381b32b207cf663e199c811d6500717a277ceb9275 |

Reproduce the native checks with the committed runner, `--pg-bin
/opt/homebrew/opt/postgresql@16/bin`, `--repository-test` pointing to the copied
`concert-service-relationship_policy`, `--binary` pointing to the copied
`concert-service-concert-service`, and a new absolute `--report` path. The runner
requires all 22 named producer cases; an old 20-case binary cannot certify this
follow-up.

This completes the assigned relationship/private-social implementation,
meaningful local validation, documentation and known owned correction. Delivery
and consumers receive the committed handoff directly; no coordinator approval is
required. Community/meeting retain event → deterministic roots → canonical
sorted pairs ordering and must prove each target/audience after its root lock.
Actor-only success never replaces target authorization. The complementary
cross-domain beacon rollback fixture belongs to the meeting owner. Real owner
fencing, intro/claim authority, root/Gateway composition, realtime invalidation
consumption and real two-account IN03 remain their separately owned obligations;
this local handoff does not claim those tasks or production acceptance.
