# Deep development task breakdown

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**. 0 of 110 tasks started.
NAVER console review is **APPROVED** (user, 2026-11-09), so the external
prerequisite that previously blocked every restricted post is resolved.

This document breaks the eleven work packages of
[implementation-plan.md](implementation-plan.md) into **110 implementation tasks**
that a single developer can execute, verify, and commit without further design
decisions. Every task names its exact files, symbols, dependencies, acceptance
tests, and the proof required to mark it done.

Machine-readable register: [task-register.json](task-register.json).

## How to read a task

```text
T-<package><n>  <title>
  Lane        BACKEND | FRONTEND | CONTRACT | INTEGRATOR
  Depends     task ids that must be INTEGRATED first
  Writes      exact paths this task may create or modify
  Reads       paths it may read but not modify
  Contract    the frozen interface it must satisfy (document + section)
  Build       implementation steps, each independently checkable
  Accept      the observable behavior that proves the task is done
  Tests       the test files and cases that must exist and pass
  Verify      the verification ids from verification-plan.md it contributes to
  Evidence    what the handoff must record
  Risk        what commonly goes wrong here
```

A task is `INTEGRATED` only when: its writes are committed, its tests pass with a
recorded command and exit code, and its `Accept` statements are demonstrated.
`ACCEPTED` additionally requires the package-level verification to pass.

## Verified foundation facts this breakdown depends on

These were read from the seed tree, not assumed. Each one shapes a task below.

| # | Fact | Consequence |
| --- | --- | --- |
| F-01 | `product_rpc::operation(event, method, target)` strips the prefix `/api/concert/events/{event}/` and returns `permission_denied` on mismatch; `ProductRequest.event_id` must be a canonical non-nil UUID (`canonical_uuid(&m.event_id)`). | Profile-owned operations **cannot** travel through the existing product RPC without an event. T-101 introduces a separate profile product RPC. |
| F-02 | `OwnerBinding` (in `platform::actor_delegation::owner_binding`) carries `event_id: Uuid` and `GuardBinding::from_parent(...)` requires it. | The same conclusion: a profile-scoped authority path needs its own binding type, not a fake event. |
| F-03 | `concert_self_profile` is a full durable-operation pipeline: `PrepareUpdate` → coordinator → `Reconcile`, with `Version { epoch, revision }`, `Operation` receipts, `native_prepare`, `settlement`. | Extending the self profile is not "add a column"; it is adding an operation to an existing two-phase pipeline. T-201..T-206 split that work. |
| F-04 | `ConcertPublicProfileService.ReadCurrent` returns only `subject_id`, `display_name`, `bio`, `observed_at`, plus binding echoes; its SQL filters `account_visibility='PUBLIC'`. | The visitor profile projection needs a new RPC and a new SQL projection; the existing one cannot carry favorites, avatar, or shared interests. |
| F-05 | `concert_media` is `(event_id, owner_id)` with `purpose IN ('POST','GIVEAWAY','REPORT')` and a `concert_participants` foreign key. | Profile media needs its own table and its own upload runtime. T-501..T-506. |
| F-06 | The gateway's `request_metadata` accepts exactly `image/jpeg`, `image/png`, `image/webp` for `/bytes` uploads. | Poster upload must be PNG; no GIF or video anywhere. |
| F-07 | `intro_live::protocol::parse_intro_target` is a closed six-operation parser bound to the event prefix; `intro_policy::IntroRecord::visible` already implements paired age bounds with fail-closed unknown age. | The intro entity exists but is event-scoped and has no gender. T-401..T-412 adapt rather than create. |
| F-08 | `concert_age_claims` is `(user_id, event_id)` with `event_id references concert_events`, and event cleanup can erase it. | Claims must move to the profile service. T-301..T-306. |
| F-09 | `age_policy::arithmetic::derive_exact_age_claim` is already the donor implementation, adapted and tested in MCR. | Do not port it again; import it. T-302 depends on it. |
| F-10 | The gateway declares 15 operations in `contracts/core/v1/operations.json` and 117 in `contracts/domain/v1/operations.json`; generators enforce agreement. | Every new operation is a contract change first. T-001..T-008. |
| F-11 | `scripts/mcr/check-ui-origin.mjs` rejects exact unreviewed Bigyeting bytes and Bigyeting-only dependencies (Mantine, dnd-kit, gsap, motion, three, ffmpeg). | Every frontend task must re-author, not copy. |
| F-12 | `frontend/src/app/routes.ts` declares six primary tabs (`/world`, `/feed`, `/meet`, `/gifts`, `/wall`, `/me`). | The three-section navigation is a router change with a compatibility redirect map. T-701..T-706. |

## Document map

| File | Contents |
| --- | --- |
| this file | Task overview, dependency graph, wave schedule, per-task detail |
| [task-register.json](task-register.json) | Machine-readable task rows with status, owner lane, dependencies, writes, verify ids |
| [implementation-plan.md](implementation-plan.md) | Work packages and ownership |
| [verification-plan.md](verification-plan.md) | Verification cases referenced by `Verify` |

## 1. Task inventory

| Package | Tasks | Count | Lane split |
| --- | --- | --- | --- |
| WP-01 Contract freeze | T-001 … T-008 | 8 | CONTRACT |
| WP-02 Schema | T-101 … T-112 | 12 | BACKEND |
| WP-04b NAVER verification | T-201 … T-212 | 12 | BACKEND + FRONTEND |
| WP-04 Claims and policy | T-301 … T-312 | 12 | BACKEND |
| WP-05 Intro posts and hearts | T-401 … T-416 | 16 | BACKEND + FRONTEND |
| WP-06 Profile posts and media | T-501 … T-512 | 12 | BACKEND + FRONTEND |
| WP-07 Room and posters | T-601 … T-612 | 12 | BACKEND + FRONTEND |
| WP-08 Avatar re-key | T-701 … T-706 | 6 | BACKEND + FRONTEND |
| WP-03 Favorites and discovery | T-801 … T-806 | 6 | BACKEND + FRONTEND |
| WP-09 Navigation and screens | T-901 … T-908 | 8 | FRONTEND |
| WP-10 Integration | T-951 … T-956 | 6 | INTEGRATOR |
| **Total** | | **110** | |

## 2. Dependency graph

```text
T-001..T-008  (contract)
      │
      ▼
T-101..T-112  (schema)
      │
      ├──────────────► T-201..T-212  (NAVER verification)
      │                        │
      │                        ▼
      │                 T-301..T-312  (claims port + visibility policy)
      │                        │
      ├────────────────────────┼──────────────┬──────────────┐
      │                        │              │              │
      ▼                        ▼              ▼              ▼
T-701..T-706            T-401..T-416    T-501..T-512   T-601..T-612
(avatar)                (intro + hearts) (posts + media) (room)
      │                        │              │              │
      └────────────┬───────────┴──────────────┴──────────────┘
                   ▼
            T-801..T-806  (favorites + discovery)
                   │
                   ▼
            T-901..T-908  (navigation + screens)
                   │
                   ▼
            T-951..T-956  (integration + release evidence)
```

Critical path: `T-001 → T-101 → T-201 → T-301 → T-401 → T-901 → T-951`.
Longest single chain is 7 tasks. Everything else is parallelizable across
disjoint write scopes.

## 3. Wave schedule

| Wave | Tasks | Parallel lanes | Gate |
| --- | --- | --- | --- |
| W0 | T-001..T-008 | 1 contract writer | `scripts/mcr/check.sh contracts` exits 0 |
| W1 | T-101..T-112 | 1 backend writer | migrations apply to empty and current-head databases |
| W2 | T-201..T-212, T-701..T-706 | backend ×1, frontend ×1 | V-12 passes |
| W3 | T-301..T-312 | backend ×1 | V-01, V-02, V-03, V-13 pass |
| W4 | T-401..T-416, T-501..T-512, T-601..T-612 | backend ×2, frontend ×2 | V-04..V-10 pass |
| W5 | T-801..T-806 | backend ×1, frontend ×1 | discovery returns a profile with no intro post |
| W6 | T-901..T-908 | frontend ×1 | every screen reachable; audience control placement correct |
| W7 | T-951..T-956 | integrator | V-11 passes with three identities |

A lane may hold at most one writer at a time. Two backend lanes are allowed in
W4 because their write scopes are disjoint (`intro_posts/` vs `profile_posts/`
vs `profile_room/`).

## 4. Status vocabulary

| Status | Meaning |
| --- | --- |
| `NOT_STARTED` | No work begun |
| `IN_PROGRESS` | Write scope claimed, work underway |
| `INTEGRATED` | Committed, tests pass with recorded evidence, acceptance demonstrated |
| `ACCEPTED` | Package-level verification passed |
| `BLOCKED` | A named external condition prevents progress |
| `NOT_RUN` | A test or verification exists but has not been executed |

No task may be marked `INTEGRATED` on the strength of a compile alone.

---

## 5. WP-01 — Contract freeze tasks

### T-001 — Declare the profile core schema definitions

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | — |
| Writes | `contracts/core/v1/core.schema.json` |
| Reads | [http-contract.md](http-contract.md) §3.1, §3.1b, §3.2 |
| Contract | `OwnProfile`, `PublicProfileV2`, `IdentityVerification`, `StartIdentityVerification`, `IdentityCallbackQuery`, `IdentityVerificationStart`, `FavoriteItem`, `UpdateProfile`, `SetFavorites`, `CatalogQuery`, `CatalogPage`, `PeopleQuery`, `PeoplePage` |

**Build**

1. Add `FavoriteItem` to `$defs` with `additionalProperties: false`, `category`
   enum `MUSIC|ANIME|GAME`, `label` 1–80, `external_url` nullable URI.
2. Add `OwnProfile` with every field of §3.1, `required` listing all of them.
3. Add `PublicProfileV2` with `gender_bucket` nullable enum
   `female|male|unknown`, and `shared_with_viewer` as an array of `FavoriteItem`.
4. Add `IdentityVerification` with `provider` enum `naver`, `status` enum
   `NOT_VERIFIED|VERIFIED|EXPIRED|REVOKED|PROVIDER_UNAVAILABLE`, nullable
   `claims` object containing `age` (14–100) and `gender_bucket`.
5. Add the three request/response messages for the verification operations.
6. Add `CatalogQuery`/`CatalogPage` with `category` enum and a bounded `q`.
7. Add `PeopleQuery`/`PeoplePage` with `category` filter, cursor, and a card
   object that carries `shared_items` (never a score).

**Accept**

- Every definition validates against the meta-schema the other `$defs` use.
- No definition contains a numeric age on a viewer-facing object.
- `additionalProperties: false` on every new object.
- `gender_bucket` appears only in `PublicProfileV2` and `IdentityVerification`.

**Tests**

- `scripts/mcr/tests/test_core.py` gains a case asserting the new `$defs` exist,
  are closed, and that no viewer-facing object exposes `age` or `minimum_age`.
- `scripts/mcr/validate-core.py` exits 0.

**Verify** CONTRACT-CHECK. **Evidence**: generator output diff, exit codes.
**Risk**: adding `age` to `PublicProfileV2` "for convenience" — the acceptance
test must fail if it appears.

### T-002 — Declare the intro post domain schemas

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-001 |
| Writes | `contracts/domain/v1/social.schema.json` |
| Contract | [http-contract.md](http-contract.md) §3.3, §3.4, §3.5, §3.6, §3.10 |

**Build**

