# C05 realtime contract v1 — review candidate

State: **CONTRACT_READY**, pending independent review and freeze. The JSON schemas,
policy, deterministic fixtures and offline oracle define the proposed producer
and consumer agreement. No WebSocket server/client, Auth session check, ticket
store, real room admission, load test or multiuser acceptance is implemented here.
The HTTP registry is an input for C07's final inherited OpenAPI assembler, not a
second independently shipped SDK. C03 core v0.1 and C06 metadata v1 are inputs.

## Wire representation and limits

One UTF-8 text message contains one JSON object. Version is exactly `1.0.0`;
unknown types, fields, versions and duplicate keys fail. Binary data closes 1003;
invalid UTF-8 closes 1007; messages above 4,096 bytes client-to-server or 65,536
bytes server-to-client close 1009. Limits apply to the assembled uncompressed
message, before parsing. Disable per-message compression in the first server.
Bound the WebSocket library's frame and aggregate buffers too; an application
check after unbounded allocation does not meet BE11. Other invalid wire input
closes 4400. Reject non-finite numbers, invalid Unicode scalars and nesting over
16. IDs are canonical lower-case UUIDs, counters are safe integers, UTC timestamps
end in Z. Schema formats and semantic checks are required, not optional hints.

The schema uses lowerCamelCase for world messages and the existing snake_case
convention for new HTTP DTOs. `WorldPlayer.avatar` references C03 AvatarState,
which references C06 AvatarConfig; there is no duplicate avatar definition.
World positions are virtual pixels with a wire ceiling, not physical coordinates.
Actual map bounds, collision, speed, interaction distance, item allowlists and
display-name validation remain server responsibilities. Theme never changes
room/map geometry or membership. C06 fixtures are synthetic metadata, not approved
character artwork; production consumers use reviewed derivatives of source pack
revision 3 and a separately reviewed layer/atlas contract revision.

## HTTP ticket and room operations

`GET /api/concert/events/{eventId}/rooms` returns `WorldRooms`: a bounded stable
room catalog, capacity ceiling and AVAILABLE/FULL/UNAVAILABLE hints. The default
room must appear exactly once. There is no private occupant roster or exact live
occupancy counter. Availability is advisory; only admission reserves capacity.

`POST /api/concert/events/{eventId}/world-tickets` receives `WorldTicketRequest`
and returns `WorldTicketResponse` with status 200. Verify bearer identity, **live
canonical session**, participation, event phase, suspension, requested room,
current map and avatar manifest versions. The JWT must contain the canonical
session ID and Auth `CheckSession(user_id, session_id)` must confirm it is active;
a valid signature or optional absent `sid` is insufficient. Concert's authenticated
workload/caller permissions must be added in BE05; impersonating an existing
service is prohibited. An unavailable policy dependency fails closed with 503.
Use C03's exact Origin allowlist AND `X-MCR-Intent: browser` guard before issuing
the ticket; it must cause zero downstream mutation on denial.

Both operations and all error responses are `Cache-Control: no-store`. The
[operation registry](../../contracts/realtime/v1/operations.json) enumerates
statuses/reasons using C03 errors. Forbidden origin uses C03 OriginErrorResponse.
Unavailable/hidden rooms use the same safe RESOURCE_UNAVAILABLE response. A room
already known full can reject issuance with 409 RESOURCE_UNAVAILABLE, but a ticket
never promises capacity. Stale asset versions use 409 MAP_VERSION_MISMATCH; closed
events use 410 EVENT_READ_ONLY. Ticket requests have no automatic replay on 401,
timeout or uncertain outcome. A new connection attempt obtains a fresh ticket.
GET may use FE01's opted-in once-after-refresh read policy after FE09 qualification.

Generate a cryptographically random 32-byte opaque ticket, base64url without
padding (43 characters). Store only a digest and bind it to canonical user,
session ID, event, room, map ID/revision, avatar manifest ID/version and the verified
access JWT's expiration. Its expiry is the earlier of issuance+30 seconds and
that access-token expiration; response time/expiry
must reflect that interval. No token or ticket enters URL/query, browser storage,
analytics, access logs or error detail. Only the issuance response and the first
authenticate/switch message carry the ticket. Hash/reuse checks and ticket issue
rate limits must operate across connections, not only per socket.

## Connection and authorization lifecycle

