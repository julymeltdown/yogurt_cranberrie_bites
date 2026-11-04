# BE26 canonical age runtime

Status: canonical runtime qualification PASS (43 regular tests plus one durable
restart case). This registers the retained Concert-owned BE26 implementation
without a new Auth claim store or commercial-provider prerequisite. Public C04
HTTP routing, external provider activation and browser delivery remain separate
Root/provider integration boundaries. Master publication is owned by the
designated integrator; its verified ancestry receipt is recorded after delivery.

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
authority (including its commit-delay wrapper) and the old unavailable-provider
producer initializer and the durable restart verifier initializer. Root additionally delegated the exact disclosure deadline
hook in `relationship_authority/guarded/scope.rs`.
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
cannot authorize another read.

Before private projection, the scoped authority also calls
`OwnerGuardScope::limit_disclosure_until(DateTime<Utc>)`. Own VERIFIED status uses
the same hook. The held scope retains both wall expiry and its captured monotonic
stop; repeated limits only shorten either clock. Invalid, expired and sealed
scopes reject the call. Root SDK3 folds this policy boundary into its existing
`disclosure_ceiling()` and `content_expires_unix_micros` response metadata. This
closes the interval between known COMMIT and the transport disclosure boundary
without renewing authority or changing settlement. Stable expired/revoked status
responses use the original request ceiling because they disclose no valid claim. The legacy `ClaimReader::read` still denies.

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

Qualified source/controller: `d85da0923cca495931b41b2253cacf8d2c3aace4`.
Canonical runtime, application and reader binaries were compiled at
`7c0ea9539129e54e4624c51abb0087ffc68a74c2`, which incorporates actual Root
SDK `25a8742638d47f940b44df638d7cca968ad96af1`. The only source delta to
`d85da092` is the explicitly granted accepted fixture source initializer in
`age_claims.rs::durable_restart_verify`. Its producer binary was rebuilt at that
new pin; the other three frozen copies were retained unchanged. The derived
checkpoint verifies that exact one-file, one-initializer delta and records every
binary source separately. Cargo reported `fresh=false` for all four successful
emitted test artifacts; dependency-cache reuse is not advertised as recompiling
every dependency.

Rust 1.88.0, locked/offline Cargo, two jobs, debug information disabled and
incremental compilation disabled were used. The two named media allocations
were accepted only for the exact test targets below, with fresh disk/process/
handle preflights. Successful service/application builds took 20.39s/2.58s;
the fixture-only producer rebuild took 17.00s. Actual shared-cache failures
(proto include paths from another checkout and stale application ABI metadata)
were preserved, then repaired only with evidenced `contracts-build` and
`concert-application` package-scoped cleans. No global cleanup, new target,
dependency or lockfile change was used.

```sh
# Only while holding the named target allocation; debug=0, incremental=0, jobs=2.
cargo +1.88.0 test --offline --locked -j2 -p concert-service \
  --test age_runtime --test age_claims --test age_claim_reader \
  --no-run --message-format=json
cargo +1.88.0 test --offline --locked -j2 -p concert-application \
  --test age_verification --no-run --message-format=json
# Fixture-only rebuild under its subsequent ONE-artifact allocation:
cargo +1.88.0 test --offline --locked -j2 -p concert-service \
  --test age_claims --no-run --message-format=json
```

Each executable was copied outside the target and SHA-256 checked before the
unconditional target release/no-handles receipt. Only after release did
`tests/age_runtime_support/qualify.py` run the copied binaries against a newly
owned socket-only PostgreSQL 16.14 cluster. No ambient or live database URL was
used. All six exact canonical cases passed, followed by retained application10,
producer16 and reader11. The one ignored producer case was then executed
explicitly after a real PostgreSQL stop/start: `durable_restart_verify` passed.
Postmaster PID changed from 81117 to 82336 and its start time changed. Registered
migration ledgers matched Auth17/Friend17/Concert29. No prepared transactions
remained in the canonical databases; cleanup passed and the owned root is absent.

| Original criterion / invariant | Actual passing evidence |
| --- | --- |
| Seoul next-birthday and expiry | Application `seoul_birthday_expiry_and_missing_provider_remain_separate`; producer `persisted_100_to_101_transition_is_exact_seoul_midnight`; reader `exact_seoul_boundary_rejects_projection_crossing_birthday` |
| Canonical newest replacement, no fallback | Producer `canonical_replacement_expiry_revocation_and_seoul_age_are_persistent`; reader `newest_expired_revoked_replaced_unsupported_never_falls_back`; canonical untrusted-newest case |
| Canonical callback-to-user/session binding and replay | Application `provider_proof_must_match_every_binding`; producer foreign-binding, atomic callback/replay and race cases; canonical `configured_canonical_callback_replay_and_unavailable_status` |
| Missing provider fails closed; accepted claims survive outage | Application unavailable verifier case; producer explicit trusted-outage case; canonical unavailable erasure and accepted-source outage/newest cases |
| Current owner authority and same transaction projection | Canonical foreign/unregistered denial and actual registered native identity/claim-lock case; reader source-loss, revocation, cancellation and commit-expiry cases |
| Age validity persists to Root disclosure | Canonical `age_expiry_shortens_actual_root_disclosure_ceiling`, including repeated limit, original authority still live at age expiry, and sealed-scope denial |
| Registered persistence survives restart | Actual migrations, native witness settlement, producer restart seed and explicit postmaster restart verification |

