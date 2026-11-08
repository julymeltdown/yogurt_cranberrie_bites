# Profile producer coverage

Status: **PARTIAL — 6 of 43 operations served**. Every remaining operation is
tracked with an owning work package and a concrete reason; none is silently
missing.

The machine-readable source of this table is
`backend/services/profile-service/src/profile_product/coverage.rs`
(`COVERAGE`). This document is generated from it by hand and is verified by
`backend/services/profile-service/tests/profile_producer_coverage.rs`, which
recomputes the profile-owned set from the frozen contract and fails if the two
disagree in either direction.

## 1. Why the count is 43, not 41

`contract-freeze.md` §7 enumerates **41** profile-owned operations (10 core + 31
domain). Commit `c79ed25c1` ("freeze intro comment create and delete")
subsequently added `createIntroPostComment` and `deleteIntroPostComment` to
`contracts/domain/v1/operations.json`. The freeze record's own reconciliation
section already anticipated this: it says the two operations "are **not** in
this freeze … adding the two operations later is an additive change with no
schema work."

So the contract today holds **43** profile-owned operations. This register
covers all 43.

**Definition used.** A profile-owned operation is one whose path does not start
with `/api/concert/events/` (concert product) and does not start with
`/api/auth/` (Auth service, served by its own routes). The coverage test
recomputes this set from the contract rather than trusting the register.

## 2. The finding that shapes this table

WP-03, WP-06, WP-07 and WP-08 each **did** build real producers. They are
nonetheless not reachable from the profile producer, and the reason is
deployment topology rather than missing code:

| Fact | Evidence |
| --- | --- |
| The profile service and the concert service are separate processes | `infra/mcr/compose.yml`: `profile-service` and `concert-service` are distinct services |
| They use separate databases | `PROFILE_DATABASE_URL` → `mcr_profile`; `CONCERT_DATABASE_URL` → `mcr_concert` |
| The WP-06/07/08 tables live in the **concert** database | `profile_media` (0080), `profile_rooms` (0081), `profile_posts` + `profile_post_attachments` (0084), `profile_avatars` (0087) are all concert-service migrations |
| A cross-database join is forbidden | `domain-contract.md` §1.1: "Every new table in the concert service references `profile_id` and reads ownership through the profile service's owner API. **No cross-database join.**" |
| `concert-service` depends on `profile-service` (dev only), so the reverse would be a cycle risk | `concert-service/Cargo.toml` `[dev-dependencies]` |

A producer running inside the profile service cannot query `mcr_concert`. Adding
one that answered these operations anyway would be exactly the "silent product
decision" the task forbids — it would return data it cannot actually read.

The integration assessment reached the same conclusion independently
(`docs/mcr/integration/implementation-assessment-2026-11-09.md`): *"Profile
feature dispatch — Gateway forwards to a closed five-operation producer. Needed
ownership clarification: **Cross-service adapter and authority scope**."*

### What would close the gap

Either of these, and both need an owner outside this work package:

1. **A concert-service product transport.** The concert service already hosts
   `profile_posts`, `profile_media`, `profile_room` and `profile_avatar` with
   closed route parsers and `handle` entry points. Mounting them on a concert
   product service and routing the gateway's profile routes there would serve 15
   operations with no new producer code.
2. **A documented cross-service call** from the profile producer to the concert
   service's gRPC surface, with its own authority scope and child guard.

Option 1 is the smaller change and matches how `concert_product` already works.

## 3. Coverage table

`Served` = a real producer answers it and the route parser accepts it.
`NOT_IMPLEMENTED` = the route parser refuses it, the gateway does not mount it,
and the owner column names who will build it.

### §2.1 Profile (core)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.1.1 | `getOwnProfile` | GET | `/api/me/profile` | **Served** | `profile_product::targets` |
| 2.1.2 | `updateOwnProfile` | PATCH | `/api/me/profile` | NOT_IMPLEMENTED | WP-04 self-profile pipeline |
| 2.1.3 | `setOwnFavorites` | PUT | `/api/me/profile/favorites` | NOT_IMPLEMENTED | WP-03 favorites command |
| 2.1.4 | `getProfile` | GET | `/api/profiles/{profileId}` | **Served** | `profile_product::targets` |
| 2.1.5 | `searchCatalog` | GET | `/api/catalog/{category}/search` | **Served** | `profile_service::catalog` |
| 2.1.6 | `discoverProfiles` | GET | `/api/discover/people` | NOT_IMPLEMENTED | WP-03 discovery (concert-service projection) |

