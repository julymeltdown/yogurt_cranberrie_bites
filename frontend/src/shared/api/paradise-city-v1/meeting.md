# M01 public Meeting wire client

Status: implementation and targeted source qualification complete (45 focused tests, scoped strict types and lint). Production dispatch and service acceptance remain separate integration work.

Exclusive scope is this document, `meeting.ts` and `__tests__/meeting.test.ts`. Source base is preserved `f9a139f6ef192d5056c631af976b8b4b68baf7f4`. The original PI08 implementation, evidence and history are unchanged. This leaf supports VENU-FE-M01 and related original QA06 readiness; it does not complete the whole original feature.

## Source contract

Initial canonical input `574b3e5a2a4533d70da3236b2c42c465a32e34a4` came from `9835999080f3a8ee14af3d37cbc06e6e7f3310f0`. The corrected contracts are now canonical at `0981774d48eab79c15666ef27612310d470b4f69`, from corrective producer source `89fa1ac6ed5bba2c0a9d21cf697c006e92f45b7b`:

- `backend/docs/api/paradise-city-v1/meeting.openapi.json`: SHA-256 `08503641a551fbacaf134ecab71bbce1102c1f7da4ba560e5d15311694e0c2de`.
- `backend/proto/venue/v1/meeting.proto`: SHA-256 `957a19de1e365b1be11c55fb7599408d6bbccaff357c49a4ee307c5923acbde7`.
- `docs/mcr/contracts/paradise-city-v1/meeting.md`: SHA-256 `4dac75747d13a443a3147bcd6abb5912c2ddabec5e3aae010b979812983cdce0`.

These are immutable public contract inputs read through Git, never runtime imports of backend implementation. Producer qualification is structural/source only; its overall source round retained an unrelated whitespace failure. No protoc, full OpenAPI semantic, generated client, HTTP/service/native or physical-device qualification is inferred.

The corrective source supplies closed canonical error envelopes and explicit status/reason tables, and requires invitation detail's reviewed-triple query parameters. `decodeMeetingError` validates that envelope and exposes only its safe reason. The initial flat shape is rejected. Producer regression Rust tests remain NOT_RUN and current Rust source is UNBUILT; source correction is not service qualification.

## API and ownership

`meetingSchemas`, `MeetingDtos` and named DTO aliases expose closed runtime validation. `decodeMeeting(name, unknown)` validates JSON-only input and returns immutable parsed values; malformed data raises a fixed `MeetingClientError` without input or server-text details. Public revisions and sequences are positive safe integers; cursors are canonical decimal strings including zero. No provider revision string or ART/SiteplanIdentity is coerced into authority.

`meetingRequests` builds the eight exact operations: `resolveContext`, `listInvitations`, `getInvitation`, `createInvitation`, `decideInvitation`, `listMessages`, `sendMessage`, `getCommand`. Requests are frozen and locally sealed. They carry protected access, domain error family, `cache: 'no-store'`, `retry: 'never'`, canonical path/query/body, and a response decoder. GET has no body, Content-Type or idempotency key. POST takes a caller-owned nonnil UUID key; the canonical transport serializes the immutable JSON body and supplies its standard headers. Builders create no requests or side effects by themselves.

MESSAGE resolution/list/send/status permits `{selectedFriendId}` with optional absent/null reviewed landmark. Opening its context is GET only; only explicit `sendMessage` can perform the first conversation write. Optional invitation association is not permission. Invite detail/create/decision requires an actual provider `{landmarkId, catalogRevision, landmarkRevision}`; decision includes `expectedRevision` CAS and explicit ACCEPT/DECLINE/CANCEL. Client syntax checks cannot establish that a landmark was reviewed or that Friends/Auth/event access is current.

`createMeetingClient(transport, lease).execute(descriptor)` dispatches once through an injected `MeetingCanonicalTransport`. The lease supplies the actual session scope, abort signal, full Auth/privacy-current fence and real clock. Late/stale results are rejected and actual returned deadlines cap disclosure. The client stores no private cache, creates no Auth/session/query client, and performs no fetch, refresh, retry, cancellation command or auto-send. An uncertain mutation retains the original `commandKey`; explicit `getCommand` uses that key and original selection. UNKNOWN is not replay permission. Receipt effects are historical snapshots; fetch current invitation before another decision.

## Dispatch gap and integration

