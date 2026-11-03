# Participant producer — Phase A

Status: **IN_PROGRESS**. This bounded slice supplies an isolated guarded producer
and default-unavailable authority ports. It does not register a product listener,
Gateway route, real owner client, or mutation capability. Whole BE07 and IN01
remain open.

Base: `03fa2cdeeb70669ca35f8393a3532971c48fbccf`, branch
`codex/mcr-be-participant-producer`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-participant-producer`. Its BE07 persistence
ancestry is the validated stack integrated as `04c928a` -> `5b7116b` -> `03fa2cd`.
Read the active specification (sections 5.2–5.3, C03/C06 and BE04/06/07), project
assessment, execution plan, AGENTS and parallel delivery authority before work.

## Producer and wire

The new internal package is `concert.participant.v1`. Its
`ConcertParticipantService` has exactly these methods:

| RPC | Exact public target bound into its signed request | C03 response |
|---|---|---|
| GetParticipant | GET `/api/concert/events/{canonical_event_uuid}/me` | PrivateParticipant |
| SetAvatar | PUT `/api/concert/events/{canonical_event_uuid}/me/avatar` | AvatarState |
| UpdatePreferences | PATCH `/api/concert/events/{canonical_event_uuid}/me` | PrivateParticipant |

`backend/proto/concert/v1/participant.proto` is an internal request envelope,
not a replacement public contract. ParticipantRequest field numbers are:
user_id=1, event_id=2, session_family_id=3, request_id=4, http_method=5,
http_target=6, request_json=7, release_digest=8,
authority_expires_unix_micros=9, auth_observed_at_unix_micros=10,
profile_observed_at_unix_micros=11. ParticipantResponse.response_json=1 contains
the frozen C03 JSON. The complete protobuf body and exact RPC path are signed
by existing actor delegation. No access token, secret, role or duplicated
identity store is added.

`ParticipantService::new(store, auth, profile, mutations)` accepts explicit
Arc ports. `ParticipantService::default()` makes every dependency unavailable.
`server()` returns the generated server with 16 KiB encode/decode bounds.
Registration must use `platform::grpc_method_policy::guard(&participant::POLICY,
service.server(), callers, actors)`. The complete policy permits only Gateway
for all three methods and binds actor field 1. Direct handler calls without the
sealed workload/signature context are denied. No listener is registered here.

The private-constructor VerifiedParticipantRequest checks canonical nonnil UUIDs,
exact actor/user match, session/request UUIDs, method/target equality without
queries, exact bounded raw JSON and current release identity. GET rejects any
body. Avatar JSON uses the existing strict C03/C06 decoder; preference PATCH
uses the exact preference-only decoder. Unknown fields, duplicate fields,
URLs, unsupported versions/types and payloads above 4096 bytes are refused
before owner/store dispatch. Raw request bytes remain unchanged and are exposed
with their SHA-256 to the mutation-authority adapter.

The signed observation and original deadline bounds can only shorten the
captured request authority. Auth, Profile, capability and persistence awaits
share this same monotonic stop through the existing deadline scope and timeout.
Checks run after every successful await, before persistence, through BE07's
per-await/precommit checks and before returning a response. Delayed errors and
successes never renew authority.

## Current owner and capability contracts

- CanonicalAuthPort returns CurrentAuth only after canonical Auth positively
  confirms an active account and the exact live session family. A JWT or signed
  read assertion alone is insufficient. Actor/session mismatches are refused.
- CanonicalProfilePort returns only the current canonical self user ID,
  display_name and optional bio. A failed Auth gate prevents its dispatch.
  Profile identity, C03 bounds and control characters are checked; no legacy
  avatar URL, handle, email, provider claim or inferred age is serialized.
- MutationAuthorityPort returns a separate MutationPermit. Every write requires
  its check against the exact verified actor/event/session/request, operation,
  target, raw body digest, release and current server mutation capability.
  Its check is repeated during native persistence. VerifiedReadAuthority is
  used only to preserve signed transport time; it never supplies this permit.
- ParticipantStore is implemented by the already validated AvatarPersistence.
  Reads do not create participant defaults. The store's release digest must
  match before any owner is called. BE07 retains event/admission locks,
  optimistic revisions, inventory validation and rollback behavior.

These ports are required integration contracts, not implemented live owner or
capability clients. CurrentAuth/Profile observations are internal server values,
not portable browser grants. Their owner freshness/expiry and the original
request bound are rechecked together. Replacing these ports with fixture grants
in product composition would violate this handoff.

Reads join persisted participant/avatar state with fresh canonical Profile and
return `age_status: UNKNOWN`; no age authority exists in this producer. Avatar
success must contain the exact submitted config and expected revision + 1.
Preference success must contain the exact submitted preferences and participant
revision + 1. Foreign store identities, malformed responses, extra/private
response fields and inconsistent successful writes fail closed. Actual response
JSON is validated against the bundled frozen C03 schema with format checks.
Responses carry `cache-control: no-store`.

Only the preference subset of UpdateParticipantRequest is supported. Valid
C03 requests containing display_name or bio are explicitly rejected by this
RPC rather than ignored. Full PATCH requires a separately owned canonical
Profile mutation/revision protocol; cross-service atomicity is not claimed.
Neither preference changes nor this producer automatically replaces an avatar.
World avatar-delta publication belongs to the separately assigned world owner.

## Errors and activation

Safe failures contain no SQL/owner diagnostics or private state. Handler errors
include static `x-mcr-core-reason` metadata for the later Gateway normalizer:

| gRPC status | C03 reason / HTTP mapping |
|---|---|
| InvalidArgument | INVALID_INPUT / 422 |
| Unauthenticated | AUTHENTICATION_REQUIRED / 401 |
| PermissionDenied | PERMISSION_DENIED / 403 |
| NotFound | RESOURCE_UNAVAILABLE / 404 |
| Aborted | VERSION_CONFLICT / 409 |
| FailedPrecondition | EVENT_READ_ONLY / 409 |
| Unavailable | DEPENDENCY_UNAVAILABLE / 503 |
| DeadlineExceeded | DEADLINE_EXCEEDED / 504 |
| Internal | INTERNAL_ERROR / 500 |

Failures generated before handler dispatch by the platform guard still require
Gateway normalization. Root transport must normalize its own body/media errors
and use the safe C03 envelope. A response/authority error after commit is
uncertain; no automatic write retry is permitted. The authorized GET can
reconcile a revision conflict without replaying the previous mutation.

Product activation requires a later exact grant for startup/config, configured
canonical owner and mutation authority adapters, generated Gateway client and
HTTP handlers. Existing BootstrapService, concert.proto, rpc.rs, Gateway,
configuration and main are unchanged by this assignment. Production art and
release qualification remain unavailable; metadata fixtures never enable them.

## Provenance, ownership and checks

All new producer, proto and tests are newly authored from the frozen contracts.
Dependencies are existing platform workload/actor/deadline guards, BE07
AvatarPersistence and C03/C06 decoders, and the bundled C03 OpenAPI schema.
There is no old rendering, UI, source asset or cross-directory frontend import.
The only shared changes granted by delivery are the service lib module export
and additive build.rs compilation of the new proto. Existing concert/post proto
compilation remains byte-for-byte unchanged; no manifest/lock migration changes.

Owned paths: new `backend/services/concert-service/src/participant/**`,
`backend/services/concert-service/tests/participant_producer.rs`, this document,
new `backend/proto/concert/v1/participant.proto`, service `src/lib.rs` export,
and the additive service `build.rs` step. The previous avatar checkout remains
stopped. The later user-authorized direct-delivery policy allows this owner to
non-force publish its qualified exclusive patch; kings and SLs are not review
queues. Shared-file ownership and physical build leases still apply.

Planning check and whitespace check: exit 0. Scoped Rust formatting: exit 0.
Exact eight-path/shared-change/import audit: exit 0. The retained vendored
`protoc-bin-vendored-macos-aarch_64-3.2.0/bin/protoc` compiled the new proto to a
descriptor and decoded its package, all eleven request fields and all three
methods successfully (exit 0), independently of Cargo.
Compilation, guarded-RPC execution and native probe: **NOT_RUN** while awaiting
an explicit fair lease of the existing shared Cargo target. Source work has no
whole-wave barrier. Expected compile command:

```sh
env RUSTUP_TOOLCHAIN=1.88.0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
    -p concert-service --test participant_producer --no-run --message-format=json
```

The emitted executable is copied and SHA-pinned before releasing Cargo. The
native probe runs separately in the existing marker/inode-owned PG16.14 fixture,
using an explicit synthetic release and test-only owner/capability ports. It
exercises the real generated guarded RPC plus the real BE07 store, not live
Auth/Profile services or a browser network. This does not certify release,
whole BE07, the real multiuser world, or IN01/IN05.

## Recovery checkpoint — 2026-11-04

The preserved source checkpoints are `a2a60245` (producer), `f9e7323b`
(deadline/withdrawal probes), and `a4c4169e` (SQL-update rollback proof).
`dc11fe42` committed the previously dirty standalone-protoc evidence without
losing it. Merge `567d933f` imports fetched master `ce9e8e6c`; its shared-file
conflicts were resolved by preserving every upstream byte and adding only the
participant module export and participant proto compile step. Relative to that
merge parent, this slice still changes exactly the eight owned paths above.
The actual persistent Phase A goal remained ACTIVE through chat recovery.

Rechecked after the merge: `python3 scripts/mcr/check-plan.py`, scoped
Rust 1.88.0 `rustfmt --check --edition 2021`, `git diff --check`, and the exact
path/shared-content/import audit all exited 0. These do not establish Rust
compilation or executed RPC behavior. No participant compiler artifact, pinned
executable, or native result exists yet. The explicit shared-resource hold
reserves MEDIA for Friend RPC and BACKEND for World closure; no Cargo or private
PostgreSQL is started here until a named slot is released with at least 8 GiB
available. A process snapshot being idle does not transfer its lease.

Current C03 `AvatarConfig` is the layered 1.0.0 / whole-character 1.1.0 union.
The imported BE07 parser still supports only layered configurations at this
checkpoint. The selected-source owner retains avatar/domain/bootstrap and
migration 0025; this producer does not modify or duplicate their implementation.
Its complete current-union compatibility remains unqualified until their API is
available and the producer's actual guarded tests execute against it. Product
activation still requires qualified canonical authority and root composition
after this bounded Phase A assignment; default-unavailable ports do not satisfy
that remaining product scope.

The focused deadline matrix additionally covers slow mutation-capability awaits
and requires the exact `DEADLINE_EXCEEDED` reason after slow Auth/Profile calls,
including a noncooperative blocking Profile implementation. It must prove no
store dispatch. This source addition is also **NOT_RUN** until compilation.
