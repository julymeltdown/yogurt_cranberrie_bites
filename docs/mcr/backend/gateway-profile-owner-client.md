# Gateway Profile current self-content client — BE04 prerequisite

Status: **IN_PROGRESS**, bounded implementation delivered for parent review.
The parent explicitly transferred the shared Rust target after the Profile RPC
worker released it. Earlier source-only drafts recorded Rust **NOT_RUN**.
This delivers the Gateway
consumer of the frozen `ReadConcertSelfProfile` wire. It does not complete BE04,
activate a public admission flow, or certify the sibling Profile handler or native
PostgreSQL behavior.

## Source, scope and frozen interface

Worktree base `d1282c6c5c27099ce7cd2adeed30ee99e5c1896b`, branch
`codex/mcr-be-gateway-profile-owner`, isolated checkout `be-gateway-profile-owner`.
That parent commit contains the reviewed Auth client and the frozen Profile
wire/policy/AppState prerequisite. The broader Auth acceptance record lives at
the parent integration branch; this client bases its source on the exact files
that are present in this checkout.

The cumulative change is limited to four paths:

- new `backend/services/api-gateway/src/adapters/out/profile_client/concert_identity.rs`;
- new `backend/services/api-gateway/tests/concert_profile_transport.rs`;
- new `docs/mcr/backend/gateway-profile-owner-client.md`;
- existing `backend/services/api-gateway/src/adapters/out/profile_client.rs` with one
  module export.

The retained foundation is port-rust
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, mapped into `backend/` by the unchanged
[source map](../source-map.md) and [import manifest](import-manifest.json).
The production module is newly authored using the accepted Auth client's bounded
read pattern. Test harness/proof/capacity patterns adapt the accepted Auth suite;
Profile trait stubs follow the retained retry transport fixture. Strict RS256
contexts use the retained RSA/JWKS fixture. Source revisions are this exact base;
source hashes and destination mappings follow:

| Retained source at base | SHA-256 | Destination / use |
|---|---|---|
| `backend/proto/profile/v1/profile.proto` | `1ac74dfaed797919c333829aeaa3aa039c5dc08ea19b97b0a55d2a85fff79485` | Frozen interface (unchanged) |
| `backend/services/profile-service/src/adapters/in/grpc/method_policy.rs` | `08c2a07bdfeb3bc5fdb85129435155c16d78ebfcf3f561ecdb7ef9237daa0b02` | Frozen interface (unchanged) |
| `backend/services/api-gateway/src/adapters/out/auth_client/concert_admission.rs` | `558f41419c9d7f98cbc87e700c0ebd674446f26044dc0441ce370feb1233c132` | New client / retained deadline and signer |
| `backend/services/api-gateway/tests/concert_admission_transport.rs` | `3f3c15f2c086ed34c1cbe9ded0c39476f9b4648993ad11234a1439dd974df418` | New signed transport tests / strict verified context |
| `backend/services/api-gateway/tests/concert_access.rs` | `5f9b31a825bc3d09fc55b9be5a26f80eebb17b2a8d89f62ee23cc35416c82d5b` | New signed transport tests / strict verified context |
| `backend/services/api-gateway/tests/read_retry_transport.rs` | `50d3a9043ee7e4051c4c31e01b6f82f7f02899cd56767517bb0c3e98ee72a168` | New signed transport tests / strict verified context |
| `backend/services/api-gateway/src/adapters/out/jwt/concert_access.rs` | `e555eea1117d5d692181cb2bdb95002d451e3a3b46cbebe9d729ce39f572e899` | New signed transport tests / strict verified context |
| `backend/crates/platform/src/grpc_deadline.rs` | `8541e27f357b318aad6b6041f83add22266373ef842b1ff75652887889211db9` | New client / retained deadline and signer |
| `backend/crates/platform/src/actor_delegation.rs` | `0eba2a8eef09b6f1a365a8fd3a176b9090907abef7b12b33e3a0b6f58eb2eac8` | New client / retained deadline and signer |
| `backend/Cargo.lock` | `6f4a6388b0aff3b1f835e6f7f0f545f4c9c99a1bc51857a86cbee3daea4a7cd7` | Unchanged retained dependencies |

