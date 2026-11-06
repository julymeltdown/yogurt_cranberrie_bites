# Task 35 — AR01 venue provenance

Status: **BLOCKED_ON_EXTERNAL_INPUT** — named gap. Documentation only; no product
code, test, contract or fixture was changed, and no venue, place, closure or
reviewer value is invented here.

Task row: `.omo/plans/mcr-finish-project.md:467-475` (task 35, closes GAP-5,
blocks 47). AR01 row: `MCR_IMPLEMENTATION_PLAN_V2_EN.md:447` — "Every physical
place has source, review state, photo permission, and closure handling".
Base: `13d2ee25f` on `codex/mcr-exec-w8-inputs` (`master`).

Related existing records: `docs/mcr/venue-provenance.md`,
`frontend/public/world/places/source-pack.json`,
`docs/mcr/backend/places.md`,
`docs/mcr/venue-feature-amendment-2026-11-05.md:84-86`,
`.omo/evidence/mcr-finish-project/task-18-venue-policy.md:86-96` (open owner
inputs), `.omo/evidence/mcr-finish-project/task-20-venu-m01.md:140-183`
(landmark-guard mutation evidence).

## 1. The missing input

The recorded candidates are Culture Park
`0b6f634a-994f-49a1-9150-000000000001` and Studio Paradise
`0b6f634a-994f-49a1-9150-000000000002`; both are **UNVERIFIED**, closure
**UNKNOWN**, photo permission **UNREVIEWED**, physical meeting publication
**UNAVAILABLE** (`docs/mcr/venue-provenance.md:29-31`). Neither is emitted as a
product place.

Still required from the user/operator (see `docs/mcr/venue-provenance.md:42-48`
and `docs/mcr/venue-feature-amendment-2026-11-05.md:84-86`):

1. A **dated event-day layout** for the actual concert day: temporary fences,
   gates, queues, closed routes and any temporary meeting points.
2. A **meeting and accessibility review**: accessible routes, safe meeting
   points, and which locations may be used for physical meetings.
3. The **named operator reviewer** who performed that review, with the review
   date and scope.
4. The **publication revision and validity/expiry** of the review (how long the
   reviewed layout remains current).
5. The **closure-update owner**: who may open/close a place, and under what
   authority.
6. **Photo and map-art reuse permission** with an explicit scope for every
   third-party pixel that would ship.
7. **Authoritative georeferencing/calibration** if any real geographic projection
   or distance claim is wanted.
8. Per-place **provenance**: source identifier, source revision, absolute source
   URI, and the retrieval date/hash that identifies the observed bytes.

## 2. Binding map

### 2.1 Place and provenance schema

| Element | Location | Content |
| --- | --- | --- |
| Provenance fields | `backend/crates/concert-domain/src/place/mod.rs:161-212` | `source_id`, `source_kind`, `source_revision`, `source_uri`, `author`, `license`, `review_state`, `reviewed_by`, `reviewed_at`, `photo_permission`. A `REVIEWED` state requires both reviewer and timestamp (`:195-201`). |
| Place kinds/states | `backend/crates/concert-domain/src/place/mod.rs:50-100` | `VIRTUAL_CONCEPT`/`PHYSICAL`; `CONCEPT_ONLY`/`UNVERIFIED`/`VERIFIED`; `OPEN`/`CLOSED`; `UNREVIEWED`/`REVIEWED`/`REJECTED`; photo permission enum. |
| Physical rules | `backend/crates/concert-domain/src/place/mod.rs:254-268` (`kind_and_provenance_valid`) | Physical + verified requires `PLACE_REFERENCE` provenance, a non-null absolute `source_uri` and `REVIEWED` provenance. |
| Selection eligibility | `backend/crates/concert-domain/src/place/mod.rs:269-275` (`permits_new_physical_selection`) | Physical, verified, closure `OPEN` and valid provenance. |
| Admin review pair | `backend/crates/concert-domain/src/place/mod.rs:518-541` (`ReviewStatus`/`ReviewedPlace`) | `Pending` is only valid for a non-verified place; `Reviewed` requires verified, matching current closure and `REVIEWED` provenance. |
| Database | `backend/services/concert-service/migrations/0003_places_map_releases.sql:12-64` | Immutable release-place rows; `concert_place_kind_consistency` `:25-29`; `concert_place_verified_review` `:30`; closed provenance key/type set `:31-64`. |
| Closure state | `backend/services/concert-service/migrations/0003_places_map_releases.sql:75-85` | `concert_place_current_state(closure_state, revision, updated_at)`; mutable, one-step revision, deletion rejected. |
| Admin input shape | `backend/crates/concert-application/src/admin/places.rs:5-37` | `PlaceChange { place, expected, reason }` re-validates provenance and place through the domain constructors. |
| Admin UI | `frontend/src/features/admin/places/PlacesOperations.tsx:32-47` | Displays review state, reviewer, reviewed-at, source kind, author, license, photo permission; edits label, closure, accessibility notes, source URI, source revision, review state, reviewed by/at. |

