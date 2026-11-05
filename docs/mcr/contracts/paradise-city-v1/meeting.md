# Paradise City meeting contract v1

VENU-BE-M01 is an additive mandatory amendment. Canonical source provenance: Main amendment
`686752254c805bbe8ec7a38d0be276513205dc06`, corrected UI packet
`6ff3e481cfcf820c2e14a9a670ae2dc3bffc9afb`, published through
`1907016b6aa41c17998acab691a83e99d0a80076` by publisher412e. The corrected
P02/P09 and Progressions explicitly require a landmark for MEET HERE only.
Selected-friend MESSAGE needs no landmark and no invitation.
The original112/61 definitions and BE26 DONE are preserved.

## Public surface

The authoritative JSON shape is the sibling new-family OpenAPI input
`backend/docs/api/paradise-city-v1/meeting.openapi.json`. The new proto file
`backend/proto/venue/v1/meeting.proto` supplies DTO vocabulary, not a competing
unguarded RPC service. Its int64 wire encoding does not redefine public JSON
numbers: all public revisions/sequences are positive integers at most
9,007,199,254,740,991. Public cursors are canonical decimal strings including0.

All routes below start `/api/concert/events/{eventId}/venue-meeting/`.
All reads use GET with no body/content type/idempotency header. Every mutation
uses `Content-Type: application/json` and a canonical nonnil UUID
`Idempotency-Key` supplied to the sealed ProductRequest, not an actor in JSON.
The event and actor come only from the canonical verified session/request.

| Method and suffix | Operation ID | Input / output |
| --- | --- | --- |
| GET context | resolveVenueMeetingContext | selection query / MeetingContext |
| GET invitations | listVenueMeetingInvitations | direction=INCOMING or OUTGOING; optional cursor / MeetingInboxPage |
| GET invitations/{invitationId} | getVenueMeetingInvitation | selection query / MeetingInvitation |
| POST invitations | createVenueMeetingInvitation | {selection} / APPLIED receipt |
| POST invitations/{invitationId}/decision | decideVenueMeetingInvitation | {selection, expectedRevision, decision: ACCEPT/DECLINE/CANCEL} / APPLIED receipt |
| GET messages | listVenueMeetingMessages | selection query; optional cursor / MeetingMessagePage |
| POST messages | sendVenueMeetingMessage | {selection, body, invitationId?: UUID or null} / APPLIED receipt |
| GET commands/{commandKey} | getVenueMeetingCommand | original selection query / APPLIED or UNKNOWN receipt |

Selection JSON is `{selectedFriendId, landmark?: {landmarkId, catalogRevision,
landmarkRevision} or null}`. selectedFriendId is canonical UUID. The optional
landmark uses a real provider identity (1–128 ASCII unreserved characters,
excluding . and ..) and two separate positive safe-integer review revisions.
MESSAGE resolve/list/send/status works with only selectedFriendId; invite create,
detail and decision require the reviewed landmark. GET uses flat query fields
selectedFriendId and optional all-or-none landmarkId/catalogRevision/
landmarkRevision. Map identity/revision, physicalPlaceId/currentPlaceRevision,
provider/source/provenance are owner facts, not client permission or invented
required message inputs. This aligns L's proposed three-field LandmarkSelection.
Query percent/plus aliases, duplicate/unknown keys, noncanonical UUIDs/numbers
and JSON unknown fields are invalid. Query order is immaterial. Request body
ceiling12,000 bytes.

The selected-friend tuple and any optional landmark pin bind context, detail,
message list/send and status. Landmark-backed and friend-only contexts have
different opaque IDs. No physical provider is consulted for a friend-only
context without an invitation association. Invitation inbox is actor/event scoped and resolves each
stored selected tuple through the current owning authority before returning it.
Denied peers are omitted; dependency failure refuses the page. A stale landmark
selection never silently upgrades to a new review. Current actor/session/policy source unavailable means503 even for an empty
inbox; physical sources are consulted only for rows with a landmark. `selection` in inbox items is rebound to the
request actor with the other participant as selectedFriendId.

## Access and writes