1. `IntroPostDetail`: `intro_id`, `author` (`core.PublicProfileV2`), `body`
   1–1000, `favorites_snapshot`, `currently_seeking` nullable 0–120,
   `interaction_style` nullable 0–120, `attachments` (max 4), `like`
   (`count`, `liked_by_viewer`), `comment_count`, `published_at`,
   `expires_at` nullable. **No** `audience`, **no** age or gender bounds.
2. `OwnIntroPost`: adds `state`, `audience` (`allowed_gender`, `minimum_age`,
   `maximum_age_exclusive`), `audience_summary` 1–80, `like_count`,
   `revision`, `updated_at`.
3. `SaveIntroPost`: `expected_revision`, `body`, `favorites_snapshot`,
   `currently_seeking`, `interaction_style`, `attachment_media_ids`,
   `audience`, `expires_at`.
4. `IntroLikeState`: `liked` boolean, `like_count` **nullable integer**.
5. `LikerPage`: items carrying `profile_id`, `nickname`,
   `profile_image_media_id`, `avatar`, `shared_with_author`,
   `interaction_style`, `liked_at`; `next_cursor`.
6. `IntroComment`, `CreateIntroComment`, `IntroCommentPage`.
7. `ActivityPage` with `kind` enum `INTRO_LIKED|INTRO_COMMENT|INTRO_REPLY|FOLLOW`.

**Accept**

- `IntroPostDetail` has no property that reveals the audience.
- `IntroLikeState.like_count` is nullable and the schema documents that it is
  null only on the lost-access removal path.
- The audience object enforces paired bounds with a `superRefine`-equivalent
  JSON Schema construct, or the pairing is enforced in the Rust type and the
  schema states `dependentRequired`.

**Tests**

- `scripts/mcr/tests/test_domain.py` case: `IntroPostDetail` rejects a payload
  containing `audience` or `allowed_gender`.
- `scripts/mcr/validate-domain.py` exits 0.

**Verify** CONTRACT-CHECK, V-04.

### T-003 — Declare the profile post, media, and room schemas

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-001, T-002 |
| Writes | `contracts/domain/v1/social.schema.json`, `contracts/domain/v1/feed.schema.json` |
| Contract | [http-contract.md](http-contract.md) §3.7, §3.8, §3.9, §4.1 |

**Build**

1. `ProfilePost`, `CreateProfilePost` (description 0–500, `media_ids` max 4,
   `visibility` enum), `ProfilePostPage`.
2. `ProfileRoom`, `OwnRoom`, `SaveRoom` (`expected_revision`, `layout`),
   `SetRoomVisibility`, `UploadPoster` (`preset` enum `FINE|CHUNKY`, `width`,
   `height`, `content_type` literal `image/png`, `bytes_base64`).
3. `RoomLayout` with `version` literal 1, `wall`, `floor`, `furniture` (max 32),
   `posters` (max 4), `character`.
4. Reuse `feed.Media` for media references; do not define a second media object.

**Accept**

- `RoomLayout.posters` has `maxItems: 4`.
- `UploadPoster.content_type` is the single literal `image/png`.
- `RoomLayout` contains no `intro_id` and no `event_id`.

**Tests**

- `scripts/mcr/tests/test_domain.py`: room layout rejects 5 posters, rejects an
  `intro_id` property, rejects a non-PNG poster content type.

**Verify** CONTRACT-CHECK, V-06, V-08.

### T-004 — Declare the 30 new operations

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-001, T-002, T-003 |
| Writes | `contracts/core/v1/operations.json`, `contracts/domain/v1/operations.json` |
| Contract | [http-contract.md](http-contract.md) §2 (all rows) |

**Build**

1. Add the 6 core profile operations and the 4 identity-verification operations
   to `contracts/core/v1/operations.json`.
2. Add the 13 intro operations, 8 profile-post/media operations, 5 room
   operations, 2 avatar operations, 3 activity/notification operations, and 2
   follow operations to `contracts/domain/v1/operations.json`.
3. Each row declares `operationId`, `method`, `path`, `request`,
   `request_location`, `response`, `success_status`, `pending_response`,
   `pending_status`, `actor`, `rule`, `write_policy`, `idempotency_key`,
   `retry`, `cache_control`, `errors`.
4. `completeIdentityVerification` declares `success_status: 303` and is the only
   operation in the file with a redirect status.
5. Every mutation declares `409 REVISION_CONFLICT` where a revision applies and
   `429 RATE_LIMITED` where a write rate limit applies.

**Accept**

- 30 new operations, no duplicate `operationId`, no duplicate `method + path`.
- Every new operation declares `cache_control: no-store`.
- `completeIdentityVerification` is the only 303.
- No new operation contains `/events/{eventId}` in its path.

**Tests**

- `scripts/mcr/tests/test_domain.py` and `test_core.py`: operation-count
  assertions, uniqueness, the no-event-path assertion, and the 303 uniqueness
  assertion.

**Verify** CONTRACT-CHECK.

### T-005 — Add the notification kind and target type

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-004 |
| Writes | `backend/proto/notification/v1/concert_inbox.proto`, `backend/proto/notification/v1/concert_delivery.proto`, `contracts/domain/v1/feed.schema.json`, `contracts/domain/v1/policy.json` |
| Contract | [http-contract.md](http-contract.md) §3.11 |

**Build**

1. `enum NotificationKind` gains `INTRO_LIKED = 7`.
2. `enum TargetType` gains `INTRO_POST` at the next free number.
3. `feed.Notification.kind` enum gains `INTRO_LIKED`; `NotificationTarget.target_type`
   gains `INTRO_POST`.
4. `policy.json` `rules` gains `intro_like`, `intro_likers`, `intro_comment_read`,
   `intro_comment_create`, `intro_comment_delete`, `room_save`,
   `room_visibility`, `room_poster_upload`, `profile_post_create`,
   `profile_post_delete`, `identity_start`, `identity_callback`,
   `identity_disconnect`, `profile_update`, `favorites_set`, `avatar_set`,
   `follow`.

**Accept**

- Existing enum numbers are unchanged (additive only).
- Every new rule has `preconditions` and `result` arrays in the same style as
  `intro_set`.

**Tests**

- `scripts/mcr/tests/test_domain.py`: enum values are append-only; every
  operation's `rule` exists in `policy.json`.

**Verify** CONTRACT-CHECK.

### T-006 — Regenerate contracts and the client

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-004, T-005 |
| Writes | `contracts/generated/mcr-core.openapi.json`, `contracts/generated/mcr-domain.openapi.json`, `frontend/src/generated/api-client/`, `backend/docs/api/` |
| Contract | generator scripts |

**Build**

1. Run `python3 backend/scripts/docs/generate-mcr-core.py`.
2. Run `python3 backend/scripts/docs/generate-mcr-domain.py`.
3. Run `python3 scripts/mcr/generate-api-client.py`.
4. Commit the generated outputs; never hand-edit them.

**Accept**

- All three `--check` invocations exit 0 afterwards.
- The generated client typechecks under the frontend's `tsc -b`.
- The generated OpenAPI contains all 30 new operations.

**Tests**

- `scripts/mcr/check.sh contracts` exits 0.
- `cd frontend && corepack pnpm@11.7.0 run typecheck` exits 0.

**Verify** CONTRACT-CHECK.

### T-007 — Allow the verification callback redirect in the gateway contract

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-004, T-006 |
| Writes | `contracts/core/v1/operations.json`, `backend/docs/api/` |
| Contract | [http-contract.md](http-contract.md) §2.1b |

**Build**

1. Document the `303` response for `completeIdentityVerification` with a
   `Location` header constrained to the stored return path.
2. Add the error set for the anonymous callback: `400`, `404`, `429`, `503`.
3. Record that the callback carries no `X-MCR-Intent` header and no CSRF token,
   because it is a provider redirect, and that it therefore must not perform any
   mutation beyond consuming the one-use state.

**Accept**

- The contract states the callback is the only anonymous operation in the set.
- The contract states the `Location` is server-derived, never echoed from input.

**Tests**

- `scripts/mcr/tests/test_core.py`: the callback operation is `actor: ANON`,
  `write_policy: SAFETY`, and its only declared success status is 303.

**Verify** CONTRACT-CHECK, V-12.

### T-008 — Contract review gate

| Field | Value |
| --- | --- |
| Lane | CONTRACT |
| Depends | T-001..T-007 |
| Writes | `docs/mcr/integration/contract-freeze.md` |
| Contract | — |

**Build**

1. Write a freeze record: operation count before and after, every new
   `operationId`, every new policy rule, the generator commands and exit codes,
   and the generated-file hashes.
2. State explicitly that after this point a contract change requires a dated
   decision-log entry.

**Accept**

- The freeze record lists 30 operations and their hashes.
- `scripts/mcr/check.sh contracts` and `check.sh guards` both exit 0.

**Tests** — (documentation task; the two check commands are the evidence).

**Verify** CONTRACT-CHECK. **Evidence**: `docs/mcr/integration/contract-freeze.md`.

---

## 6. WP-02 — Schema tasks

### T-101 — Profile product RPC transport

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-008 |
| Writes | `backend/proto/profile/v1/profile_product.proto`, `backend/services/profile-service/src/profile_product/` |
| Reads | `backend/proto/concert/v1/product.proto`, `backend/services/concert-service/src/product_rpc/scoped.rs` |
| Contract | F-01, F-02 |

**Why this task exists**: the existing `ProductRequest` requires a non-nil
`event_id` and the producer strips `/api/concert/events/{event}/`. A profile
operation has no event, so reusing that transport would require a fake event and
would put profile data under an event's authority scope. That is exactly the
coupling this integration removes.

**Build**

1. Define `profile.product.v1.ProfileRequest` mirroring `ProductRequest` but with
   `profile_id` instead of `event_id`, and with `authority_expires_unix_micros`,
   `auth_observed_at_unix_micros`, `profile_observed_at_unix_micros`.
2. Define `ProfileResponse` with `status`, `content_type`, `body`, `delivery`,
   `content_expires_unix_micros` — the same delivery discipline as the concert
   product response, so cancellation and settlement behave identically.
3. Define `ProfileProductService.Dispatch`.
4. Implement the service shell in the profile service with the same
   `ScopedProductOwner` pattern: acquire a scope, publish a delivery token, run
   the producer, settle.
5. Reuse `platform::actor_delegation` unchanged except for a new
   `ProfileOwnerBinding` that carries `profile_id` instead of `event_id`.

**Accept**

- A profile request with a nil `profile_id` is rejected.
- A profile request whose `http_target` contains `/events/` is rejected: profile
  operations never carry an event path.
- The delivery token and settlement behavior match the concert product service
  (same unknown-outcome protection).

**Tests**

- `backend/services/profile-service/tests/profile_product_transport.rs`:
  nil-profile rejection, event-path rejection, delivery token issued, settlement
  required before response completion.

**Verify** GUARDS.
**Risk**: copying the concert transport wholesale brings the event binding with
it. The event-path rejection test is the guard.

### T-102 — Gateway route for profile product dispatch

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-101 |
| Writes | `backend/services/api-gateway/src/adapters/in/http/profile_product/`, `backend/services/api-gateway/src/adapters/out/profile_product_client.rs` |
| Reads | `backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs` |

**Build**

1. Add a route module that matches the profile operation paths from T-004 and
   dispatches to the profile product client.
2. Reuse the existing browser-origin, `X-MCR-Intent`, CSRF, and
   `account_context_version` checks unchanged.
3. Apply the same body-limit discipline: 0 for reads, the declared limit for
   mutations, `MAX_BODY` for `/bytes`.
4. Apply the same image content-type allowlist for `/bytes`: JPEG, PNG, WebP.