### 2.2 Qualification gate that keeps unreviewed places unavailable

| Layer | Location | Refusal |
| --- | --- | --- |
| Service repository | `backend/services/concert-service/src/adapters/out/repository/place.rs:27-77` | Reads the release place and current closure under `for share`, then returns `Closed`, `ConceptOnly`, `Unverified` or `Eligible`. |
| Application seam | `backend/crates/concert-application/src/place/mod.rs:82-93,182-215` | `Closed`/`ConceptOnly`/`Unverified` all map to `PlaceError::Ineligible`; the returned detail must also pass `permits_new_physical_selection`. |
| Domain tests | `backend/crates/concert-domain/tests/place.rs:190-229` | Unverified/`UNREVIEWED` and verified/`CLOSED` both refuse selection; `ReviewedPlace::new(reviewed, Pending)` errors. |
| Application tests | `backend/crates/concert-application/tests/place.rs:196-215` | Every non-eligible outcome is a typed refusal. |
| Service tests (real PG) | `backend/services/concert-service/tests/places.rs:167-171,291-296` | Published virtual place selects as `ConceptOnly`; closed place selects as `Closed`. |

### 2.3 Meeting and landmark gate

| Element | Location | Content |
| --- | --- | --- |
| Landmark selection | `backend/crates/concert-domain/src/venue_meeting.rs:15-40` | `landmark_id`, `catalog_revision`, `landmark_revision`; identifiers are lookup keys, never grants. |
| INVITE requirement | `backend/crates/concert-application/src/venue_meeting/mod.rs:152-159` | `CreateInvitation::new` rejects a selection without a landmark. |
| Review gate | `backend/crates/concert-application/src/venue_meeting/mod.rs:17-22,87-105` | The review must match both revisions, report `can_meet` and have `valid_until > now`; otherwise `ResourceUnavailable`. |
| Default authority | `backend/crates/concert-application/src/venue_meeting/mod.rs:42-70` | `UnavailableVenueMeetingAuthority` returns `PolicyUnavailable` for friend, landmark and policy reads. |
| Meeting policy | `backend/crates/concert-domain/src/venue_meeting.rs:253-277` | `valid_until` plus invitation/content/purge windows and the event timezone. |
| Persistence | `backend/services/concert-service/migrations/0074_venue_invitations.sql:13-16,27` | `landmark_id` is optional context on a meeting context/conversation; no venue data is imported. |
| Authority implementations | `backend/services/concert-service/src/venue_meeting/mod.rs:5-44`; tests only | Product code only forwards to an injected `VenueMeetingAuthority`; the concrete implementations found in the repository are test fixtures (`backend/services/concert-service/tests/venue_meeting.rs:37`, `backend/services/concert-service/tests/venue_authz_matrix.rs:216`). |

### 2.4 Location and georeferencing gate

