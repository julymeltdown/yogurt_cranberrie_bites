# Paradise City venue policy freeze v1

Status: **FROZEN** for VENU-BE-L01, VENU-BE-M01 and VENU-BE-N01 implementation.
Date: 2026-11-06.
Scope: the policy inputs consumed by the location (L), contextual meeting (M) and
place-note (N) families. This document decides behavior; it does not implement it.

Authority: the human-authorized [venue amendment](../venue-feature-amendment-2026-11-05.md)
and [Progressions](../progressions.md), bounded by the existing contracts
[location](location.md), [meeting](meeting.md) and [place-notes](place-notes.md).
Where this document narrows a previously open input, it supersedes the earlier
"provider input required" wording for that input. It changes no wire field name,
enum value or protobuf message.

## Frozen rules

1. **Identifiers are never capabilities.** A session, event, selected-friend,
   landmark, invitation, conversation, note or cursor identifier is only a
   lookup key. Possession of it never grants a read or a write. Every protected
   read and effect re-evaluates the complete current authority set.
2. **Friendship is an eligibility gate, not an audience.** The only venue friend
   predicate is `VENUE_FRIEND_V1` below. A friend sees another person's location
   only when the owner has independently selected that friend in an active
   sharing lease. There is no "all friends" location audience.
3. **A location request does not grant access.** ASK LOCATION creates metadata
   only. Approval can add the requester to an already active, unexpired lease;
   it never enables sharing, resumes paused sharing or extends a lease.
4. **A friendship is not GPS consent.** Friendship does not start sharing,
   device permission, a request approval, a conversation or a note audience.
   Device location permission, sharing consent, selected viewers and check-in are
   four separate states.
5. **An invitation is not conversation permission.** MESSAGE authority is
   `VENUE_FRIEND_V1` plus current event context plus the reviewed conversation
   policy. A reviewed landmark is not a MESSAGE prerequisite. `invitationId` is
   optional context and never grants, extends or restores conversation access.
6. **INVITE requires a reviewed landmark.** An invitation, invitation detail and
   invitation decision are bound to the complete current reviewed landmark
   tuple. An unreviewed, closed, stale or unconfigured landmark fails closed.
7. **Server time and one event timezone.** Freshness, TTLs, expiry and end-day
   are evaluated with PostgreSQL time and `concert_events.timezone`. The event
   timezone is `Asia/Seoul` (the current database CHECK constraint). A device
   timezone, browser midnight, photo timestamp or server-local zone is never
   authority.
8. **Unknown authority fails closed.** Missing, stale, invalid, unreachable or
   contradictory owner data produces 503 (or a deny) and no content. There is no
   permissive default for friendship, audience, freshness, TTL, retention,
   timezone, landmark review or conversation policy.
9. **W04 is not this policy.** The original virtual party-room invitation
   question remains a pending owner input and is not reused by L, M or N. See
   "Open owner inputs".

## `VENUE_FRIEND_V1`

`VENUE_FRIEND_V1(a, b, event)` is true only when all of the following hold at
the instant of evaluation:

- `a != b`. The requesting actor carries a current canonical session. The
  counterpart must be an active, non-deleted, non-restricted account, but need
  not be online; friendship persists while the account and graph are current.
- Both `a` and `b` are current `concert_participants` of `event`.
- A live follow edge exists in both directions: rows in the Friend service
  `follows` table for `(a -> b)` and `(b -> a)` with `deleted_at IS NULL`.
  A pending follow request, a one-way follow, a follow that was withdrawn, a
  shared handle or a party membership is not enough.
- No live block exists in either direction in `user_blocks`
  (`blocker_id`/`blocked_id` either order, `deleted_at IS NULL`).
- The event phase is `OPEN` or `LIVE` for any venue read or write.

Owner: Friend service for the follow edges and blocks; Auth for account/session
state; Concert for event membership and phase. The composed read is owned by the
venue implementation (`VenueFriendEligibility` port), but it must not copy or
cache the graph. It takes a current owner read inside the same transaction that
protects the effect. Any owner being unavailable fails closed with 503; a
partially evaluated predicate is never treated as true.

This is deliberately stricter than a single directional "following" flag. The
retained Friend proto exposes only that flag plus block/mute metadata, so it is
not sufficient evidence for `VENUE_FRIEND_V1`. The Friend owner must supply the
bilateral read; until that read exists, the venue friend gate returns 503.

