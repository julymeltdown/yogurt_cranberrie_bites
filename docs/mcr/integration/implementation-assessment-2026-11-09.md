# Implementation assessment — 2026-11-09

## Finding and scope

**Yogurt Cranberrie Bites is being developed as a profile-centered social
application. Its individual feature modules are substantially ahead of its
production composition. The integrated product is not yet demonstrated or
accepted for release.**

The main unfinished work is the common path from an Auth account to a canonical
profile, authorized service calls, conformant responses, visible screens, and
eventual account deletion. Completing those connections requires application
logic and explicit service ownership. Adding navigation links alone is
insufficient.

| Assessment boundary | Value |
| --- | --- |
| Delivery repository and branch | `julymeltdown/yogurt_cranberrie_bites`, `master` |
| Purpose-change baseline | [745ea437](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/745ea437e28deef6d989d075e444e250c3c4ebe9) |
| Assessed application revision | [02c0b321](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/02c0b32163bad1226332b5174aa32c43abee033a) |
| Snapshot timestamp | 2026-11-09 06:27:35 Asia/Seoul |
| Compared range | Baseline excluded, assessed revision included |
| Changes after the baseline | 19 commits: 12 non-merge commits and seven merges; 278 changed files |
| This assessment | Source, contract, task-graph, package-evidence, and hosted CI analysis |
| Not performed for this assessment | Application startup, live database scenarios, browser execution, real NAVER authorization, or new application test runs |

Source links pin the assessed revision. Documentation published after it does
not imply a new application implementation. This assessment does not amend
frozen interfaces, remove required features, accept tasks, or reopen the original
MCR task register.

### Reading evidence correctly

- **Observed in source:** a route, query, schema, emitted object, dependency, or
  missing production reference was inspected directly.
- **Recorded execution:** a package report or GitHub Actions log records a run.
  Its environment and limitations remain part of the result.
- **Inferred consequence:** the inspected paths imply a likely failure, but that
  browser/database/provider sequence was not executed during this assessment.

An isolated handler test can be useful without establishing that the real
gateway reaches the handler. A generated-contract check can pass while a
handwritten response or decoder disagrees with that contract.

## 1. What the purpose change means

The retained MCR application centers on a concert world: event admission,
multiplayer presence, meetups, giveaways, and a shared wall. The new product
centers on a persistent profile and the people who share its interests.

| Dimension | Retained MCR application | Integrated product requirement |
| --- | --- | --- |
| Primary context | Concert/event and shared world | Profile and selected music/anime/game interests |
| Main navigation | World, Feed, Meet, Gifts, Wall, Me | Discover, Activity, My Profile |
| Discovery | People and actions in the event context | Shared catalog items and a separate mutuals-intro list |
| Expression | Avatar and world activity | Profile, ordinary photos/artwork, intro posts, and one personal room |
| Heart | Existing event behavior | Explicit intro-post like; no application, acceptance, reward, or automatic follow |
| Audience | Existing event admission/age machinery | Per-intro visibility based on verified NAVER viewer attributes |
| Room | Shared world lifecycle | Saved profile-owned room, accessible while its owner is offline |
| Photographs | Retained event media paths | Independent profile posts; pixel conversion only for room posters |

Music, especially emo, is the entry point. Favorites are optional, with at most
five selections per category; K-pop is excluded from the music catalog policy.
Shared items are shown explicitly rather than converted into compatibility
scores. Intro posts, photo posts, and rooms have independent ownership and
visibility. Photo comments are explicitly in scope. Existing concert screens
remain secondary; direct messaging and live room presence are outside the
initial scope.

**Authority:** [product plan](product-plan.md), including product entities,
conversations, room behavior, and decisions P-02 through P-07;
[integration charter](integration-charter.md), including release boundaries.
The retained event shell is an implementation gap, not evidence that the product
decision reverted.

## 2. Work delivered after the baseline

The comparison contains these twelve non-merge commits; seven additional
commits merge their branches.

