# Gateway Auth current-admission read — BE04 prerequisite

Status: **IN_PROGRESS**, bounded implementation/verification delivered pending
parent review. Whole BE04 and actual email-only Concert participation remain
incomplete. This client obtains request-local Auth flags; admission also requires
the outstanding owner/Profile/event and final dispatch protocol.

## Source and exact ownership

Clean published base `e6efa4c4d6888415835f732de1b1f8c89aa9d460`, branch
`codex/mcr-be-gateway-auth-owner`, isolated worktree `be-gateway-auth-owner`.
Only these four paths are delivered:

- `backend/services/api-gateway/src/adapters/out/auth_client/concert_admission.rs`
- `backend/services/api-gateway/tests/concert_admission_transport.rs`
- `backend/services/api-gateway/src/adapters/out/auth_client.rs`: one module export
- `docs/mcr/backend/gateway-auth-owner-client.md`: this record

The production child module is newly authored. It reads the existing client's
private configured channel, identity interceptor, tracing/deadline wrapper and
`ActorDelegationClient`; it changes no connection/configuration or legacy method.
No Auth implementation, owner store or alternate signing-key source is copied.
The canonical upstream foundation remains port-rust
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`; see the unchanged
[source map](../source-map.md) and [import inventory](import-manifest.json).

At this base the existing wire is
`backend/proto/auth/v1/auth.proto::ReadConcertAdmission`, SHA-256
`807a94ba92b9b7bdaf8fbd4f69d9c16d89906f8487e28249fa2cd36b0bd32235`.
The installed Auth policy requires Gateway and actor field 1, with Auth workload
audience. Policy SHA-256:
`c4640dda67fe00fc0652760d211cddae050cbca5be48b19e8b98c6a033181c63`.
The tests use that exact read-policy subset with the public production guard,
and pin its source semantics explicitly; they do not widen any installed policy.

Test provenance, all at the exact base above:

- Generated Auth trait signatures and owned loopback shutdown pattern adapt
  `backend/services/api-gateway/tests/verification_transport.rs`, SHA-256
  `61af006230f216eee30234677aae282a3768853d7602aa5ab001da6164a36a8e`.
- Cached RSA-2048/JWKS fixture adapts
  `backend/services/api-gateway/tests/concert_access.rs`, SHA-256
  `5f9b31a825bc3d09fc55b9be5a26f80eebb17b2a8d89f62ee23cc35416c82d5b`.
  Every strict context comes from actual configured-key RS256 verification.
- Guard/registry/deadline setup follows the retained
  `backend/crates/platform/tests/grpc_method_policy_network.rs` and Auth's
  `backend/services/auth-service/tests/concert_admission_rpc.rs`. No fixture is
  edited; no private `VerifiedReadAuthority` or access-context constructor is used.

Retained dependencies only: generated Tonic/Prost, Tokio, existing platform,
Chrono, UUID and thiserror. Test signing/framing uses existing RSA/jsonwebtoken,
base64ct and HTTP-body utilities. Locked jsonwebtoken remains 9.3.1. Retained
lock SHA-256 is
`6f4a6388b0aff3b1f835e6f7f0f545f4c9c99a1bc51857a86cbee3daea4a7cd7`.
No manifest, lock, proto, platform, Auth, strict JWT module, HTTP, ports, domain,
Profile, Concert, frontend, artwork or shared fixture is changed.

## Request, deadline and response behavior

`GrpcAuthClient::read_concert_admission` requires a borrowed sealed
`ConcertAccessContext`. It derives the canonical actor and exact signed JWT
family from its checked accessors. Optional `BindingReference` is a shape-only
value with canonical nonnil Auth identity-row UUID, closed EMAIL/GOOGLE/APPLE/X
provider and epoch in `1..=9007199254740991`. Auth must still check actual current
ownership/generation. Ordinary entry sends no binding. Browser bearer, global
roles, provider subject, email and private Profile data never cross this wire.

`request_for` installs the actor extension; the existing configured
`ActorDelegationClient` signs the actual protobuf frame, audience, actor, method
and body digest. The service identity interceptor carries the registered caller.
These authenticated paths are exercised over generated loopback transport; plain
caller metadata is not treated as signed actor authority.

At call entry the client captures the context's existing `AuthorityRpcDeadline`.
Remaining time is clipped to the context, that captured deadline and the existing
GatewayAuth local edge ceiling: 900ms endpoint budget minus its retained 100ms
local margin = 800ms. A pre-existing task deadline can only shorten the stop.
One absolute stop wraps the complete generated future, including channel
readiness, serialization/body collection, configured signing, server and response
decoding. The request gets the clipped transport timeout; `call_deadline::scope`
lets the configured tracing layer propagate remaining time. The client checks
context before the await, after it, before projection and before return, and
rejects late results at the same stop. Earlier waits consume the original
context budget; there is no fresh full request budget, retry or positive cache.
The edge ceiling is a retained resource guard, not a revocation promise.

Response decoding is bounded at 128 bytes, including unknown protobuf fields.
Only known nonzero binding states with requested/not-requested consistency can
project. The signed `i64` database observation must fit Chrono's UTC representation.
No observation TTL, future-clock/skew policy or revocation SLO is invented:
valid old/epoch timestamps remain representable observations, not leases.
Unknown/unspecified enums, inconsistent binding replies, invalid timestamp
representation, oversize data and upstream failures cannot become positive facts.

`AdmissionObservation<'context>` has private fields, no public constructor,
Clone or serialization/deserialization. It borrows the exact context used for
that read and retains its exact optional binding shape; it has no rebind API.
Every fact/reference/time accessor checks that context again. It stores only
account/session flags, closed binding state, database time and those private
request ties. Debug is fixed/redacted. Inactive account, ended session and
NotBound remain explicit non-granting outcomes; no admission decision is made.
Upstream diagnostic strings and details are dropped in favor of fixed safe
errors. Raw bearer tokens or private claim/error canaries are not logged/stored.