Opening MESSAGE/GET context is a read. It returns an opaque `contextId`, actor,
event, full selection, explicit `canRead/canSend/canInvite`, relationship and
policy revisions, serverTime/validUntil, actual eventTimezone, reviewed
content expiration and purge instants, nullable invitation expiration when no landmark is selected, and nullable existing
conversationId. It does not create an invitation, conversation or message.
No standalone arbitrary-user DM route exists. IDs/hashes are identifiers, never
capabilities. The pair is unordered for context identity, but actor rights are
evaluated separately. A map revision does not erase a conversation's identity.

Explicit SEND may create the event/pair/(optional landmark) conversation atomically with
its first message. Optional invitationId must belong to the same event/pair and
landmark when the context selects one, but no invitation or accepted state is
required. A friend-only message may optionally associate an actual invitation
from that same event/pair; its referenced landmark is separately reauthorized. Invitation ACCEPT
does not assert physical arrival or give GPS access. CANCEL/DECLINE/EXPIRED
alone does not revoke independently authorized contextual read/send rights.

The current real Auth session, event admission, bilateral Friends owner, blocks,
suspension/deletion, reviewed conversation policy and, only for landmark selections/associations,
actual reviewed landmark publication/closure/revisions govern reads, sends, decisions, receipt replay,
status, inbox and reconnect. Following, handle sharing, party membership, map
movement and old Beacon consent are not those grants. The injected owner must
enroll all current source fences into the exact canonical request scope and hold
source row locks/native registration through final COMMIT. Every effect checks
again before commit. The default adapter refuses all access. No production
friend definition, timezone, TTL, retention or physical provider is invented.

Only recipient ACCEPT/DECLINE on PENDING and sender CANCEL on PENDING or
ACCEPTED before expiry are legal, with
matching expectedRevision. Expiry is computed by database clock; a read never
writes EXPIRED. Decisions use revision CAS. Conflicting decision/key returns409.
Foreign/absent resources share404 RESOURCE_UNAVAILABLE after actor checks.
Malformed400; technical rate429; missing authority/persistence503.

## Receipts, reconnect and retention

Create/decision/send keys are unique per actor/event, bind operation+target+
exact original body bytes and persist atomically with the effect. Reusing the
key with different bytes conflicts. Same bytes replay the original effect
snapshot after current access checks. Receipt invitation state/actions are
**historical command snapshots**; fetch current invitation before deciding.
They are not present-day capabilities. Status reads original commandKey under
the original current selection: APPLIED includes operation/result; UNKNOWN
does not distinguish absent, in-flight, expired or purged outcome and NEVER
permits automatic mutation retry. The client retains the original key through
timeouts/disconnects and reconciles with GET. Do not generate a new key merely
because a response was lost. Reauthentication/reconnect always reacquires
current server authority; no cached context grants access.

Message order is committed per-conversation increasing sequence, stable UUID
dedupe, ascending page size50, with cursor0 initially. A page returns conversationId, cursor,
hasMore and gap. If a reviewed purge removes an orphan conversation and later
explicit SEND creates a new one, reset the cursor on changed conversationId. gap means sequences are unavailable (for example expired), not
permission to resurrect cached content. An exhausted page advances to observed
sequence watermark. Reload/resume with this cursor; duplicates dedupe by ID and
sequence. Inbox uses immutable insertion ordinal cursor and page size50; denied
rows can yield an empty page with nextCursor. Restart from cursor0 to discover
new inbox rows. No realtime subscription is claimed by this component.

Plain-text messages have1–2,000 Unicode scalar values after rejecting all-blank
text, and at most8,000 UTF-8 bytes. Newline/tab are allowed, other controls are
rejected. Technical limits:30 sends/actor/event/minute and10 new invitations/
actor/event/minute; retries with the same successful key do not consume more.
These capacity limits are not location freshness or invitation TTL defaults.

Reviewed policy supplies validUntil, invitationExpiresAt, contentExpiresAt,
purgeAt, policyRevision and actual IANA event timezone. No fabricated day-end
timezone/retention default exists. Unfriend/block/source revoke denies subsequent
reads/sends/status even for retained rows. Retained content is hidden at expiry;
physical purge removes due messages/invitations/receipts and orphan contexts
under the existing worker/account-deletion owner's integration. That integration
is required and not implemented by inventing an independent worker. Rows retain
no GPS. All responses use canonical guarded no-store delivery; disclosure is
capped by grant and each returned message/pending invitation expiration.