| Commit | Delivered change | What the commit does not establish |
| --- | --- | --- |
| [0db29465](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/0db29465f6967997fbac87aa8ca18aef9f990a3f) | Records verified NAVER age/gender sourcing. | Executed provider verification. |
| [81a1eb8b](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/81a1eb8bd8cf789e21b5885c72611004a350ae11) | Adds the 110-task breakdown and machine register. | Completion of those tasks. |
| [20b061c0](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/20b061c0e8fff589a3d14a20c7b8be8e8a73f9a3) | Freezes 41 added profile/intro/room/identity operations. | Runtime support for every operation. |
| [c79ed25c](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/c79ed25c1e0bca8baf8b705de9b262f01a0e7044) | Adds intro comment create/delete contracts; total additions become 43 operations. | Photo-post comments or comment implementations. |
| [765ea1c2](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/765ea1c2cc19b16054e472ef8478aa2b6e017025) | Adds profile/social schemas and event-free product transport. | Canonical lifecycle provisioning and complete dispatch. |
| [d0331fec](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/d0331fec7925f6114a1ab6f7ef8defaf7ab13424) | Adds catalog, favorites picker, shared-interest query/client/components. | Favorites saving, projection updates, or mounted discovery APIs. |
| [ed67aa65](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/ed67aa65a95ce5340991a20cf61f7eee01005ee4) | Records the discovery commit in its evidence. | Additional product behavior. |
| [ac8e8ce1](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/ac8e8ce11771bac92b0fe6d3cf599e62cfa96415) | Re-keys avatar implementation to profile ownership. | Producer dispatch or complete server account-context enforcement. |
| [b0b4f18f](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/b0b4f18fa82075f5846dc99419b7bf1b70155a8e) | Adds event-free NAVER verification and UI modules. | A composed return screen or real-provider round trip. |
| [b9314d93](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/b9314d93ab4f1873aef722cedb016f9208bdf0f1) | Adds saved room, furniture/posters, pixel conversion, and scoped scene. | Reachable owner editing or shared-shell/browser qualification. |
| [37ffb00b](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/37ffb00bb041e55515914949bd82a096a92530d9) | Adds photo posts/media and composer/gallery modules. | Full profile responses, comments, or production dispatch. |
| [7b276bc2](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/7b276bc2dbcd77d4ea3756e5f4df6dc926968fb4) | Resolves duplicate migration 0086 by moving avatars to 0087. | Application integration or a passing combined CI run. |

The raw operation registries grow from **15 to 25 core operations** and **117 to
150 domain operations**, with no removed operation IDs in this range. Fourteen
new migrations span profile-service 0011–0016 and concert-service 0080–0087.
The 278-file diff includes large generated artifacts and fixtures. Neither file
count nor line count measures feature completion.

