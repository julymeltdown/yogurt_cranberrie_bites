# Contract freeze record — WP-01

Status: **CONTRACT_READY**. Work package WP-01 (T-001 … T-008) is implemented,
regenerated, and verified. This document is the freeze record required by
[T-008](task-breakdown.md#t-008--contract-review-gate).

**After this point a contract change requires a dated decision-log entry in
[integration-charter.md](integration-charter.md) §7.** Adding an operation, a
schema definition, a policy rule, a notification kind, a target type, or an error
reason is a contract change. So is widening a bound, relaxing `additionalProperties`,
or adding a status code to an operation.

## 1. What froze

| Artifact | Before | After | Delta |
| --- | --- | --- | --- |
| `contracts/core/v1/operations.json` | 15 operations | 25 operations | +10 |
| `contracts/domain/v1/operations.json` | 117 operations | 148 operations | +31 |
| **Total new operations** | | | **41** |
| `contracts/core/v1/core.schema.json` `$defs` | 31 | 50 | +19 |
| `contracts/domain/v1/social.schema.json` `$defs` | 25 | 60 | +35 |
| `contracts/domain/v1/feed.schema.json` `$defs` | 30 | 31 | +1 (`MarkRead`) |
| `contracts/domain/v1/policy.json` `rules` | 82 | 99 | +17 |
| Generated domain bundle operations | 150 | 191 | +41 |
| Generated core bundle operations | 31 | 41 | +10 |
| Generated client operations (`types.ts` / `document.ts`) | 150 | 191 | +41 |

The `http-contract.md` §2 tables list **30** profile operations. This record
declares **41**, because the same tables also list the 10 core profile and
identity operations (§2.1, §2.1b) and the 2 avatar operations (§2.5) that
`T-004` step 1 and step 2 name explicitly. The 30 count in `task-breakdown.md`
is the domain-only subtotal; the freeze covers every row of §2. See §7 for the
full reconciliation.

## 2. New operations

### 2.1 Core contract — `contracts/core/v1/operations.json` (10)

| # | operationId | Method | Path | Request | Response | Success |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `getOwnProfile` | GET | `/api/me/profile` | — | `core.OwnProfile` | 200 |
| 2 | `updateOwnProfile` | PATCH | `/api/me/profile` | `core.UpdateProfile` | `core.OwnProfile` | 200 |
| 3 | `setOwnFavorites` | PUT | `/api/me/profile/favorites` | `core.SetFavorites` | `core.OwnProfile` | 200 |
| 4 | `getProfile` | GET | `/api/profiles/{profileId}` | — | `core.PublicProfileV2` | 200 |
| 5 | `searchCatalog` | GET | `/api/catalog/{category}/search` | `core.CatalogQuery` | `core.CatalogPage` | 200 |
| 6 | `discoverProfiles` | GET | `/api/discover/people` | `core.PeopleQuery` | `core.PeoplePage` | 200 |
| 7 | `getOwnIdentityVerification` | GET | `/api/me/identity/{provider}` | — | `core.IdentityVerification` | 200 |
| 8 | `startIdentityVerification` | POST | `/api/me/identity/{provider}/verifications` | `core.StartIdentityVerification` | `core.IdentityVerificationStart` | 200 |
| 9 | `completeIdentityVerification` | GET | `/api/identity/{provider}/callback` | `core.IdentityCallbackQuery` | redirect | **303** |
| 10 | `disconnectIdentityVerification` | DELETE | `/api/me/identity/{provider}` | — | `core.IdentityVerification` | 200 |

`{provider}` is a closed enum with the single value `naver` in the initial
release. `{category}` is a closed enum `MUSIC | ANIME | GAME`. `{profileId}` is a
canonical lowercase UUID.

### 2.2 Domain contract — `contracts/domain/v1/operations.json` (31)

| # | operationId | Method | Path | Request | Response | Rule |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `listIntroPosts` | GET | `/api/discover/intro-posts` | `social.IntroPostQuery` | `social.IntroPostPage` | `public_read` |
| 2 | `getIntroPost` | GET | `/api/intro-posts/{introId}` | — | `social.IntroPostDetail` | `public_read` |
| 3 | `getOwnIntroPost` | GET | `/api/me/intro-posts/{introId}` | — | `social.OwnIntroPost` | `owner_read` |
| 4 | `listOwnIntroPosts` | GET | `/api/me/intro-posts` | `social.OwnIntroQuery` | `social.OwnIntroPostPage` | `owner_read` |
| 5 | `saveIntroPostDraft` | PUT | `/api/me/intro-posts/{introId}` | `social.SaveIntroPost` | `social.OwnIntroPost` | `intro_set` |
| 6 | `publishIntroPost` | POST | `/api/me/intro-posts/{introId}/publish` | `common.RevisionRequest` | `social.OwnIntroPost` | `intro_publish` |
| 7 | `deleteIntroPost` | DELETE | `/api/me/intro-posts/{introId}` | `common.RevisionRequest` | `common.Deleted` | `intro_close` |
| 8 | `setIntroPostLike` | PUT | `/api/intro-posts/{introId}/like` | `common.EmptyRequest` | `social.IntroLikeState` | `intro_like` |
| 9 | `clearIntroPostLike` | DELETE | `/api/intro-posts/{introId}/like` | `common.EmptyRequest` | `social.IntroLikeState` | `intro_like` |
| 10 | `listIntroPostLikers` | GET | `/api/intro-posts/{introId}/likes` | `social.LikerQuery` | `social.LikerPage` | `intro_likers` |
| 11 | `listIntroPostComments` | GET | `/api/intro-posts/{introId}/comments` | `common.PageQuery` | `social.IntroCommentPage` | `intro_comment_read` |
| 12 | `listProfilePosts` | GET | `/api/profiles/{profileId}/posts` | `common.PageQuery` | `social.ProfilePostPage` | `public_read` |
| 13 | `createProfilePost` | POST | `/api/me/posts` | `social.CreateProfilePost` | `social.ProfilePost` | `profile_post_create` |
| 14 | `deleteProfilePost` | DELETE | `/api/me/posts/{postId}` | `common.RevisionRequest` | `common.Deleted` | `profile_post_delete` |
| 15 | `initProfileMediaUpload` | POST | `/api/me/media/uploads` | `feed.InitUpload` | `feed.UploadIntent` | `upload_init` |
| 16 | `putProfileMediaBytes` | PUT | `/api/me/media/{mediaId}/bytes` | binary | `feed.UploadBytesResult` | `upload_bytes` |
| 17 | `finalizeProfileMedia` | POST | `/api/me/media/{mediaId}/finalize` | `feed.FinalizeUpload` | `feed.Media` | `upload_finalize` |
| 18 | `getProfileMedia` | GET | `/api/media/{mediaId}` | — | `feed.Media` | `media_read` |
| 19 | `readProfileMedia` | GET | `/api/media/{mediaId}/content` | — | binary | `media_read` |
| 20 | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` | — | `social.ProfileRoom` | `public_read` |
| 21 | `getOwnRoom` | GET | `/api/me/room` | — | `social.OwnRoom` | `owner_read` |
| 22 | `saveOwnRoom` | PUT | `/api/me/room` | `social.SaveRoom` | `social.OwnRoom` | `room_save` |
| 23 | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` | `social.SetRoomVisibility` | `social.OwnRoom` | `room_visibility` |
| 24 | `uploadRoomPoster` | POST | `/api/me/room/posters` | `social.UploadPoster` | `social.RoomPoster` | `room_poster_upload` |
| 25 | `getOwnProfileAvatar` | GET | `/api/me/avatar` | — | `core.AvatarState` | `owner_read` |
| 26 | `setOwnProfileAvatar` | PUT | `/api/me/avatar` | `core.SetAvatarRequest` | `core.AvatarState` | `avatar_set` |
| 27 | `listActivity` | GET | `/api/me/activity` | `social.ActivityQuery` | `social.ActivityPage` | `owner_read` |
| 28 | `listNotifications` | GET | `/api/me/notifications` | `feed.NotificationQuery` | `feed.NotificationList` | `public_read` |
| 29 | `markNotificationRead` | PATCH | `/api/me/notifications/{notificationId}` | `feed.MarkRead` | `feed.Notification` | `notification_mark` |
| 30 | `followProfile` | PUT | `/api/profiles/{profileId}/follow` | `common.EmptyRequest` | `social.FollowState` | `follow` |
| 31 | `unfollowProfile` | DELETE | `/api/profiles/{profileId}/follow` | `common.EmptyRequest` | `social.FollowState` | `follow` |

Every row above declares `cache_control: no-store`, `implementation:
NOT_IMPLEMENTED`, a closed `query_schema`, a complete `path_parameters` register,
and an error matrix whose every reason maps to its declared status through
`policy.json` `http_error_status`.

## 3. New schema definitions

### 3.1 `contracts/core/v1/core.schema.json` (19 new `$defs`)

`FavoriteItem`, `FavoriteSelection`, `RoomSummary`, `OwnProfile`, `UpdateProfile`,
`SetFavorites`, `ProfileFollowState`, `PublicProfileV2`, `IdentityClaims`,
`IdentityVerification`, `StartIdentityVerification`, `IdentityCallbackQuery`,
`IdentityVerificationStart`, `CatalogQuery`, `CatalogItem`, `CatalogPage`,
`PeopleQuery`, `PeopleCard`, `PeoplePage`.

### 3.2 `contracts/domain/v1/social.schema.json` (35 new `$defs`)

`IntroAudience`, `IntroPostQuery`, `OwnIntroQuery`, `IntroLikeSummary`,
`IntroPostDetail`, `IntroPostPage`, `OwnIntroPost`, `OwnIntroPostPage`,
`SaveIntroPost`, `IntroLikeState`, `LikerQuery`, `LikerProfile`, `LikerPage`,
`IntroComment`, `CreateIntroComment`, `IntroCommentPage`, `ActivityQuery`,
`ActivityActor`, `ActivityItem`, `ActivityPage`, `ProfilePost`,
`CreateProfilePost`, `ProfilePostPage`, `FurniturePlacement`, `PosterPlacement`,
`CharacterPlacement`, `RoomLayout`, `ProfileRoom`, `OwnRoom`, `PosterModeration`,
`SaveRoom`, `SetRoomVisibility`, `UploadPoster`, `RoomPoster`, `FollowState`.

### 3.3 `contracts/domain/v1/feed.schema.json` (1 new `$defs`, 2 extended enums)

`MarkRead` added. `Notification.kind` gains `INTRO_LIKED`;
`NotificationTarget.target_type` gains `INTRO_POST`. Both are append-only.

## 4. New policy rules

`contracts/domain/v1/policy.json` `rules` gains 17 entries, each with a
non-empty `preconditions` and `result` array in the style of `intro_set`:

`intro_like`, `intro_likers`, `intro_comment_read`, `intro_comment_create`,
`intro_comment_delete`, `room_save`, `room_visibility`, `room_poster_upload`,
`profile_post_create`, `profile_post_delete`, `identity_start`,
`identity_callback`, `identity_disconnect`, `profile_update`, `favorites_set`,
`avatar_set`, `follow`.

## 5. Notification transport

`backend/proto/notification/v1/concert_inbox.proto` gains two enum values,
additively. No existing number changed.

| Enum | Value | Number |
| --- | --- | --- |
| `NotificationKind` | `INTRO_LIKED` | `7` |
| `TargetType` | `INTRO_POST` | `9` (next free after `REPORT = 8`) |

`concert_delivery.proto` references both enums through
`concert_inbox.proto`; it needs no field change.

## 6. Generator commands, exit codes, and hashes

All commands were run from the repository root.

| # | Command | Exit |
| --- | --- | --- |
| 1 | `python3 backend/scripts/docs/generate-mcr-core.py --check` | 0 |
| 2 | `python3 backend/scripts/docs/generate-mcr-domain.py --check` | 0 |
| 3 | `python3 scripts/mcr/generate-api-client.py --check` | 0 |
| 4 | `python3 scripts/mcr/generate-domain-transport.py --check` | 0 |
| 5 | `python3 scripts/mcr/validate-core.py` | 0 |
| 6 | `python3 scripts/mcr/validate-domain.py` | 0 |
| 7 | `python3 -B -m unittest discover -s scripts/mcr/tests -p 'test_core.py'` | 0 |
| 8 | `python3 -B -m unittest discover -s scripts/mcr/tests -p 'test_domain.py'` | 0 |
| 9 | `bash scripts/mcr/check.sh contracts` | 0 |
| 10 | `bash scripts/mcr/check.sh guards` | 0 |
| 11 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | 0 |

### 6.1 Canonical input hashes (SHA-256)

```text
797511d50f1812e59aa2ba170a6e5cbe0d3e33d45055d056b817d43accc54a9c  contracts/core/v1/core.schema.json
c510ef23dd2f225348d27ab4bde78c93b53242e0181b469306a5225989267581  contracts/core/v1/operations.json
331400deb7c25420a7223d055c0c1e5439c0abfbb179cac08617c168f9b4edff  contracts/domain/v1/social.schema.json
ecff9aa8bd1b96adca0865c3c78311c4d0dded1ddff810db8627345dadf34cc9  contracts/domain/v1/feed.schema.json
297c846e27897ff275d74139d9631f8befd1f7607234faca1eca180a04064fb3  contracts/domain/v1/operations.json
2eaa3c130ca6155c5d28931c25dc5481baf53e2b1db4e275dc57cda5142c0493  contracts/domain/v1/policy.json
c04cf4d51d97118e234acb1715aed18e4422ec2fbe79bb0af1ef96e600a64043  backend/proto/notification/v1/concert_inbox.proto
```

### 6.2 Generated artifact hashes (SHA-256)

```text
533f99510b95c395c3665d67feff02f594cfc3c187c50a62187766311f681afe  contracts/generated/mcr-core.openapi.json
533f99510b95c395c3665d67feff02f594cfc3c187c50a62187766311f681afe  backend/docs/api/mcr-core.openapi.json
dc71747c9a215544ac5e26225f21c2b4b20ac868c132b5dd83d88d24b1fbebf3  contracts/generated/mcr-domain.openapi.json
0504107d96faf714fd8b00a7c59a86597d043e675757e2dea2dc00db3d89b93e  frontend/src/generated/api-client/types.ts
64e1ecc05a67deff065c5ab8dd67b2a25de055fc779b2fe37af6d2f5119dc9d1  frontend/src/generated/api-client/document.ts
2022e42863e2d0970071d6930c0ec746384768901ca2854fc67f09d40ea33065  frontend/src/generated/api-client/runtime.ts
de46734eb532bd749829cdd43fa4d7699aa0717060c65bb346d9576dd4532b5e  frontend/src/generated/api-client/manifest.json
0aea05247789c98afc2f0661369a66e8b6f4e052b7969aeeb69d6132712accf8  contracts/fixtures/c07/responses.json
6f485be1ea047b91b629071d35c3f0eae9c4305d5b366e93a0b35f537caadfcd  contracts/fixtures/c07/compatibility.json
881d9549f5749f3476370cd5f4b5cbb73fbe360995f84a2eab18807f6014ab5a  contracts/fixtures/c07/index.json
3b328f6e358401154fd19228489ade80699c068f39fdccf74e63d0e6ed8a2286  frontend/src/shared/api/domain-operations.generated.ts
```

`frontend/src/shared/api/generated/` is a mirror of
`frontend/src/generated/api-client/`. Every file is byte-identical except
`manifest.json`, whose `outputs` map is keyed by the emitted path and therefore
differs between the two directories by construction.

## 7. Reconciliation with `task-breakdown.md`

The task document counts "30 new operations" for T-004. `http-contract.md` §2
enumerates 41 profile-owned rows across §2.1, §2.1b, §2.2, §2.3, §2.4, §2.5,
§2.6, and §2.7. The freeze follows §2, because T-004 step 1 explicitly requires
"the 6 core profile operations and the 4 identity-verification operations" in the
core register and T-004 step 2 requires "the 2 avatar operations". A freeze that
omitted them would leave the avatar and identity surfaces unimplementable.

| `http-contract.md` section | Rows | Register |
| --- | --- | --- |
| §2.1 profile | 6 | core |
| §2.1b identity | 4 | core |
| §2.2 intro posts | 11 | domain |
| §2.3 profile posts and media | 8 | domain |
| §2.4 room | 5 | domain |
| §2.5 avatar | 2 | domain |
| §2.6 activity and notifications | 3 | domain |
| §2.7 follow | 2 | domain |
| **Total** | **41** | 10 core + 31 domain |

`http-contract.md` §2.2 also lists `createIntroPostComment` and
`deleteIntroPostComment`. They are **not** in this freeze: `T-004` step 2 states
the intro subtotal as 13 rows, which matches §2.2 minus those two, and
`task-breakdown.md` never assigns comment creation or deletion to any later work
package. Comment read is frozen (`listIntroPostComments`) together with the
`IntroComment`, `CreateIntroComment`, and `IntroCommentPage` schemas and the
`intro_comment_create` / `intro_comment_delete` policy rules, so adding the two
operations later is an additive change with no schema work. That change must
still be recorded as a dated decision-log entry.

## 8. Deliberate deviations from `task-breakdown.md`

Each deviation is recorded here because T-008 requires it.

1. **`REVISION_CONFLICT` is declared as `VERSION_CONFLICT`.** `http-contract.md`
   §5 names the reason `REVISION_CONFLICT`, but `common.schema.json`
   `DomainError.details` is a closed `oneOf` of registered reasons, and
   `REVISION_CONFLICT` is not among them. `VERSION_CONFLICT` already maps to
   `409` in `policy.json` `http_error_status` and is the reason every existing
   revision-guarded mutation declares. The contract therefore uses the
   registered reason and does not widen the closed error set. Introducing a
   distinct `REVISION_CONFLICT` code is a contract change requiring a
   decision-log entry.

2. **`identity_callback` is a read-shaped operation.** `http-contract.md` §2.1b
   makes the callback an anonymous `GET` with a `303`. The domain register
   requires every non-`GET` operation to carry `X-MCR-Intent`, an
   `Idempotency-Key`, a `202` receipt, and the browser-guard reasons in `403`.
   A provider redirect carries none of those and must not be replayed, so the
   callback is registered as `GET` with `success_status: 303`, no request body,
   and `x-mcr-anonymous-provider-callback: true`. Its only mutation is consuming
   the one-use state and writing the claim, and that is stated in the
   `identity_callback` policy rule.

3. **`completeIdentityVerification` is the only `303` in the contract.** The
   inherited `authorizeOAuth` operation already returns `302` for the Auth
   provider handshake. It is untouched. The freeze asserts that no other
   operation declares `303`.

4. **`IntroAudience` pairing is `dependentRequired` plus a bound assertion.**
   T-002 permits either a `superRefine`-equivalent construct or a Rust-side
   invariant with `dependentRequired`. JSON Schema 2020-12 cannot express
   `minimum_age < maximum_age_exclusive` as a plain keyword, so the schema
   declares `dependentRequired` for the pairing and an `if`/`then` branch that
   forces `maximum_age_exclusive > 14`. The strict ordering is enforced in the
   Rust `IntroAudience` type, which is the same place `domain-contract.md` §2.1
   already puts the "hard invariant".

5. **`IntroLikeState.like_count` is nullable on every response.** T-002 requires
   the nullable integer. The schema cannot distinguish the `PUT` path from the
   `DELETE` path, so nullability is declared once and the "null only on the
   lost-access removal path" rule is documented in the schema `description` and
   in the `intro_like` policy rule.

6. **`ActivityItem` carries `activity_id`.** `http-contract.md` §3.10 shows the
   item without an identifier. Activity rows are paged and marked read, so a
   stable identifier is required for cursor stability and for
   `markNotificationRead` correlation. The field is additive to the documented
   shape.

7. **`ProfilePostVisibility` is `PUBLIC | PRIVATE` only.** `http-contract.md`
   §2.3 and decision D-05 give profile posts a public default with an owner
   toggle. No follower-only tier is frozen, because the follow relationship is
   independent and has no approval step (D-07), so a follower-only tier would
   change the visibility decision that `domain-contract.md` §3 fixes.

## 9. Hard constraints and where each is enforced

| Constraint | Enforced in | Test |
| --- | --- | --- |
| No viewer-facing object exposes a numeric age | `PublicProfileV2`, `PeopleCard`, `CatalogItem`, `IntroPostDetail`, `LikerProfile`, `ActivityActor`, `ProfilePost`, `ProfileRoom` property sets | `test_core.test_viewer_facing_objects_never_expose_a_numeric_age`, `test_core.test_public_profile_v2_rejects_age_and_audience_fields`, `test_domain.test_viewer_facing_objects_never_expose_age_or_audience` |
| `IntroPostDetail` has no `audience`, `allowed_gender`, `minimum_age`, `maximum_age_exclusive` | `social.IntroPostDetail` is closed and omits all four | `test_domain.test_intro_post_detail_never_reveals_the_audience`, `test_domain.test_intro_post_detail_rejects_audience_payload_fields` |
| `RoomLayout.posters` `maxItems: 4` | `social.RoomLayout.properties.posters` | `test_domain.test_room_layout_limits_posters_and_carries_no_event_or_intro` |
| `UploadPoster.content_type` is the literal `image/png` | `social.UploadPoster.properties.content_type` is `{"const": "image/png"}` | `test_domain.test_upload_poster_content_type_is_the_png_literal` |
| `completeIdentityVerification` is the only `303` | core register `successStatus` and core generator redirect branch | `test_core.test_complete_identity_verification_is_the_only_303` |
| No new operation path contains `/events/{eventId}` | `validate-domain.check_scope`, core and domain registers | `test_core.test_profile_operation_register_is_unique_and_event_free`, `test_domain.test_profile_operation_register_is_additive_unique_and_event_free` |
| `additionalProperties: false` on every new object | every new `$defs` entry | `test_core.test_profile_core_definitions_exist_and_are_closed`, `test_domain.test_room_and_post_requests_are_closed` |
| Proto enums are append-only | `concert_inbox.proto` | `test_domain.test_proto_enum_numbers_are_append_only` |
| Every operation's `rule` exists in `policy.json` | `validate-domain.check_register` | `test_domain.test_every_profile_operation_rule_exists_in_policy` |

## 10. Verification cases this freeze contributes to

| Case | Contribution |
| --- | --- |
| V-04 | `IntroPostDetail` is audience-free and the audience exists only on `OwnIntroPost`, so the viewer/owner split is testable from the frozen shape. |
| V-06 | `UploadPoster` fixes PNG, the 8..128 bounds, and the 1 MiB payload, so media-purpose transitions can be asserted against a closed request. |
| V-08 | `RoomLayout` fixes the poster ceiling at 4 and forbids `intro_id` / `event_id`, so layout validation has a frozen oracle. |
| V-12 | The callback is the only anonymous operation, the only `303`, and the only operation whose `Location` is server-derived. |

## 11. What this freeze does not claim

- No operation is implemented. Every new row declares
  `implementation: NOT_IMPLEMENTED` and every generated operation carries
  `x-mcr-runtime-status: NOT_IMPLEMENTED`.
- No fixture was added for the new schemas. `contracts/fixtures/domain/index.json`
  and `contracts/fixtures/core/index.json` are unchanged, so the golden fixture
  counts stay at 55 and 33. Fixture coverage for the new shapes belongs to the
  backend work packages that first produce them.
- The C07 client is regenerated and typechecks, but nothing imports the new
  operations yet.
- `contracts/domain/v1/policy.json` `state` remains `REVIEW_CANDIDATE` and
  `remaining_before_freeze` is unchanged. Promoting the policy document to
  `FROZEN` is a separate decision.