Evidence: `backend/services/friend-service/migrations/0001_init_friend.sql:1`,
`0002_governance.sql:1`, `0005_blocks_mutes.sql:1`;
`backend/crates/friend-domain/src/relationship_policy.rs:1`;
`backend/services/concert-service/src/relationship/mod.rs` (`lock_state` and
`FencedRelationshipAuthority`); `backend/proto/friend/v1/friend.proto:55`.

## Policy matrix

Every field consumed by L, M or N appears below. "Policy" means a
source-identified, revisioned, currently valid, event/venue-scoped policy
supplied by the event operator through the existing policy provider. Policy
values are deployment inputs, not open product decisions: no code may invent a
fallback. If the policy or any owner is missing, stale or invalid, the
fail-closed rule applies.

### Shared gates

| # | Field / behavior | Owner | Audience | TTL / expiry | Fail-closed rule | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | Canonical actor, session and event | Auth + Gateway admission; Concert verifies its local event/participant state | Actor only | Original signed request/authority deadline; re-evaluated on reconnect | Missing/invalid session: 401 `AUTHENTICATION_REQUIRED`; owner outage: 503; JSON never supplies the actor | `identity-admission-decision.md`; `concert_events`/`concert_participants` migration `0001` |
| S2 | `VENUE_FRIEND_V1` eligibility | Friend (follows/blocks) + Auth + Concert (membership/phase) | The pair only; never returned as a global friend list | Evaluated fresh at every protected read and every effect commit; no cached friendship | Not eligible: deny/403 or 404; one-way follow: deny; any owner unavailable: 503; higher layers must not reconstruct friendship from a cached flag | `0001_init_friend.sql`, `0002_governance.sql`, `0005_blocks_mutes.sql`; `relationship_policy.rs` |
| S3 | Block, suspension, deletion, event ineligibility | Friend + Auth + Concert | The pair and server | Current state at each evaluation | Any hit hides the object and denies the operation (403/404); cached private rows are invalidated; no partial "safe" row is returned | `user_blocks`, `concert_participants`; `relationship/mod.rs` |
| S4 | Event phase | Concert | Event members | Current event revision | Writes outside `OPEN`/`LIVE`: 403/409/404; venue reads outside `OPEN`/`LIVE`: 403/404 and private caches cleared; archive/cleanup remains with its owner | `0001_concert_core.sql` phase CHECK |
| S5 | Operator/admin identity | Concert operations/Auth role evidence | Not a venue viewer | Current role evidence | An operator gets no location, conversation or note content unless separately the owner or an explicitly selected viewer; absence of role evidence denies the operator action | `identity-admission-decision.md`; BE38 role checks |

### L: location and sharing

