# Community backend implementation

Status: **IN_PROGRESS**. Internal BE16–BE18 slice; public HTTP/RPC composition,
real Auth/Friend authority, BE09/BE19 physical flows and IN02 are **NOT_RUN**.
The persistent goal remains active through BE20–BE23 and concrete review fixes.

Assignment base: `f0e44bd1bba943b02319f9d9a42433e253e8de6f`.
Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-community`, branch
`codex/mcr-be-community`. Delivery owner is chat
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; this worker never pushes master.

## Contract and implementation boundary

Pin C04 candidate files `contracts/domain/v1/{party.schema.json,giveaway.schema.json,
operations.json,policy.json}` at assignment base. The nine discovery operations
have the separately documented `ca9f955` bounded consumer freeze. Whole C04 and
all endpoint/feature acceptance remain separate. Candidate transition decisions:
capacity includes owner; applications consume capacity only on approval;
CANCELLED/COMPLETED parties and decided applications are terminal. CLOSED retains
members. Party owner must cancel or transfer before departure. A membership
removal leaves its approved application as history.

All product implementation is newly authored. No legacy logic, rendering or art
is copied. Retained dependencies: chrono, serde/serde_json, UUID, SHA2,
async-trait, SQLx, Tokio and existing BE15 HMAC receipt/outbox infrastructure;
no dependency/lock changes. The test-only owned-cluster validator is adapted from
`backend/services/concert-service/tests/command_receipts.rs` at assignment base,
changing the allowed database to `mcr_community`; it retains marker, inode,
permissions, socket and actual data-directory checks. The Python harness imports
`backend/scripts/mcr/migration_smoke.py` at assignment base rather than copying
its lifecycle implementation. It creates a fresh no-TCP owned PostgreSQL cluster
and removes only its validated directory. `docs/source/` is untouched.

Party aggregate paths are `backend/crates/concert-domain/src/community/**`;
typed canonical command paths are `backend/crates/concert-application/src/community/**`;
SQL adapters/audience port are `backend/services/concert-service/src/community/**`.
Only exact `pub mod community;` lines change the three library exports.
Migration `backend/services/concert-service/migrations/0007_community.sql` is
exclusively owned here; 0006/0008/0009 remain other owners' files.

## Transaction behavior

Typed commands build their own BE15 canonical input from actor/event/operation,
path, expected revisions and complete body. DELETE revisions are query fields;
no DELETE JSON body. Timestamp fingerprints retain subsecond precision. Stable,
domain-separated server resource IDs derive from actor/event/operation/key, so a
lost create/apply response can be reconciled after process restart. Keys and
pickup secrets never appear in logs or public projections.

The caller's transaction locks event then party; BE37 and place adapters must
retain their fences through commit. BE15 locks its receipt budget, reauthorizes
stored targets on replay, persists only safe receipt references and emits a
metadata-only outbox record in the same transaction. Approval locks the party,
checks expected party/application revisions and capacity, then adds a unique
membership. Simultaneous approvals at the same revision can yield one success
and VERSION_CONFLICT; a current-revision approval at capacity yields PARTY_FULL.
A same-key replay returns the original safe receipt without repeating effects.
Different input conflicts. A newly keyed duplicate pending application is denied.
Business errors currently roll back the receipt transaction; delivery is adding
the exact durable BE15 domain-failure variants and migration0009. Adopting that
extension remains required work.

The internal root stores a typed aggregate JSON document plus checked indexed
columns. Private application/member indexes enforce pending uniqueness and one
owner. No arbitrary request/result body, private physical instructions, handles,
provider tokens or age claims are accepted by this encoding. SQL counters are
validated against the loaded domain aggregate; root locking serializes writes.
The internal `PublicParty` projection omits applications and member identities;
transport must resolve its owner UUID through the canonical public-profile owner.

Removal/leave/cancel/completion and meeting moves advance `audience_revision`
inside the transaction. `community::audience::party_audience_in(connection,event,
party,actor,expected_audience)` returns exact event/party/actor/owner, party,
meeting and audience revisions while retaining a SHARE lock. BE09/BE19 must call
it plus BE37 on **each** private read/command, and subscriptions must reauthorize
before disclosure. Outbox delay never grants continued membership. Closing
retains members; terminal parties deny private audience. Membership proves no
physical arrival. This seam does not authorize venue guidance.

`CommunityAuthority` is a temporary injected composition port, never a second
policy engine. No permissive production implementation exists;
`UnavailableAuthority` fails closed. The adapter must call the BE37 transaction
API with the exact actor/subject/event and enforce canonical session, event,
feature and current reviewed-place restrictions. BE37 is implementing
SafetyInteraction for READ_ONLY privacy-restricting commands. Synthetic local
SQL authority used in native tests is explicitly test-only and proves no real
Auth/Friend integration. Public routes/startup are not added here.

## Validation

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance only.
- Rust 1.88.0 `rustfmt --edition 2021` on owned Rust paths: exit 0.
- `python3 -m py_compile backend/scripts/mcr/community_smoke.py`: exit 0.
- Focused Cargo domain/application/native tests: **NOT_RUN**, awaiting shared
  `/Users/lhs/Desktop/mcr-build/backend` lease (bootstrap → admission → delivery
  combined → community). No duplicate Rust target is created.
- Owned native harness: **NOT_RUN** until the test executable is compiled.
  It exercises last-slot approval, exact replay/input conflict, duplicate apply,
  actor/event isolation, denied/outage authority, removal before outbox delivery,
  READ_ONLY safety, owner/place checks, restart replay and owned cleanup.
- Real multiuser HTTP/WS, provider policy fences, runtime subscriptions, database
  migration ledger composition, physical arrival, browser/device/load/release:
  **NOT_RUN**. Mock/unit/native synthetic authority is not end-to-end acceptance.

First checkpoint SHA and exact changed paths are delivered separately so this
file does not claim its own future commit hash or remote ancestry.

Delivery's member-result convention: `SafeReference.resource = Member.user_id`,
revision = that Member's revision, only under exact authorized event/party/user
request binding. The canonical HMAC includes party and user path IDs. Community
`RESOURCE_INVALIDATED` consumption is **coarse event invalidation**: refresh
party/member queries under fresh authorization. Do not infer party from user ID
or compare member revisions across parties. A future targeted consumer needs an
explicit internal metadata extension. Delivery authorized this convention in
chat, 2026-11-04; FE23 and BE09/19 owners were notified.

## BE20–BE23 source checkpoint

Giveaway implementation is authored under the same owned community roots.
Status remains IN_PROGRESS pending compilation/native proof and public/policy
composition. Giveaway states are DRAFT/OPEN/PAUSED/CLOSED/CANCELLED. Public
catalogue excludes DRAFT; authorized owner detail may read it. Public summaries
contain no reservations, recipients, private instructions or code fields.

The root row serializes reservations, cancellation, stock edits and claims.
Quantity cannot drop below held+claimed units; outstanding holds prohibit
pickup window/place changes. Publication requires an explicitly acknowledged
reviewed pickup meeting. Server settings capture grace at creation, and an
injected cutoff/BE08/media authority must validate it; no default operator or
place truth is invented. PAUSED allows existing pickup; CLOSED requires no
outstanding holds. Cancellation revokes private meeting/code access and releases
all outstanding units once. Reservations are one unit and unique while active.
Expiry is pickup end + captured grace, using DB time, never a creation-time
10-minute timer. HELD becomes READY at pickup start. Terminal claim never releases
stock, including later expiry scans.

`PgGifts::sweep(limit1..100)` scans a durable due index with one transaction per
root, DB clock and SKIP LOCKED; each root changes at most100 reservations per
pass. `reservation` performs the same lazy READY/EXPIRED transition before
projection. Mutation plus delegated BE15 maintenance enqueue share one
transaction; a failed enqueue rolls back stock changes. Crash/restart resumes
from durable state. It does not impersonate a user or reuse a reserve receipt.

Pickup codes are cryptographically sampled six-digit values; only a keyed HMAC
bound to exact event/reservation/generation/expiry is persisted. HMAC verify is
constant-time, with injected active/retained versioned keys and no defaults.
Rotation excludes the prior plaintext by verifying new candidates against the
old keyed hash. Code expiry is min(DBnow+300s,reservation expiry). Code/hash/key
materials have no Debug/Serialize implementations. Application/domain plain
code storage is prohibited. Code issuance returns plaintext only in a transient
`IssuedCode`; same-key replay yields only the original safe receipt, never a
recreated secret. Exact public response semantics for that replay are pending
C04 owner decision, so public code issuance remains disabled.

Wrong-code attempts commit durable budget consumption independently from the
BE15 command's rollback savepoint. Sorted DB buckets enforce reservation5,
recipient10, owner25 and trusted keyed-IP25 attempts per15minutes. IP context
must come from trusted authenticated runtime composition; browsers cannot choose
it and no raw IP is retained. Missing context fails closed. Code rotation does
not reset budgets. Claim reauthorizes/locks owner+recipient, verifies the current
reservation-bound code, then moves reserved--/claimed++ once. The stored pickup
receipt ID/revision survives later authorized duplicate claim commands. It is
separate from the BE15 command receipt UUID.

Proposed gift result convention sent to delivery: reserve/cancel/code/claim use
reservation_id and post-action Reservation.revision; code DTO exposes the same
reservation_revision. Pickup meeting uses giveaway_id and Giveaway.revision.
No public DTO/schema/contract change is made by this worker.

Additional authored validation: five-buyer/one-unit race, keyed input replay,
code rotation and one-shot replay, missing-key failure, reservation/generation
binding, shared persisted guessing limits, claim duplicates, expiry/cancel race,
one-time stock release, terminal private-access denial, restart expiry and
coarse metadata invalidation. Native command:
`python3 -B backend/scripts/mcr/community_smoke.py --giveaway --repository-test <absolute-native-test> --report <new-absolute-json>`.
Compilation/native test execution is still NOT_RUN until the shared lease.

A separate lightweight owned PostgreSQL16.14 fixture successfully applied
0001/0002/0007/0011 and checked both new origin constraints; exit0 and validated
owned cleanup PASS. This is DDL evidence only, not transaction/runtime evidence.
