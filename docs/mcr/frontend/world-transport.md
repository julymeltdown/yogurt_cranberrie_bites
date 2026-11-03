# FE16 world transport

Status: **MOCK_READY**, pending integrator review and integration. Real BE10/BE11,
world multiuser, IN01, load, mobile-device and release gates are **NOT_RUN**.
No Phaser, artwork, production lifecycle port, root composition or FE17 controller
is added by this increment.

## Delivery and ownership

- Assignment base: `376cf2127b5ae5b81fa8743293ec9cc0079f6bfd`.
- Worker branch: `codex/mcr-fe-FE16`.
- Isolated checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-world-transport`.
- The integrator alone reviews, integrates and pushes master. This branch is not
  pushed by the worker. Commit ancestry is supplied in the committed handoff;
  an integration/remote SHA remains **NOT_RUN** until the integrator records it.
- Changed application files: `frontend/src/world/network/WorldTransport.ts`,
  `mock.ts`, `websocket.ts`, and `transport/{contract,engine,wire-types}.ts`.
- Assigned tests: `frontend/src/world/network/__tests__/transport.test.ts`,
  `generate-types.mjs`, `browser-smoke.mjs`, and `fixtures/browser.{html,ts}`.
- This document is the only documentation change. No backend, shared contracts,
  session/HTTP/privacy code, central manifests, locks, configuration, replication,
  entities, scenes, assets, routes or production composition changes.

The implementation is newly authored. The existing FE01 schema-checking approach
in `frontend/src/features/auth/model/contract.ts` informed the feature-local
checker; no Bigyeting or backend implementation code was copied. Dependencies are
retained Zod and browser WebSocket/Web Crypto/AbortController APIs. Test tooling is
the retained Vitest, Vite and Playwright stack plus Node built-ins; no dependency
or lockfile change is needed.

## Interface and composition contract

`MockWorldTransport` and `WebSocketWorldTransport` implement the same
`WorldTransport` interface and shared `TransportEngine`. Callers subscribe to typed
state, validated server frames, safe error labels, and `resync-needed` events.
States are idle, ticket, opening, authenticating, live, resync, switching, closed,
and disposed. `connect(resources, signal)` resolves after validated INITIAL
admission; acquisition, opening and authentication can be cancelled. Reconnect is
an explicit new `connect`, with no automatic retries or mutation replay.

Construction requires the existing FE01 `SessionPort`, the exact browser origin,
and a typed `AcquireWorldTicket` function. This function is an adapter seam, not
an implemented HTTP endpoint or new identity source. The future integrator adapter
must use frozen C05 `issueWorldTicket` semantics:

- `POST /api/concert/events/{eventId}/world-tickets`, authenticated through the
  canonical memory session; request body is `WorldTicketRequest`.
- Exact Origin policy and `X-MCR-Intent: browser`; `Cache-Control: no-store`.
- No automatic replay on 401, timeout or uncertain outcome. Do not opt this
  mutation into read-after-refresh retry behavior.
- Propagate existing FE01 `TransportError` kinds and parsed C05 status/reason
  errors rather than throwing raw response data. This permits safe target-only
  ticket-failure handling without inferring authority from error messages.
- Check captured canonical user/event/generation against current identity before
  returning; propagate the supplied AbortSignal and decode bounded HTTP bodies.
- The transport independently validates returned ticket schema, resource bindings,
  protocol, path, 0–30-second TTL and acquisition elapsed time. Canonical Auth
  session-ID ownership stays with the existing session/backend implementation;
  the transport does not parse JWTs or invent another session identity.

Only same-origin `/world-ws` is selected. HTTPS becomes WSS. WS is accepted solely
for exact localhost/loopback HTTP origins used by local development/qualification.
Origin strings containing paths, query, credentials or fragments fail; no remote
URL or server-provided redirect is accepted. C05 does not define a WebSocket
subprotocol: the constructor supplies none and an unexpected negotiated protocol
fails closed. The first application frame is authenticate. Each new connection
and switch obtains a ticket through the injected function. Ticket digests detect
repeated issuance responses without retaining raw credentials; the bounded
instance admits at most 1,024 ticket fingerprints before requiring disposal.
The raw ticket is released after its one authenticate/switch send or cancellation.

No token or ticket is placed in URL, storage, logs, telemetry, query caches or
broadcasts. Production imports never reach `mock.ts` or test harnesses. The mock
has explicit socket controls and a deterministic clock; it never supplies a fake
product roster or automatically succeeds. Its test-only sent-frame recorder
contains synthetic fixture tickets and must not be used with real credentials.

## Validation, ordering and lifecycle

Canonical C05 JSON schemas are imported read-only, including C03 AvatarState and
C06 AvatarConfig/common references. A deliberately limited checker compiles the
reachable assertions into retained Zod custom schemas. Unsupported assertions,
formats, reference targets or types fail closed. `wire-types.ts` is generated from
those exact source definitions; `generate-types.mjs --check` verifies exact output
and source hashes. It is not an independently editable message authority.

Before schema validation, the wire parser rejects binary messages, invalid Unicode,
malformed JSON, escaped duplicate keys, unsafe/non-finite numbers, nesting over 16,
and assembled message sizes above 4,096 client or 65,536 server bytes. Browser
WebSocket delivers already-assembled messages; these checks cannot bound the
browser's internal allocation before delivery. Server frame/aggregate-buffer
limits remain a BE11 obligation. All unknown fields/types/versions fail.

The engine retains only current snapshot metadata and at most 50 roster UUIDs for
wire invariants, not player/position/avatar caches. It checks canonical self and
resource identity, avatar manifest binding, stream/epoch, input acknowledgement,
tick order, unique/disjoint player changes, known non-self removals, complete
snapshot replacement and immutable captured access expiry. Snapshot callbacks do
not retain the old player array. Replication, prediction, interpolation and
rendered roster ownership belong to FE17.

A normal delta gap/replay or stream/epoch mismatch freezes input and emits one
`resync-needed` with GAP or EPOCH_CHANGED. The consumer sends one explicit
`resync(requestId, reason)` using current stream/epoch and last accepted sequence.
A correlated RESYNC snapshot replaces the protocol baseline. While suspended,
schema-valid queued deltas are discarded without advancing players/sequence/ack
or requesting again. Invalid wire still closes. Server GAP/EPOCH_CHANGED hints
use the same bounded path. An unsolicited valid EPOCH_CHANGED snapshot resets the
stream; conflicting pending control/reset transitions fail closed. No reconnect
or recovery scheduling/controller is installed here.

Switches keep the old room authoritative until a correlated ROOM_SWITCH snapshot.
A correlated rejection preserves it. Before any switch frame is sent, documented
ordinary target issuance denials/unavailability or a network/issuance timeout
also preserve the still-current old room/socket, metadata, counters and roster.
The switch promise rejects safely, its pending control clears, and live input
continues. Existing FE01 TransportError kinds and the frozen C05 operation
registry distinguish these cases: PERMISSION_DENIED, RESOURCE_UNAVAILABLE,
RATE_LIMITED, INTERNAL_ERROR, DEPENDENCY_UNAVAILABLE and DEADLINE_EXCEEDED must
match the registered HTTP status. Network/timeout failures may have issued an
unused ticket but cannot have committed target membership without a switch frame.
Unknown/malformed errors, identity/auth loss, invalid ticket/resource/protocol
proofs, expiry, socket loss and conflicting resets remain terminal. A bounded
issuance timeout aborts only its ticket request and ignores its late result. Once
the switch frame is sent, uncertain failure or response timeout closes the
connection rather than guessing which room committed. Explicit retries require
a new request ID and another ticket request; there is no automatic retry.
Results arriving before the switch ticket
has actually been sent are rejected. Event/map/manifest changes require a fresh
connection. Request/stream identifiers cannot repeat within a connection and are
bounded at 1,024 each. Controls time out rather than retaining pending references.

Local ticket acquisition and opening/authentication each have a 3,000-ms bound.
Recovery/control completion uses the same conservative bound. A pending explicit
resync is limited to one and spaced by at least 1,000 ms. MAP_VERSION_MISMATCH and
BACKPRESSURE hints remain terminal. Captured access expiry is enforced separately
from the 10-second application pong and 20-second no-ping watchdogs; heartbeat
never renews authorization. `clientPong` requires the outstanding matching ID.

Client sending enforces the canonical input/all-message token buckets, unit input
vectors and increasing safe input sequence. There is no application replay queue.
Native buffered bytes are capped at 256 KiB and conservatively counted buffered
frames at 64 until the native buffer fully drains. Overflow closes the connection;
reconnect starts without queued movement or one-shot interaction replay.

Every attempt has its own generation and AbortController. Session changes abort
immediately. Late tickets, callbacks and timers cannot mutate a later generation.
Close, invalid data, timeout, revoke, disconnect and disposal clear socket listeners,
timers, pending state, current metadata, roster IDs and references. Subscriber
exceptions cannot interrupt cleanup; reentrant lifecycle transitions are fenced.
C05 validation errors report their canonical code locally. Browser `close()` only
accepts 1000 or application codes, so standard wire error codes (1003/1007/1009)
map to application 4400 for a client-initiated close; safe error events retain the
original classification. Documented numeric server close codes are exposed through safe error events.
Server close reasons/payloads are never copied into
errors or logs. Closing handshakes are browser-managed after listeners are removed.

Client checks support safety/UX; server admission, capacity, live policy/session
checks, inventory, privacy and authoritative movement remain server obligations.

## Source provenance

Frozen C05 v1.0.0 and the C03/C06 references are read at the assignment base.
`transport/wire-types.ts` embeds individual `source_hash` values for its four
schema inputs. Additional SHA-256 provenance and all fixture hashes are recorded
in `/Users/lhs/Desktop/mcr-build/fe16/fixture-source-hashes.txt` and the verification
receipt. Canonical oracle: `scripts/mcr/validate-realtime.py`; it is not imported
into production and does not claim server/runtime acceptance.

Additional frozen source hashes (SHA-256):

| Input | source_hash |
|---|---|
| C05 world schema | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |
| C05 policy | `3e77e21b036386803c7d5bff0145c434f111ba51b861219a211e853a43d0628b` |
| C05 HTTP operation registry | `400d28e861ca5a87c54fde447f77b1680f7ea58e3047ee43b46db25816af6da0` |
| C05 Python oracle | `0934aedbd1c7004398b4521c6f304271e104a6e7d3b5119d8118ff0b5ae7d40e` |
| Sorted fixture manifest, including index (25 files) | `497d4e6bd1783cbfd702f576934a52b5d977cce36c6560b66fe4bf040a82db00` |

The fixture-manifest hash covers UTF-8 lines `repository/path SHA256\n`, sorted
by path. The index lists 24 fixtures; the index itself is the 25th hashed file.

## Verification and evidence

Toolchain: retained Node **24.22.2**, `corepack pnpm@11.7.0`. Evidence directory:
`/Users/lhs/Desktop/mcr-build/fe16`; earlier failing/intermediate logs are preserved.
The final receipt under `verification-03/` records exact commands and exit codes.

The common mock/native-adapter suite has **158 passing tests**; the full frontend
suite has **512 passing tests across 13 files**. It consumes all five canonical traces, all seven
wire rejection fixtures, and the privacy projection frames. It additionally tests
handshake deadlines, pending/open/active abort, canonical session replacement,
stale tickets/callbacks/timers, incompatible protocol/resources, numeric/schema and
roster failures, expiry, resync freeze/one notification/queued discard/timeout,
switch success/rejection/pending-ticket cancellation, ping/pong, send gating,
byte/frame/rate bounds, fresh-ticket reconnect and complete cleanup. Parent review
regressions additionally cover pre-send ticket denials/outages/timeout preserving
old input and stream counters, fresh-ID/fresh-ticket retry, terminal authority or
protocol failures, and distinct post-send timeout/close/reset behavior.

Installed Chrome **156.0.8078.80** uses a fresh owned context and an ephemeral
loopback fixture server with explicitly test-only entrypoints. Actual native
WebSockets replay all five canonical traces: movement/heartbeat, resync, room
switch rejection/success, epoch replacement and revoke. Additional Chrome checks
cover detected gap recovery, queued-delta discard, handshake timeout, explicit
fresh-ticket reconnect, session cancellation, closed sockets, target-ticket denial
preserving old socket/input, explicit fresh-ticket retry and terminal post-send
timeout, safe URL/storage/log
behavior, and built-production exclusion. The server is not a backend or substitute
for BE10/BE11. No existing browser profile or user server is used.

Final commands include frozen install, full frontend tests, strict build, lint,
generated type drift, C08 guards, all contract checks, plan validation, lane and
exact-assignment boundaries, and whitespace validation. Build retains the existing
538.34-kB entry chunk warning; FE16 is not imported into that production entry.

The persistent goal stays active through the committed handoff and integrator
review fixes. MOCK_READY is not task acceptance, production integration, real Auth
or world qualification, mobile/device evidence, capacity proof or release readiness.
