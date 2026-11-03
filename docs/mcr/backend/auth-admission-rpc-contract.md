# Auth admission RPC composition contract

This internal BE04 increment follows the reviewed Auth-owned current-state read.
It does not change C03 browser routes or enable Concert admission. The integrator
owns the proto, platform context and method policy; the assigned Auth worker owns
the exact handler/composition exceptions recorded in its handoff.

`auth.v1.AuthService/ReadConcertAdmission` accepts canonical authenticated actor
UUID, JWT session-family UUID and an optional exact Auth provider binding
reference. Omit the binding for ordinary email-only entry. The response has only
database observation time and account/session/binding facts. It is not a signed
capability and does not echo event/purpose as evidence. No provider subject,
email, Profile content, Auth role or bearer/refresh token crosses this RPC.

Only registered Gateway calls are allowed. Method policy declares request field
1 as the signed actor. The existing verifier authenticates caller, audience,
full method, exact protobuf frame/body digest, actor and replay token before
installing `VerifiedReadAuthority`. The handler also requires Gateway identity,
the verified actor and this private-constructor context to agree with the parsed
canonical request actor. Bare metadata or an injected UUID is not verified
authority. Binding ID/type/epoch are shape-validated and the real owner read
checks the exact user binding; this does not create or attach an identity.

An original caller deadline is required. The verified context captures guard
entry before body buffering and the original task deadline. Strict read deadlines
are the minimum of original transport stop, guard-entry resource ceiling,
captured signed-proof monotonic stop and current signed wall expiry. The handler
uses the Auth read's two-second maximum resource ceiling, never a new two-second
budget after earlier awaits. It checks authority before and after the owner read
and fences a delayed completion. A root proof tolerated by legacy clock-skew
verification after its actual expiry receives no strict read context; existing
legacy method behavior is preserved. No raw delegation token is re-parsed or
exported to establish this context.

Owner failures map to fixed safe gRPC categories; no database error, request body,
pool URL or provider contents may be returned/logged. Current negative facts are
a successful internal fact response, not fabricated authority. Owner timeout,
malformed input, missing caller/proof/deadline and proof expiry fail closed. The
outer bound covers the complete handler read and rechecks the same deadline
after completion, even when a test port ignores cancellation. Local cancellation
does not promise immediate database cancellation; existing transaction-local
timeouts and cleanup semantics remain.

Prerequisite source tests: 103 filtered platform authority tests (including eight
new strict-context cases), two live method-policy network tests and two Auth
proto/policy/actor-field tests pass. The existing Gateway verification transport
suite must also pass after its generated fake service receives the new method.
The prerequisite Auth method returns fixed unavailable until the assigned real
handler is integrated. This stub is not a delivered owner RPC implementation.
Do not publish the prerequisite alone as functioning admission.

The worker must qualify the actual generated Auth server behind production
`guard()` and deadline layers on owned loopback sockets, including denial before
owner dispatch and successful real PostgreSQL reads. Tests cover wrong/missing
caller, signature, audience, method/body/actor, replay, malformed canonical IDs,
missing/elapsed original deadline, strict proof expiry, optional exact binding,
owner failure and late success. All database fixtures require marker/inode-owned
stores, SQLx migrations and cleanup. Read-only socket tests do not certify
Gateway JWT validation, Profile/event policy, signed Concert admission,
revocation SLOs, multi-replica replay prevention or production stream invalidation.

The parent alone publishes the combined reviewed implementation to master.