| Element | Location | Content |
| --- | --- | --- |
| Location policy | `backend/crates/concert-domain/src/venue_location.rs:193-241` | `venue_key`, `valid_until`, timezone and freshness/lease/TTL bounds; invalid or expired policy is `PolicyUnavailable`. |
| Capabilities | `backend/crates/concert-domain/src/venue_location.rs:406-421` | Defaults are `can_share_phone: false`, empty `check_in_landmark_ids` and `geographic_projection: "UNAVAILABLE"`. |
| Service snapshot | `backend/services/concert-service/src/venue_location/mod.rs:339-347` | Sets only `can_share_phone`/`can_pause`/`can_revoke` and leaves the projection and check-in landmarks at their unavailable defaults. |
| Arrival flag | `backend/crates/concert-domain/src/venue_location.rs:328,377` | `physical_arrival_verified` is always `false`; a virtual position never implies physical arrival. |
| Policy provider | `backend/crates/concert-application/src/venue_location.rs:11-14` | `VenueLocationPolicyProvider` has no product-code implementation (only test fixtures), so no event location policy can be served today. |

### 2.5 Source pack and prior records

- `frontend/public/world/places/source-pack.json` holds five official references
  with retrieval times, response SHA-256 values, supported facts, rights, two
  physical candidate ids and explicit operational limits. Every entry states
  `review_state: RESEARCH_CHECKED_NOT_OPERATOR_VERIFIED` and
  `photo_permission: UNREVIEWED`.
- `docs/mcr/venue-provenance.md:36-40` records that C06 has only `OPEN` and
  `CLOSED`, so unknown closure is not encoded as either.
- `docs/mcr/backend/places.md:23-39` describes the domain rules; its validation
  section records the domain/application/native runs.
- `docs/mcr/venue-feature-amendment-2026-11-05.md:84-86` lists the unresolved
  inputs: authoritative georeferencing/calibration, current permitted meeting
  locations/access/closure reviewer and validity, and third-party physical-source
  rights.

## 3. Field-by-field mapping of a reviewed record

| Required input | Exact field | Constraint | Present today |
| --- | --- | --- | --- |
| Source identifier | `Provenance.source_id` | `^[a-z][a-z0-9_]{0,63}$` | No approved source id for a physical place |
| Source kind | `Provenance.source_kind` | `PLACE_REFERENCE` for any physical place | No physical place published |
| Source URI | `Provenance.source_uri` | Absolute URI, required for physical places | Candidate fact URLs only (research pack) |
| Source revision | `Provenance.source_revision` | 1-512 non-control characters | Not supplied |
| Retrieval/observation date | `Provenance.reviewed_at` and the pack's `retrieved_at`/`retrieved_sha256` | ISO timestamp; review timestamp required with `REVIEWED` | Page fetch dates only, not an operator review |
| Named operator reviewer | `Provenance.reviewed_by` | 1-512 non-control characters, paired with `reviewed_at` | Not supplied |
| Review decision | `Provenance.review_state` | `REVIEWED` for `VERIFIED` (DB `backend/services/concert-service/migrations/0003_places_map_releases.sql:30`) | `UNREVIEWED` |
| Photo/map rights | `Provenance.photo_permission` | `GRANTED`/`DENIED`/`UNREVIEWED`/`NOT_APPLICABLE` | `UNREVIEWED` |
| Accessibility / meeting review | `Place.accessibility_notes` (1-512) plus `LandmarkReview.can_meet` | Non-empty notes; meeting review must set `can_meet` and `valid_until` | Not supplied |
| Closure | `Place.closure` and `concert_place_current_state.closure_state` | `OPEN` permits new physical selection | Closure unknown; no product row |
| Publication revision/expiry | No place field. Nearest: `MeetingPolicy.valid_until` (`backend/crates/concert-domain/src/venue_meeting.rs:253-257`), `VenueLocationPolicy.valid_until` (`backend/crates/concert-domain/src/venue_location.rs:193-200`), `LandmarkReview.valid_until` (`backend/crates/concert-application/src/venue_meeting/mod.rs:17-22`) | Event policy inputs | Not supplied |
| Closure responsibility | **No field.** `PlaceChange` carries only a free-text `reason` (`backend/crates/concert-application/src/admin/places.rs:5-9`); `concert_place_current_state` has no owner column (`backend/services/concert-service/migrations/0003_places_map_releases.sql:75-85`) | Would need a schema/contract change or an external audit record | Not supplied |
| Dated event-day layout | **No date field on a place.** It must live in the approved source document referenced by `source_uri`, with its date in `source_revision`/`reviewed_at` | Findings of the layout feed `accessibility_notes`, closure and the immutable release bytes | Not supplied |