No dependency is added. Production uses retained Tonic/Prost, Tokio, platform,
Chrono, UUID and thiserror. Tests use retained RSA/jsonwebtoken, base64ct,
http-body-util, serde_json and Tokio test utilities; jsonwebtoken remains 9.3.1.
No proto, policy, platform, manifest, lock, router, HTTP, Auth/JWT client, Profile
implementation, frontend, asset, or existing test is changed. The frozen wire is
`/profile.v1.ProfileService/ReadConcertSelfProfile`, with canonical nonnil selector
field 1, minimal user UUID, display name, optional bio and statement observation.
The frozen policy accepts registered Gateway only and binds field 1 to its signed
actor.

## Behavior and bounds

The method accepts only a borrowed sealed `ConcertAccessContext`. Actor identity
comes from its checked accessor. The configured `GrpcProfileClient` channel,
service identity interceptor, tracing layer and actor-delegation signer create and
sign the exact protobuf frame. No bearer, email, roles, provider/session proof,
legacy Profile fields or alternate signing key crosses this read.

The existing context retains its original monotonic request and JWT stops. The
method clips those and the existing GatewayProfile local ceiling, then applies any
already-active task deadline. One absolute `timeout_at` is enclosed by the retained
non-retrying `platform::grpc_deadline::with_deadline` helper. That helper first
checks the process-wide GatewayProfile limiter and its actual
`Edge::max_in_flight`, preserving standard in-flight/deadline metrics. It holds
the permit through readiness, signing, dispatch, response decoding or any failure
and drops it on normal return, owner error, timeout and cancellation. No queue,
retry, fresh budget, or per-call limiter is introduced.

Decoded responses are capped at 1024 bytes including unknown fields. Valid unknown
fields within the cap follow normal protobuf forward compatibility; they do not
add projected content. The framing fixture preserves the original gRPC trailers,
so the exact-boundary success proves decoding rather than a missing-status error.
Success requires a present Profile, exact canonical returned user matching the current
context subject, nonblank display name of at most 40 Unicode scalars with no
controls, optional bio of at most 160 scalars with no controls, and a UTC
timestamp in calendar years 1 through 9999. Only protobuf's exact empty bio is
normalized to `None`; spaces, trim, fallback and caching are absent. Debug output
for the borrowed observation is redacted. It has no public constructor, Clone,
serialization, broad DTO conversion or request rebind. Every accessor rechecks the
borrowed context, and checks also occur before dispatch, after await, before
projection and before return; a late noncooperative result cannot succeed after
the stop.

`NotFound` is retained distinctly from rejected identity (`Denied`), expired
context/budget, local or peer `CapacityExhausted`, malformed/decode failure and
general dependency `Unavailable`. Tonic DataLoss/OutOfRange map to
`InvalidResponse`; other dependency codes use `Unavailable`, so diagnostic text
is never needed to distinguish a peer error from a decoder error. Those errors are fixed safe values: raw peer
messages and details are dropped. The client never parses diagnostic text or
turns a missing/misbound malformed owner reply into content.

## Signed loopback evidence

The controlled generated server is registered under the actual frozen
ReadConcertSelfProfile policy and caller registry. It requires
`VerifiedCaller(Gateway)`, a `VerifiedActor` equal to field 1, a private
`VerifiedReadAuthority`, and a live original deadline. The client suite exercises
two fresh visits with changed content, absent/misbound/malformed content, exact
length/control/empty-bio/calendar boundaries, valid unknown fields at the exact 1024/1025-byte boundary and oversized frames, configured
caller, token and signer rejection, raw actor/body/audience/signature/missing-proof
and missing-deadline attacks, stalled channel readiness, dependency denials,
actual wall JWT expiry and captured monotonic JWT expiry, expiry during delay and
blocking work, consumed observation expiry, and no owner dispatch for rejection.

The capacity case runs itself alone in a child process, uses the real global
GatewayProfile limiter and reports its actual ceiling rather than assuming a
fixed count. It holds all permits; requires zero owner calls,
`CapacityExhausted`, rejection count one and deadline count zero; then releases
them and verifies capacity after success, owner error and peer deadline. A fresh
hold scenario requires a new owner call, one active handler and one held permit
before caller cancellation; afterward it requires all capacity restored, zero
deadline/rejection counters, cleanup and a successful subsequent signed read.