**Accept**

- An unauthenticated request is `401 AUTHENTICATION_REQUIRED`.
- A stale `account_context_version` is `409 ACCOUNT_CONTEXT_STALE`.
- A mutation without `X-MCR-Intent` is rejected before any side effect.
- A `/bytes` upload with `image/gif` is `415 INVALID_MEDIA_TYPE`.

**Tests**

- `backend/services/api-gateway/tests/profile_product_routes.rs`.

**Verify** GUARDS.

### T-103 — `profiles` table and profile identity

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-101 |
| Writes | `backend/services/profile-service/migrations/00NN_profile_identity.sql` |
| Reads | `backend/services/profile-service/src/adapters/out/persistence/profile_repository.rs` |

**Build**

1. Create `profiles(id uuid primary key, account_id uuid unique not null,
   nickname text, bio text, profile_image_media_id uuid, visibility text,
   created_at, updated_at, deleted_at)`.
2. Backfill from the existing `profiles` table keyed by `user_id`, generating
   `id` deterministically so the migration is repeatable in tests.
3. Add `CHECK (nickname 1..40)`, `CHECK (bio <= 160)`,
   `CHECK (visibility IN ('PUBLIC','PRIVATE'))`.
4. Add a unique index on `account_id` where `deleted_at is null`.

**Accept**

- Every existing profile row has exactly one `profiles.id`.
- Inserting a second row for the same `account_id` fails.
- `id` is never updated by any statement in the migration.

**Tests**

- `backend/services/profile-service/tests/profile_identity_migration.rs`:
  backfill count equality, uniqueness violation, immutability.

**Verify** MIGRATION-LEDGER.

### T-104 — `profile_favorites` table

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103 |
| Writes | `backend/services/profile-service/migrations/`, `backend/services/profile-service/src/profile_favorites/` |

**Build**

1. `profile_favorites(profile_id, category, catalog_item_id, position,
   created_at)` with `PRIMARY KEY(profile_id, category, catalog_item_id)`.
2. `CHECK (category IN ('music','anime','game'))`.
3. `CHECK (position BETWEEN 0 AND 4)`.
4. A trigger or application-level invariant enforcing at most 5 rows per
   `(profile_id, category)`; prefer a `CHECK`-able design by storing at most 5
   rows and rejecting the 6th insert in the command transaction.

**Accept**

- A sixth favorite in one category is rejected.
- The same catalog item twice in one category is rejected.
- Deleting a profile cascades its favorites.

**Tests**

- `backend/services/profile-service/tests/profile_favorites.rs`.

**Verify** DISCOVERY-NO-INTRO.

### T-105 — `identity_claims` and `identity_verifications` tables

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103 |
| Writes | `backend/services/profile-service/migrations/00NN_identity_claims.sql` |
| Contract | [domain-contract.md](domain-contract.md) §4.3 |

**Build**

1. Create `identity_claims` exactly as specified in §4.3, including every CHECK.
2. Create `identity_verifications(verification_id, user_id, verifier,
   source_revision, state, state_digest, return_path, created_at, expires_at,
   completed_at)` with `CHECK (expires_at <= created_at + interval '10 minutes')`.
3. `CREATE UNIQUE INDEX identity_claims_one_verified ON identity_claims(user_id)
   WHERE status = 'VERIFIED'`.
4. `CREATE INDEX identity_claims_canonical ON identity_claims(user_id,
   verified_at DESC, expires_at DESC, claim_id DESC)`.
5. No `event_id` column anywhere.

**Accept**

- A second `VERIFIED` claim for the same user violates the unique index.
- A `VERIFIED` claim without `age_at_verification` violates the CHECK.
- `expires_at > verified_at + 90 days` violates the CHECK.
- A `next_birthday_at` on or before the verification Seoul date violates the
  CHECK.
- `information_schema.columns` for both tables contains no `event_id`.

**Tests**

- `backend/services/profile-service/tests/identity_claims_schema.rs`: one case
  per constraint above, each asserting the specific constraint name fires.

**Verify** V-12. **Risk**: dropping a CHECK to "make the migration pass" — each
constraint has its own negative test precisely so that cannot happen silently.

### T-106 — `profile_media` table

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103 |
| Writes | `backend/services/concert-service/migrations/00NN_profile_media.sql` |
| Contract | [http-contract.md](http-contract.md) §4.1 |

**Build**

1. Create `profile_media` exactly as specified in §4.1.
2. `CHECK (purpose IN ('PROFILE_IMAGE','PROFILE_POST','INTRO_POST','ROOM_POSTER'))`.
3. `CHECK (content_type IN ('image/jpeg','image/png','image/webp'))`.
4. State machine CHECK mirroring `concert_media`: `PENDING` has no bytes,
   `UPLOADED` has staged bytes, `READY` has sanitized bytes plus width/height.
5. No `event_id`, no participant foreign key.

**Accept**

- A row with `purpose = 'POST'` is rejected (that purpose belongs to the concert
  table).
- A `READY` row without `sanitized` is rejected.
- Width or height above 4096 is rejected; `width * height` above 16 M is rejected.

**Tests**

- `backend/services/concert-service/tests/profile_media_schema.rs`.

**Verify** V-06.

### T-107 — `profile_rooms` table

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103 |
| Writes | `backend/services/concert-service/migrations/00NN_profile_rooms.sql` |

**Build**

1. `profile_rooms(owner_profile_id uuid primary key, layout jsonb not null,
   visibility text not null, revision bigint not null, created_at, updated_at)`.
2. `CHECK (visibility IN ('PUBLIC','PRIVATE'))`,
   `CHECK (revision BETWEEN 1 AND 9007199254740991)`.
3. `CHECK (jsonb_typeof(layout) = 'object')`.
4. No `intro_id`, no `event_id`.

**Accept**

- A second row for the same owner violates the primary key.
- A layout that is not a JSON object is rejected.

**Tests**

- `backend/services/concert-service/tests/profile_room_schema.rs`.

**Verify** V-08.

### T-108 — `intro_posts` table with audience columns

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103, T-105 |
| Writes | `backend/services/concert-service/migrations/00NN_intro_posts.sql` |

**Build**

1. `intro_posts(intro_id uuid primary key, author_profile_id uuid not null,
   body text not null, favorites_snapshot jsonb not null default '[]',
   currently_seeking text, interaction_style text,
   audience_gender text not null default 'any', audience_min_age smallint,
   audience_max_age_exclusive smallint, state text not null,
   revision bigint not null, published_at timestamptz, expires_at timestamptz,
   created_at, updated_at)`.
2. `CHECK (audience_gender IN ('any','female','male','unknown'))`.
3. `CHECK ((audience_min_age IS NULL) = (audience_max_age_exclusive IS NULL))`.
4. `CHECK (audience_min_age IS NULL OR (audience_min_age BETWEEN 14 AND 100
   AND audience_max_age_exclusive BETWEEN 15 AND 101
   AND audience_min_age < audience_max_age_exclusive))`.
5. `CHECK (state IN ('DRAFT','PUBLISHED','DELETED'))`.
6. Partial index for discovery: `(published_at DESC, intro_id DESC) WHERE
   state = 'PUBLISHED'`.
7. No `room_id`, no `event_id`.

**Accept**

- An unpaired age bound is rejected.
- `minimum_age >= maximum_age_exclusive` is rejected.
- An `audience_gender` outside the enum is rejected.
- `information_schema.columns` contains no `room_id` and no `event_id`.

**Tests**

- `backend/services/concert-service/tests/intro_posts_schema.rs`.

**Verify** V-01, V-08.

### T-109 — `intro_attachments`, `intro_likes`, `intro_comments`

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-106, T-108 |
| Writes | `backend/services/concert-service/migrations/00NN_intro_children.sql` |

**Build**

1. `intro_attachments(intro_id, media_id, position)` with
   `PRIMARY KEY(intro_id, media_id)` and `CHECK (position BETWEEN 0 AND 3)`.
2. `intro_likes(intro_id, actor_profile_id, created_at)` with
   `PRIMARY KEY(intro_id, actor_profile_id)` and
   `CHECK (actor_profile_id <> intro owner)` enforced in the command, not the
   table (the owner is in a different table).
3. `intro_comments(comment_id, intro_id, author_profile_id, parent_comment_id,
   body, state, created_at)` with `CHECK (char_length(body) BETWEEN 1 AND 500)`,
   `CHECK (state IN ('PUBLISHED','DELETED'))`, and a self-referencing
   `parent_comment_id` foreign key.
4. Index `intro_likes_page ON intro_likes(intro_id, created_at DESC,
   actor_profile_id DESC)`.
5. **No** reward trigger. Assert this in a test.

**Accept**

- A fifth attachment violates the position CHECK or the command limit.
- A duplicate like violates the primary key.
- A comment whose parent belongs to a different intro is rejected.
- `pg_trigger` contains no trigger named like a reward on `intro_likes`.

**Tests**

- `backend/services/concert-service/tests/intro_children_schema.rs`.

**Verify** V-05.

### T-110 — `profile_posts` and attachments

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-106 |
| Writes | `backend/services/concert-service/migrations/00NN_profile_posts.sql` |

**Build**

1. `profile_posts(post_id, author_profile_id, description, visibility, state,
   created_at, deleted_at)`.
2. `profile_post_attachments(post_id, media_id, position)` with
   `PRIMARY KEY(post_id, media_id)`, `CHECK (position BETWEEN 0 AND 3)`.
3. `CHECK (visibility IN ('PUBLIC','PRIVATE'))`,
   `CHECK (state IN ('PUBLISHED','DELETED'))`.
4. Index `(author_profile_id, created_at DESC, post_id DESC) WHERE
   state = 'PUBLISHED'`.
5. No `intro_id`.

**Accept**

- A post cannot reference an `INTRO_POST`-purpose media asset (enforced in the
  command; the schema test asserts the command rejection).

**Tests**

- `backend/services/concert-service/tests/profile_posts_schema.rs`.

**Verify** V-06, V-07.

### T-111 — Account deletion coverage for the new tables

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-104..T-110 |
| Writes | `backend/services/concert-service/migrations/00NN_profile_social_deletion.sql`, `backend/services/profile-service/migrations/` |

**Build**

1. Add the new tables to the existing account-deletion saga in the same order
   the saga already uses (children before parents).
2. Add `ON DELETE CASCADE` where the parent is the profile, and an explicit
   deletion step where a media file must be reference-checked first.
3. Extend the deletion progress counters with the new tables.

**Accept**

- Deleting an account removes every row the account owns in the new tables.
- A media file referenced by two posts survives until both are deleted.
- The saga's existing ordering guarantees are unchanged.

**Tests**

- `backend/services/concert-service/tests/profile_social_account_deletion.rs`.

**Verify** MIGRATION-LEDGER.

### T-112 — Migration ledger and head check

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103..T-111 |
| Writes | — (verification only) |

**Build**

1. Confirm the highest existing concert migration number before authoring; the
   new files must be strictly greater.
2. Run the migration ledger test on an empty database and on a database at the
   previous head.
3. Record the applied order and the resulting schema hash.

**Accept**

- Both runs exit 0.
- No applied migration file was edited.
- The ledger test reports the expected number of new migrations.

**Tests**

- `backend/scripts/mcr/migration_smoke.py` (existing harness) plus the ledger
  test.

**Verify** MIGRATION-LEDGER.

---

## 7. WP-04b — NAVER verification tasks

### T-201 — Provider profile decoding

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-105 |
| Writes | `backend/services/profile-service/src/identity/naver_profile.rs` |
| Reads | donor `oauth_profiles.rs` |
| Contract | [migration-plan.md](migration-plan.md) §2b |

