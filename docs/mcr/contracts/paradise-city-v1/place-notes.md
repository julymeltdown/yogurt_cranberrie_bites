# VENU-BE-N01 place notes — contract 1.0.0

Status: SOURCE_AUTHORED, qualification NOT_RUN. This additive N01 contract is
separate from the 112 original task IDs and 61 IA entries. It is not service,
native, physical-place, production-provider or expanded-product acceptance.

Authority: the human venue amendment source
`e8f576d7702483f6f1bbfa14e0f5134c4c5051cb:docs/mcr/venue-feature-amendment-2026-11-05.md`
and UI reference packet `d3bf31f6481ed7f43e6c50d8c885c89de64ee0d5`, especially
Photo 10 SHA256 `1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63`.
The publisher subsequently delivered the amendment as `686752254c805bbe8ec7a38d0be276513205dc06`
and corrected UI reference packet as `6ff3e481cfcf820c2e14a9a670ae2dc3bffc9afb`,
included in verified master `1907016b6aa41c17998acab691a83e99d0a80076`.
The corrected packet's contextual MESSAGE cases supersede the earlier packet;
the original supplied Photo 10 bytes remain unchanged.
No example location, identity, text, count, time or illustration anchor is live data.
Root1fc assigned the exact three new N01 contract files to developer213 after the
former contract custodian returned them. Root153 owns frontend wire mapping;
40fde owns the frontend capability projection. Neither projection is another wire.

## Operations and boundaries

Let `L=/api/concert/events/{eventId}/venue/landmarks/{landmarkId}/notes`.

| Operation | Method and path | Result |
| --- | --- | --- |
| listVenuePlaceNotes | GET L | viewer-filtered roots, current exact visible root count |
| createVenuePlaceNote | POST L | original safe command receipt; 201 when succeeded |
| getVenuePlaceNote | GET L/{noteId} | root plus first reply page |
| withdrawVenuePlaceNote | DELETE L/{noteId}?expected_revision=N | safe receipt; root withdrawal removes whole thread |
| listVenuePlaceReplies | GET L/{noteId}/replies | visible reply page, current visible reply count, parent |
| createVenuePlaceReply | POST L/{noteId}/replies | safe receipt; inherited audience and absolute parent expiry |

Lists accept only `limit=1..50` (default20) and optional
`before={positiveCanonicalEpochMicros}.{canonicalLowercaseNonnilUUID}`. Sorting
is descending `(created_at,id)`. A cursor is only an ordering position, never
authority, an unfiltered count or permission to read a cached item. Changing
viewer/event/place still runs complete current authorization. Every private
response is `Cache-Control: no-store`; there is no public Internet/archive feed.

Mutation headers require one canonical nonnil UUID `Idempotency-Key`; POST is
exact `application/json`, maximum8192 raw bytes. GET/DELETE have no body or
Content-Type; DELETE has exactly one expected_revision. Unknown, duplicate and
null fields, unsupported enums, invalid/cross-event IDs, trailing paths and
unrecognized query fields fail. Client revision values are JS-safe positive
integers below the terminal maximum. `text` is nonblank, at most1000 Unicode
scalars and4000 UTF-8 bytes; newline is allowed, other controls are rejected.
The frontend must also reject unpaired UTF-16 surrogates before encoding.

The full schemas and deterministic examples are in the adjacent OpenAPI file.
The protobuf is a DTO family, not an unauthenticated alternate RPC. Existing
signed canonical Product RPC, one actual Root scope, one Auth identity and
actual Gateway EOF delivery remain mandatory. Root retains route registration,
exports, builds/codegen, manifests, numbered migrations and composition.

## Audience, replies and truthful time

`EVERYONE` means currently authenticated, eligible participants in the SAME
event with current actor and author identities. It does not mean anonymous,
all events, all site visitors or a location-sharing audience. `FRIENDS` adds
actual current mutual-friend permission with the note author, except an
author's own view. Following, social-handle consent, an invitation, party
membership, art examples and cached flags are not friendship. Both block
directions, suspension, deletion, event eligibility, current local content
moderation and actual landmark review/closure/validity apply to every list,
detail, reply, visible count/marker, linked entry and mutation/replay. Dependency
unavailable is503, never an allow or an empty fabricated successful snapshot.

Replies are one level only. They have no audience/expiry selector; they inherit
the exact parent audience and absolute expires_at. A reader must pass BOTH the
current parent's permission and current reply-author permission. Thus Friends
replies use the intersection of both authors' current authorized audiences.
The server never broadens the parent or discloses a hidden reply count. An
expired, withdrawn, moderated, inaccessible or invalid-place parent removes
its full thread and all its marker/count contribution. This intersection is
an explicit technical privacy rule, not an extra claimed human decision.

