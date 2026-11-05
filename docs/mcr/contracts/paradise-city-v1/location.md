# Paradise City L01 protocol v1.0.1 (additive request inbox)

Source candidate: IN_PROGRESS / NOT_VALIDATED. No API/service/native/physical
acceptance. Producer:16e. Public frontend mapping:Root153. M/N consume the same
L-owned reviewed-landmark selection/grant; they do not implement a parallel
landmark or accepted-Friends authority. Exact field spelling is snake_case.

Public source: `backend/docs/api/paradise-city-v1/location.openapi.json`.
DTO companion: `backend/proto/venue/v1/location.proto`. Protobuf does not create a
listener or alternate identity system; public JSON follows OpenAPI, including
safe integer revisions rather than protobuf-JSON int64 strings. Root owns actual
operation registration, canonical signed Product forwarding and generated types.

## Common landmark and clock ABI

`LandmarkSelection = {landmark_id:string, catalog_revision:i64,
landmark_revision:i64}`. ID is opaque reviewed-provider-issued unreserved ASCII
1..128 bytes, excluding `.` and `..`; preserve case. Art candidate IDs are only
presentation labels. No UUID, slug or GPS authority is fabricated from the image.

`LandmarkGrant` contains `event_id`, `selection`, `provider_id`, `source_revision`,
`venue_key`, `map_id`, `map_revision`, `policy_revision`, `label`, `review_reference`,
`reviewed_at`, `server_time`, `valid_until`, optional paired `physical_binding`,
optional `presentation_id`, and actual `can_check_in/can_meet/can_note`. The
nullable `physical_binding` is `{physical_place_id:actual BE08 UUID,
current_place_revision:i64}` only where a real reviewed provider binds it.
Catalog/landmark/map/current-place revisions are distinct. M/N persist and
recheck the complete provider/source/venue/map/catalog/landmark/policy tuple at
the protected effect; a cached grant or expected revision is not authority.

`VenueClock` contains event/venue/policy revision, authoritative IANA `timezone`,
PostgreSQL `server_time`, actual `end_of_day` and `valid_until`. No browser midnight,
photo timestamp or inferred timezone. Required providers/config are currently
unavailable per Root's direct source disposition; fail503 until supplied.

The L Rust facade will take actual `&mut PgConnection`, the SAME
`Arc<OwnerGuardScope>`, event, opaque landmark ID and database time. Its current
source must retain actual review/closure/expiry exclusion through final effect
and HTTP EOF/cancellation. A successful scalar snapshot without retained custody
cannot satisfy this port. Missing source has no success default.

## Surface and authorization

All operations are under `/api/concert/events/{eventId}/venue`. Actor/session/
event/request come from canonical Auth and the original signed request scope.
Bodies never choose an actor or supply `is_friend`, owner proofs or permission.
Every response, including error/status, uses private no-store handling. Read
representations are clipped by current actual owner/disclosure/consent expiry.

| Method/path suffix | Operation | Result |
| --- | --- | --- |
| GET `/location` | getVenueLocationSnapshot | Current own sharing plus up to50 currently authorized friend rows |
| PUT `/location/sharing` | setVenueLocationSharing | Redacted command receipt |
| PUT `/location/fix` | publishVenueLocationFix | Redacted command receipt |
| POST `/location/requests` | requestVenueLocation | Redacted command receipt |
| POST `/location/requests/{requestId}/decision` | decideVenueLocationRequest | Redacted command receipt |
| PUT `/location/check-in` | setVenueLocationCheckIn | Redacted command receipt |
| GET `/location/commands/{key}?operation=...` | getVenueLocationCommand | Original actor/event operation/key receipt; never raw location replay |
| GET `/landmarks` | listVenueLandmarks | Up to50 actually reviewed provider grants |
| GET `/landmarks/{landmarkId}` | getVenueLandmark | Current reviewed provider grant |

A real injected accepted-Friends authority must define the actual relationship
and fence both participants' current Auth/event eligibility, suspension/deletion,
both block directions and friendship revocation. The retained Friend proto has
following/block/mute only; following or mutual following is not this predicate.
A location sharing grant never grants invitation, MESSAGE, note or reply access.
M selected-friend MESSAGE requires its own context authority; no invitation or
fixed-landmark prerequisite is imposed by L. A fixed reviewed landmark is required
for MEET HERE. N Everyone remains its event-scoped
content policy and never becomes a GPS audience.

## Explicit sharing and reported positions

Sharing starts OFF, revision0. PUT ACTIVE requires expected revision and explicit
unique selected current-friend viewer UUIDs, bounded by configured max_viewers
and wire50. It replaces the selected audience and starts a finite configured
service lease. Device permission remains separate and client managed. This
operation never opens geolocation, silently expands to future friends or grants
all followers access. PAUSED/REVOKED require empty viewers, remove current grants
and delete precise phone/check-in state atomically. Resume needs new publication.