**Sources:** [core operation registry](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/contracts/core/v1/operations.json),
[domain operation registry](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/contracts/domain/v1/operations.json), and
the commit links above. Package evidence is indexed in
[README.md](README.md#current-implementation-and-evidence).

## 3. The production path is incomplete

The gateway declares profile, favorites, room, avatar, photo, and media routes.
Most are forwarded to a profile-service product dispatcher whose closed
operation set recognizes only:

1. `getOwnProfile`
2. `getProfile`
3. `getOwnIdentityVerification`
4. `startIdentityVerification`
5. `disconnectIdentityVerification`

The anonymous NAVER callback has a separate transport. Other operations do not
become implemented merely because the gateway can match their URLs. The parser
rejects targets outside its admitted set.

Room, avatar, photo, and media handlers live in concert-service and operate on
its database. There is no completed production adapter from the profile
dispatcher to those handlers. Discovery's catalog/search and people query lack
gateway mounts; their local profile/favorite/block mirrors lack a production
writer.

**Sources:** [gateway route definitions](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/src/adapters/in/http/profile_product/mod.rs),
[profile operation parser and producer](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs),
[profile transport authority](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/mod.rs),
[profile RPC contract](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/proto/profile/v1/profile_product.proto),
[discovery limitations](evidence/discovery-wp03.md),
[photo/media evidence](../backend/wp06-profile-posts.md),
[room evidence](../backend/wp07-room.md), and
[avatar evidence](../backend/wp08-avatar.md).

The connection must preserve canonical actor/profile binding, deadline,
authorization freshness, revision/idempotency behavior, and response completion
rules. Calling a persistence function with a UUID does not supply those
guarantees.

## 4. Profile identity and lifecycle are the first shared blockers

### 4.1 Migration backfill is not ongoing profile provisioning

The identity migration creates a separate `profile_identities` model. It
backfills profiles present at migration time, deriving a distinct profile UUID
from each legacy Auth user UUID. The retained profile repository continues to
insert/update the old `profiles` table. No production insert or synchronization
path into `profile_identities` was found in the profile/auth implementation.

The new own-profile read exclusively queries the new table. **Inferred
consequence:** a user registered after migration can complete retained onboarding
without obtaining the identity required by the new product. Retained nickname,
bio, privacy, and deletion changes can leave the backfilled copy stale. This
was not reproduced through live signup.

**Sources:** [identity migration and backfill](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/migrations/0011_profile_identity.sql),
[retained profile persistence](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/adapters/out/persistence/profile_repository.rs),
[new profile reads](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs).

**Completion evidence needed:** migrate an empty database, register normally
without manual SQL, obtain exactly one stable profile identity, edit nickname,
bio, and visibility, restart services, and read the same canonical state through
the new product. Repeat for a migration-backfilled account.

### 4.2 Account UUID and profile UUID are currently conflated

The migration deliberately makes `profile_id` different from `account_id`.
The gateway nevertheless populates both RPC `user_id` and `profile_id` with the
Auth actor UUID. The profile transport proves the account identity but only
checks that the profile field has UUID syntax; that does not establish the
account-to-profile ownership relation.

The present own-profile read uses the account actor in SQL, so the mismatch
does not by itself prove that this read fails. Connecting modules to the
supplied profile scope can, however, address the wrong identity namespace.

**Sources:** [gateway request construction](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/src/adapters/out/profile_product_client.rs),
[profile request authentication](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/mod.rs),
[owner-scoped execution](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/scoped.rs),
and the identity migration above.

**Completion evidence needed:** account/profile UUIDs must differ in fixtures.
Resolve the mapping from authoritative data, bind it to service calls, and prove
post/media/room/avatar writes use the profile UUID. Reject a tampered internal
profile binding before mutation.

### 4.3 Profile editing and favorites saving are missing commands

The frozen operations include `updateOwnProfile` and `setOwnFavorites`; the
producer implements neither. The favorites controller supports catalog search
and local selection but has no save request. T-104 specifies table constraints;
it does not supply the complete mutation and projection path.

Discovery reads `discovery_profiles`, `discovery_favorite_items`, and
`discovery_blocks`. Tables and seeded fixtures are not a synchronization
mechanism. Until a writer is connected, real profile changes do not drive
discovery results.

**Sources:** [profile producer](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs),
[favorites controller](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/favorites/controller.ts),
[discovery repository](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/discovery/repository.rs),
and [WP-03 limitations](evidence/discovery-wp03.md).

**Completion evidence needed:** save favorites through the gateway, restart,
read them from the profile, and discover overlap from a second account without
manually inserting projection rows.

## 5. Frozen contracts and actual payloads disagree

| Surface | Frozen requirement | Actual producer/consumer | Consequence |
| --- | --- | --- | --- |
| Own profile | Twelve required fields | Seven emitted; no `favorites`, `avatar`, `room_summary`, `created_at`, `updated_at` | HTTP success does not mean a conformant own profile. |
| Public profile | Twelve required fields | Four emitted; no `avatar`, `favorites_public`, `shared_with_viewer`, `interaction_style`, `gender_bucket`, `follow`, `blocked`, `room_available` | Visitor profile composition is unfinished. |
| Discovery card | `PeopleCard.avatar` required | Backend omits it; strict frontend decoder excludes it | A correct backend response will also require a decoder update. |
| Photo-post author | Full `PublicProfileV2` | Backend and strict frontend decoder use only `{profile_id}` | Compatible local mocks conceal the contract mismatch. |
| Profile media | New operations reference inherited feed media/upload schemas | Implementation uses profile purposes and event-free paths | Generated and handwritten clients describe different payloads. |

The profile gateway checks status, content type, and JSON syntax; it does not
validate the emitted object against the full frozen response schema.

**Sources:** [core schemas](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/contracts/core/v1/core.schema.json),
[social schemas](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/contracts/domain/v1/social.schema.json),
[actual profile responses](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs),
[discovery decoder](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/discovery/contract.ts),
[photo projection](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/profile_posts/read.rs),
[photo/media decoder](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/profile-media/contract.ts),
[gateway response checks](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/src/adapters/in/http/profile_product/mod.rs).

### Media needs one consistent contract

Inherited feed schemas allow purposes `POST | GIVEAWAY | REPORT | BEACON`
and event-scoped upload/read paths. The new profile implementation uses
`PROFILE_IMAGE | PROFILE_POST | INTRO_POST | ROOM_POSTER`,
`/api/me/media/...`, and `/api/media/{mediaId}/content`.

The room poster projection adds a third interpretation: purpose `POST` to fit
the old enum and `/api/profiles/room/media/{mediaId}/content`. No matching gateway
read route was found; the mounted media-content route is
`/api/media/:media_id/content`.

The room loader reconstructs the unregistered path, leaves its `_auth` argument
unused, and calls plain `fetch` without the bearer transport used elsewhere.
Changing the URL alone would not restore normal authenticated image loading.

**Sources:** [feed media/upload schemas](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/contracts/domain/v1/feed.schema.json),
[room poster projection](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/crates/concert-domain/src/profile_room.rs),
[room route and poster loader](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/profile-room-entry/RoomPanel.tsx),
[registered media routes](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/src/adapters/in/http/profile_product/mod.rs),
[shared authenticated HTTP](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/shared/api/http.ts).

**Completion evidence needed:** validate actual gateway payloads against
canonical schemas and pass the same responses through production frontend
decoders. Cover media init, byte upload, finalize, metadata, and protected bytes
from both gallery and room. Any frozen-contract amendment must follow the
charter decision process; do not silently weaken schemas to match partial code.

## 6. Privacy, deletion, and provider identity need explicit invariants

### 6.1 Public profile reads omit the complete relationship policy

The implemented new public-profile read checks its copied row's visibility and
whether the viewer owns it. It does not consult both-direction blocks or the
canonical target lifecycle through the retained policy path.

The retained profile gateway has explicit lifecycle and relationship checks.
The new transport's preliminary identity read concerns the caller, not the
requested target. A stale copied visibility/deleted state compounds this gap.

Room, posts, and media helpers depend on supplied visibility facts or narrower
local predicates. Their missing production dispatch must supply complete target
policy before exposing those helpers.

**Sources:** [new public profile read](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs),
[retained target visibility enforcement](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/crates/gateway-application/src/service/profile_gateway_service.rs),
[caller identity precheck](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/src/adapters/out/concert_identity.rs),
[room read predicate](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/profile_room/repository.rs),
[photo read predicate](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/profile_posts/read.rs),
[media authorization facts](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/profile_media/authorization.rs).

**Confidence boundary:** missing policy calls are observed in source. A live
blocked-user disclosure was not executed in this assessment.

**Completion evidence needed:** two accounts, either-direction block, privacy
change, and deletion; recheck direct profile, post list, room, media metadata,
and media bytes, including previously obtained URLs and concurrent reads.

### 6.2 New deletion helpers are disconnected from the real saga

New migrations provide erasure functions and progress ledgers. No production
Rust callers were found for the new profile erasure functions or profile-social
ledger. The retained deletion receipt still scrubs legacy profiles/avatars and
records its existing completion proof.

The concert-side guard also takes `author_profile_id` or `owner_profile_id` and
compares it with `concert_world_account_deletion_fences.user_id`. Terminal
checks likewise compare profile-owned rows with an account UUID. Tests using
identical UUIDs for both identities cannot establish behavior for distinct IDs.
The avatar migration's comment that the saga will remove rows is not a runtime
deletion hook.

**Sources:** [new profile erasure functions](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/migrations/0015_profile_account_deletion.sql),
[retained deletion transaction](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/deletion_receipt/mod.rs),
[profile-social fence and ledger](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/migrations/0085_profile_social_deletion.sql),
[production deletion category registry](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/src/config/private_data_runtime.rs),
[avatar migration](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/migrations/0087_profile_avatars.sql),
[schema test fixtures](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/tests/profile_social_schema.rs).

**Inferred consequence:** the old deletion step can report its legacy completion
without cleaning the new data. This was not observed through a live deletion.

**Completion evidence needed:** populate every new resource under account UUID
different from profile UUID, delete through the actual saga, retry and restart
midway, and prove claims, favorites, identities, posts/children/media, room,
avatar, and discovery projections are erased or terminal. Race writes against
the correct account fence.

### 6.3 NAVER re-verification uses inconsistent subject keys

The claim store checks duplicate live provider identities using canonical
`claim.subject_hash`. When replacing a prior claim or following a non-live
binding, it stores the new VERIFIED row using `superseded_subject_hash(...)`
instead. The adjacent comment says the live row always retains the canonical
hash, which does not match the insertion code.

This lookup/write invariant inconsistency is observed. Its possible effect on
cross-account provider uniqueness has not been reproduced. In particular, the
combined case of an existing user switching provider subjects and another user
verifying that same new subject needs a focused test. Separate first-link
duplicate and self-reverification tests do not cover that composition.

**Sources:** [claim replacement and duplicate checks](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/identity/repository.rs),
[claim uniqueness and immutable evidence](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/migrations/0013_identity_claims.sql),
[existing callback tests](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/tests/identity_callback.rs).

**Completion evidence needed:** real PostgreSQL, two application accounts, and
multiple fake-provider subjects; initial verification, same-subject retry,
subject switch, revoke/expire/reverify, and concurrent callbacks. Require one
authoritative current binding per actual provider subject while preserving
historical evidence separately.

### Provider approval is separate from provider execution

NAVER developer-console review is recorded as **APPROVED by the user** in
[task-breakdown.md](task-breakdown.md) and [task-register.json](task-register.json).
The verification report separately says its environment lacked the configured
test application, credentials, and callback needed for a real NAVER round trip.
Tests used real PostgreSQL and the provider HTTP client against a fake loopback
provider.

That evidence limitation is a configuration/execution prerequisite; it does
not ask the user to approve the scope again. Verification applies to the viewer
of an age/gender-restricted intro, not an additional default publication
requirement for its author. The first-time `NOT_VERIFIED` DTO itself is
supported; no intrinsic first-time decoder deadlock was found.

**Sources:** [NAVER evidence and limitations](evidence/v12-naver-verification.md),
[viewer verification policy](product-plan.md),
[identity status projection](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/identity/status.rs).

## 7. Frontend composition has additional concrete gaps

### 7.1 The application still boots through the event shell

The router defines six inherited tabs and places panels under `WorldBoundary`.
`WorldShell` performs admission/event bootstrap; startup still refers to
`VITE_MCR_EVENT_ID`. The new Discover, favorites, verification, photo
composer/gallery, and complete profile surfaces are not composed into the
planned primary journey.

Discovery links to `/profiles/{profile_id}`, while the current person route is
`/people/:userId`. This is a second route mismatch even after mounting the
component.

**Sources:** [route definitions](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/routes.ts),
[router composition](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/router.tsx),
[world shell](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/WorldShell.tsx),
[startup](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/main.tsx),
[Discovery links](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/discovery/Discovery.tsx).

### 7.2 The registered room entry cannot expose the owner editor

The route `/people/:userId/room` passes its path value as `profileId`.
`RoomPage` therefore calls the visitor read. Owner controls appear only when no
profile ID was provided or the returned object has `isOwner === true`.
The visitor decoder does not supply that flag; the separate own-room decoder
does.

**Inferred consequence:** the current app entry cannot display room editing even
when its visitor is the owner. The editor exists, but its owner flow is not
composed.

**Sources:** [room route adapter](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/profile-room-entry/RoomPanel.tsx),
[room loading and editor condition](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/profile-room/RoomPage.tsx),
[own/visitor room decoders](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/profile-room/api/room.ts).

### 7.3 Room and shell own separate Phaser instances

The world host remains mounted beside the panel outlet. Hiding the world sleeps
its loop rather than destroying its game. The room scene separately constructs
`new Phaser.Game` and owns its destruction.

When the world is available and the room reaches ready state, the code allows
two retained game instances. The charter excludes a second instance and the
implementation plan specifies a shared room lease. No memory, frame-rate, or
listener-interference measurement was performed here.

The room scene also uses a flat character rectangle rather than consuming the
saved emo avatar/customization required by the product plan.

**Sources:** [world host lifetime](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/WorldHost.tsx),
[world game construction and visibility](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/world/scenes/MapScene.ts),
[room game and flat character texture](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/profile-room/scene/room-scene.ts),
[integration charter](integration-charter.md),
[implementation plan](implementation-plan.md).

### 7.4 Successful verification defaults to an unregistered return screen

Backend and frontend default to `/me/identity`. The callback redirects there
with an identity result. The app has no registered identity route, and the new
status/callback components are not mounted in production composition.

**Inferred consequence:** a successful provider callback can return to a
not-found screen. Choosing another allowlisted URL still requires composing
the status refresh there.

**Sources:** [allowed/default return paths](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/identity/start.rs),
[callback redirect](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/profile-service/src/profile_product/targets.rs),
[verification controller](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/identity/controller.ts),
[application routes](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/app/routes.ts).

**Completion evidence for this section:** actual owner/visitor entries,
first-time verification and return, restricted-content retry, saved room
editing/posters/avatar, route transitions, and narrow-viewport captures through
the real application shell.

## 8. Required behavior missing from implementation or task ownership

| Required behavior | Current gap | Needed ownership clarification |
| --- | --- | --- |
| Profile provisioning/synchronization | Backfill only; retained writes do not maintain the new model. | Profile/Auth lifecycle implementation, beyond migration ownership. |
| Profile edit and favorites save | Declared operations, no producer commands or saving controller. | Profile commands and frontend save flow. |
| Projection synchronization | Discovery reads mirrors with no production writer. | Profile/Friend changes to Concert projection, including freshness/deletion. |
| Profile feature dispatch | Gateway forwards to a closed five-operation producer. | Cross-service adapter and authority scope. |
| Photo-post comments | Product P-05 requires them; comments table/API remain intro-only. | Shared parent model, contract amendment, backend/frontend scopes. |
| Profile-shaped follow | Contract and UI call exist; production adapter absent. | Friend adapter preserving independent, no-approval behavior. |
| Activity/notification operations | Emission/frontend tasks do not implement all read/mark endpoints. | Backend adapter/read-model tasks as well as frontend composition. |
| New-resource account deletion | SQL helpers/ledgers are disconnected from the saga. | Runtime cleanup/fencing owner per resource/projection. |
| Account-context generation | Client recognizes `ACCOUNT_CONTEXT_STALE`; server enforcement remains partial. | Server/client account-switch behavior across profile operations. |

Photo comments are not an optional enhancement. The product plan says they
share an implementation with intro comments. Current `intro_comments.intro_id`
references `intro_posts` and cannot accept an independent photo-post ID.
Contract-freeze prose saying intro create/delete comment operations were not
frozen is stale: those operations were subsequently added. The missing portion
is photo comments.

The inherited Friend repository can produce a PENDING request in some flows.
Reusing it requires deliberate adaptation to the new no-approval semantics,
not merely renaming a call.

**Sources:** [product decision P-05](product-plan.md),
[HTTP operation plan](http-contract.md),
[task definitions](task-breakdown.md),
[intro-comment foreign key](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/concert-service/migrations/0083_intro_children.sql),
[photo gallery controls](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/frontend/src/features/profile-media/ProfileMediaGallery.tsx),
[retained follow repository](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/friend-service/src/adapters/out/persistence/follow_repository.rs),
[partial account-context evidence](../backend/wp08-avatar.md).

## 9. The task register cannot currently drive a release decision

### Status values are stale

All 110 task rows and all eleven package rows say `NOT_STARTED` while separate
implementation/evidence documents describe delivered work. The thirteen
verification rows remain planning entries, not a consolidated execution result.
Neither “0 of 110 complete” nor “seven package areas committed, therefore
seven-elevenths complete” is supported.

Use separate dimensions: declared contract, implementation module, production
reachability, real-service evidence, and browser acceptance. Reconcile task
status with the responsible implementation owner's evidence and disposition.

### The advertised seven-task critical path is not the registered graph

The breakdown describes
`T-001 → T-101 → T-201 → T-301 → T-401 → T-901 → T-951` as a seven-task longest
chain. Reading the actual `depends_on` graph produces:

| Graph property | Result |
| --- | --- |
| Task rows | 110 |
| Lane totals | 61 backend, 35 frontend, eight contract, six integrator |
| Unknown dependency IDs | 0 |
| Dependency cycles | 0 |
| Longest structural chain by task count | 48 tasks / 47 edges |
| Terminal nodes | T-112, T-806, T-956 |
| Release T-956 prerequisite closure, including itself | 102 of 110 tasks |
| Tasks outside that closure | T-111, T-112, T-801 through T-806 |

None of the six adjacent pairs in the advertised sequence is a direct
dependency. It summarizes package order but is not the registered chain.
Durations are absent, so **48 tasks is not an elapsed-time critical-path
estimate**.

The release omission matters more than the chain length: account deletion
(T-111), migration verification (T-112), and all favorites/discovery work
(T-801–T-806) remain outside the release task's prerequisite closure. At package
level WP-09 depends on WP-03; task T-901 does not depend on T-806.

### “Integration” is not an assigned production implementation scope

WP-10 owns `frontend/e2e/` and integration evidence documents. Its tasks prepare
fixtures, run the scenario, collect evidence, and record the release decision.
They do not assign profile producer changes, inter-service RPC composition,
provisioning, or projection writers.

Missing production connections need explicit owners and write scopes before
WP-10 can verify them. This report recommends reconciliation; it does not
silently transfer production files to the integrator.

**Sources:** [task register](task-register.json),
[task breakdown](task-breakdown.md), [work package register](work-packages.json),
[implementation plan](implementation-plan.md).

### The existing planning checker has a different scope

`scripts/mcr/check-plan.py` checks the preserved original MCR baseline,
generated exports, hashes, routing, and required release checklist. It does not
validate this integration's 110-task register or establish completion.
A passing result remains useful for preservation; it cannot certify the
new dependency graph or implemented product behavior.

**Source:** [planning checker](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/scripts/mcr/check-plan.py).

## 10. CI distinguishes inherited failures from new failures

The baseline and assessed snapshot have hosted runs, so the comparison need not
be guessed.

| Check | Baseline 745ea437 | Assessed application 02c0b321 |
| --- | --- | --- |
| Hosted run | [42493910623](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42493910623) | [42525306588](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588) |
| Frontend tests | 5,707 passed; 16 failed; 14 skipped; two failing files | 5,894 passed; 19 failed; 14 skipped; five failing files |
| Frontend typecheck/lint/build | Passed | Passed |
| Backend check/core-tests job | Passed | Failed during Cargo check; subsequent core tests did not run |
| Plan/guards/contracts job | Passed | Failed at QA01 coverage after plan/guards/generated checks passed |
| QA10 evidence consistency | Passed | Passed |

The inherited sixteen failures are fifteen photo-client tests in the old
meet-beacon feature and one map-cozy historical-asset test. The latter refers to
a pinned historical revision lacking `handoff.json`.

The three additional frontend failures concern fixed expectations affected by
additive contracts: the old core-schema byte pin, a C07 input-hash manifest, and
the generated operation count expected as 140 but now 173. All five failing
test files are unchanged between the compared revisions. Correcting them must
preserve retained guarantees while acknowledging authorized additions; simply
deleting assertions is not an acceptance criterion.

The new backend failure is an existing integration fixture constructing
`profile_service::config::AppConfig` without `naver_oauth` and
`identity_subject_salt`. The fixture is unchanged in the compared range.
QA01 reports drift in thirteen pinned inputs; contract generators themselves
passed.

**Job evidence:** baseline frontend
[128771936295](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42493910623/job/128771936295);
current frontend
[128867527805](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588/job/128867527805);
current backend
[128867528099](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588/job/128867528099);
current contracts
[128867528112](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588/job/128867528112).
Backend source:
[affected AppConfig fixture](https://github.com/julymeltdown/yogurt_cranberrie_bites/blob/02c0b32163bad1226332b5174aa32c43abee033a/backend/services/api-gateway/tests/support/concert_profile_owner_fixture.rs).

These logs do not establish the new product's acceptance. Conversely, the
nineteen frontend failures must not all be attributed to the new feature work.
The comparison identifies what was inherited and what the combined change adds.

## Remaining work sequence

This sequence keeps discovery, photo comments, and rooms in the approved scope.
The lanes below are suggested accountable implementation areas, not a transfer
of existing file ownership.

| Order | Outcome | Implementation areas needing explicit assignment | Evidence resolving the gap |
| --- | --- | --- | --- |
| 1 | Reconciled execution plan | Contract lead and implementation owners | Missing commands/composition/lifecycle scopes assigned; release dependencies cover required work; status reconciled from evidence. |
| 2 | Trustworthy combined baseline | Affected tests/configuration/evidence owners | AppConfig fixture fixed; additive contract changes reconciled with pins; inherited failures tracked and resolved separately. |
| 3 | Working profile-to-discovery flow | Auth/Profile, Concert projection, gateway, profile/favorites UI | Fresh signup, canonical profile, edit/save favorites, complete profile read, projection update, and second-account discovery through real HTTP. |
| 4 | Consistent identity and authorization lifecycle | Profile, Friend, Concert, gateway | Distinct account/profile IDs; both-direction blocks; subject uniqueness; account-context checks; deletion/restart/race evidence. |
| 5 | Intro completion and existing feature connections | WP-04/WP-05 plus media/room/avatar owners | Audience on every read path; intro/hearts/likers/comments/notifications; dispatch to persisted media/room/avatar handlers. |
| 6 | Composed product UI | Frontend shell/profile/discovery/identity/media/room owners | Three tabs; secondary concert access; valid routes; owner editor; authenticated posters; shared Phaser lifetime; saved avatar. |
| 7 | Integrated acceptance | Implementation owners and WP-10 evidence owner | Real-service three-identity scenario; real NAVER execution; privacy/deletion/independence checks; required viewport evidence. |

The first useful vertical milestone is a newly registered account editing its
real profile/favorites and becoming discoverable to another account through the
actual request path. That exercises canonical identity, missing commands,
response composition, projection updates, gateway routing, and frontend saving
together.

Intro visibility and heart/comment work can proceed in parallel once their
shared prerequisites have explicit owners. Room and ordinary photo ownership
remain independent. A release claim must follow observable behavior and
required evidence, not package titles, generated volume, or stale status rows.

## Reproducing the read-only inventory

Counts can be recomputed without starting the application:

```sh
git rev-list --count 745ea437e28deef6d989d075e444e250c3c4ebe9..02c0b32163bad1226332b5174aa32c43abee033a
git rev-list --count --no-merges 745ea437e28deef6d989d075e444e250c3c4ebe9..02c0b32163bad1226332b5174aa32c43abee033a
git diff --name-only 745ea437e28deef6d989d075e444e250c3c4ebe9 02c0b32163bad1226332b5174aa32c43abee033a
```

For the task graph, run this at the assessed revision:

```python
import json
from functools import lru_cache

with open("docs/mcr/integration/task-register.json") as source:
    rows = json.load(source)["tasks"]
tasks = {row["id"]: row for row in rows}
assert len(tasks) == len(rows)
assert all(dep in tasks for row in rows for dep in row["depends_on"])
visiting = set()

@lru_cache(None)
def longest(task_id):
    if task_id in visiting:
        raise ValueError(f"Cycle at {task_id}")
    visiting.add(task_id)
    parents = [longest(dep) for dep in tasks[task_id]["depends_on"]]
    result = max(parents, key=len, default=()) + (task_id,)
    visiting.remove(task_id)
    return result

paths = [longest(task_id) for task_id in tasks]
closure = set()

def include_prerequisites(task_id):
    if task_id in closure:
        return
    closure.add(task_id)
    for dependency in tasks[task_id]["depends_on"]:
        include_prerequisites(dependency)

include_prerequisites("T-956")
used = {dep for row in rows for dep in row["depends_on"]}
print("tasks:", len(tasks))
print("longest structural chain:", len(max(paths, key=len)))
print("terminal tasks:", sorted(set(tasks) - used))
print("release closure including T-956:", len(closure))
print("outside release closure:", sorted(set(tasks) - closure))
```

This is dependency inspection, not task execution. It returns 110 tasks, a
48-task longest chain, three terminal tasks, and a 102-task release closure.

The accompanying documentation change is limited to the root README,
integration index, and this report. Application code, schemas, task statuses,
historical evidence, and release dispositions remain unmodified.
