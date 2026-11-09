# Profile producer coverage

Status: **PARTIAL — 34 of 43 operations served** (6 by the profile service,
28 by the concert service). Every remaining operation is tracked with an owning
work package and a concrete reason; none is silently missing.

All thirteen frozen intro operations are now served: WP-04's visibility predicate
runs inside the page and detail statements, so the two discovery reads answer
instead of failing.

The cross-database gap described in §2 is **closed**. The fifteen operations
whose tables live in `mcr_concert` are now served by the concert service and
routed there by the Gateway; see §2.1 and §5.

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

### How the gap was closed

Option 1 from the original analysis, implemented:

1. **A concert-service profile product transport.**
   `concert_service::profile_product` mounts the existing `profile_posts`,
   `profile_media`, `profile_room` and `profile_avatar` producers on a dedicated
   `concert.profile_product.v1` service. It is the event-free profile analogue of
   `concert.product.v1`: the same message shape, the same parent-frame
   comparison, the same owner scope and delivery/settlement discipline, keyed by
   `profile_id` and refusing any event path. It could not reuse
   `concert.product.v1` itself, because that transport requires a non-nil
   `event_id` and an `/api/concert/events/{event}/` target, and inventing an
   event for a profile operation is exactly the silent product decision the
   integration forbids.
2. **A routing rule in the Gateway.** `api_gateway::adapters::in::http::profile_owner`
   is a `(operation, owner)` table, not a `match` arm. Every mounted profile
   route is looked up in it, and an operation with no owner is refused rather
   than defaulted to a service that cannot answer it.
3. **No new producer code.** The transport parses the target with each module's
   own closed parser and calls its own `handle`/command function. What it adds is
   the authority discipline a bare persistence call does not supply.

The rule is **which database holds the tables the operation reads**, not which
service is named `profile`. That is why `getOwnRoom` routes to the concert
service while `getOwnProfile` stays in the profile service, even though both are
mounted under `/api/me/`.

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
| 2.1b.3 | `completeIdentityVerification` | GET | `/api/identity/{provider}/callback` | **Served** | `profile_service::adapters::in::grpc::identity_verification` |
| 2.1b.4 | `disconnectIdentityVerification` | DELETE | `/api/me/identity/{provider}` | **Served** | `profile_service::identity` |

### §2.2 Intro posts (domain)

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.2.1 | `listIntroPosts` | GET | `/api/discover/intro-posts` | **Served** (concert) | `concert_service::intro_posts::repository` |
| 2.2.2 | `getIntroPost` | GET | `/api/intro-posts/{introId}` | **Served** (concert) | `concert_service::intro_posts::repository` |
| 2.2.3 | `getOwnIntroPost` | GET | `/api/me/intro-posts/{introId}` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.4 | `listOwnIntroPosts` | GET | `/api/me/intro-posts` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.5 | `saveIntroPostDraft` | PUT | `/api/me/intro-posts/{introId}` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.6 | `publishIntroPost` | POST | `/api/me/intro-posts/{introId}/publish` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.7 | `deleteIntroPost` | DELETE | `/api/me/intro-posts/{introId}` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.8 | `setIntroPostLike` | PUT | `/api/intro-posts/{introId}/like` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.9 | `clearIntroPostLike` | DELETE | `/api/intro-posts/{introId}/like` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.10 | `listIntroPostLikers` | GET | `/api/intro-posts/{introId}/likes` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.11 | `listIntroPostComments` | GET | `/api/intro-posts/{introId}/comments` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.12 | `createIntroPostComment` | POST | `/api/intro-posts/{introId}/comments` | **Served** (concert) | `concert_service::profile_product::intro` |
| 2.2.13 | `deleteIntroPostComment` | DELETE | `/api/intro-posts/{introId}/comments/{commentId}` | **Served** (concert) | `concert_service::profile_product::intro` |

Rows 2.2.12 and 2.2.13 were added by `c79ed25c1` after the freeze record.

### §2.3 Profile posts and media (domain)

WP-06 built these producers. They read `profile_posts`,
`profile_post_attachments` and `profile_media`, which live in `mcr_concert`, so
they are **served by the concert service** and routed there by the Gateway.

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.3.1 | `listProfilePosts` | GET | `/api/profiles/{profileId}/posts` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.2 | `createProfilePost` | POST | `/api/me/posts` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.3 | `deleteProfilePost` | DELETE | `/api/me/posts/{postId}` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.4 | `initProfileMediaUpload` | POST | `/api/me/media/uploads` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.5 | `putProfileMediaBytes` | PUT | `/api/me/media/{mediaId}/bytes` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.6 | `finalizeProfileMedia` | POST | `/api/me/media/{mediaId}/finalize` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.7 | `getProfileMedia` | GET | `/api/media/{mediaId}` | **Served** (concert) | `concert_service::profile_product` |
| 2.3.8 | `readProfileMedia` | GET | `/api/media/{mediaId}/content` | **Served** (concert) | `concert_service::profile_product` |

### §2.4 Room (domain)

WP-07 built these producers. They read `profile_rooms` and `profile_media`, both
in `mcr_concert`, so they are **served by the concert service**.

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.4.1 | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` | **Served** (concert) | `concert_service::profile_product` |
| 2.4.2 | `getOwnRoom` | GET | `/api/me/room` | **Served** (concert) | `concert_service::profile_product` |
| 2.4.3 | `saveOwnRoom` | PUT | `/api/me/room` | **Served** (concert) | `concert_service::profile_product` |
| 2.4.4 | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` | **Served** (concert) | `concert_service::profile_product` |
| 2.4.5 | `uploadRoomPoster` | POST | `/api/me/room/posters` | **Served** (concert) | `concert_service::profile_product` |