## Verification and evidence

Rust 1.88.0, `--locked --offline`, two jobs, disabled debug information, retained
target `/Users/lhs/Desktop/mcr-build/backend`. The final combined run passed
**16 new guarded transport + 13 strict context + 209 retained Gateway unit +
13 verification transport** cases, zero failed/ignored/filtered, exit **0**.
The transport server is a clearly controlled owner double behind the actual
registered caller/signature/body guard and deadline layer; it requires the guard's
private verified authority and its original deadline before counting dispatch.
This qualifies transport, not native Auth/database behavior.

Coverage includes ordinary email-only reads, exact Bound/NotBound references,
honest inactive/ended flags, changed owner replies across fresh calls, shape
bounds, unknown/inconsistent state, timestamp representation and absence of an
invented TTL, wire oversize, private upstream errors/outage, wrong configured
service token/caller/key, raw wrong actor/body/audience/signature, missing proof/
deadline, pre-call expired context, earlier waits, channel readiness, slow and
noncooperative late completion, observation consumption after expiry, cancellation
and recovery. Dropped-future cleanup is tested against the owned double without
claiming instantaneous server database cancellation. Real signed JWT wall expiry
is exercised during a held reply and must win before the existing edge ceiling; test-owned
Tokio advancement after real owner dispatch demonstrates captured JWT monotonic
expiry while signed UTC time and the original request stop are still live (the
edge is elapsed there as well).

The initial compile attempt exposed two test-fixture composition/borrow errors.
The next run passed 14 cases and exposed RSA first-use initialization occurring
after selection of a short expiry. Key initialization now finishes before the
one-token timing fixture; there is no retry of an unexpected denial. Both
development failures are retained separately from the final passing invocation.

Reproduction from this worktree uses `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --test concert_admission_transport --test concert_access --test unit --test verification_transport -- --test-threads=2
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --doc
cargo +1.88.0 check --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --lib
cargo +1.88.0 build --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --bin api-gateway
rustfmt +1.88.0 --edition 2021 --check backend/services/api-gateway/src/adapters/out/auth_client.rs backend/services/api-gateway/src/adapters/out/auth_client/concert_admission.rs backend/services/api-gateway/tests/concert_admission_transport.rs
python3 scripts/mcr/check-plan.py
python3 scripts/mcr/check-boundaries.py --base e6efa4c4d6888415835f732de1b1f8c89aa9d460 --lane backend --allow-doc docs/mcr/backend/gateway-auth-owner-client.md
git diff --check
```

All **six compile-fail doctests** (three retained and three new), locked library
check and Gateway binary build pass, exit **0**, without warnings. Rustfmt,
whitespace, planning provenance and the exact four-path backend boundary check
pass, exit **0**. The existing `auth_client.rs` differs by exactly one export;
all protected inputs and strict-context sources remain byte-identical to base.

The external handoff receipt retains exact source/contract/fixture/log hashes,
commands/exits and committed ancestry at
`/Users/lhs/Desktop/mcr-build/gateway-auth-owner-handoff.json`. Logs use the
`gateway-auth-owner-` prefix there. Parent alone reviews, integrates and pushes;
the bounded goal stays active through concrete review fixes.

## Unqualified composition

No production caller, HTTP route or accepting Concert policy is activated.
Trusted HTTP original-entry capture, key-refresh budget propagation and key
withdrawal across awaits, real current owner/Profile/event eligibility, signed
Concert admission, revocation/epoch fencing, stream invalidation and production
mTLS remain parent-owned requirements. An observation is not an Auth-signed
portable OwnerProof or a guarantee against subsequent revocation. The final
dispatch must recheck context and all relevant owner/local evidence under its
explicit protocol. No numeric freshness or revocation bound is fabricated here.

No DB/Docker/default service or unrelated owner cluster is started. Prior native
Auth evidence and its separate unchanged Docker-only schema/privilege fixture
failure are neither rerun nor certified by these tests. Whole BE04, full-project
tests and release acceptance remain incomplete.
