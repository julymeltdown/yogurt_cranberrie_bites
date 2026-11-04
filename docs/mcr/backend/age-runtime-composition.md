# BE26 canonical age runtime

Status: IN_PROGRESS. Authored source is not native or HTTP acceptance. This
successor registers the retained Concert-owned BE26 implementation; it creates
no Auth claim store and does not require a commercial provider to compose the
unavailable runtime.

## Source and ownership

Base: `cb9abce48d78227bbdedf7ea59ce95e9e0270c4a`. Initial source commits:
`a1b6a0f0` (interfaces, registration, witness), `ec2399cd` (canonical native
fixtures and exact frozen route correction). Existing provenance is recorded in
`age-verification.md`, `age-verification-combined.md`, and `age-claim-reader.md`;
their historical integration-only restrictions do not supersede current owner
assignments. `0038_age_runtime.sql` registers the retained `schema-v1.sql` and
adds a private native transaction witness. No raw birth date or proof is added.

This owner changes only age application/provider subtrees, the named migration,
new age runtime tests/support, this document, and exact delegated module exports
and the `age_claims.rs` aliases needed by its retained path-included adapter. Exact
additional test grants declare accepted fixture provenance in the old reader
authority and the old unavailable-provider producer initializer.
Root owns Operation variants, Gateway/Concert HTTP dispatch, process config,
capability composition, and the composite settlement resolver. Intro owns its
read/write policy, row locks, routes, schema and runtime acceptance.

## Concrete composition

`age_provider::runtime::AgeRuntime::new(pool, clock, fingerprint_keys, mode)`
implements `ScopedProductProducer`. The root passes the original held
`Arc<OwnerGuardScope>` after actual owner acquisition and durable delivery-token
publication. It owns response EOF/abort and scope completion. The producer never
finishes the root or renews its deadline.

`ProviderMode::Unavailable` keeps verification unavailable while permitting
canonical own-status reads. `ProviderMode::Configured { config, verifier }`
requires an explicit exact-allowlisted HTTPS `ProviderConfig` and an independently
verifying server adapter. `verification_available()` reflects both mode and
adapter availability. A synthetic test verifier is not an activation input for
production. No permissive default or commercial brand is selected.

Own operations use `claims(scope)` with `ScopedAgeAuthority`; every SQL phase
asserts the original canonical actor/event/session-family/request/deadline and
operation, registers the actual native transaction with both owners and the
root before private effects, and checks the current canonical owner authority.
Event and current participant locks remain in that same transaction. Erasure
requires current canonical owner/session authority and deliberately permits
closed events and departed participants.

Exact C04 paths are `/api/concert/events/{eventId}/me/age-status`,
`/me/age-verifications`, and `/me/age-verifications/callback` under that event.
The producer checks method, exact target, body hash and the sealed root binding;
DTOs deny unknown fields. New initiation exposes the authorization URL once.
Same-key initiation replay returns the existing safe `CommandReceipt` as HTTP202,
which C04 already permits. It never fabricates or recovers a raw state token.
Callback replay returns the same receipt/status, never another exchange.
Uncertain exchange/commit preserves Pending and requires the original command
identity for recovery. An available persisted claim remains valid when the
provider is unavailable. Own-status reassesses temporal validity after COMMIT. Both own-status and restricted
projection require the canonical newest record to match explicit accepted source
provenance before assessment.

## Intro shared-transaction projection

`trust::ClaimTrust::new(&[(verifier_id, source_revision)])` defines the immutable
accepted source set (up to16 pairs). `ClaimTrust::none()` denies unknown sources.
Configured mode derives its active source by default; unavailable mode derives
an empty set. `runtime.with_claim_trust(trust)` explicitly retains accepted prior
sources during an operational outage or rotates the accepted set. Omitting the
configured active source also disables new starts. Source trust is not a browser
input, a proof, or an operational availability flag.

`runtime.projection(scope)` returns `PgClaimProjection` using
`scoped::ScopedProjectionAuthority::with_claim_trust(scope, verification_available, trust)`.
The compatibility `new(scope, bool)` constructor has empty trust and denies claims.
It reuses Intro's already registered native transaction and does not allocate
another settlement reference. It checks the viewer's exact original scope and
current participant eligibility. Unrestricted discovery bypasses claim reads.
Original Intro discovery bounds apply only to the viewer.

`project_in(&mut PgConnection, query, closure)` lends both that same connection
and a private `ProjectionAge` to the closure. The closure must contain all
remaining private Intro row, relationship and age-dependent projection work.
The adapter selects the global canonical newest claim before assessment,
retains event/owner/claim locks, and repeats canonical authority after projection.
The returned `PendingAgeProjection<T>` keeps its value private. The caller must
commit the SAME transaction, discard the value on rollback or uncertain commit,
and call synchronous `.after_commit()` before returning its DTO. That performs
expiry/request/Seoul-birthday checks after commit. It is not a claim lease and
cannot authorize another read. The legacy `ClaimReader::read` still denies.

Unknown, expired, unsupported or unavailable verification denies only restricted
rows; a source dependency failure fails the projection. A current persisted
valid claim remains usable during provider unavailability. No fallback to an
older claim is allowed.

## Native settlement and limits

`settlement::AgeTransactionResolver::new(pool, producer_instance)` composes with
root's other resolvers. Private insert-only witnesses bind the exact guard,
cluster, database OID, full XID, backend PID/start, effect digest, instance and
opaque settlement reference. A trigger verifies insertion in that very native
transaction. Updates/deletes/truncation are refused. Registration precedes the
witness and all protected effects. Resolver visibility of an exact immutable
witness proves that native transaction committed; raw `pg_xact_status=committed`
without that witness remains Unknown. Aborted/in-progress resolution delegates
to the existing native resolver. Root/owner durable settlement remains separate
from successful producer return. Witness retention needs a coordinated policy;
this patch does not delete evidence early.

The new tests use actual canonical Auth/Friend services over fixture mTLS and an
owned socket-only PostgreSQL16 store. Only provider proof verification is
synthetic; the dedicated provenance-policy case also seeds explicitly synthetic
private records to exercise rejection of an unaccepted newest source. Fixture support is adapted from `tests/owner_authority_support/` at
the base pin; migration/tool ownership checks remain intact. Tests and receipts
must be run from frozen hash-checked artifacts after releasing the named Cargo
target. Runtime HTTP composition and real provider/browser callback delivery
remain separate, explicitly recorded acceptance boundaries.

## Qualification record

Pending the current named-target queue. No native, HTTP, commercial-provider,
restart, browser, whole-product or global task-count acceptance is claimed by
these source commits.