**Build**

1. `NaverProfileResponse` / `NaverProfile` with `id`, `email`, `age`, `gender`,
   `birthyear`, `birthday`, all optional strings, mirroring the donor.
2. `normalized_gender() -> Option<NaverGender>` with the donor mapping:
   `F|female → Female`, `M|male → Male`, `U|unknown → Unknown`, anything else or
   missing → `None`.
3. Reject a response body above the donor's size bound.

**Accept**

- `F`, `M`, `U` map as specified; `X`, `""`, and absent map to `None`.
- A body above the bound is rejected before deserialization.

**Tests**

- `backend/services/profile-service/tests/naver_profile.rs`: the seven mapping
  cases plus the size bound.

**Verify** V-12 (cases c, d, e, f, g).

### T-202 — Provider client

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-201 |
| Writes | `backend/services/profile-service/src/identity/naver_provider.rs` |
| Contract | donor `oauth_naver_provider.rs` |

**Build**

1. `exchange_authorization_code(code, state) -> access_token` against the
   configured token URL.
2. `load_profile(access_token) -> NaverProviderProfile { subject, gender,
   birthyear, birthday }`.
3. Provider errors are classified, never logged with token or body content.
4. Only the configured endpoint is used; no caller-supplied URL.

**Accept**

- A provider 4xx/5xx yields a typed failure with no token in the error.
- The access token never appears in `Debug` output or logs.

**Tests**

- `backend/services/profile-service/tests/naver_provider.rs` with a fake
  provider: success, 401, 500, timeout, oversized body, and a token-redaction
  assertion on the `Debug` render.

**Verify** V-12 (case o).

### T-203 — Provider configuration with production refusal

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-202 |
| Writes | `backend/services/profile-service/src/config/` |
| Contract | donor `config/mod.rs` `require_production_setting` |

**Build**

1. `NAVER_OAUTH_CLIENT_ID`, `NAVER_OAUTH_CLIENT_SECRET` (file-or-env),
   `NAVER_OAUTH_AUTHORIZE_URL`, `NAVER_OAUTH_TOKEN_URL`, `NAVER_OAUTH_PROFILE_URL`,
   `NAVER_OAUTH_REDIRECT_URI`, `NAVER_OAUTH_SCOPES`.
2. In production mode, refuse placeholder or default values and refuse to start.
3. The redirect URI must equal the console-registered callback exactly.

**Accept**

- Production start with `local-naver-client` fails with a clear error.
- Production start with a redirect URI that does not match the configured
  callback fails.
- Development start with placeholders succeeds but logs a warning that is not a
  secret.

**Tests**

- `backend/services/profile-service/tests/naver_config.rs`.

**Verify** V-12 (case p).

### T-204 — Verification start

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-203 |
| Writes | `backend/services/profile-service/src/identity/start.rs`, `backend/services/profile-service/src/identity/repository.rs` |
| Contract | donor `begin_naver_verification` |

**Build**

1. Authenticated start: create an `identity_verifications` row with a random
   `state_digest`, a 10-minute expiry, and the validated `return_path`.
2. Return the provider authorization URL with `client_id`, `redirect_uri`,
   `state`, and `response_type=code`.
3. Reject a `return_path` not in the closed same-origin allowlist.
4. Reject a second in-flight verification for the same user beyond a bounded
   limit.

**Accept**

- The returned URL contains the configured redirect URI exactly.
- A `return_path` of `https://evil.example` or `//evil.example` is rejected.
- The state is stored only as a digest.

**Tests**

- `backend/services/profile-service/tests/identity_start.rs`.

**Verify** V-12 (cases m, n).

### T-205 — Verification callback

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-204 |
| Writes | `backend/services/profile-service/src/identity/callback.rs` |
| Contract | donor `validated_claim`, [migration-plan.md](migration-plan.md) §2b |

**Build**

1. Consume the state exactly once inside the transaction; a second use or an
   expired state fails.
2. Exchange the code, load the profile, and validate in this exact order:
   subject present → gender present → derive age → store.
3. Derive the age with `concert_domain::age_policy::arithmetic::derive_exact_age_claim`
   (do not reimplement).
4. Reject a duplicate provider subject already backing another user.
5. Supersede an existing claim: cap the old row's `expires_at` at the replacement
   instant and set `status = 'REPLACED'`.
6. Return a redirect to the stored `return_path` with a result marker that does
   not carry claim values.

**Accept**

- Missing gender → `profile_incomplete`, no claim row.
- Unrecognized gender → `profile_incomplete`, no claim row.
- Missing birthyear → `profile_incomplete`, no claim row.
- Derived age outside 14–100 → `age_not_supported`, no claim row.
- Duplicate subject → rejected, no second claim row.
- Re-verification → one `VERIFIED` row and one `REPLACED` row.

**Tests**

- `backend/services/profile-service/tests/identity_callback.rs`: one case per
  bullet above, each asserting the row count after the call.

**Verify** V-12 (cases a, b, f, g, h, i, j, k, m, n).

### T-206 — Verification status read

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-205 |
| Writes | `backend/services/profile-service/src/identity/status.rs` |
| Contract | donor `naver_verification_response_at` |

**Build**

1. Select the canonical claim with the donor's ordering: newest `verified_at`,
   then newest `expires_at`, then newest `claim_id`.
2. Compute the current age with the Seoul-date rule.
3. Map to `IdentityVerification`: `NOT_VERIFIED` when no row,
   `VERIFIED` when live, `EXPIRED`, `REVOKED`, `PROVIDER_UNAVAILABLE`.
4. Return `claims` only when `VERIFIED`.

**Accept**

- No row → `NOT_VERIFIED`, `claims: null`.
- Expired row → `EXPIRED`, `claims: null`.
- Live row → `VERIFIED` with the derived age and bucket.
- An older usable row is never selected over a newer expired one.

**Tests**

- `backend/services/profile-service/tests/identity_status.rs`.

**Verify** V-12 (case b), V-02 (cases b, j).

### T-207 — Verification disconnect

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-206 |
| Writes | `backend/services/profile-service/src/identity/disconnect.rs` |

**Build**

1. `DELETE /api/me/identity/naver` sets the live claim to `REVOKED` and caps its
   `expires_at` at the revocation instant.
2. Revocation is idempotent: a second call returns the same `REVOKED` state.
3. No other data is touched: intro posts keep their audience, rooms and posts are
   untouched.

**Accept**

- After disconnect, `getOwnIdentityVerification` returns `REVOKED`.
- The user's own restricted posts still exist with their audience unchanged.
- The user can no longer view others' restricted posts.

**Tests**

- `backend/services/profile-service/tests/identity_disconnect.rs`.

**Verify** V-12 (case l), V-08.

### T-208 — Identity routes in the gateway

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-205, T-206, T-207, T-102 |
| Writes | `backend/services/api-gateway/src/adapters/in/http/identity_routes.rs` |

**Build**

1. Register the four identity operations.
2. The callback route is anonymous: no session, no CSRF, no `X-MCR-Intent`.
3. The callback emits a `303` with a server-derived `Location`.
4. The callback is rate limited per state and per client address bucket.
5. The callback response is `no-store` and carries no claim values.

**Accept**

- The callback without a valid state is `400` and reveals nothing.
- The callback cannot be used to reach any other operation.
- The `Location` is always a same-origin path from the allowlist.

**Tests**

- `backend/services/api-gateway/tests/identity_routes.rs`.

**Verify** V-12.

### T-209 — Identity status UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-208 |
| Writes | `frontend/src/features/identity/IdentityStatus.tsx`, `frontend/src/features/identity/api/identity.ts` |
| Reads | donor `NaverAccountStatus.tsx` |

**Build**

1. Show the verification state in one line: verified (with the derived age and
   bucket, owner-only), expired, revoked, or not verified.
2. Offer "Connect NAVER" / "Reconnect" / "Disconnect" as appropriate.
3. Show the expiry date.
4. Explain in one sentence what verification is used for and that it is not
   required for the rest of the product.
5. Rebuild on destination primitives (`frontend/src/shared/ui/`), no Tailwind
   utility classes copied from the donor.

**Accept**

- The age and bucket are visible only on the owner's own screen.
- The disconnect action asks for confirmation.
- Keyboard reachable, labelled, focus visible.
- No decorative left accent bar.

**Tests**

- `frontend/src/features/identity/__tests__/IdentityStatus.test.tsx`: the four
  states, the confirmation, and the accessibility assertions.

**Verify** V-12, UI-360-390-430.

### T-210 — Verification start and callback UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-208 |
| Writes | `frontend/src/features/identity/VerificationStart.tsx`, `frontend/src/features/identity/VerificationCallback.tsx` |

**Build**

1. Start: call the start operation, then `location.assign(authorization_url)`.
2. Callback landing: read the result marker, show success or a specific failure,
   and return to the stored path.
3. Never auto-submit the work the user was doing before verification.
4. A failure shows the provider-independent message, never a raw provider code.

**Accept**

- A cancelled provider consent returns to the app with a cancellable message.
- The callback landing does not re-trigger the original action.

**Tests**

- `frontend/src/features/identity/__tests__/VerificationCallback.test.tsx`.

**Verify** V-12.

### T-211 — Gated-post prompt

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-209 |
| Writes | `frontend/src/features/identity/VerificationPrompt.tsx` |

**Build**

1. When a request returns `404 RESOURCE_UNAVAILABLE` for a post the user reached
   from a list, show a neutral panel: the post is not available to you; if you
   have not verified your age and gender, you can do so.
2. The panel must not reveal the post's author, body, attachments, or audience.
3. The panel must not appear for a genuinely deleted post; distinguish only by
   what the server returned, never by guessing.

**Accept**

- No restricted content is rendered in the panel under any state.
- The prompt disappears after successful verification and a refetch.

**Tests**

- `frontend/src/features/identity/__tests__/VerificationPrompt.test.tsx`.

**Verify** V-13 (case c).

### T-212 — Verification package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-201..T-211 |
| Writes | `docs/mcr/integration/evidence/v12-*.json` |

**Build**

1. Execute every V-12 case against real services with a test NAVER application.
2. Record each case with its command, exit code, and observed result.
3. Record the console approval reference as the external prerequisite evidence.

**Accept**

- All 16 V-12 cases `PASS`, or a case is `BLOCKED` with the exact reason.

**Tests** — (execution task).

**Verify** V-12, V-13, NAVER-CONSOLE.

---

## 8. WP-04 — Claims port and visibility tasks

### T-301 — `IdentityClaimsPort` trait

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-212 |
| Writes | `backend/crates/concert-application/src/identity_claims.rs` |
| Contract | [domain-contract.md](domain-contract.md) §1.3 |

**Build**

1. Define the trait exactly as in §1.3 with `Attribute<T>`,
   `Gender`, `ClaimSource`, `ClaimsError`.
2. No `Debug` or `Serialize` on `IdentityClaims` that would render the age.
3. Provide a test double `StaticClaims` for the case tables.

**Accept**

- `format!("{:?}", claims)` does not contain the numeric age.
- The trait is object-safe or explicitly generic with a documented reason.

**Tests**

- `backend/crates/concert-application/tests/identity_claims.rs`.

**Verify** V-02.

### T-302 — Claim adapter over the profile service

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-301 |
| Writes | `backend/services/concert-service/src/intro_posts/claims_adapter.rs`, `backend/proto/profile/v1/` (read RPC) |

**Build**

