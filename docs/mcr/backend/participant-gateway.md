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