`ONE_HOUR` uses the actual database creation clock + exactly1hour.
`VENUE_END_DAY` uses an actual approved venue/event policy's timezone and
absolute day-end boundary. No browser midnight, server-local timezone, fixed
18:30/24h cutoff or illustration time is used. Missing reviewed day-end policy
disables that choice with503; it does not disable independently available1h
notes. The created absolute expiry is immutable, including retries. A reply
created later still expires at the parent's original deadline.

Reads carry actual `server_now` and `access_until`; the latter is the earliest
Root/authority/landmark/note/meeting-entry disclosure ceiling. Clients withdraw
bodies and counts at expiry/revocation and invalidate user+event-scoped caches
on logout/account/friend/block/content changes. Counts must come from an
authorized list or later qualified count producer; static geometry may cluster
those permitted per-landmark counts locally. There is no unfiltered total,
private count endpoint, GPS claim or server-generated example marker.

## Actual landmark and meeting dependencies

`landmark_id` is an opaque actual L-provider-issued1..128-character unreserved
ASCII identity, excluding `.` and `..`. It is not invented from an art label or
assumed to be a Place UUID. `landmark_selection` carries the L-owned tuple
`{landmark_id,catalog_revision,landmark_revision}`; catalog and individual
landmark revisions are distinct. The actual grant additionally binds event,
provider_id, source_revision, map_id/map_revision, venue policy and validity.
All of that actual tuple is persisted and rechecked. An adapter to an existing
physical Place must explicitly resolve its current UUID/revision/provenance;
the note contract does not manufacture that binding. L owns the shared ABI;
N01's typed projection is not a competing reviewed-place service.

Meet Here is an actual M-authorized composer entry, not an invitation write,
acceptance, physical arrival or location grant. `meet_here=null` when the actual
M adapter cannot authorize that entry or is unconfigured; no inert success is
reported. The M owner reauthorizes the selected landmark and recipient when the
user proceeds. Contextual MESSAGE is owned by M and is neither vetoed by the
historical no-DM rule nor silently created by viewing a note.

## Receipts, unknown outcomes and privacy cleanup

Create/reply/withdraw use the BE15 actor+event+operation+key receipt and keyed
canonical fingerprint. Same input/key returns the original safe result without
another effect; changed input/key conflicts409. Resource identity is stable for
the original key. No receipt or outbox stores text/profile/body snapshots;
safe references contain resource UUID/revision only. Receipt expires_at is
receipt TTL, never an extension of note expiry. Status uses the existing
`GET /api/concert/events/{eventId}/commands?operation_id=...&idempotency_key=...`
with one of the three N01 mutation operation IDs. Actual owner authorization
must be routed to the N01 status producer; registration alone is not a mounted
handler. Even after note expiry it returns only the original safe own command
metadata, and never recovers private bodies. A replay may refuse current
inaccessible context; status is the reconciliation path. Unknown remains
unknown with the original key and draft retained; no autosend/new key or
fabricated FAILED result. Explicit expired receipt stays retired, not executable.

Withdrawal scrubs the body and, for a root, every reply body in the same actual
transaction. Expired content is immediately invisible; a bounded resumable
expiry purge scrubs persisted bodies. Account-deletion and moderation/event
cleanup need real owner composition, not a best-effort detached timer. No raw
private content is emitted to audit, analytics, error messages, notification
or command/outbox payloads. There is no public note archive.

Proposed privacy policy is earliest feasible scrubbing at expiry, withdrawal,
moderation or account removal, with safe BE15 receipt metadata retained under
the existing receipt policy. Actual scheduler cadence/maximum persisted-body
retention, event close policy and approved operational venue-day-end setting
remain provider/operator inputs. N01 does not fabricate an indefinite raw-body
retention permission or invent a precise purge SLA. These gaps are routed once
to Root/Main; immediate access withdrawal is mandatory meanwhile.

## Qualification and remaining integration

Source authoring is funded by ordinary `VENU-BE-N01-PREP128-0122` through
2026-11-05T02:10Z. This is not a static-check, Cargo/native/database/browser or
physical-provider admission. At initial authorship, L/Friend/day-end/M and
prepared-native Notes read-retention interfaces are required injected owners;
there is no qualified configured physical/friend/policy producer. No permissive
fallback is installed. Reads fail503 before protected content when actual read
retention is absent: committing the read before HTTP EOF would release the
note-row locks and is not sufficient. Root/shared SDK owns the new typed child,
durable intent/ACK, Composite settlement and actual prepared rollback/recovery
contract; no old Social/Profile/Evidence child is retagged as Notes.

Developer-owned source/schema/native/public/restart/privacy/fault tests and
fixes remain required. All unexecuted checks are NOT_RUN. Contract qualification
may be published independently with its exact commands/results/source hashes;
it does not certify mounted routes, actual providers or the full N01 feature.
BE36 remains wholly unfinished behind this explicit priority transfer. The
historical incompatible BE24 BLOCKED persistent goal is preserved unchanged.