1. Add a minimal internal read RPC to the profile service returning the raw claim
   attributes for a subject, bound to the caller's authority (not a public API).
2. Implement `IdentityClaimsPort` over that RPC.
3. Map every non-live status to `Attribute::Unavailable`; map explicit provider
   `unknown` to `Attribute::Unspecified`.
4. Fail closed on any transport error: `Attribute::Unavailable`, never a default.

**Accept**

- A transport error yields `Unavailable` for both attributes.
- A live claim yields `Known(age)` and `Known(gender)` or `Unspecified`.
- No cross-database join exists.

**Tests**

- `backend/services/concert-service/tests/claims_adapter.rs` with a fake profile
  service: live, expired, revoked, transport error, explicit unknown.

**Verify** V-02.

### T-303 — `IntroAudience` value and validation

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-108 |
| Writes | `backend/crates/concert-domain/src/intro_audience.rs` |

**Build**

1. `IntroAudience { gender: GenderCondition, minimum_age, maximum_age_exclusive }`.
2. `GenderCondition { Any, Female, Male, Unknown }` with the donor wire names.
3. Strict deserialization rejecting unknown fields and unpaired bounds.
4. `is_restricted()` returning true when an age bound is set or gender is not
   `Any`.

**Accept**

- Unpaired bounds rejected.
- `minimum >= maximum` rejected.
- `is_restricted()` is false only for `any` with no bounds.

**Tests**

- `backend/crates/concert-domain/tests/intro_audience.rs`.

**Verify** V-01, V-13.

### T-304 — `can_view_intro` pure decision

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-302, T-303 |
| Writes | `backend/crates/concert-application/src/intro_visibility.rs` |
| Contract | [domain-contract.md](domain-contract.md) §3.2 |

**Build**

1. Implement the twelve-row decision table exactly.
2. Return `Decision::Denied(reason)` with the internal reason enum.
3. The author path returns `Allowed` before any claim read.
4. An unrestricted audience returns `Allowed` without reading claims.

**Accept**

- Every row of the table has a test.
- No code path reads claims when the audience is unrestricted.

**Tests**

- `backend/crates/concert-application/tests/intro_visibility.rs` with the full
  case table shared with T-305.

**Verify** V-01, V-02, V-03, V-13.

### T-305 — SQL visibility predicate

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-304 |
| Writes | `backend/services/concert-service/src/intro_posts/visibility_sql.rs` |

**Build**

1. A fragment builder producing the same decision as T-304 for a given
   `(viewer_profile_ref, now_ref)`.
2. The fragment is used inside the page query, not applied to returned rows.
3. The fragment joins the claim source through a values list supplied by the
   adapter, not a cross-database join.

**Accept**

- For the shared case table, the SQL predicate and the pure function agree on
  every case.
- A page query with the predicate returns at most `limit` rows and a correct
  `next_cursor` derived from the filtered set.

**Tests**

- `backend/services/concert-service/tests/intro_visibility_contract.rs`: the same
  case table executed through SQL, plus a pagination case asserting no
  post-filtering.

**Verify** V-01, V-02, V-03 (case i).

### T-306 — Discovery query predicate

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-305 |
| Writes | `backend/services/concert-service/src/intro_posts/repository.rs` |

**Build**

1. `list_intro_posts(viewer, query, cursor)` applying the predicate in the query.
2. `get_intro_post(viewer, intro_id)` applying the same predicate.
3. Cursor encoding reuses `concert_application::intro::cursor` with the actor and
   query binding, so a cursor cannot be replayed by a different viewer.

**Accept**

- A cursor issued to viewer A is rejected for viewer B.
- A changed audience invalidates the visible set on the next request.

**Tests**

- `backend/services/concert-service/tests/intro_visibility_contract.rs`.

**Verify** V-03 (cases a, g, h).

### T-307 — Attachment and media authorization

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-305, T-106 |
| Writes | `backend/services/concert-service/src/profile_media/authorization.rs` |

**Build**

1. Media access is authorized through the attached post, never through file
   ownership alone.
2. For an intro attachment, re-evaluate the post's audience on every request.
3. For a profile post attachment, evaluate the post's own visibility.
4. A media asset with no live attachment is accessible only to its owner.

**Accept**

- An unauthorized viewer gets `404` for both metadata and bytes.
- The response for an authorized viewer is `no-store` and `nosniff`.
- Removing the attachment removes access without deleting the file while another
  post still references it.

**Tests**

- `backend/services/concert-service/tests/profile_media_authorization.rs`.

**Verify** V-03 (cases c, d), V-06 (case f).

### T-308 — Shared link resolution

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-306 |
| Writes | `backend/services/concert-service/src/intro_posts/links.rs` |

**Build**

1. Resolve a shared intro link through the same predicate.
2. An unauthorized resolution returns a metadata-only response with no body,
   no author, no attachments, and no thumbnail.
3. The response must not differ in shape between "does not exist" and "not
   available to you".

**Accept**

- An anonymous or unauthorized resolver cannot distinguish the two cases.
- No restricted bytes are included in any preview.

**Tests**

- `backend/services/concert-service/tests/intro_links.rs`.

**Verify** V-03 (case e).

### T-309 — Like creation and removal policy

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-305, T-109 |
| Writes | `backend/crates/concert-application/src/intro_likes.rs`, `backend/crates/concert-application/src/ports/intro_likes.rs`, `backend/services/concert-service/src/intro_posts/likes.rs` |

**Build**

1. `PUT`: evaluate `can_view_intro` before insert; `ON CONFLICT DO NOTHING` for
   duplicate prevention; reject self-like.
2. `DELETE`: delete first, then decide the response. If the viewer still
   satisfies the audience, return the current count; otherwise return
   `like_count: null`.
3. The unlike path must not read or return any restricted content.
4. No follow, request, reward, or contact side effect anywhere in this module.

**Accept**

- Two `PUT`s produce one row.
- `DELETE` by a viewer who lost access returns `like_count: null`.
- `DELETE` by a viewer who still has access returns the current count.
- No other table is written by either operation.

**Tests**

- `backend/services/concert-service/tests/intro_likes_contract.rs`.

**Verify** V-04, V-05.

### T-310 — Likers list

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-309 |
| Writes | `backend/services/concert-service/src/intro_posts/likers.rs` |

**Build**

1. Author-only: a non-author gets `403 PERMISSION_DENIED`.
2. Paginate with the same cursor discipline as discovery.
3. Omit likers who are blocked, deleted, or otherwise inaccessible; a page may be
   shorter than `limit` and must still carry a correct `next_cursor`.
4. Include `shared_with_author` computed from catalog IDs.

**Accept**

- Non-author `403`.
- A blocked liker is absent and the page remains well-formed.
- The response contains no numeric age or gender of the liker.

**Tests**

- `backend/services/concert-service/tests/intro_likers.rs`.

**Verify** V-05 (case f).

### T-311 — `INTRO_LIKED` notification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-309, T-005 |
| Writes | `backend/services/concert-service/src/notification_live/`, `backend/crates/concert-domain/src/notification/mod.rs` |

**Build**

1. Emit one `INTRO_LIKED` notification on the first successful like only.
2. Dedupe key `(recipient, kind, intro_id, actor_profile_id)`.
3. Target `INTRO_POST` with the intro id.
4. `safe_summary()` returns a fixed neutral string; the actor's nickname is
   resolved at read time, never interpolated into stored text.
5. Opening the notification re-checks authorization; the notification grants
   nothing.

**Accept**

- Two `PUT`s produce one notification.
- Removing and re-adding a like produces a second notification only if the
  dedupe window allows it, and that behavior is documented and tested.
- The stored payload contains no restricted content.

**Tests**

- `backend/services/concert-service/tests/intro_like_notification.rs`.

**Verify** V-05 (case b).

### T-312 — Claims and policy package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-301..T-311 |
| Writes | `docs/mcr/integration/evidence/v01-*.json`, `v02-*.json`, `v03-*.json`, `v13-*.json` |

**Build**

1. Execute V-01, V-02, V-03, V-13 against real services.
2. Record the shared case table result showing pure and SQL agreement.

**Accept**

- Every case `PASS`, or `BLOCKED` with the exact reason.

**Verify** V-01, V-02, V-03, V-13.

---

## 9. WP-05 — Intro post and hearts tasks

### T-401 — Intro post draft and publish commands

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-312 |
| Writes | `backend/crates/concert-application/src/intro_posts/mod.rs`, `backend/services/concert-service/src/intro_posts/commands.rs` |

**Build**

1. `save_draft`: validate body (0–1000 in draft, 1–1000 when publishing), the
   single-line fields, and the audience; check `expected_revision` in the
   transaction.
2. `publish`: require `is_restricted()` implies nothing extra at publish time,
   but require a live claim for the **author** only if the product later decides
   so — the default is no author-side requirement.
3. `delete`: soft-delete the post, cascade likes and comments, keep the room and
   profile posts untouched.
4. Contact-bypass rules from `intro_policy::contact` apply to published bodies.

**Accept**

- A draft with an empty body saves; publishing it fails.
- A body with contact-bypass content fails to publish.
- Deleting a post removes its likes and comments and nothing else.

**Tests**

- `backend/services/concert-service/tests/intro_post_commands.rs`.

**Verify** V-08.

### T-402 — Favorites snapshot import

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-401, T-104 |
| Writes | `backend/services/concert-service/src/intro_posts/snapshot.rs` |

**Build**

1. On draft creation, copy the author's current favorites into
   `favorites_snapshot`.
2. Editing the profile afterwards must not change a published post's snapshot.
3. The snapshot is a display copy; discovery uses the profile favorites, not the
   snapshot.

**Accept**

- Changing favorites after publishing leaves the post snapshot unchanged.
- The snapshot contains catalog item ids and labels only.

**Tests**

- `backend/services/concert-service/tests/intro_snapshot.rs`.

**Verify** V-08.

### T-403 — Attachment rules

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-401, T-106 |
| Writes | `backend/services/concert-service/src/intro_posts/attachments.rs` |

**Build**

1. At most 4 attachments, unique, each owned by the author, each `READY`, each
   purpose `INTRO_POST`.
2. Reject a media id owned by someone else with the same error as a missing one.
3. Reject an asset whose purpose is `PROFILE_POST` or `ROOM_POSTER`.

**Accept**

- A foreign media id is rejected without revealing whether it exists.
- A fifth attachment is rejected.

**Tests**

- `backend/services/concert-service/tests/intro_attachments.rs`.

**Verify** V-06 (cases c, d).

### T-404 — Comments and replies

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-401, T-305 |
| Writes | `backend/services/concert-service/src/intro_posts/comments.rs` |

**Build**

1. Create/list/delete comments, each gated by the parent post's audience.
2. A reply's parent must belong to the same intro post.
3. Deleting a comment keeps its replies or deletes them, per a documented rule;
   the rule must be stated in the contract and tested.
4. Comment listing is paginated and never post-filtered.

**Accept**

- A comment on a restricted post by an unauthorized viewer is `404`.
- A cross-post parent is rejected.

**Tests**

- `backend/services/concert-service/tests/intro_comments.rs`.

**Verify** V-03.

### T-405 — Intro routes in the profile product transport

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-401..T-404, T-102 |
| Writes | `backend/services/concert-service/src/intro_posts/http.rs`, `backend/services/api-gateway/src/adapters/in/http/profile_product/` |

**Build**

1. Register the 13 intro operations on the profile product route.
2. Closed target parsing: unknown paths, unknown query fields, and wrong methods
   are rejected before any producer work.
3. Body limits per operation.

**Accept**