| # | Field / behavior | Owner | Audience | TTL / expiry | Fail-closed rule | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| L1 | `sharing.state` | L | Owner sees full state; each selected viewer sees only its permitted `VisibleLocation` | `OFF` is the initial state. `ACTIVE` ends at `expires_at`. `PAUSED` and `REVOKED` are terminal until a new explicit `ACTIVE` publication. `EXPIRED` is derived from `now >= expires_at`; a read does not mutate it. | Unknown/missing state: 503, no coordinate. `OFF`/`PAUSED`/`REVOKED`/`EXPIRED` never return a position. | `location.openapi.json` `SharingState`; `location.proto:71` |
| L2 | `sharing.viewer_ids` | L | Owner and server only | Current sharing revision and lease; a viewer not eligible at read time is omitted even if the stored ID remains | Non-`VENUE_FRIEND_V1`, duplicate, self, or more than `max_viewers`: 400/403; never auto-expand to future friends or followers; removal/revocation is effective on the next read | `location.openapi.json` `SharingInput`/`SharingState`; `location.md` |
| L3 | `sharing.expires_at` | L + venue policy | Owner; server | `min(now + policy.sharing_lease_ms, policy.valid_until, request access deadline)`; no default | Missing/invalid policy or non-positive lease: 503; at expiry the state is `EXPIRED` and all coordinates disappear | `VenueLocationPolicy.sharing_lease_ms` |
| L4 | Phone fix source, position and receipt | L | Only selected `VENUE_FRIEND_V1` viewers; owner sees own | `fresh_ms` for `PHONE`; `last_known_ms` for `LAST_KNOWN`; never beyond `coordinate_retention_ms` | `captured_at` older than `fix_max_age_ms`, later than `now + future_skew_ms`, accuracy over `max_accuracy_m`, invalid coordinates: 400 for the client input. Missing policy: 503. Stale-but-within-last-known: `LAST_KNOWN`, never labelled current. Older than last-known: omitted, not fabricated. | `PhoneFixInput`, `PhonePosition`, `VisibleLocation`; `location.md` "Explicit sharing" |
| L5 | `observed_at`, `fresh_until` | L | Selected viewers | `fresh_until = observed_at + policy.fresh_ms`; `last_known_until = observed_at + policy.last_known_ms` | Reads never extend either instant; pause/revoke/expiry sets position and timestamps to null; client-supplied freshness is invalid | `VisibleLocation`; `location.md` |
| L6 | Explicit check-in | L | Selected viewers | `checked_in_at + policy.check_in_ttl_ms`; immutable for the check-in | Requires a current reviewed landmark with `can_check_in=true`; unreviewed/closed/unknown landmark: 404/403. Never produces a phone coordinate, never sets `physical_arrival_verified=true`, and does not degrade to `LAST_KNOWN` after its TTL. | `CheckInInput`, `VisibleLocation.landmark`; `location.md` |
| L7 | ASK LOCATION request | L | Requester and subject only | `created_at + policy.request_ttl_ms`; a repeated live pair request reuses the existing row and does not extend expiry | Non-friend, blocked or non-participant: 403/404. Creating or reading a request grants no coordinate, device permission or sharing state. Expired rows are `EXPIRED` and never transition again. | `LocationRequest`, `LocationRequestInput`; `location.md` "Requests and recovery" |
| L8 | APPROVE / DENY / CANCEL | L | Subject may approve/deny; requester may cancel | Request `expires_at`; decision revision CAS | APPROVE is accepted only when sharing is already `ACTIVE`, unexpired, the requester is currently `VENUE_FRIEND_V1`, and the expected revision matches; otherwise 409/403/404. It adds only that viewer, does not extend the lease, and never resumes `PAUSED` or enables device GPS. | `location.openapi.json` request decision operation; `location.md` |
| L9 | Pause and revoke | L | Owner; effect visible to all viewers | Immediate, same transaction; no grace period | `PAUSED`/`REVOKED` require empty `viewer_ids` (otherwise 400), clear the selected audience, delete the precise phone fix and check-in, increment the visibility revision, and leave no current marker. A cached or in-flight fix is rejected after the revision changes. | `location.openapi.json` sharing operation; `location.md` |
| L10 | Coordinate retention | L | Server/purge only | `min(observed_at + policy.coordinate_retention_ms, sharing expiry, event close, account deletion)`; only one replaceable fix/check-in per owner, never a history | Values older than the cap are not readable and are scrubbed durably and resumably; missing policy: 503. No movement track, no raw GPS in receipts, outbox or audit. | `VenueLocationPolicy.coordinate_retention_ms`; `location.md` "Policy, retention and composition limits" |
| L11 | Command receipt and status | L | Original actor only, same event/operation/key | `policy.command_retention_ms`; receipt expiry is not a visibility extension | Missing/foreign receipt: 404; `UNKNOWN` never authorizes an automatic retry; no coordinates or request bodies are replayed | `LocationCommandReceipt`; `location.md` |
| L12 | Reviewed landmark grant | Reviewed provider + L | Event members may read landmark metadata; this is not a location audience | `valid_until`, `reviewed_at`, `catalog_revision`, `landmark_revision`, `map_revision`, `policy_revision`; all must be current | Unreviewed, closed, expired, mismatched or unconfigured provider: 404 or 503; never substitute artwork labels, map pixels, World IDs or an example pin; `can_meet`/`can_check_in`/`can_note` are authoritative booleans | `LandmarkGrant`, `LandmarkSelection`; `location.md` "Common landmark and clock ABI" |
| L13 | Venue clock | Concert timezone + L | Event members | `server_time` each read; `valid_until`; `end_of_day` computed as below | Missing/invalid timezone or clock: 503. No browser/device/local-zone fallback. | `VenueClock`; `0001_concert_core.sql:7` |
| L14 | Venue policy | Event operator through the policy provider | Server; safe policy facts may be returned to the actor | `policy.valid_until`; revisions must match the effective policy | Missing, stale, non-finite or internally inconsistent policy: 503 before any protected effect or disclosure. No production default. | `VenueLocationPolicy` |
| L15 | `LocationSnapshot.access_until` | L | Actor | `min(policy.valid_until, sharing.expires_at, each returned grant/request expiry, original access deadline)` | Any authority gap yields no rows or 503; the snapshot never outlives the earliest included disclosure | `LocationSnapshot` |