No default transport is implemented. Passing null fails with DEPENDENCY_UNAVAILABLE. Meeting operation IDs are separate from the closed generated C04 type; none is cast into that registry. Current canonical `HttpClient` cannot dispatch these descriptors without Root-owned registration and the actual canonical error/operation registration. Root must map registered operation IDs and bind the canonical transport; inventing a flat-to-domain mapper or bypass fetch is prohibited.

Adapter owner204 owns normalized consumers and provider-to-wire revision mapping; feature owner40fde owns capability ports. Root owns central registration/provider/mount. This leaf does not duplicate their controllers, caches or authorities. Message pages enforce increasing sequence, unique IDs and a cursor at least the returned watermark; changed conversation IDs, cross-page dedupe, gap handling and cache invalidation remain the actual adapter/controller's responsibility. No synthetic day-end timezone, retention period, Friends grant or physical source is supplied.

## Validation

SOURCE01 at `6a23d219a4073248bff54152073f02ac04091b90` passed all 42 focused tests, scoped lint and driver syntax. Its overall result is FAIL: strict TypeScript found one reason-array type narrowing error. Raw private receipts remain unchanged. Actual FULL was `1793845695.965847`; no children, dependency link or scratch remained. The type annotation is fixed, and two added cases cover foreign message senders and expired pending-invitation/inbox disclosure. SOURCE02 subsequently qualified that corrected candidate, as recorded below. Private controls/evidence are outside the published three-file scope; earlier HUD results provide no M01 credit. Meaningful tests cover contract route/shape compatibility, malformed closed DTOs/query/UUID/revision/text, no-write MESSAGE opening, required INVITE triple, explicit idempotent send/status, UNKNOWN without retry, stale privacy/abort callbacks, safe errors and immutable request bytes.

SOURCE02 at `75fd73d110195be5f633ed46c97dac8a9bd969f4` passed 44 focused tests, scoped strict TypeScript with zero diagnostics, deny-warning lint and driver syntax. Literal FULL was `1793845827.214078`, with no children/link/scratch. Private `source02/FULL.json` SHA-256 is `e6d98dfd1f5cc120e2296f35dc4fd1bcfbf98e5c4cb8f654d961c4a5947d08d5`; `source02/output/source.json` is `6e36eebf8377a86856ce407f2aeca7e858b04265d9bfd98febbc5819e0360ebd`. The cumulative resource maxima were NEW 501,355,953 bytes, WHOLE 6,889,254,912 bytes, eight simultaneous owned processes and 720,076,800 bytes RSS. Minimum available disk was 23,883,415,552 bytes. The whole 16 MiB prep/document allowance was already carried; no deletion credit or reset applies.

Before final handoff, the client gained a final current-scope check after the caller-provided clock callback, with a 45th test. This prevents synchronous privacy revocation in that callback from disclosing a late result. SOURCE02's pass remains pinned to its earlier source; SOURCE03 qualified this last implementation change, as recorded below. The test clock is a deterministic unit fixture, not a product retention/timezone default.


SOURCE03 at `6e4e53457f997f9253775ef693a5771847864549` passed all 45 focused tests, scoped strict TypeScript with zero diagnostics, deny-warning lint and driver syntax. This is the final implementation/test pin; the subsequent change updates this document only. Literal FULL was `1793846112.259768` (2026-11-05 02:35:12.259768 UTC); workflow, reservation and owned children were zero, the borrowed dependency link and private scratch were absent, and guardian/test exit codes were zero within the fixed deadline. Cumulative NEW reached 509,793,713 bytes inside the single 640 MiB cap, WHOLE remained 6,889,254,912 bytes, peak simultaneous ownership was six processes / 705,396,736 bytes RSS, and minimum available disk was 23,853,510,656 bytes. Earlier rounds, the entire prep allowance and guard reserves remain carried without reset or deletion credit.

Private SOURCE03 evidence is retained at `/private/tmp/mcr-venu-fe-m01-40c576-0215/source03/`: `FULL.json` SHA-256 `f9ce0a580af2f6b346d72719a4d6ebff9717a664b6692a1b2582cea2bfac52b4`; `output/source.json` SHA-256 `6e36eebf8377a86856ce407f2aeca7e858b04265d9bfd98febbc5819e0360ebd`; `output/vitest.json` SHA-256 `c4cf7c91a14b20fead627b781ca3e1e4502f7f7bde0055227cd69cbca758ad21`. The frozen private readset SHA-256 is `b4c944ffd1b1edb42bb1e59795c4432bfab091e7f062b9354203f1586492afa9`. These are local audit references, not shipped runtime dependencies or an extra published file. No frontend-wide, native, browser, service, physical-device or whole-venue acceptance is claimed.