Connect to same-origin `/world-ws` (WSS in production). The handshake enforces the
configured exact Origin allowlist, rejecting absent/null/untrusted origins for
this browser-only endpoint. The first application message must be `authenticate`
within three seconds. Before successful admission, send no snapshot, presence or
private state and accept no input/subscription. Failures close 4401/4403/4408/4500
as appropriate; pre-admission failures use close only because there is no stream.

The server atomically consumes the one-use unexpired ticket before admission.
Even denied/full admission does not restore it. Verify every binding and recheck
live Auth/Concert policy at consumption; never trust client-supplied user identity.
An atomic room-owner transaction/reservation then creates exactly one active
membership lease for `(user_id,event_id)`, bound to the current session and
connection. This v1 contract chooses newest authorized connection takeover:
a simultaneous connection replaces the old lease and sends CONNECTION_REPLACED
to the old socket before it can receive further input/presence authority. It does
not revoke the underlying Auth session. The room counts the user's membership
once and never emits duplicate WorldPlayer IDs. Serialize admission, takeover,
and room switching at the room owner so a reconnect cannot duplicate capacity
or leave the old socket controlling the new lease. Real concurrent takeover and
session-revocation races remain BE11/BE13 obligations, not oracle evidence.
CONNECTION_REPLACED disables automatic reconnect on the old client; taking back
control requires an explicit user action, avoiding two tabs repeatedly evicting
each other.

Authorization is revalidated no less frequently than every five seconds using
server monotonic deadlines, and sooner on known logout/block/suspension/closure.
Fail closed when the lease expires or validation is unavailable. The verified
access token has an independent absolute deadline and cannot be extended by
heartbeat/input. `accessExpiresAt` is that captured JWT expiration, not permission
to skip live session checks. Inherited CheckSession returns active/current roles,
not an expiry timestamp; this contract does not invent an additional upstream
response field. Access expiration requires a new authorized ticket/connection
using the refreshed memory bearer. The separate five-second live check detects
earlier session logout, refresh-family expiry or revocation; a valid JWT alone
never preserves a revoked session.
Replacement snapshots on the same connection preserve its captured access
deadline exactly, including resync, room switches and epoch replacement.
Revoke removes membership and subscription, emits at most one sanitized `revoke`
when possible, clears client world/private state, then closes. Losing the revoke
frame does not retain authority. Never leave an expired connection merely frozen
while continuing to reveal other users.

## Snapshots, inputs and delivered streams

The first `snapshot` is INITIAL with null requestId, a fresh streamId, streamSeq=0,
ackInputSeq=0, room epoch, server tick and the exact ticket resource versions.
It contains self once and every permitted currently visible player once, at most
the configured capacity (ceiling 50). Every avatar matches the advertised manifest.
Public player projection has no email, exact age, handles, grants or physical
meeting state. The snapshot and subsequent deltas come from one consistent room
baseline: changes concurrent with snapshot capture must be included or queued
after it, never lost between separate reads/subscription registration.

Client `input` carries streamId, epoch, strictly increasing input seq, normalized
axis vector of length at most one (1e-9 rounding tolerance), and the optional
`interact` intent. It never supplies authoritative coordinates. Gaps in input seq
are permitted for coalescing before send; duplicate/decreasing values are rejected.
The server acknowledges only inputs consumed by its simulation. On every new
stream, discard old pending inputs, reset input seq/ack to zero and send the
current held-input state afresh. One-shot interactions are never replayed.

`delta` has a full public player upsert list and UUID removal list. Lists are
unique and disjoint; removals must refer to known non-self players. Membership
removal of self uses revoke or a room-switch snapshot. Apply each valid delta
atomically, with no partial roster mutation on an error. Retain a single current
roster; do not keep an initial snapshot.players copy that preserves later-blocked
people. Drop the consumed ticket after admission. Revoke also clears snapshot
metadata and pending control state. serverTick cannot decrease
within a room epoch but may skip simulation ticks. Input acknowledgements cannot
decrease or exceed the greatest sent input for this stream.

Allocate streamSeq **after** viewer authorization/filtering and before enqueueing
the final immutable message. The next delivered delta is exactly prior+1. A
filtered-out room change allocates no number. Motion may be coalesced only before
sequence allocation; never overwrite/drop an already numbered frame and continue
the stream. `filtered-viewers.json` has three viewers: one sees every motion, two
block each other. Different delivered counts retain contiguous per-viewer sequences
while their serverTick values differ. Blocking is bidirectional; remove already
visible blocked users before further presence/interaction delivery.