The canonical verifier remains explicitly synthetic (`synthetic`,
`synthetic-v1`); legacy fixtures use (`fixture`, `fixture-v1`). Dedicated
provenance/timing cases label their direct private records synthetic. They prove
canonical runtime behavior and persisted-source policy, not an external provider
or real public-principal verification. No credentials, DOB or proof values are
part of these receipts or public response DTOs.

### Immutable evidence

Private artifact directory: `mcr-age-runtime-sdk3-n1fibr_g`; the local absolute
locator is `/private/tmp/mcr-be26-sdk3-artifacts`. The final native receipt is
`native-2.json` and the exact mixed-source manifest is
`frozen-2/checkpoint.json`. Original build JSON, stderr, preflight/release
receipts, all four old/new frozen copies and failed native reports remain there.
The earlier partial directory `mcr-age-runtime-build-do414p8a` also remains
preserved. No failed receipt was overwritten or converted to PASS.

| Evidence | SHA-256 |
| --- | --- |
| `native-2.json` | `a4a331d7e99f7d7301d66f5ebcfbd790dff5116b6f5aaa113525a81b61b56bfb` |
| `frozen-2/checkpoint.json` | `df74eead08d35adfacb82709d7a9d8d3c62423b157893f395a78cec22a1f54f8` |
| `native-1.json` | `2371c97017b35c30d5c6dd0e436b8cfea4d864659a7901ac78303a1d7e755d04` |
| `service-3.jsonl` | `6fbed3027ad250a9ebfbe74488162a4200840c830dc583ecbf260b576068239d` |
| `application.jsonl` | `470e92b531a358990a148a0f8591cff9913dffe6f233f3864558b6f566dc5e68` |
| `restart-fixture-3.jsonl` | `9f9c6141e40ccdd1eec5a6f934c16a7d0f5d9a75794e966121465fb8508663c2` |

| Executable | Binary source | SHA-256 |
| --- | --- | --- |
| `age_runtime` | `7c0ea9539129e54e4624c51abb0087ffc68a74c2` | `f0c4550fd3bccebdedbeb03a0811cc15bab1f49dad355b23f4234332644e29e1` |
| `age_claim_reader` | `7c0ea9539129e54e4624c51abb0087ffc68a74c2` | `55b26ca6de9e8e5ca62e84fc5dbc4fc234632ba4c5924e92c74ad67945bb4c22` |
| `age_claims` | `d85da0923cca495931b41b2253cacf8d2c3aace4` | `47dfd127da8370ec7492af0e48b3a9fc88ba317898121f80b841d5d2df069ef7` |
| `age_verification` | `7c0ea9539129e54e4624c51abb0087ffc68a74c2` | `c51209048f032b3ecc4b9fb6548f18a50b0ffdfd3671162731078c2103b6b843` |

Contract inputs are unchanged at both binary source pins and the final fixture
controller pin. Full backend/protocol/contract JSON hashes are retained in each
checkpoint; relevant frozen C04 inputs are:

| Contract | SHA-256 |
| --- | --- |
| `contracts/domain/v1/operations.json` | `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839` |
| `contracts/generated/mcr-domain.openapi.json` | `79a066b88119e53f6f16876d7e3fe59a18ee956be6ed6d2af7ee1efa187e0feb` |

### Delivery and remaining integration boundaries

The owner publishes `codex/mcr-be-age-runtime` directly without force. Only the
designated Root integrator routes it to master; the owner verifies remote HEAD
and ancestry afterward and retains `master-delivery.json` in the private
artifact directory. Documentation-only delivery commits do not change the
qualified runtime or contract inputs.

Root must compose the actual public age Operation variants, HTTP dispatch,
process/provider configuration, truthful capability input and composite native
resolver. Intro consumes the exact same-transaction projector described above.
The existing unavailable runtime has no production proof verifier selected;
activation requires an independently verifying server adapter and its explicit
accepted source policy. These open public composition/provider/browser boundaries
do not replace the completed original persistence criteria with an external
commercial-provider or whole-World prerequisite. No IN03, whole-product, release,
real-provider or browser acceptance is claimed here.