- An unknown query field is `400`.
- A method mismatch is `400`, not a fallthrough.

**Tests**

- `backend/services/concert-service/tests/intro_routes.rs`.

**Verify** V-03.

### T-406 — Intro composer reducer

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-405 |
| Writes | `frontend/src/features/intros/composer-reducer.ts` |
| Reads | donor `intro-composer-reducer.ts` |

**Build**

1. Steps: audience → content → review, with the destination's field names.
2. Draft state survives step navigation.
3. Validation state is derived, not stored twice.

**Accept**

- Moving back and forward preserves every field.
- An invalid audience blocks the review step.

**Tests**

- `frontend/src/features/intros/__tests__/composer-reducer.test.ts`.

**Verify** V-08.

### T-407 — Audience fields component

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-406 |
| Writes | `frontend/src/features/intros/IntroAudienceFields.tsx`, `frontend/src/features/intros/IntroAgeRangeControl.tsx` |
| Reads | donor `AudienceStep.tsx`, `WritePostAgeRangeControl.tsx` |

**Build**

1. Heading exactly "Who can view my mutuals-intro post".
2. Gender select: `any`, `female`, `male`, `unknown`.
3. Age range: an enable toggle plus inclusive minimum and maximum numeric inputs,
   converted to the half-open wire form at the boundary.
4. No Mantine. No `RangeSlider`. Keyboard operable, error focus management.
5. A one-line explanation that this decides who may see the post, not who the
   author wants to see.

**Accept**

- Selecting 18–29 sends `minimum_age: 18, maximum_age_exclusive: 30`.
- An inverted range shows an inline error and blocks review.
- The heading text is exactly as specified.
- The explanation sentence is present.

**Tests**

- `frontend/src/features/intros/__tests__/IntroAudienceFields.test.tsx`: the
  conversion, the inverted range, keyboard operation, and the heading text.

**Verify** V-01, UI-360-390-430.

### T-408 — Intro composer screen

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-407 |
| Writes | `frontend/src/features/intros/IntroComposer.tsx` |

**Build**

1. Body, `currently_seeking`, `interaction_style`, favorites import, attachments.
2. Review step shows the server-derived `audience_summary`, not a client-computed
   one.
3. Publish and save-draft actions with distinct outcomes.

**Accept**

- The review step never computes the summary locally.
- Publishing without a body is blocked client-side and also rejected server-side.

**Tests**

- `frontend/src/features/intros/__tests__/IntroComposer.test.tsx`.

**Verify** V-08.

### T-409 — Media picker and preparation

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-102 |
| Writes | `frontend/src/features/media/` |
| Reads | donor `ApplicationPhotoPicker.tsx`, `media-policy.ts`, `media-preparation.ts`, `static-image-compressor.ts` |

**Build**

1. File selection, preview, removal, ordering (no dnd-kit).
2. Client-side format and size pre-check matching the server's JPEG/PNG/WebP and
   10 MiB / 4096 px / 16 M pixel limits.
3. Compression and resize with cancellation.
4. Progress and error states.

**Accept**

- A GIF is rejected client-side with the same message the server would produce.
- Cancelling mid-preparation leaves no partial upload.

**Tests**

- `frontend/src/features/media/__tests__/preparation.test.ts`.

**Verify** V-10 (cases a, b).

### T-410 — Profile media upload client

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-409 |
| Writes | `frontend/src/features/media/api/profile-media.ts` |

**Build**

1. Init → PUT bytes → finalize, with failure cleanup.
2. Retry on a transient failure without duplicating the asset.
3. Account-context change aborts in-flight uploads and clears state.

**Accept**

- A failed finalize deletes the staged asset.
- An account switch mid-upload does not leak the upload into the new account.

**Tests**

- `frontend/src/features/media/__tests__/profile-media-upload.test.ts`.

**Verify** V-09 (case c), V-10.

### T-411 — Heart button

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-309, T-405 |
| Writes | `frontend/src/features/intros/IntroLikeButton.tsx`, `frontend/src/features/intros/api/intro-likes.ts` |
| Reads | donor `PostDetailPage.tsx` heart behavior |

**Build**

1. `aria-pressed` reflects the current state.
2. In-flight guard prevents duplicate requests.
3. Optimistic update with rollback on failure.
4. A `404` on like means the post is not available; show the neutral prompt, not
   an error toast.
5. `like_count: null` renders as "—" and never as 0.

**Accept**

- Double-tapping sends one request.
- A failed like reverts the visual state.
- `null` count is never rendered as zero.

**Tests**

- `frontend/src/features/intros/__tests__/IntroLikeButton.test.tsx`.

**Verify** V-04, V-05.

### T-412 — Liker list panel

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-310, T-411 |
| Writes | `frontend/src/features/intros/IntroLikersPanel.tsx` |

**Build**

1. Author-only panel with pagination.
2. Each row: profile image or emo character, nickname, shared interests, one-line
   interaction style, and a single tap to the profile.
3. No acceptance screen, no room screen in the path.

**Accept**

- Tapping a row navigates directly to the profile.
- The shared-interest line is omitted when there is no overlap.

**Tests**

- `frontend/src/features/intros/__tests__/IntroLikersPanel.test.tsx`.

**Verify** V-05.

### T-413 — Intro post card and detail

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-411 |
| Writes | `frontend/src/features/intros/IntroPostCard.tsx`, `frontend/src/features/intros/IntroPostDetail.tsx` |

**Build**

1. Card: body excerpt, favorites snapshot, attachment count, heart, comment count.
2. Detail: full body, attachments, comments, heart, author link.
3. No room preview anywhere in either component.

**Accept**

- Neither component imports anything from `features/profile-room/`.
- A restricted post renders the neutral prompt, never partial content.

**Tests**

- `frontend/src/features/intros/__tests__/IntroPost.test.tsx`: including an
  import-boundary assertion.

**Verify** V-08 (case e).

### T-414 — Draft recovery

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-408 |
| Writes | `frontend/src/features/intros/draft-recovery.ts` |
| Reads | donor `write-draft-recovery.ts` |

**Build**

1. Persist an in-progress draft locally, scoped by account.
2. Recover it after reload or a failed publish.
3. Clear it on account switch.

**Accept**

- A draft from account A is invisible to account B.
- A recovered draft does not auto-publish.

**Tests**

- `frontend/src/features/intros/__tests__/draft-recovery.test.ts`.

**Verify** V-09 (cases c, d).

### T-415 — Activity feed

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-311 |
| Writes | `frontend/src/features/activity/` |

**Build**

1. List likes received, comments, replies, follows.
2. Each entry navigates to the target in one tap.
3. No unanswered-request indicator, no acceptance deadline, no "pending" state.

**Accept**

- A like entry opens the liker's profile.
- No entry contains an approval affordance.

**Tests**

- `frontend/src/features/activity/__tests__/ActivityFeed.test.tsx`.

**Verify** V-05 (case c).

### T-416 — Intro package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-401..T-415 |
| Writes | `docs/mcr/integration/evidence/v04-*.json`, `v05-*.json`, `v08-*.json` |

**Accept**

- V-04, V-05, V-08 `PASS`, or `BLOCKED` with the exact reason.

**Verify** V-04, V-05, V-08.

---

## 10. WP-06 — Profile post and media tasks

### T-501 — Profile media upload lifecycle

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-106, T-102 |
| Writes | `backend/services/concert-service/src/profile_media/upload.rs`, `backend/crates/concert-application/src/media/` (extend) |

**Build**

1. Extend `Purpose` with the four profile purposes; do not add a second media
   domain.
2. `init` → `put bytes` → `finalize` with the existing normalization and
   sanitization.
3. Orphan cleanup after the existing retention window.
4. Reject a purpose/context mismatch.

**Accept**

- A finalized asset is `READY` with width, height, and sanitized bytes.
- An abandoned upload is cleaned after the orphan window.
- A `ROOM_POSTER` asset cannot be finalized through the profile-post path.

**Tests**

- `backend/services/concert-service/tests/profile_media_upload.rs`.

**Verify** V-07.

### T-502 — Profile post commands

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-501, T-110 |
| Writes | `backend/services/concert-service/src/profile_posts/commands.rs` |

**Build**

1. Create with description and up to 4 `PROFILE_POST` media ids owned by the
   author.
2. Delete with revision check.
3. Reject an `INTRO_POST` media id.
4. No pixel conversion anywhere in this module.

**Accept**

- A post with an intro-post media id is rejected.
- The stored bytes are the normalized photograph, not a conversion.

**Tests**

- `backend/services/concert-service/tests/profile_posts.rs`.

**Verify** V-06 (case d), V-07 (cases b, c).

### T-503 — Profile post reads

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-502 |
| Writes | `backend/services/concert-service/src/profile_posts/read.rs` |

**Build**

1. Visitor-facing list gated by the post's own visibility.
2. The author sees their own private posts.
3. Pagination inside the query.

**Accept**

- A private post is absent from a visitor's list and `404` on direct read.
- The author sees it.

**Tests**

- `backend/services/concert-service/tests/profile_post_read.rs`.

**Verify** V-07 (case d), V-08 (case a).

### T-504 — Profile post routes

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-503 |
| Writes | `backend/services/concert-service/src/profile_posts/http.rs` |

**Build**

1. Register the three profile post operations and the five media operations on
   the profile product route.
2. Closed target parsing and body limits.

**Accept**

- Unknown routes rejected before producer work.

**Tests**

- `backend/services/concert-service/tests/profile_post_routes.rs`.

**Verify** V-07.

### T-505 — Profile media routes and byte serving

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-307, T-405, T-501 |
| Writes | `backend/services/api-gateway/src/adapters/in/http/profile_product/` |

**Build**

1. Serve metadata and bytes with the same authorization as T-307.
2. `no-store`, `nosniff`, `inline` content disposition, same-origin resource
   policy.
3. Only JPEG/PNG/WebP responses.

**Accept**

- An unauthorized byte request is `404` with no body.
- Response headers match the concert media path exactly.

**Tests**

- `backend/services/api-gateway/tests/profile_media_routes.rs`.

**Verify** V-06 (case f).

### T-506 — Profile media composer UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-409, T-505 |
| Writes | `frontend/src/features/profile-media/ProfileMediaComposer.tsx` |

**Build**

1. Pick up to 4 images, optional description, optional alt text.
2. Visibility control with the default `public`.
3. Publish without any intro post or room involvement.

**Accept**

- Publishing works with a default room and no intro post.
- No poster or pixel conversion is offered here.

**Tests**

- `frontend/src/features/profile-media/__tests__/ProfileMediaComposer.test.tsx`.

**Verify** V-07 (case a).

### T-507 — Profile media gallery

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-506 |
| Writes | `frontend/src/features/profile-media/ProfileMediaGallery.tsx` |

**Build**

1. Grid with preserved aspect ratio, lazy loading, retry on failure.
2. Author-only delete affordance.
3. Empty state that does not imply the visitor is restricted.

**Accept**

- A restricted intro attachment never appears here.
- The empty state is distinguishable from "not available to you".

**Tests**

- `frontend/src/features/profile-media/__tests__/ProfileMediaGallery.test.tsx`.

**Verify** V-06 (cases a, b).

### T-508 — Alt text and description validation

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-506 |
| Writes | `frontend/src/features/profile-media/` |

**Build**

1. Alt text up to 160 characters, description up to 500.
2. Character counting matches the server's code-point counting.

**Accept**

- A 161-character alt text is blocked client-side and rejected server-side.

**Tests**

- `frontend/src/features/profile-media/__tests__/validation.test.ts`.

