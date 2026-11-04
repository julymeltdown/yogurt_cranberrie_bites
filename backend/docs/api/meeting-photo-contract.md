# Meeting photo v1: producer/consumer agreement

Frozen producer/consumer input port `meeting-photo/v1`, based on master
`0c76e42790106501f3988fe7cb46113b8cd7784e`. This document is the explicitly
assigned shared contract path; it does not claim the existing generated C04
schemas or live routes already accept these additions. Media owner 213 has confirmed the exact fields and semantics. FE owner 1ab7
has explicitly agreed the exact fields and lifecycle semantics. Authority owner
207 has pinned the canonical guard SDK at published `888cb6bd`. No king/SL implementation review or approval is required.

This completes original BE09/FE20 optional authorized photos (§5.4), not the
specification's giveaway BE20. Existing registered beacon operation IDs, HTTP
paths and safe receipt shapes remain unchanged.

## Exact wire additions

| Shape | Addition | Meaning |
|---|---|---|
| `feed.InitUpload`, `feed.Media` | `purpose: "BEACON"` | Dedicated private beacon image; existing POST/GIVEAWAY/REPORT never substitute |
| `beacon.MeetingDescription` | optional nullable `media_id: UUID` | Missing/null means no photo; identifier only, never a remote URL |
| `beacon.CreateBeacon`, `beacon.UpdateBeacon` | optional `photo_permission_acknowledged: boolean`, default false | Must be true when `meeting.media_id` is non-null |

Update replaces the complete meeting description: missing/null `media_id` removes
the old attachment. Keeping the same attachment while changing place/audience/
expiry also requires a new permission acknowledgement and binds the image to the
new beacon revision. Explicit physical-presence acknowledgement remains separate.
Text-only requests remain valid. The implementation omits absent media and false
photo acknowledgement in its canonical typed form to preserve text-only receipt
fingerprints. Unknown/duplicate fields and noncanonical identifiers are rejected.

The acknowledgement records the uploader's assertion that they own the photo or
have permission to share it. It does not prove a license or official venue status.
No image is generated or selected as production content by this contract. Place
review/source/closure checks remain BE08's independent requirement. A photo never
proves GPS position, physical arrival, walking time or official route safety.

## Upload, attach and read flow

1. Canonical FE04 initialize request uses `purpose: BEACON`, then bounded bytes
   PUT and finalize with the existing operation IDs and idempotency rules.
2. Only a freshly authorized `READY` result with matching event/uploader/purpose
   can supply `meeting.media_id`. Client checks support UX; server repeats every
   invariant and actual target authority within the mutation transaction.
3. Create/update binds the sanitized media, exact resulting beacon revision,
   current audience/place fences, permission acknowledgement and expiry on the
   same transaction as beacon effect, BE15 receipt and outbox. A failure rolls
   back all effects. Safe receipts never include bytes, URLs or instructions.
4. After successful receipt, fetch the current authorized beacon, then authorized
   media metadata and `/media/{mediaId}/content`. Protected paths are fetched with
   the canonical authenticated transport; the browser may render an owned Blob
   URL. Never use an API path directly as an `<img src>` or accept a remote URL.
5. Metadata/content reads of linked BEACON media repeat current beacon audience,
   owner, block, membership, place, revision and DB-time expiry authorization,
   including owner access. Media IDs, cached metadata and prior permission do not
   authorize content. Denials expose neither instructions nor image bytes.
6. Replace/remove/revoke makes old content unavailable atomically and tombstones
   its sanitized bytes through BE24. Expiry immediately denies reads; bounded
   cleanup removes bytes. Current policy/identity/event invalidation clears FE
   image URLs, drafts, pending uploads and late responses.

Unlinked READY uploads remain owner-only staging previews until orphan cleanup;
they do not grant shared beacon access. A media ID linked to another beacon or
purpose is rejected. Recovery retains the original operation/key and requires
fresh current authorization; it must never restore replaced or revoked content.

## Deterministic fixtures and FE handoff