Phone publication requires ACTIVE, current sharing revision, unexpired lease,
current owner authority and explicit source DEVICE_GEOLOCATION_REPORTED. Validate
finite WGS84 latitude[-90,90]/longitude[-180,180], finite nonnegative reported
accuracy and sample time against actual configured accuracy/skew/max-age bounds.
Use actual captured_at and server received_at. Browser location is device
reported, not attested GPS, measured accuracy or proof of attendance.

Current phone, SELF_REPORTED_CHECK_IN and LAST_KNOWN are distinct. A check-in is
an explicit active-sharer action at a current reviewed fixed landmark; it creates
no inferred phone coordinate and always physical_arrival_verified=false. An old
phone fix is LAST_KNOWN only within current grant/policy validity and retention.
Denied/blocked/paused/revoked/expired data yields no coordinate or hidden marker.
A current check-in cannot be promoted to GPS. Full illustration stays usable
while geographic_projection is UNAVAILABLE. No World/image/GPS coordinate bridge.

The server returns freshness/expiry and actual clock. Clients may cluster only
returned authorized members; no hidden count/centroid leakage. Lists remain
usable without geographic projection. Clear pending samples/watch on pause,
background/disposal or scope loss; server never accepts a queued fix after the
sharing revision changed. Private caches scope actor/session/event/venue and
expire at access_until; Root's actual invalidation transport remains an explicit
integration dependency for cross-client revocation.

## Requests and recovery

ASK LOCATION creates a pending request between current accepted friends. It does
not grant access, prompt another device or enable sharing. Repeated live pair
requests reuse the current request without extending expiry. Subject can APPROVE
or DENY; requester can CANCEL. Require exact request revision, PENDING and current
server time before expiry. APPROVE explicitly adds the requester only to an
already ACTIVE, unexpired selected-viewer lease after current owner/friend checks;
it never resumes paused sharing or enables GPS. Request and sharing consent stay
distinct. Expired/terminal requests never transition again.

Every mutation requires UUID Idempotency-Key. Bind actor/event/operation/key and
canonical validated payload using server-secret HMAC; store metadata/receipt,
never coordinate-bearing replay bodies or raw GPS request logs. Same key+payload
returns the original redacted outcome; changed payload conflicts. Optimistic
sharing/request revision and uniqueness make concurrent decisions deterministic.
A timeout is unknown: retrieve the original operation/key or retry the identical
idempotent mutation.404 status is not evidence that an uncertain write failed.
Current location reads always reauthorize rather than replay old disclosure.

## Policy, retention and composition limits

No production defaults are invented. Inject source-identified, revisioned,
currently valid event/venue policy with actual timezone; freshness, last-known,
max fix age, clock skew, max accuracy, sharing/request/check-in lease, coordinate
and command retention; max viewers. Validate finite/positive bounds and ordering
(fresh<=last-known<=coordinate retention; fix max age<=last-known). PostgreSQL
validates timezone and computes day boundary from that configuration. Missing or
stale policy is503. Example durations and timestamps in images are not values.

Persist one replaceable phone fix/check-in per owner, not a movement track.
Expiry/withdrawal/deletion must scrub precise values durably and resumably.
Minimal redacted idempotency/recovery metadata follows configured retention;
audit/outbox/invalidation contains IDs/revisions, never coordinates or messages.
Friends/block/consent/policy/place changes invalidate relevant state and recheck
at the protected effect. Root owns schema migration numbers/registration,
actual current providers and the shared venue read kind15 and same-runtime
Composite mounting. L owns the concrete location prepared runtime, native
intent/witness/recovery and same-Arc settler; N owns the bounded shared SDK
variant extension. Reads need real retained prepared native custody
and actual terminal handling; no reused fake SocialRead/LocalPublic/Gift grant.

Source/schema qualification may be delivered independently. It does not claim
actual compilation, persistence, multiuser, native owner, device GPS, reviewed
physical catalog or calibrated projection acceptance. Backend implementation and
actual relevant validation/fixes continue in their assigned leaves.


## Version1.0.1 request inbox correction

The location snapshot adds required `incoming_requests` and `outgoing_requests`,
each up to50 existing `LocationRequest` objects, ordered by created_at then ID
descending. Only current authorized Friends returned in this bounded snapshot
are eligible; revoked/blocked/unavailable counterparts produce no request rows.
These metadata rows supply the actual request ID/revision for APPROVE/DENY/CANCEL;
a redacted command receipt alone is not the current request. Pending rows whose
server expiry passed are returned EXPIRED; terminal states never regain authority.
Root's invalidation consumer must refetch the affected recipient as well as
requester. No phone permission, service consent, sharing lease or coordinate is
created by receiving/opening this inbox. Native witness binds the complete exact
selected request IDs/revisions/state/expiry within the actor's redacted fact.

This additive wire correction does not add a route or change the common landmark
ABI. The previously source-qualified v1 three-file publication is canonical
`217c52c25180217b9fd1264af9b8cae52ec88a1a`; v1.0.1 source/protobuf qualification
requires a new exact-input round. Application/native/physical acceptance remains
open independently.