Ping/pong, switchRejected, resyncRequired and revoke are ordered control messages
outside the delta sequence; they never increment streamSeq. Except ping/pong,
controls bind the current stream. Send one application ping every ten seconds,
requiring its matching pong within ten seconds. Only one may be outstanding.
Native WebSocket ping/pong is separate and does not substitute for this browser
application heartbeat. A heartbeat does not renew policy or session authority.

## Recovery, room switching and backpressure

A sequence gap, epoch mismatch, foreground resume or explicit resyncRequired
freezes input/prediction until a replacement snapshot. Keep one resync request
pending, with a fresh requestId, current stream/epoch and last applied streamSeq;
server minimum spacing is one second. A correlated RESYNC snapshot creates a new
stream and replaces the whole roster atomically; never append it to stale players.
While waiting, schema-valid deltas already queued for the old stream are discarded
without applying players, sequence or input acknowledgements. They do not cause
an additional reconnect loop; the server could have sent them before it received
the resync request. Invalid wire data still fails validation.
An unsolicited EPOCH_CHANGED snapshot also creates a new stream, requires a new
epoch and null requestId, and preserves event/room/map identity. Concurrent pending
control requests during such a reset fail the connection rather than guessing
which request won. Map/manifest changes require closing 4409, refetching assets
and obtaining a fresh ticket, rather than interpreting old coordinates with new
geometry. Obsolete stream frames never update current state.
MAP_VERSION_MISMATCH and BACKPRESSURE resyncRequired hints are terminal for the
current connection: clear its state and reconnect through fresh authorization
(and refetched assets for mismatch). They cannot be satisfied by an in-place
RESYNC snapshot with the stale resource versions.

For `switchRoom`, first issue a fresh ticket for a different room in the same
event with identical map ID/revision and avatar manifest ID/version. Asset changes
require a new connection as above. Send one request with a new requestId and
current streamId. The old lease,
roster and stream remain authoritative until target admission commits. A failed
target reservation yields correlated switchRejected and preserves the old room;
the consumed target ticket is not reusable. Success atomically transfers the
active lease and emits a correlated ROOM_SWITCH snapshot with a new stream,
target room epoch and complete roster. No interval may count/authorize the user
in both rooms. Account/event changes create a new connection, never switchRoom.
Control request IDs and stream IDs are not reused within one connection.

The [policy](../../contracts/realtime/v1/policy.json) caps the outgoing queue at
64 frames AND 256 KiB, input at 30/s with burst 60, and all client messages at
40/s with burst 80 (separate token buckets). Server simulation 20 Hz and delta
at most 10 Hz are tuning targets. When queued data cannot remain bounded, stop
that stream and close 4409; a resyncRequired hint is best-effort only and cannot
justify another unbounded queue. The client discards queued commands and resumes
through a fresh ticket/snapshot, never blind delta or HTTP-command replay. Actual
capacity of 50 per room and 200 across four rooms is **NOT_RUN** and remains QA08.

Close reasons contain only a stable safe label, never payloads or credentials,
at most 123 UTF-8 bytes. The 1003/1007/1009 standard codes, 4000–4999 application
range and 125-byte control-frame limit (two bytes reserved for close status) follow
[RFC 6455 §§5.5 and 7.4](https://www.rfc-editor.org/rfc/rfc6455.html).

## Evidence and boundaries

`python3 scripts/mcr/validate-realtime.py` validates 24 indexed fixtures, including
explicit malformed/oversize negative payloads. `python3 -B -m unittest discover
-s scripts/mcr/tests -p test_realtime.py` covers wire limits, decoding, schema refs,
identity binding, input/stream ordering, resync, switch failure/success, revocation,
heartbeats and privacy projection. The offline trace oracle validates delivered
transcripts; it does not implement real ticket issuance, clocks, rate limiting,
network fragmentation, queues, room simulation, Auth calls or DB concurrency.
Producer/consumer runtime tests must reuse these fixtures in BE11–BE14/FE16–FE17;
real reconnect, stale policy, concurrent ticket reuse/capacity/switch tests remain.