Formulas and invariants for L:

```text
sharing.expires_at = min(now + policy.sharing_lease_ms, policy.valid_until, access_deadline)
request.expires_at = created_at + policy.request_ttl_ms
check_in.expires_at = checked_in_at + policy.check_in_ttl_ms
fresh_until = observed_at + policy.fresh_ms
last_known_until = observed_at + policy.last_known_ms
retention_until = observed_at + policy.coordinate_retention_ms

0 < fresh_ms <= last_known_ms <= coordinate_retention_ms
0 < fix_max_age_ms <= last_known_ms
0 <= future_skew_ms
0 < max_accuracy_m
1 <= max_viewers <= 50
0 < sharing_lease_ms, request_ttl_ms, check_in_ttl_ms, command_retention_ms
```

A violation is a policy error (503), not a reason to clamp silently.

### Field coverage map

The rows above are the policy rules. This map confirms that every response or
request field consumed by L/M/N has a rule, so a missing field cannot become an
implicit permissive default.

| Object / field group | Covered by | Additional rule |
| --- | --- | --- |
| `LandmarkSelection` (`landmark_id`, `catalog_revision`, `landmark_revision`) | L12, M3 | Opaque reviewed-provider identity and two separate positive revisions; no UUID/slug/GPS/art-label substitution. Missing or partial triple: 400. |
| `LandmarkGrant` (`event_id`, `selection`, `provider_id`, `source_revision`, `venue_key`, `map_id`, `map_revision`, `policy_revision`, `label`, `review_reference`, `reviewed_at`, `server_time`, `valid_until`, `physical_binding`, `presentation_id`, `can_check_in`, `can_meet`, `can_note`) | L12, L13 | Persist and recheck the complete tuple at the protected effect. `presentation_id` has no geographic meaning. `physical_binding` is absent unless an actual BE08 place UUID/revision is bound. Any review/validation failure: 404/503. |
| `VenueClock` (`timezone`, `server_time`, `end_of_day`, `valid_until`, `policy_revision`) | L13, N4 | Server/database time only; `Asia/Seoul`; `end_of_day` is the capped next local midnight. Missing/invalid: 503. |
| `VenueLocationPolicy` (all 16 policy values) | L3, L4, L7, L10, L11, L14 | Validated ordering and positivity; missing/contradictory: 503. Values are operator configuration, never code defaults. |
| `SharingInput` (`expected_revision`, `state`, `viewer_ids`) | L1, L2, L3 | ACTIVE requires 1..max_viewers eligible viewers; PAUSED/REVOKED require empty; revision CAS. |
| `SharingState` (`state`, `revision`, `viewer_ids`, `expires_at`, `updated_at`) | L1, L2, L3 | Owner-only full state. `updated_at` is server truth; no client-supplied state. |
| `PhoneFixInput` / `PhonePosition` (`source`, `latitude`, `longitude`, `accuracy_m`, `captured_at`, `received_at`) | L4 | One replaceable fix per owner; WGS84 finite bounds, accuracy and time checks; no movement history. |
| `VisibleLocation` (`user_id`, `source`, `position`, `landmark`, `observed_at`, `fresh_until`, `expires_at`, `access_until`, `visibility_revision`, `physical_arrival_verified`, `can_request_location`) | L1-L6, L12, L15 | Viewer-specific recomputation; null position on pause/revoke/expiry; `physical_arrival_verified` is always false; no hidden marker. |
| `LocationRequest` / decisions (`request_id`, `state`, `revision`, `created_at`, `expires_at`) | L7, L8 | Metadata only; PENDING/terminal semantics; approval only into an active lease. |
| `LocationSnapshot` (`event_id`, `actor_id`, `server_time`, `access_until`, `policy`, `clock`, `sharing`, `self_location`, `friends`, `capabilities`, `incoming_requests`, `outgoing_requests`) | S1, L1-L15 | `friends` contains only currently authorized selected viewers; requests only for returned friends; `capabilities` is recomputed, not permission. |
| `LocationCommandReceipt` / status (`command_key`, `operation`, `state`, `resource_*`, `revision`, `created_at`, `expires_at`) | L11 | Actor-only, redacted; no coordinates or bodies; UNKNOWN never auto-retries. |
| `MeetingSelection` / `Selection` (`selectedFriendId`, optional `landmark`) | S2, M1-M3, M6 | Selected friend must satisfy `VENUE_FRIEND_V1`. MESSAGE allows no landmark; INVITE requires one. |
| `MeetingContext` (`contextId`, `eventId`, `actorId`, `selection`, `conversationId`, `canRead`, `canSend`, `canInvite`, `relationshipRevision`, `policyRevision`, `serverTime`, `validUntil`, `eventTimezone`, `invitationExpiresAt`, `contentExpiresAt`, `purgeAt`) | M1, M2, M7, M8 | `contextId` is opaque and non-authoritative; capabilities are false when any owner is missing; `validUntil` is the earliest ceiling. |
| `CreateInvitation` / `DecideInvitation` (`selection`, `expectedRevision`, `decision`) | M3, M4 | Reviewed landmark required; revision CAS; wrong actor/state 403/409. |
| `MeetingInvitation` (`id`, `eventId`, `senderId`, `recipientId`, `landmarkId`, `revision`, `state`, `createdAt`, `expiresAt`, `allowedInvitationActions`) | M3, M4 | Pair-only audience; EXPIRED at `expiresAt`; actions are current capabilities, not historical receipts. |
| `SendMessage` (`selection`, `body`, `invitationId`) | M5, M6 | `body` bounds already in the schema; `invitationId` same event/pair only and never permission. |
| `MeetingMessage` / `MeetingMessagePage` (`id`, `sequence`, `senderId`, `body`, `invitationId`, `createdAt`, `expiresAt`, `cursor`, `hasMore`, `gap`) | M5, M8, M9 | Pair-only; immutable sequence; expired/gap content is unavailable and not resurrected. |
| `MeetingInboxPage` (`items`, `selection`, `invitation`, `nextCursor`, `serverTime`) | M3, M4 | Reauthorize every row; denied rows omitted; source outage refuses the page. |
| `CreatePlaceNote` (`text`, `audience`, `expiry`, `landmark_selection`) | N1-N4, L12 | Real reviewed landmark required; explicit audience/expiry; server computes `expires_at`. |
| `CreatePlaceReply` (`text`, `expected_parent_revision`) | N5 | No audience/expiry override; inherits parent exactly; one level. |
| `PlaceNote` (`id`, `landmark_id`, `landmark_revision`, `author`, `text`, `created_at`, `expires_at`, `revision`, `audience`, `expiry`, `parent_id`, `reply_count`, `can_reply`, `can_withdraw`, `meet_here`) | N1-N9 | Current-viewer projection; author is a real current PublicProfile; counts/actions are recomputed; hidden content absent. |
| `NotePage` / `NoteThread` (`items`, `visible_count`, `parent`, `replies`, `next_before`, `server_now`, `access_until`) | N5, N6, N11 | Cursor is ordering only; server time is authoritative; access ceiling is the earliest expiry. |
| `NoteCommand` (`receipt_id`, `operation_id`, `state`, `resource`, `failure_reason`, `created_at`, `expires_at`) | N10 | Actor-only safe receipt; no text/profile/body snapshot; receipt TTL never extends the note. |