### §2.5 Avatar (domain)

WP-08 built these producers. They read `profile_avatars`, in `mcr_concert`
(migration 0087), so they are **served by the concert service**.

| # | operationId | Method | Path | Producer | Owner |
| --- | --- | --- | --- | --- | --- |
| 2.5.1 | `getOwnProfileAvatar` | GET | `/api/me/avatar` | **Served** (concert) | `concert_service::profile_product` |
| 2.5.2 | `setOwnProfileAvatar` | PUT | `/api/me/avatar` | **Served** (concert) | `concert_service::profile_product` |

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
| **Served by the profile service** | **6** | `getOwnProfile`, `getProfile`, `searchCatalog`, `getOwnIdentityVerification`, `startIdentityVerification`, `disconnectIdentityVerification` |
| **Served by the concert service** | **28** | the eight `profile_posts`/`profile_media` operations, the five room operations, the two avatar operations, and all thirteen intro operations |
| NOT_IMPLEMENTED | 9 | everything else |
| **Total** | **43** | |

By blocker:

| Blocker | Count |
| --- | --- |
| Cross-database (producer exists in the concert service) | **0 — closed** |
| Feature not yet built (WP-04, WP-05, WP-03 gaps) | 9 |

## 5. What this change delivered

1. **The cross-database gap is closed.** Fifteen operations moved from
   `NOT_IMPLEMENTED` to `ServedByConcertService`. The producers already existed;
   what was missing was a transport home and a routing rule.
2. **`concert_service::profile_product`** mounts them on
   `concert.profile_product.v1`, with its own event-free profile owner guard,
   durable scope ledger (`concert_profile_product_scopes`, migration 0088) and
   delivery/settlement discipline. It reuses each module's closed parser and
   `handle` entry point; it adds no product behaviour.
3. **The routing rule is data.** `api_gateway::adapters::in::http::profile_owner`
   holds the `(operation, owner)` table. `profile_product::dispatch` looks each
   mounted route up in it and refuses an operation with no owner rather than
   defaulting.
4. **The register records the split.** `Producer` gained
   `ServedByConcertService`, so the register states which database answers each
   operation. `counts()` now returns `(served_here, served_by_concert, owed)`.
5. **The gap is documented** rather than rediscovered by the next work package.

## 6. Verification

Run from `backend/`. The database is the private marker-owned `mcr-pg-*` cluster
created by `backend/scripts/mcr/wp04b_identity_cluster.py`, which now creates
both a profile database and a concert database so each service's suite runs
against its own store, exactly as the compose topology separates them.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 check --locked --workspace` | **0** | compiles |
| `cargo +1.88.0 test --locked -p profile-service --test profile_producer_coverage` | **0** | 8 passed |
| `cargo +1.88.0 test --locked -p api-gateway --test profile_product_routes` | **0** | 11 passed |
| `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_post_routes` | **0** | 14 passed |
| `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_room_routes` | **0** | 12 passed |
| `cargo +1.88.0 test --locked -p api-gateway --test profile_routing` | **0** | 7 passed (new) |
| `cargo +1.88.0 test --locked -p concert-service --test concert_profile_product_routes` | **0** | 9 passed (new) |
| `CONCERT_TEST_DATABASE_URL=… … --test profile_avatar_routes` | **0** | 12 passed |
| `… --test profile_media_upload` | **0** | 16 passed |
| `… --test profile_posts` | **0** | 16 passed |
| `… --test profile_post_read` | **0** | 11 passed |
| `… --test profile_room_persistence` | **0** | 15 passed |
| `… --test profile_room_posters` | **0** | 10 passed |
| `… --test profile_avatar_persistence` | **0** | 13 passed |
| `cargo +1.88.0 test --locked -p api-gateway --test unit` | **0** | 218 passed |
| `cargo +1.88.0 test --locked -p api-gateway --test identity_routes` | **0** | 12 passed |
| `cargo +1.88.0 test --locked -p api-gateway --test browser_auth_http` | **0** | 14 passed |
| `PROFILE_TEST_DATABASE_URL=… … -p profile-service --test unit` | **0** | 47 passed |
| `… --test profile_product_transport` | **0** | 8 passed |
| `python3 backend/scripts/mcr/wp02_migration_ledger.py --report …` | **0** | PASS |

Migration numbering: concert-service gained **0088** (previous head 0087). No
applied migration was edited, and the ledger check confirms the empty-database
and previous-head runs still reach the same schema hash.

## 7. Constraints honoured

- **No fabricated producer.** Every `NOT_IMPLEMENTED` row is refused by the
  route parser and asserted so by
  `a_served_operation_parses_and_a_not_implemented_one_is_refused`. A
  `ServedByConcertService` row is likewise asserted to be refused by the
  *profile* parser, because its tables are not in that database.
- **No cross-database join.** The concert service serves the concert-owned
  operations from its own database; the profile service never queries
  `mcr_concert`. `domain-contract.md` §1.1 is honoured by routing, not by a join.
- **The routing rule is checked against the register.**
  `profile_routing::the_owner_agrees_with_the_coverage_register` parses the
  register and fails if the Gateway routes an operation to a different service
  than the one the register names.
- **No event path.** Every register row is asserted event-free, and the parser
  is asserted to refuse a mutated event target for all 43
  (`no_register_path_carries_an_event_segment`).
- **No weakened test.** The WP-02 transport suite gained a test and lost none;
  the two scope-count assertions were made *stricter* (per-actor rather than
  global) so parallel tests can no longer mask a spurious scope.