Exact producer/consumer JSON is in
`backend/services/concert-service/tests/meeting_photo_support/contract-fixtures.json`.
It contains synthetic IDs and a far-future timestamp for deterministic clock
injection. These are fixture states, not operator evidence, approved photo rights
or current live capabilities. The fixture contains no image URL/bytes. FE can
implement upload/attachment/display/cleanup and denial states independently of
backend live/device qualification. It must gate real HTTP photo submission until
the generated C04/producer composition declares this version available.

Required FE cases: text-only; upload/finalize failure; READY attachment; missing
permission; changed/replaced image; stale beacon revision; expired/revoked;
removed/blocked/unrelated viewer; account/event switch; cancelled upload; late
metadata/content completion; Blob URL disposal. No stale HERE or virtual movement
can become photo-based physical arrival.

## Internal producer and exact shared dependencies

New implementation lives in `concert-service/src/meeting_photo/**` and consumes
existing BE09 `mutate_in`/`read_in` and BE24 caller-transaction link/read/delete
ports. Root composition exports/dispatches it after named ownership transfer.
Retain canonical owner exclusion and the actual transaction through commit and
qualified terminal response settlement, not merely a cached signed Context.

Lock order is event → beacon/party roots → sorted policy pairs → sorted old/new
media → BE15 budget. Linked-media readers must prelock the target/root from an
untrusted lookup hint, lock media, then recheck exact stored linkage. Calling
beacon authority only after locking media inverts writer order. Target hints
never grant access. Mutations, retries, finalize/recovery of linked media and
cleanup must respect compatible order or deny the unsupported operation.

Exact ungranted shared changes identified early:

- `contracts/domain/v1/beacon.schema.json`, `feed.schema.json`, derived OpenAPI/
  public types and fixture index: owning root must name one writer.
- Dedicated forward migration for BEACON purpose and beacon-media association
  with owner/event/beacon revision/permission/expiry binding; preserve published
  0008 and 0012. Request a reserved migration number from root before editing.
- BE24 purpose enum and per-media target-aware erase/read/prelock ports: media213
  owns these unless explicitly transferred. No direct BE24 storage mutation here.
- Concert `src/lib.rs` module export and product/Gateway dispatch: root1fc owns.
- Existing BE09/domain values may remain text-only internally if the new strict
  adapter separates photo input and combines the authorized read projection;
  no implicit old-beacon/domain source edit is required for that composition.

Native/HTTP/production rights qualification is NOT_RUN for this new slice.

The input-port marker is **meeting-photo/v1**. It is an explicit opt-in client
configuration name, not a server capability or automatic root activation flag.
Absent configuration preserves text-only behavior. Schema inputs are frozen here;
generated OpenAPI/TypeScript outputs remain with root/230 until their coordinated
regeneration. Runtime support is NOT_RUN and must not be inferred from this marker.

## Producer composition port

The owned producer facade is `meeting_photo::PhotoBeacons<A, G>`, composed with
actual `PgMeetingAuthority<GuardedAuthority>` and request-bound
`CanonicalPhotoGuard::from_request(actual_scope, &frame)`. Construct the frame
from verified canonical actor/event plus the actual method target, key, JSON
content type and raw body; `PhotoFrame::mutation` closes/compares the typed input.
The root must use the actual held scope, register/retain it before work, and keep
it beyond reply production/serialization through qualified terminal settlement.
No success fixture/default owner authority may be composed in production.

`execute(frame, mutation)` returns the original safe BE15 receipt;
`read(frame)` returns the authorized combined beacon/photo-ID projection.
`metadata(frame, media_id)` and `content(frame, media_id)` use BE24 same-transaction
ports and reject disagreement with the sealed HTTP media path. Root may use
`target_hint(event, media_id)` only as an internal untrusted lookup, then prelock
and recheck the exact current association. There is no public beacon query added
to media routes. A bounded `cleanup_expired_in` caller-transaction helper requires
an actual qualified current owner/job scope and exact expired target; it is not a
browser delete API or an enabled scheduler. Its caller owns durable job outcome.

MEETING_PHOTO settlement references are closed version 1 and include the full
photo-frame fingerprint. Root must authenticate the owner workload, match the
actual durable registration and exact stored reference, and use the producer's
`canonical::validate_settlement_reference` opaque getters. Shape alone is not
SQL outcome or a release grant. Photo reference resolution and product dispatch
remain separate root composition obligations before runtime enablement.