### M: invitations and contextual MESSAGE

| # | Field / behavior | Owner | Audience | TTL / expiry | Fail-closed rule | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | `MeetingContext.canRead` / `canSend` | M | The selected pair only | `validUntil = min(policy.valid_until, content expiry, access deadline)` | Computed from current `VENUE_FRIEND_V1`, event, block, suspension, deletion and conversation policy. If any source is unavailable: false/403/503, never a cached allow. Opening the context performs no write. | `MeetingContext`; `meeting.md` "Access and writes" |
| M2 | `MeetingContext.canInvite` | M + reviewed landmark | The selected pair | `validUntil` and the landmark's `valid_until` | False when the landmark is not currently reviewed/open, when either participant is ineligible, or when policy is missing. A true value is a capability projection, not an invitation. | `MeetingContext`; `meeting.md` |
| M3 | Invitation create/detail/decision | M | Sender and recipient of that invitation only | `invitationExpiresAt` supplied by reviewed policy; no default. PENDING becomes EXPIRED at that instant. | Missing invitation TTL or landmark authority: 503; malformed selection: 400; unreviewed/closed/foreign/absent landmark: 404/403; expired/terminal transition: 409. Never fabricate an invitation or arrival. | `MeetingInvitation`, `InvitationSelection`; `meeting.md` |
| M4 | Invitation decision rights | M | Recipient ACCEPT/DECLINE while PENDING; sender CANCEL while PENDING or ACCEPTED before expiry | Invitation `expires_at` plus expected-revision CAS | Wrong actor, wrong state, expired, or revision mismatch: 403/409/404. Decision is idempotent only for the original key/body. Cancellation does not revoke independent conversation rights. | `meeting.openapi.json` decision operation; `meeting.md` |
| M5 | Message list/send | M | The current selected pair in the event; no global bare-user DM | `contentExpiresAt` from reviewed policy; `purgeAt >= contentExpiresAt`; no default | Non-`VENUE_FRIEND_V1`, blocked, non-participant, suspended/deleted: 403/404. Expired content: 404/gap, never restored from cache. Missing content policy: 503. | `MeetingMessage`, `MeetingMessagePage`; `meeting.md` |
| M6 | `invitationId` association | M | Same event and same selected pair | Same lifetime as the associated message; historical only | Must resolve to an invitation in the same event and pair (and matching landmark when the context selects one); otherwise 404/400. It never changes `canRead`/`canSend`, never requires PENDING/ACCEPTED, and never grants access to the invitation or landmark. | `SendMessage.invitationId`; `meeting.md` |
| M7 | `eventTimezone` | Concert | Event members | Current event row | `Asia/Seoul` only; missing/invalid: 503. Never device timezone or browser midnight. | `concert_events.timezone`; `0001_concert_core.sql:7` |
| M8 | `contentExpiresAt` / `purgeAt` | M + reviewed policy | Pair/server | Absolute instants from policy; `purgeAt >= contentExpiresAt`; no default | Missing/invalid policy: no send, 503; purge may never occur before expiry; receipts never contain bodies. | `MeetingContext`; `meeting.md` "Receipts, reconnect and retention" |
| M9 | Conversation cursor and sequence | M | Pair | Content expiry and conversation identity | Gap means unavailable, not permission to resurrect cached content; a changed conversation ID resets the cursor; duplicates dedupe by ID and sequence. | `MeetingMessagePage`; `meeting.md` |
| M10 | W04 virtual room invitation | Pending owner input (not M) | None assigned | None assigned | No route, producer, capability or mapping in L/M/N; see "Open owner inputs". | `task-progress-2026-11-04.json` `W04_virtual_party_invitation`; `execution-plan.md` |