### §2.1b Identity verification (core)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.1b.1 | `getOwnIdentityVerification` | GET | `/api/me/identity/{provider}` | **Served** | `profile_service::identity` |
| 2.1b.2 | `startIdentityVerification` | POST | `/api/me/identity/{provider}/verifications` | **Served** | `profile_service::identity` |
| 2.1b.3 | `completeIdentityVerification` | GET | `/api/identity/{provider}/callback` | NOT_IMPLEMENTED | WP-04b callback transport |
| 2.1b.4 | `disconnectIdentityVerification` | DELETE | `/api/me/identity/{provider}` | **Served** | `profile_service::identity` |

### §2.2 Intro posts (domain)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.2.1 | `listIntroPosts` | GET | `/api/discover/intro-posts` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.2 | `getIntroPost` | GET | `/api/intro-posts/{introId}` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.3 | `getOwnIntroPost` | GET | `/api/me/intro-posts/{introId}` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.4 | `listOwnIntroPosts` | GET | `/api/me/intro-posts` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.5 | `saveIntroPostDraft` | PUT | `/api/me/intro-posts/{introId}` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.6 | `publishIntroPost` | POST | `/api/me/intro-posts/{introId}/publish` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.7 | `deleteIntroPost` | DELETE | `/api/me/intro-posts/{introId}` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.8 | `setIntroPostLike` | PUT | `/api/intro-posts/{introId}/like` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.9 | `clearIntroPostLike` | DELETE | `/api/intro-posts/{introId}/like` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.10 | `listIntroPostLikers` | GET | `/api/intro-posts/{introId}/likes` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.11 | `listIntroPostComments` | GET | `/api/intro-posts/{introId}/comments` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.12 | `createIntroPostComment` | POST | `/api/intro-posts/{introId}/comments` | NOT_IMPLEMENTED | WP-05 intro posts |
| 2.2.13 | `deleteIntroPostComment` | DELETE | `/api/intro-posts/{introId}/comments/{commentId}` | NOT_IMPLEMENTED | WP-05 intro posts |

Rows 2.2.12 and 2.2.13 were added by `c79ed25c1` after the freeze record.

### §2.3 Profile posts and media (domain)

WP-06 built these producers. They read `profile_posts`,
`profile_post_attachments` and `profile_media`, which live in `mcr_concert`.

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.3.1 | `listProfilePosts` | GET | `/api/profiles/{profileId}/posts` | NOT_IMPLEMENTED | WP-06 `profile_posts` (concert-service) |
| 2.3.2 | `createProfilePost` | POST | `/api/me/posts` | NOT_IMPLEMENTED | WP-06 `profile_posts` (concert-service) |
| 2.3.3 | `deleteProfilePost` | DELETE | `/api/me/posts/{postId}` | NOT_IMPLEMENTED | WP-06 `profile_posts` (concert-service) |
| 2.3.4 | `initProfileMediaUpload` | POST | `/api/me/media/uploads` | NOT_IMPLEMENTED | WP-06 `profile_media` (concert-service) |
| 2.3.5 | `putProfileMediaBytes` | PUT | `/api/me/media/{mediaId}/bytes` | NOT_IMPLEMENTED | WP-06 `profile_media` (concert-service) |
| 2.3.6 | `finalizeProfileMedia` | POST | `/api/me/media/{mediaId}/finalize` | NOT_IMPLEMENTED | WP-06 `profile_media` (concert-service) |
| 2.3.7 | `getProfileMedia` | GET | `/api/media/{mediaId}` | NOT_IMPLEMENTED | WP-06 `profile_media` (concert-service) |
| 2.3.8 | `readProfileMedia` | GET | `/api/media/{mediaId}/content` | NOT_IMPLEMENTED | WP-06 `profile_media` (concert-service) |

### §2.4 Room (domain)

WP-07 built these producers. They read `profile_rooms` and `profile_media`, both
in `mcr_concert`.

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.4.1 | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` | NOT_IMPLEMENTED | WP-07 `profile_room` (concert-service) |
| 2.4.2 | `getOwnRoom` | GET | `/api/me/room` | NOT_IMPLEMENTED | WP-07 `profile_room` (concert-service) |
| 2.4.3 | `saveOwnRoom` | PUT | `/api/me/room` | NOT_IMPLEMENTED | WP-07 `profile_room` (concert-service) |
| 2.4.4 | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` | NOT_IMPLEMENTED | WP-07 `profile_room` (concert-service) |
| 2.4.5 | `uploadRoomPoster` | POST | `/api/me/room/posters` | NOT_IMPLEMENTED | WP-07 `profile_room` (concert-service) |