## Assigned migration SQL — not applied

Root explicitly assigned migration0102_venue_meeting.sql; actual isolated
PostgreSQL validation and the loader remain Root-owned integration.
This block is implementation input, not evidence of migration/native success.
Run only through the canonical migration owner; no source-default DB is allowed.

```sql
create table concert_venue_meeting_conversations(
 id uuid primary key, event_id uuid not null, context_id text not null,
 member_low uuid not null, member_high uuid not null, landmark_id text,
 next_sequence bigint not null check(next_sequence between 0 and 9007199254740991),
 check(member_low<member_high), unique(event_id,context_id), unique(event_id,id)
);
create table concert_venue_meeting_invitations(
 id uuid primary key, ordinal bigserial unique not null check(ordinal between 1 and 9007199254740991),
 event_id uuid not null, context_id text not null, sender_id uuid not null,
 recipient_id uuid not null, selection jsonb not null,
 state text not null check(state in ('PENDING','ACCEPTED','DECLINED','CANCELLED','EXPIRED')),
 revision bigint not null check(revision between 1 and 9007199254740991),
 created_at timestamptz not null, expires_at timestamptz not null,
 purge_at timestamptz not null, policy_revision bigint not null,
 check(sender_id<>recipient_id),check(created_at<expires_at),check(purge_at>=expires_at),
 unique(event_id,id)
);
create index concert_venue_meeting_incoming on concert_venue_meeting_invitations(event_id,recipient_id,ordinal);
create index concert_venue_meeting_outgoing on concert_venue_meeting_invitations(event_id,sender_id,ordinal);
create table concert_venue_meeting_messages(
 id uuid primary key,event_id uuid not null,conversation_id uuid not null,
 sequence bigint not null check(sequence between 1 and 9007199254740991),
 sender_id uuid not null,body text not null check(octet_length(body)<=8000 and char_length(body) between 1 and 2000),
 invitation_id uuid,created_at timestamptz not null,expires_at timestamptz not null,
 purge_at timestamptz not null,policy_revision bigint not null,
 check(created_at<expires_at),check(purge_at>=expires_at),unique(conversation_id,sequence),
 foreign key(event_id,conversation_id) references concert_venue_meeting_conversations(event_id,id)
);
-- invitation_id is a validated historical association, not an FK that could
-- prevent the invitation's independent reviewed purge deadline.
create index concert_venue_meeting_send_rate on concert_venue_meeting_messages(event_id,sender_id,created_at);
create table concert_venue_meeting_receipts(
 event_id uuid not null,actor_id uuid not null,command_key uuid not null,
 operation text not null,request_digest bytea not null check(octet_length(request_digest)=32),
 context_id text not null,result jsonb not null,expires_at timestamptz not null,purge_at timestamptz not null,
 check(purge_at>=expires_at),primary key(event_id,actor_id,command_key)
);
create function concert_venue_meeting_effect_deadline() returns trigger language plpgsql as $$
declare ceiling timestamptz;
begin
 ceiling:=nullif(current_setting('mcr.venue_meeting_authority_until',true),'')::timestamptz;
 if ceiling is null or clock_timestamp()>=ceiling then
   raise exception 'Venue meeting effect authority expired';
 end if;
 return null;
end $$;
create constraint trigger concert_venue_meeting_invitation_deadline after insert or update on concert_venue_meeting_invitations deferrable initially deferred for each row execute function concert_venue_meeting_effect_deadline();
create constraint trigger concert_venue_meeting_message_deadline after insert on concert_venue_meeting_messages deferrable initially deferred for each row execute function concert_venue_meeting_effect_deadline();
create constraint trigger concert_venue_meeting_context_deadline after insert or update on concert_venue_meeting_conversations deferrable initially deferred for each row execute function concert_venue_meeting_effect_deadline();
create constraint trigger concert_venue_meeting_receipt_deadline after insert on concert_venue_meeting_receipts deferrable initially deferred for each row execute function concert_venue_meeting_effect_deadline();
```

The same migration must also install the same-native witness table/immutable
identity trigger described in the backend implementation handoff. Canonical
confirmed Auth/Friend registration must precede witness insertion. The matching
VenueMeetingTransactionResolver is composed into the Root resolver, never a new
ledger or synthetic committed default.