Required MESSAGE/INVITE statements:

- INVITE (create, detail, decision) requires a currently reviewed fixed landmark
  and its complete revision tuple. No landmark is a 400; an unknown, unreviewed,
  closed or stale landmark is a 404/403, not a silently upgraded selection.
- MESSAGE opens from an authorized selected friend and the event context. It
  requires no landmark, no invitation, no acceptance and no phone location.
  Opening it performs no invitation, send or other business write.
- `invitationId` is optional linkage only. It is not a permission input, not a
  prerequisite, and not a way to bypass the friend or event check.
- Invitation ACCEPT and map/World movement do not assert physical arrival and do
  not grant location access.

### N: place notes and replies

| # | Field / behavior | Owner | Audience | TTL / expiry | Fail-closed rule | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| N1 | `NoteAudience.FRIENDS` | N + Friend/Auth/Concert | Current `VENUE_FRIEND_V1` readers of the author, in the same event, with no block in either direction | Evaluated on every list/detail/reply read; friendship revocation is immediate | One-way follow, a shared handle, party membership or a cached friend flag is not enough; hide the note/reply and its counts (404/omitted). Owner outage: 503. | `place-notes.openapi.json` `NoteAudience`; `place-notes.md` "Audience, replies and truthful time" |
| N2 | `NoteAudience.EVERYONE` | N | Currently authenticated, currently eligible participants in the same event; never anonymous, all events or the public Internet | Evaluated on every read | A viewer outside current event eligibility sees nothing (403/404/omitted); no location-sharing implication; owner outage: 503. | `NoteAudience`; `place-notes.md` |
| N3 | `NoteExpiry.ONE_HOUR` | N | Per the note audience | Actual database `created_at` + exactly 3,600,000 ms; immutable for the note and all retries | After `expires_at`: 404/hidden; a retry returns the original absolute expiry and never extends it. Missing database clock: 503. | `NoteExpiry`; `place-notes.md` |
| N4 | `NoteExpiry.VENUE_END_DAY` | N + L-provided venue clock | Per the note audience | `clock.end_of_day`, where `end_of_day` is the next 00:00:00 in `concert_events.timezone` strictly after `server_time`, capped by an earlier approved event cutoff or the event access deadline; immutable | Missing/invalid clock, timezone or policy: 503 at creation; never use browser midnight, server-local time or an illustration time. Existing notes keep their original absolute expiry. | `VenueClock.end_of_day`; `0001_concert_core.sql:7`; `0008_meeting_beacons.sql` `event_cutoff` |
| N5 | Reply audience and expiry | N | Intersection of the current parent-note audience and the current reply-author audience; the reader must pass both | Exactly the parent's absolute `expires_at`; replies cannot outlive the parent and have no override | If either check fails: hide the reply and do not include it in counts; replies are one level only; withdrawal/expiry/moderation of the parent removes the thread. | `CreatePlaceReply`, `NoteThread`; `place-notes.md` |
| N6 | `reply_count` / `visible_count` | N | Current viewer only | Current read; never a cached total | Counts only rows the viewer may currently read; hidden/expired/moderated replies are excluded; no unfiltered count or count endpoint. | `PlaceNote.reply_count`, `NotePage.visible_count`; `place-notes.md` |
| N7 | `meet_here` | M | Current note viewer | M context `access_until` | Null when M cannot currently authorize the selected landmark/recipient; it is a composer entry, never an invitation write, acceptance, arrival or location grant. | `NoteMeetingEntry`; `place-notes.md` |
| N8 | Note author profile | Profile + N | Current viewer | Current read | Only an actual current allowed C03 PublicProfile may be embedded; if it cannot be resolved, omit the note (or 503 on an owner outage). Never synthesize or accept a client profile. | `PlaceNote.author`; `place-notes.md` |
| N9 | Moderation, withdrawal and purge | N + moderation owner | Current viewer; server | Immediate invisibility at the effect; body purge durable and resumable | Withdrawal removes the body and, for a root, every reply; expired/moderated/hidden content is not returned from cache; no public archive; no raw body in audit/outbox/error. | `place-notes.md`; `place-notes.openapi.json` |
| N10 | Receipt and status | N + BE15 | Original actor only | Receipt TTL from the retained BE15 policy; never extends the note | Missing/foreign receipt: 404; `UNKNOWN` keeps the original key and never permits automatic retry; status never recovers private bodies. | `NoteCommand`; `place-notes.md` |
| N11 | Note pagination cursor | N | Current authorized viewer | Current authorization and sort position | A cursor is an ordering position only; changing viewer/event/place reauthorizes completely; no cached item becomes readable through a cursor. | `NotePage.next_before`; `place-notes.md` |