**Verify** V-07.

### T-509 — Attachment isolation test

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-502, T-507 |
| Writes | `docs/mcr/integration/evidence/v06-*.json` |

**Build**

1. Execute every V-06 case against real services.
2. Record the specific attempt to republish a restricted intro attachment.

**Accept**

- All V-06 cases `PASS`.

**Verify** V-06.

### T-510 — Upload failure and cancellation handling

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-506, T-508 |
| Writes | `frontend/src/features/profile-media/` |

**Build**

1. A failed upload keeps the composer state and offers retry.
2. A cancelled upload leaves no orphan.
3. Leaving the screen mid-upload cancels cleanly.

**Accept**

- No orphan asset after cancellation.
- The composer retains the description after a failed upload.

**Tests**

- `frontend/src/features/profile-media/__tests__/failure.test.tsx`.

**Verify** V-10 (cases a, b).

### T-511 — Profile post deletion independence

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-503 |
| Writes | `backend/services/concert-service/tests/profile_post_lifecycle.rs` |

**Build**

1. Deleting a profile post leaves the room, avatar, intro posts, and favorites
   untouched.
2. Deleting an intro post leaves profile posts untouched.

**Accept**

- Both directions verified with row-count assertions before and after.

**Tests**

- `backend/services/concert-service/tests/profile_post_lifecycle.rs`.

**Verify** V-08 (cases a, b).

### T-512 — Profile post package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-501..T-511 |
| Writes | `docs/mcr/integration/evidence/v07-*.json` |

**Accept**

- V-07 `PASS`, or `BLOCKED` with the exact reason.

**Verify** V-07.

---

## 11. WP-07 — Room and poster tasks

### T-601 — Room layout value and validation

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-107 |
| Writes | `backend/crates/concert-domain/src/profile_room.rs` |
| Contract | [domain-contract.md](domain-contract.md) §5 |

**Build**

1. `RoomLayout` with wall, floor, furniture, posters, character.
2. Bounds: grid size, at most 4 posters, at most 32 furniture items, unique cell
   per layer, known style ids.
3. Strict deserialization; unknown fields rejected.

**Accept**

- 5 posters rejected.
- Two furniture items on the same cell in the same layer rejected.
- An unknown wall style rejected.

**Tests**

- `backend/crates/concert-domain/tests/profile_room.rs`.

**Verify** V-08, V-10 (case c).

### T-602 — Room persistence

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-601 |
| Writes | `backend/services/concert-service/src/profile_room/repository.rs` |

**Build**

1. `get_room(profile_id)` and `save_room(owner, layout, expected_revision)`.
2. Revision conflict returns the current room so the client can resolve.
3. A save by a non-owner is `403` with no write.

**Accept**

- Two concurrent saves with the same revision: one succeeds, one gets `409` with
  the current layout.
- A non-owner save writes nothing.

**Tests**

- `backend/services/concert-service/tests/profile_room_persistence.rs`.

**Verify** V-09 (case a), V-10 (case c).

### T-603 — Room visibility

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-602 |
| Writes | `backend/services/concert-service/src/profile_room/visibility.rs` |

**Build**

1. `PATCH /api/me/room/visibility` with the default `public`.
2. A private room is `404` for visitors; the profile reports
   `room_available: false`.
3. Room visibility never reads or writes intro audience.

**Accept**

- Changing room visibility leaves every intro post's audience unchanged.
- Changing an intro audience leaves room visibility unchanged.

**Tests**

- `backend/services/concert-service/tests/profile_room_visibility.rs`.

**Verify** V-08 (cases c, d).

### T-604 — Poster upload

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-602, T-501 |
| Writes | `backend/services/concert-service/src/profile_room/posters.rs` |

**Build**

1. Accept a PNG up to 128 px per side and 1 MiB.
2. Validate the PNG signature and decode it; reject a mismatch between declared
   and actual dimensions.
3. Store as a `ROOM_POSTER` media asset; do not re-pixelate.
4. Reject an asset referenced by another profile.

**Accept**

- A JPEG poster is rejected.
- A PNG whose declared dimensions differ from the actual ones is rejected.
- The stored bytes equal the submitted bytes after sanitization.

**Tests**

- `backend/services/concert-service/tests/profile_room_posters.rs`.

**Verify** V-06 (case e).

### T-605 — Room routes

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-603, T-604 |
| Writes | `backend/services/concert-service/src/profile_room/http.rs` |

**Build**

1. Register the five room operations.
2. Closed target parsing and body limits.

**Accept**

- Unknown routes rejected before producer work.

**Tests**

- `backend/services/concert-service/tests/profile_room_routes.rs`.

**Verify** V-08.

### T-606 — Pixel conversion

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-604 |
| Writes | `frontend/src/features/profile-room/pixelize.ts` |

**Build**

1. Crop to the poster frame, then downsample to 64×64 (fine) or 32×32 (chunky)
   with nearest-neighbour sampling.
2. Deterministic: identical input, crop, and preset produce identical bytes.
3. Runs in a worker so the UI stays responsive.
4. No color-count, dithering, or layer controls.

**Accept**

- Two runs on the same input produce byte-identical PNGs.
- A face and a logo remain recognizable at 360 px width, demonstrated by a
  captured comparison.

**Tests**

- `frontend/src/features/profile-room/__tests__/pixelize.test.ts`: determinism
  and output dimensions.
- Visual comparison captured at 360 px.

**Verify** V-10 (case e), UI-360-390-430.

### T-607 — Poster importer UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-606 |
| Writes | `frontend/src/features/profile-room/PosterImporter.tsx` |

**Build**

1. Exactly three steps: import → crop and preview → place on the wall.
2. Two presets only.
3. The preview uses the actual conversion, not a CSS filter.
4. Upload on placement, not before.

**Accept**

- No separate workshop screen exists.
- The preview matches the placed result.

**Tests**

- `frontend/src/features/profile-room/__tests__/PosterImporter.test.tsx`.

**Verify** V-10 (case e).

### T-608 — Room scene runtime

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-602 |
| Writes | `frontend/src/features/profile-room/scene/` |
| Reads | `frontend/src/world/runtime.ts`, `frontend/src/world/illustrated/` |

**Build**

1. Acquire a scoped Phaser instance for the room screen; release on unmount.
2. Nearest-neighbour filtering, depth ordering by foot position.
3. Failure cleanup: a failed asset load releases everything acquired.
4. A character in the room never implies the owner is online; no presence
   indicator is rendered.

**Accept**

- Leaving the screen destroys the scene and releases textures.
- Re-entering restores the saved layout exactly.
- No presence or "online" affordance exists.

**Tests**

- `frontend/src/features/profile-room/__tests__/scene-lifecycle.test.ts`.

**Verify** V-10 (case d).

### T-609 — Room editor UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-608 |
| Writes | `frontend/src/features/profile-room/RoomPage.tsx` |

**Build**

1. Wall and floor selection, furniture drag with grid snapping, poster slots.
2. Save with revision; on conflict keep the unsaved layout and offer explicit
   resolution.
3. Room visibility toggle.
4. Keyboard alternative for placement.

**Accept**

- A conflict never silently discards the user's layout.
- Every placement is reachable by keyboard.

**Tests**

- `frontend/src/features/profile-room/__tests__/RoomPage.test.tsx`.

**Verify** V-10 (case c), UI-360-390-430.

### T-610 — Room entry from the profile

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-609 |
| Writes | `frontend/src/features/profile/RoomEntry.tsx` |

**Build**

1. A "Room" entry inside the profile that opens the room as a subordinate screen.
2. The room is never embedded in the intro post or the profile header.

**Accept**

- No intro post component renders a room preview.
- The room opens only from the profile entry.

**Tests**

- `frontend/src/features/profile/__tests__/RoomEntry.test.tsx`.

**Verify** V-08 (case e).

### T-611 — Room persistence across sessions

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-609 |
| Writes | `frontend/src/features/profile-room/__tests__/persistence.test.tsx` |

**Build**

1. Save, reload, and verify the layout is identical.
2. Verify a visitor sees the saved layout while the owner is offline.

**Accept**

- Layout equality after reload.
- Visitor view equals owner view for a public room.

**Tests**

- `frontend/src/features/profile-room/__tests__/persistence.test.tsx`.

**Verify** V-10 (case e).

### T-612 — Room package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-601..T-611 |
| Writes | `docs/mcr/integration/evidence/v08-*.json`, `v10-*.json` |

**Accept**

- V-08 and V-10 `PASS`, or `BLOCKED` with the exact reason.

**Verify** V-08, V-10.

---

## 12. WP-08 — Avatar tasks

### T-701 — Profile avatar domain

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-103 |
| Writes | `backend/crates/concert-domain/src/profile_avatar.rs` |
| Reads | `backend/crates/concert-domain/src/avatar/mod.rs` |

**Build**

1. Port `SetAvatar::parse`, `parse_config`, `AvatarCatalog::validate` with
   `owner_profile_id` instead of `ParticipantKey`.
2. Keep the strict parsing and the manifest inventory checks unchanged.
3. No event, no participant.

**Accept**

- The same invalid inputs are rejected as before.
- No function signature accepts an event id or a participant key.

**Tests**

- `backend/crates/concert-domain/tests/profile_avatar.rs`.

**Verify** V-09.

### T-702 — Profile avatar persistence

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-701 |
| Writes | `backend/services/concert-service/src/profile_avatar/repository.rs` |

**Build**

1. `read(profile_id)` and `set_avatar(profile_id, config, expected_revision)`.
2. Revision conflict returns `409` with the current config.
3. Ownership is verified server-side; a non-owner write is `403`.

**Accept**

- A non-owner write changes nothing.
- Concurrent writes with the same revision produce one success and one conflict.

**Tests**

- `backend/services/concert-service/tests/profile_avatar_persistence.rs`.

**Verify** V-09 (case b).

### T-703 — Avatar routes

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-702 |
| Writes | `backend/services/concert-service/src/profile_avatar/http.rs` |

**Build**

1. `GET /api/me/avatar`, `PUT /api/me/avatar`.
2. No event path, no participant.

**Accept**

- The route rejects any target containing `/events/`.

**Tests**

- `backend/services/concert-service/tests/profile_avatar_routes.rs`.

**Verify** V-09.

### T-704 — Avatar editor migration

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-703 |
| Writes | `frontend/src/features/profile-avatar/`, `frontend/src/entities/emo-avatar/` |
| Reads | `frontend/src/features/avatar/motion-adoption/` |

**Build**

1. Migrate `MotionEditor` and its controller, compositor, runtime, and art
   modules to the profile-avatar paths.
2. Point persistence at `/api/me/avatar`.
3. Preserve the source pack and the atlas path/hash contract.
4. Remove the concert wording and the event gate from the editor.

**Accept**

- The editor saves and reloads the same configuration.
- No fallback character art is used.
- No event id appears in any request.

**Tests**

- `frontend/src/features/profile-avatar/__tests__/editor.test.tsx`.
- An assertion that `allow_legacy_character_fallback` remains `false` and no
  fallback path is referenced.

**Verify** V-09.

### T-705 — Avatar cache isolation on account switch

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-704 |
| Writes | `frontend/src/features/profile-avatar/` |

**Build**

1. Scope the avatar cache by account version.
2. Clear it on `ACCOUNT_CONTEXT_STALE` and on account switch.

**Accept**

- After switching accounts, the previous account's avatar never renders.

**Tests**

- `frontend/src/features/profile-avatar/__tests__/account-switch.test.tsx`.

**Verify** V-09 (cases c, d, e).