### §2.5 Avatar (domain)

WP-08 built these producers. They read `profile_avatars`, in `mcr_concert`
(migration 0087).

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.5.1 | `getOwnProfileAvatar` | GET | `/api/me/avatar` | NOT_IMPLEMENTED | WP-08 `profile_avatar` (concert-service) |
| 2.5.2 | `setOwnProfileAvatar` | PUT | `/api/me/avatar` | NOT_IMPLEMENTED | WP-08 `profile_avatar` (concert-service) |

### §2.6 Activity and notifications (domain)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.6.1 | `listActivity` | GET | `/api/me/activity` | NOT_IMPLEMENTED | WP-05 activity feed |
| 2.6.2 | `listNotifications` | GET | `/api/me/notifications` | NOT_IMPLEMENTED | WP-05 notification inbox |
| 2.6.3 | `markNotificationRead` | PATCH | `/api/me/notifications/{notificationId}` | NOT_IMPLEMENTED | WP-05 notification inbox |

### §2.7 Follow (domain)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.7.1 | `followProfile` | PUT | `/api/profiles/{profileId}/follow` | NOT_IMPLEMENTED | WP-03 follow |
| 2.7.2 | `unfollowProfile` | DELETE | `/api/profiles/{profileId}/follow` | NOT_IMPLEMENTED | WP-03 follow |

## 4. Totals

| Producer | Count | Operations |
| --- | --- | --- |
| **Served** | **6** | `getOwnProfile`, `getProfile`, `searchCatalog`, `getOwnIdentityVerification`, `startIdentityVerification`, `disconnectIdentityVerification` |
| NOT_IMPLEMENTED | 37 | everything else |
| **Total** | **43** | |

By blocker:

| Blocker | Count |
| --- | --- |
| Cross-database (producer exists in the concert service) | 15 |
| Feature not yet built (WP-04, WP-05, WP-03 gaps) | 22 |

## 5. What this change delivered

1. **`searchCatalog` is now served.** The profile service's `catalog` module
   (WP-03, `catalog_items` in `mcr_profile`) is wired into the profile producer
   with a closed query parser (`q`, `cursor`, `limit`; unknown or repeated fields
   refused) and validated bounds. Proven end to end against a real database by
   `the_catalog_producer_answers_search_catalog`.
2. **Both WP-03 routes are mounted.** `/api/catalog/{category}/search` and
   `/api/discover/people` now exist in the gateway's closed route table.
   `discoverProfiles` reaches dispatch and is refused by the producer, because
   its tables are a concert-service projection; the route is mounted so the
   frontend contract is stable while that composition is owned.
3. **The register exists** (`coverage.rs`) and is enforced by eight tests.
4. **The gap is documented** rather than rediscovered by the next work package.

## 6. Verification

| Command | Exit |
| --- | --- |
| `cargo +1.88.0 check --locked --workspace` | 0 |
| `cargo +1.88.0 test --locked -p profile-service --test profile_producer_coverage` | 0 (8 passed) |
| `cargo +1.88.0 test --locked -p profile-service --test profile_product_transport` | 0 (8 passed) |
| `cargo +1.88.0 test --locked -p api-gateway --test profile_product_routes` | 0 (11 passed) |
| `cargo +1.88.0 test --locked -p api-gateway --lib profile_product` | 0 (7 passed) |
| `python3 backend/scripts/mcr/wp02_migration_ledger.py --report /tmp/coverage-ledger.json` | 0 |

No migration was added or edited by this change, so the ledger result is
unchanged from WP-02: profile-service head 15, concert-service head 85 (plus the
later 0086 and 0087 from WP-03 and WP-08, which this change does not touch).

## 7. Constraints honoured

- **No fabricated producer.** Every `NOT_IMPLEMENTED` row is refused by the
  route parser and asserted so by
  `a_served_operation_parses_and_a_not_implemented_one_is_refused`.
- **No event path.** Every register row is asserted event-free, and the parser
  is asserted to refuse a mutated event target for all 43
  (`no_register_path_carries_an_event_segment`).
- **No weakened test.** The WP-02 transport suite gained a test and lost none;
  the two scope-count assertions were made *stricter* (per-actor rather than
  global) so parallel tests can no longer mask a spurious scope.