Consequence: the current schema can carry **provenance, review identity, photo
permission, accessibility notes and closure**, but it cannot carry a
publication-expiry date, a closure owner or an event-day layout date. If the
accepted review requires those as first-class value, that is a contract change
owned by the contract owner, not an AR01 data fill.

## 4. Fail-closed behaviour (recorded evidence)

1. **Domain** — unverified physical place with `UNREVIEWED` provenance and
   verified-but-closed place both return false from
   `permits_new_physical_selection`; `ReviewedPlace::new(reviewed, Pending)`
   is an error (`backend/crates/concert-domain/tests/place.rs:190-229`).
2. **Database** — `concert_place_verified_review` forbids `VERIFIED` without
   `REVIEWED` (`backend/services/concert-service/migrations/0003_places_map_releases.sql:30`); `concert_place_provenance_fields` pins the
   closed key set and requires reviewer/timestamp exactly when reviewed
   (`backend/services/concert-service/migrations/0003_places_map_releases.sql:31-64`).
3. **Selection** — the repository returns `Unverified`/`ConceptOnly`/`Closed`
   for those rows (`backend/services/concert-service/src/adapters/out/repository/place.rs:60-73`), and the application seam turns
   every one of them into `PlaceError::Ineligible`
   (`backend/crates/concert-application/src/place/mod.rs:196-210`). The real-PostgreSQL test
   asserts `ConceptOnly` (`backend/services/concert-service/tests/places.rs:167-171`)
   and `Closed` (`:291-296`).
4. **Meeting** — `require_invitation_landmark` rejects revision mismatch,
   `can_meet = false`, expired and unavailable reviews
   (`backend/crates/concert-application/src/venue_meeting/mod.rs:87-105`);
   `.omo/evidence/mcr-finish-project/task-20-venu-m01.md:146-152,178-183`
   records those cases and the mutation check that removing the condition makes
   `landmark_guard_denies_unreviewed_or_closed_or_stale_reviews` fail.
5. **Recorded runs** — `docs/mcr/evidence/places/native-a91ed08.json` records
   `result: PASS`, `native_test_exit: 0` and `restart_persistence: PASS`
   for the place repository; `docs/mcr/backend/places.md` records 4/4 domain and
   5/5 application tests passing and the native gate.
6. **Current state** — neither candidate is emitted as a product place
   (`docs/mcr/venue-provenance.md:29-40`); the location snapshot keeps
   `geographic_projection: UNAVAILABLE` and an empty `check_in_landmark_ids`
   (`backend/crates/concert-domain/src/venue_location.rs:413-421`, `backend/services/concert-service/src/venue_location/mod.rs:339-347`).

## 5. Verification performed for this record

- Every `file:line` above was re-read at base `13d2ee25f`; the cited SQL, Rust,
  TypeScript and JSON artefacts exist at those paths.
- No Rust, database or browser run was performed for this record. The native
  place gate needs an owned PostgreSQL 16 cluster plus a copied test binary, and
  a fresh compile on this host is outside the current resource floor. The
  recorded runs cited in section 4 are the behavioural evidence of record.
- No venue, place, closure, reviewer, date or permission value was created,
  seeded or edited for this record.

## 6. Inputs required to close task 35

1. The dated event-day layout and its source document (fences, gates, queues,
   closed routes, temporary meeting points), with retrieval hash and revision.
2. The meeting/accessibility review decision per approved meeting location, with
   the reviewer's name, date and scope.
3. Publication revision and validity/expiry for that review.
4. The closure-update owner and the authority under which a place is closed;
   state explicitly whether a schema field is required for it.
5. Photo/map-art permission and scope for every shipped third-party pixel, or an
   explicit decision that none ships.
6. Georeferencing/calibration input, or an explicit decision that the venue
   remains non-geographic and `geographic_projection` stays `UNAVAILABLE`.
7. Operator publication of the reviewed place record through the admin place
   change seam, followed by developer acceptance: the reviewed place selects as
   verified/open, and an unreviewed or closed place remains unavailable.