## Required invariants

- **A location request does not grant access.** `POST /venue/location/requests`
  creates a pending request; it does not start sharing, open device permission,
  add a viewer or disclose any coordinate. Only an APPROVE into an already
  active, unexpired lease can add that requester.
- **A friendship is not GPS consent.** Even a current `VENUE_FRIEND_V1` pair
  sees nothing unless the owner's active sharing state explicitly selects the
  viewer and a current coordinate or check-in exists.
- **An invitation is not conversation permission.** MESSAGE is authorized by
  current friendship, event context and conversation policy. Landmark selection,
  invitation creation, acceptance or `invitationId` are optional context, never
  permission.
- **A pause is immediate.** `PAUSED` and `REVOKED` remove the audience and the
  precise phone/check-in state in the same transaction; no cached row, in-flight
  fix or pending request can restore "current" data.
- **Expiry is absolute and immutable.** A retry, a later reply or a read never
  extends a sharing lease, request, invitation, message or note expiry.
- **No hidden aggregates.** Clusters, counts and markers are computed only from
  rows the viewer is currently authorized to see.

## Open owner inputs

### W04: virtual party-room invitation policy (PENDING; do not implement)

The original IA W04 virtual party-room invitation remains
`OPEN / NOT_IMPLEMENTED` with one existing human policy question still pending.
The recorded proposal is: host invites accepted party members into the current
virtual room; five-minute expiry/revoke; ordinary capacity-checked admission;
no reservation. **That proposal is not frozen and this document does not adopt
it.** No audience, authority, TTL, revocation rule or admission rule for W04 is
decided here.

