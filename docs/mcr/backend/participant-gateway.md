# C03 Gateway implementation

Status: SOURCE_ONLY / IN_PROGRESS. No Gateway build, native public flow or startup
activation is claimed for this draft. The independent Participant native join at
6767ee8e is a qualified producer checkpoint, not acceptance of this adapter.

Root transferred these exact paths to Participant: new outbound/inbound
`concert_participant/` modules; `tests/concert_participant.rs` and its support;
this document; one additive isolated Participant/Decision proto pass in Gateway
`build.rs`; and one module export in each parent module. Root retains AppState,
configuration, manifests and root/router/startup composition. Backend only.
Base is qualified Participant handoff88de486f.

Construction:

- `GrpcConcertParticipantClient::connect(endpoint, token, tls, signer, release_digest)`
  validates a trusted configured release digest. It never reads release or actor
  identity from browser JSON or headers.
- `concert_participant::router(Arc<AppState>, Arc<GrpcConcertParticipantClient>)`
  returns the three C03 routes using its own typed state. Sealed HttpConcertAccess
  and actual configured Auth/Profile identity clients remain mandatory.
- Root mounts `protect_response` outside transport rejection layers and retains
  the actual outer RetainedScopeLayer to finish only terminal HTTP framing.

GET/PATCH `/api/concert/events/:event_id/me` and PUT `.../me/avatar` retain original
request bytes and authenticated identity/session. Browser mutations require an
allowed single Origin, browser intent and JSON content type, with an 8KiB body
limit. No idempotency key is invented for these optimistic-CAS operations.
Canonical owner reads and signed RPC dispatch share the original wall/monotonic
stop. C03 success disclosure checks exact object keys, nested public fields,
actor/event identity, revisions, preferences and the frozen avatar variants.
Unknown private fields or inconsistent failure code/reason pairs fail closed.

Success and closed typed errors retain the same internal completion token. The
capability never enters browser JSON/headers. Dropping/erroring a body does not
finish a scope. A real root-wide disclosure ceiling and per-DATA-frame expiry
guard are being coordinated with Root; the draft is not ready for public
activation until this is implemented and qualified.

The focused test target uses actual sealed JWT extraction and actual generated
signed client/guard transport with explicitly controlled Auth/Profile/Participant
peers. It covers forwarding, closed conflicts, EOF versus drop, private-response
refusal and pre-dispatch browser/auth/body refusal. It cannot certify a real
canonical-owner/public-stack flow. The reusable controlled identity fixture is
copied from `tests/concert_identity_composition.rs` at88de486f, without its test
cases or synthetic context constructor; production never imports it.

Root granted the exact response-wide disclosure extension after the draft:
ParticipantResponse field4 carries the actual scope ceiling, captured after
success or typed failure with the original scope Arc retained. Gateway refuses
absent, expired or extended ceilings and wraps both forms of response with the
shared DisclosureBody. The original C04 implementation and its three tests move
unchanged except crate visibility to `http/response_disclosure.rs`; the former
private C04 path is a narrow re-export shim. Party-owned C04 route code is
unchanged. Source provenance: Root published07791018 (which contains4d5c044's
qualified body guard and existing http-body workspace dependency). No new
manifest or lockfile change is required.

The new tests additionally cover actual C03 data withheld after an earlier
response ceiling expires and fresh EOF settlement after already consumed valid
data. The same original wall/monotonic bounds are retained. Production fields
are never copied into browser headers or JSON. The earlier draft caveat about a
missing body guard is resolved in source; source/build/runtime qualification is
still pending. Both Gateway and Participant compile/native evidence are required
for this additive internal-wire revision. Phase A/test response initializers
explicitly use zero, which the real Gateway refuses for public disclosure.

First qualification at3f763d4a: both exact Gateway and Concert no-run builds pass;
shared disclosure tests pass3, Participant focused8/integration8 pass, and the
actual three-main native join passes with the new field4 ceiling assertions,
COMMIT/save/reload/ABORT/owner-delivery/cleanup. The Gateway transport target
aborts on its first default-stack test. A diagnostic-only16MiB-stack replay of
that one case passes, which is not acceptance. The follow-up boxes large
identity/RPC/dispatch futures (as existing Gateway clients do) and test setup
futures. Default-stack fresh Gateway validation remains required; no production
stack-size workaround is introduced. The original compiled3f producer inputs
are unchanged by this Gateway-only correction.