### T-706 — Avatar package verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-701..T-705 |
| Writes | `docs/mcr/integration/evidence/v09-*.json` |

**Accept**

- V-09 `PASS`, or `BLOCKED` with the exact reason.

**Verify** V-09.

---

## 13. WP-03 — Favorites and discovery tasks

### T-801 — Catalog search

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-104 |
| Writes | `backend/services/profile-service/src/catalog/` |

**Build**

1. Search across three categories.
2. Music catalog excludes K-pop by catalog policy, not by string filtering at
   query time.
3. Bounded result size and a stable ordering.

**Accept**

- A K-pop artist is absent from music search results.
- The same query returns the same order twice.

**Tests**

- `backend/services/profile-service/tests/catalog_search.rs`.

**Verify** GUARDS.

### T-802 — Profile discovery query

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-801, T-104 |
| Writes | `backend/services/concert-service/src/discovery/repository.rs` |

**Build**

1. Start from `profiles`, not from `intro_posts`, so a profile with no intro post
   is discoverable.
2. Compute overlap from catalog item ids only.
3. Apply profile visibility and blocks.
4. No score, no ranking by count.

**Accept**

- A profile with no intro post appears.
- A profile with no overlap appears without a shared-interest claim.
- The response contains no numeric score field.

**Tests**

- `backend/services/concert-service/tests/discovery.rs`.

**Verify** DISCOVERY-NO-INTRO.

### T-803 — Discovery routes

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-802 |
| Writes | `backend/services/concert-service/src/discovery/http.rs` |

**Build**

1. `GET /api/discover/people` with category filter, cursor, and bounded query.
2. Closed query parsing.

**Accept**

- An unknown query field is `400`.

**Tests**

- `backend/services/concert-service/tests/discovery_routes.rs`.

**Verify** DISCOVERY-NO-INTRO.

### T-804 — Favorite picker UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-801 |
| Writes | `frontend/src/features/favorites/` |
| Reads | donor `TasteCatalogSearchPanel.tsx`, `catalog-provider.ts` |

**Build**

1. Three tabs in the order music, anime, games.
2. Search and select up to five per category.
3. Skipping every category is allowed and does not block anything.
4. No intensity, importance, or preference controls.

**Accept**

- A sixth selection in one category is blocked.
- Skipping all categories leaves the profile usable.

**Tests**

- `frontend/src/features/favorites/__tests__/FavoritePicker.test.tsx`.

**Verify** DISCOVERY-NO-INTRO.

### T-805 — Discovery UI

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-803, T-804 |
| Writes | `frontend/src/features/discovery/` |

**Build**

1. People list with shared-interest line, interaction style, open profile, follow.
2. Mutuals-intro list as a separate tab with heart buttons.
3. The two lists are never merged.

**Accept**

- The shared-interest line is omitted when there is no overlap.
- Opening the room is not offered from a discovery card.

**Tests**

- `frontend/src/features/discovery/__tests__/Discovery.test.tsx`.

**Verify** DISCOVERY-NO-INTRO.

### T-806 — Favorites and discovery verification

| Field | Value |
| --- | --- |
| Lane | BACKEND |
| Depends | T-801..T-805 |
| Writes | `docs/mcr/integration/evidence/discovery-*.json` |

**Accept**

- The discovery checks `PASS`, or `BLOCKED` with the exact reason.

**Verify** DISCOVERY-NO-INTRO.

---

## 14. WP-09 — Navigation and screens

### T-901 — Three-section navigation

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-416, T-512, T-612, T-706 |
| Writes | `frontend/src/app/routes.ts`, `frontend/src/app/navigation.tsx` |

**Build**

1. Primary tabs: Discover, Activity, My Profile.
2. The concert screens move to a secondary menu.
3. Compatibility redirects for the old paths.

**Accept**

- Every old path still resolves, either directly or by redirect.
- The primary navigation has exactly three entries.

**Tests**

- `frontend/src/app/__tests__/navigation.test.tsx`.

**Verify** UI-360-390-430.

### T-902 — Profile page

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-901 |
| Writes | `frontend/src/features/profile/ProfilePage.tsx` |

**Build**

1. Introduction, favorites, profile image, emo character, posts, room entry.
2. Owner and visitor variants.
3. No room preview inline.

**Accept**

- The room opens only from its entry point.
- The visitor variant never shows owner-only controls.

**Tests**

- `frontend/src/features/profile/__tests__/ProfilePage.test.tsx`.

**Verify** V-08, UI-360-390-430.

### T-903 — Visitor profile page

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-902 |
| Writes | `frontend/src/features/profile/PublicProfilePage.tsx` |

**Build**

1. Shows introduction, shared interests, photos, artwork, follow.
2. Shows `gender_bucket` only when the owner opted in.
3. No numeric age anywhere.

**Accept**

- No age is rendered for any visitor state.
- Shared interests are omitted when empty.

**Tests**

- `frontend/src/features/profile/__tests__/PublicProfilePage.test.tsx`.

**Verify** V-08.

### T-904 — Router integration

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-903 |
| Writes | `frontend/src/app/router.tsx` |

**Build**

1. Register every new screen: profile, public profile, room, intro composer,
   intro detail, photo composer, activity, identity settings.
2. Lazy-load each screen.
3. Error and not-found states that do not leak restricted content.

**Accept**

- Every screen is reachable by URL.
- A not-found route renders no profile or post data.

**Tests**

- `frontend/src/app/__tests__/router.test.tsx`.

**Verify** V-11.

### T-905 — Shell and tab bar

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-901 |
| Writes | `frontend/src/app/WorldShell.tsx`, `frontend/src/shared/ui/` |

**Build**

1. Tab bar with the three sections.
2. Secondary menu for the concert screens.
3. No decorative left accent bars; focus outlines preserved.

**Accept**

- Keyboard reachable, labelled, focus visible at 360/390/430.
- No prohibited decorative pattern.

**Tests**

- `frontend/src/app/__tests__/shell.test.tsx`.

**Verify** UI-360-390-430.

### T-906 — Identity settings screen

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-209, T-904 |
| Writes | `frontend/src/features/identity/IdentitySettings.tsx` |

**Build**

1. Place `IdentityStatus` and the connect/disconnect actions in the profile
   settings.
2. Explain the purpose and the non-requirement in one sentence each.

**Accept**

- The screen is reachable from My Profile.
- Disconnect is confirmed and reversible by reconnecting.

**Tests**

- `frontend/src/features/identity/__tests__/IdentitySettings.test.tsx`.

**Verify** V-12.

### T-907 — Accessibility and responsive pass

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-905 |
| Writes | `frontend/src/features/**` (accessibility fixes only) |

**Build**

1. Keyboard reachability, focus order, labels, `aria-pressed` on the heart,
   live regions for async results.
2. 360/390/430 layout checks for every new screen.

**Accept**

- No keyboard trap.
- Every interactive control has an accessible name.
- Captures at three widths show no overflow.

**Tests**

- `frontend/src/app/__tests__/accessibility.test.tsx`.

**Verify** UI-360-390-430.

### T-908 — Navigation package verification

| Field | Value |
| --- | --- |
| Lane | FRONTEND |
| Depends | T-901..T-907 |
| Writes | `docs/mcr/integration/evidence/ui-*.json` |

**Accept**

- The UI checks `PASS`, or `BLOCKED` with the exact reason.

**Verify** UI-360-390-430.

---

## 15. WP-10 — Integration tasks

### T-951 — Three-identity fixture

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-908 |
| Writes | `frontend/e2e/real/fixtures/` |

**Build**

1. Three accounts: A (author), B (verified woman 18–29), C (verified outside the
   audience, or unverified).
2. A test NAVER application with three provider profiles whose attributes are
   fixed.
3. Deterministic catalog items for MCR and NANA.

**Accept**

- Each identity can complete verification against the test provider.
- The fixture is reproducible from a clean database.

**Tests**

- `frontend/e2e/real/profile-social.spec.ts` setup step.

**Verify** V-11.

### T-952 — End-to-end scenario

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-951 |
| Writes | `frontend/e2e/real/profile-social.spec.ts` |

**Build**

1. Execute the six steps of [product-plan.md](product-plan.md) §9.
2. Assert at each step that the expected content is visible and nothing more.

**Accept**

- All six steps pass against real services.
- Step 6 asserts `404` for the post, its attachments, and its bytes.

**Tests**

- `frontend/e2e/real/profile-social.spec.ts`.

**Verify** V-11.

### T-953 — Cross-surface consistency sweep

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-952 |
| Writes | `docs/mcr/integration/evidence/v03-sweep.json` |

**Build**

1. For one restricted post, request every surface as C: list, detail,
   attachment, thumbnail, bytes, shared link, like, comment.
2. Assert identical denial on all eight.

**Accept**

- Eight surfaces, eight identical denials, no content in any response.

**Verify** V-03.

### T-954 — Lifecycle independence sweep

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-952 |
| Writes | `docs/mcr/integration/evidence/v08-sweep.json` |

**Build**

1. Delete the intro post; assert room, avatar, and photo posts unchanged.
2. Change room visibility; assert the intro audience unchanged.
3. Change the intro audience; assert room and photo visibility unchanged.

**Accept**

- Every pairing in V-08 verified with before/after state capture.

**Verify** V-08.

### T-955 — Release evidence assembly

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-953, T-954 |
| Writes | `docs/mcr/integration/release-evidence.md` |

**Build**

1. Assemble every verification result with commit, command, exit code, artifact.
2. List every `BLOCKED` item with its exact condition.
3. State plainly what is not proven.

**Accept**

- No verification is claimed `PASS` without an artifact.
- The document distinguishes `PASS`, `FAIL`, `NOT_RUN`, and `BLOCKED`.
- `scripts/mcr/check.sh guards` exit code is recorded.

**Verify** GUARDS.

### T-956 — Release decision

| Field | Value |
| --- | --- |
| Lane | INTEGRATOR |
| Depends | T-955 |
| Writes | `docs/mcr/integration/release-decision.md` |

**Build**

1. Read the assembled evidence and decide.
2. Record what the product does and does not guarantee.
3. Record the NAVER console approval and its scope.

**Accept**

- The decision cites the evidence file for every claim.

**Verify** GUARDS.

---

## 16. Cross-cutting rules for every task

1. **Never weaken an existing guarantee.** Session, CSRF, origin, revision,
   no-store, fail-closed policy, and the account-deletion saga stay intact.
2. **No fake authority.** No event id, no test-only authority, no placeholder
   claim in a production path.
3. **No second implementation.** Reuse the destination's media domain, age
   arithmetic, cursor codec, notification pipeline, and Phaser runtime. A task
   that needs behavior those do not provide extends them; it does not fork them.
4. **Evidence or it did not happen.** Each task records its command, exit code,
   and artifact path.
5. **Status honesty.** A compile is not integration; a unit test is not a
   multiuser flow; a mock is not a real service.
6. **One writer per scope.** If a task needs a file outside its `Writes`, it
   requests a transfer and records it.
7. **English only** in code, tests, commits, and documentation.

## 17. What is deliberately not broken down here

| Item | Why |
| --- | --- |
| Visual design of each screen | The design target is a separate concern; tasks specify behavior and accessibility, not pixel composition. |
| Infrastructure and deployment | Out of scope for this breakdown; the foundation's existing infra applies. |
| Performance qualification | Deferred by the earlier MCR decision; not a gate for this integration. |
| Licensing resolution | Recorded as unresolved in [source-register.md](source-register.md) §6; must be settled before any public release, but it does not block development. |