These checks use configured local-parity signing and a controlled owner double.
They prove the signed Gateway transport, validation, deadline, limiter and cleanup
path, not the Profile application handler, PostgreSQL, TLS, production keys or
whole admission. The sibling Profile RPC implementation and native database tests
remain separate.

## Verification status

Rust 1.88.0, retained shared target, `--locked --offline -j 2`, both
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`:

```sh
cargo +1.88.0 test --locked --offline -j 2 -p api-gateway \
  --test concert_profile_transport --test concert_admission_transport \
  --test concert_access --test unit --test verification_transport \
  --test read_retry_transport -- --nocapture
cargo +1.88.0 test --doc --locked --offline -j 2 -p api-gateway
cargo +1.88.0 check --lib --locked --offline -j 2 -p api-gateway
cargo +1.88.0 build --bin api-gateway --locked --offline -j 2 -p api-gateway
```

The final combined transport/context/unit/retry run exits **0**, with **268**
top-level cases: 15 new Profile, 17 retained Auth, 13 strict context, 209 Gateway
unit, 13 verification transport and one retained Profile retry test. No top-level
failure, ignore or filter. All **10 compile-fail doctests**, the locked library
check and Gateway binary build also exit **0**. The doctests cover the observation
constructor, serialization, deserialization and owned Clone prohibition.
Both global-capacity cases reexecute their own single
case in isolated child processes; those are not additional functional cases.
The Profile child uses the actual 256-permit global pool. Its asserted final
lifecycle counters are five owner calls, one capacity refusal and two deadlines
(one peer-reported and one earlier local task stop), with 256/256 restored.
Its fresh cancellation/reuse harness has two calls, zero refusals and zero
deadlines, with 256/256 restored after observing a genuinely active held call.
No PostgreSQL or Docker is used by this controlled client suite.

Verification logs and the immutable hashed receipt are retained outside Git in
`mcr-build/gateway-profile-owner/`. The receipt records the delivered commit,
base, exact path/file hashes, commands, real exit codes, log hashes, source
provenance and target-release audit. It avoids a self-referential commit hash in
this committed document. The parent integration record owns delivered ancestry.

The initial 14-case candidate passed (`01-profile-suite.log`). A new exact-boundary
case then found a fixture-only defect: response reframing dropped gRPC trailers,
so valid bounded protobuf appeared unavailable (`02-combined.log`, exit 101).
The first trailer-preservation compile attempt used the wrong Option/Result
shape (`03-boundary-fix.log`, exit 101); correcting it retained original trailers
and the exact boundary test passed (`04-boundary-fix.log`, exit 0). The subsequent
complete suite is `05-combined-final.log`, exit 0. No failed attempt was removed.
An initial boundary-check invocation used an invalid composition exception for
an ordinary adapter path; it exited 1 and established no boundary evidence.
The corrected invocation requires only the assigned documentation exception.

Static verification commands (each exit 0):

```sh
python3 scripts/mcr/check-plan.py
python3 scripts/mcr/check-boundaries.py \
  --base d1282c6c5c27099ce7cd2adeed30ee99e5c1896b --lane backend \
  --allow-doc docs/mcr/backend/gateway-profile-owner-client.md
git diff --check
rustup run 1.88.0 rustfmt --edition 2021 --check \
  backend/services/api-gateway/src/adapters/out/profile_client/concert_identity.rs \
  backend/services/api-gateway/tests/concert_profile_transport.rs \
  backend/services/api-gateway/src/adapters/out/profile_client.rs
```

Manual exact-path/import review requires only the four assigned files, no
frontend imports, immutable retained source/proto/lock hashes and an existing
client file byte-identical to base after removing the single module export.
Parent alone integrates and pushes. The bounded goal remains ACTIVE through
review and concrete fixes; whole BE04 remains open.

## Unqualified composition

No HTTP original-entry capture, key-refresh/key-withdrawal continuation, real
Profile service handler, native Profile/PostgreSQL read, Auth grant, event/role
authority, signed Concert admission, revocation fencing, stream invalidation,
production mTLS or public endpoint is supplied. A Profile observation is current
owner content at one statement snapshot, not an admission decision or a freshness
guarantee. Whole BE04 and real email-only participation remain incomplete.