Fail-closed default until the owner answers:

- L, M and N must not read, create, infer or reuse a W04 invitation, party
  membership or virtual-room grant as venue authority.
- No W04 route, producer, capability or migration is authorized by this freeze.
  Any caller that requires W04 authority gets 404 (no resource) or 503 (authority
  unavailable), never a permissive default.
- W04 is distinct from the physical VENU-M invitation, MEET HERE and contextual
  MESSAGE in this document. Do not map one onto the other.

Evidence: `docs/mcr/task-progress-2026-11-04.json`
(`open_integration_boundaries.W04_virtual_party_invitation`, state `OPEN`,
`policy_question.frozen=false`); `docs/mcr/execution-plan.md` W04 rows.

### External provenance inputs (physical claims only)

These are not venue privacy-policy decisions and must not be used to weaken the
matrix above. They block physical claims, not the L/M/N privacy rules:

- Authoritative georeferencing/calibration for the illustrated site. Until
  supplied, `LocationCapabilities.geographic_projection` is `UNAVAILABLE`, and
  no image, World or GPS bridge is inferred.
- The current permitted meeting locations, access/closure reviewer and validity
  window. Until supplied, landmark `can_meet`/`can_check_in` are false or the
  landmark is absent, and affected operations return 404/503.
- Third-party rights for physical source material not established by the
  supplied art. Until supplied, do not publish the affected physical provenance
  as reviewed.

### Deployment inputs (values, not open product decisions)

The numeric policy values (freshness, last-known, fix age, skew, accuracy,
sharing/request/check-in lease, coordinate and command retention, max viewers,
invitation/content TTLs, purge deadlines) must be supplied by a source-identified,
revisioned, currently valid venue/event policy. The semantics and invariants are
frozen here; the values are operator configuration. There is no fallback:
missing, stale or invalid configuration returns 503 and discloses nothing.

## Evidence and application

- Wire descriptions: `backend/docs/api/paradise-city-v1/location.openapi.json`,
  `meeting.openapi.json`, `place-notes.openapi.json`. This freeze adds only
  descriptions; it changes no field, enum value or protobuf shape.
- Existing contracts: `docs/mcr/contracts/paradise-city-v1/{location,meeting,place-notes}.md`.
- Product authority: `docs/mcr/venue-feature-amendment-2026-11-05.md`
  ("Venue overview and Friends", "Fixed-landmark invitations and contextual
  messages", "Place notes", "Inputs and completion order") and
  `docs/mcr/progressions.md` (FRIENDS, Fixed-landmark meetings, NOTES).
- Event time: `backend/services/concert-service/migrations/0001_concert_core.sql:7`
  (`timezone='Asia/Seoul'`), `0008_meeting_beacons.sql` (`event_cutoff`),
  `backend/services/concert-service/src/bootstrap/mod.rs:243`.
- Friendship source: `backend/services/friend-service/migrations/0001_init_friend.sql`,
  `0002_governance.sql`, `0005_blocks_mutes.sql`;
  `backend/crates/friend-domain/src/relationship_policy.rs`;
  `backend/proto/friend/v1/friend.proto`.
- W04: `docs/mcr/task-progress-2026-11-04.json` and
  `docs/mcr/execution-plan.md`.

Implementation tasks L/M/N must consume this document as a frozen input. Any
proposed deviation changes this document first and records the owner decision;
it must not be smuggled in as a permissive code default.
