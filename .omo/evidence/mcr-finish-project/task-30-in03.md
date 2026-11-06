# Task 30 — IN03 social and privacy gate

- Plan row: 30 (`Pass IN03 social and privacy gate`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in03`
- Branch: `codex/mcr-exec-w5-in03`
- Base: `2f2f11b61ee2f33812ea848ba21c890d529e10b9` (`master`)
- Gate driver: `backend/scripts/mcr/in03_social_gate.py`
- Machine report: `task-30-in03-report.json` (this directory)
- Latest run report: `/tmp/in03-gate-30.json`
- Result: **PASS** on every executed case, with **13 recorded NOT_RUN entries** named below.
  `result_scope = PASS_WITH_RECORDED_NOT_RUN`; `cleanup = PASS`; exit code 0.

Banner: **development stack, NOT_RELEASE_QUALIFICATION, QA08 NOT_RUN,
NO_CLIENT_ONLY_PRIVACY_CLAIM.** The run proves server-side friendship, block,
profile, concert-feed and wall privacy fences on this checkout's native
services. It does not claim provider verification, venue producer completion,
client-cache behavior, a world-socket frame, a physical device, capacity or a
release decision.

## Stack and inputs

- Seven native binaries built from this checkout:
  `api-gateway`, `auth-service`, `friend-service`, `profile-service`,
  `post-service`, `notification-service`, `concert-service`.
- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 cargo build
  -p concert-service -p auth-service -p friend-service -p profile-service
  -p post-service -p notification-service -p api-gateway -j 3` → exit 0.
- Binaries copied to the private directory
  `/Users/lhs/Desktop/mcr-build/exec-w5-in03-bins` (mode 0700).
- Owned disposable PG16.14 cluster with `max_prepared_transactions=256`,
  owned Redis, private local HTTP mail sink, generated mTLS PKI and a private
  temp root: `/private/tmp/mcr-pg-ldow6aub` (removed by cleanup).
- Event: `f1fd7b85-d393-5410-8bc3-5ea6b71aeb77`; concert product release digest
  `undefined`.
- Gateway readiness: database, gateway→auth/friend/notification/post/profile
  and signing-keys all `up`. Auth and Friend owner-authority readiness `up`.
- Four independent principals (`alice`, `bob`, `carol`, `outsider`) completed
  real enrollment → mail verify → login → consent → profile initialization on
  attempt 1 each. Alice, Bob and Carol were admitted to the event;
  `outsider` was authenticated but deliberately not admitted or enrolled.
- Social composition enabled: `CONCERT_SOCIAL_MODE=live`,
  `CONCERT_NOTIFICATION_INBOX_MODE=live`,
  `CONCERT_NOTIFICATION_TARGET_MODE=live`, shared Intro/Participant live and
  the World listener. Concert public feed reads enabled:
  `CONCERT_PUBLIC_FEED_MODE=read` and `POST_CONCERT_READ_MODE=read`.
- Enrollment policy: revision `6e8d17f2-21f0-4239-b29e-87ccd56f1bfb`,
  `approvalReference = development-not-admitted`. The checkout's policy schema
  has **no `productionApproval` field**; the development label is the explicit
  nonproduction publication. No legal approval is claimed.

## Commands

```sh
cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03/backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in03 \
  cargo build -p concert-service -p auth-service -p friend-service \
  -p profile-service -p post-service -p notification-service -p api-gateway -j 3
# exit 0

cd /Users/lhs/Desktop/mcr-worktrees/exec-w5-in03
python3 -m py_compile backend/scripts/mcr/in03_social_gate.py
# exit 0

python3 -B backend/scripts/mcr/in03_social_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w5-in03-bins \
  --report /tmp/in03-gate-30.json
# exit 0; result PASS; result_scope PASS_WITH_RECORDED_NOT_RUN; cleanup PASS
```

## Case matrix

Exact response bodies above are from the latest run. Denials are asserted as an
exact status plus an exact closed envelope; no substring acceptance.

| Case | Actors | Request | Result |
| --- | --- | --- | --- |
| Follow (public target) | alice → bob | `POST /api/follows` `{"targetUserId":bob}` | 200 `{"following":true,"followedAt":"…","requestId":null,"status":"ACTIVE"}`; durable `follows` row |
| Remove friend | alice | `DELETE /api/follows/bob` | 200 `{"following":false,"followedAt":null,"requestId":null,"status":"INACTIVE"}`; live `follows` rows = 0 |
| Private profile | bob | `PUT /api/profile/privacy` `{"accountVisibility":"PRIVATE"}` | 200 `{"accountVisibility":"PRIVATE",…}`; DB `profiles.account_visibility=PRIVATE` |
| Follow request (private target) | alice → bob | `POST /api/follows` | 200 `{"following":false,"requestId":"6576753a-…","status":"PENDING"}`; `follow_requests.status=PENDING` |
| Incoming request | bob | `GET /api/follows/requests/incoming` | 200; request id present |
| Decline request | bob | `POST /api/follows/requests/6576753a-…/reject` | 200 `{"id":"6576753a-…","status":"REJECTED"}`; DB `REJECTED` |
| Accept request | bob | `POST /api/follows/requests/869f37dd-…/accept` | 200 `{"following":true,"status":"ACTIVE",…}`; DB request `ACCEPTED`, edge 1 |
| Private profile fence | carol (not following) → bob | `GET /api/profile/bob` | 404 exact `{"code":"GATEWAY_404","message":"Not found","traceId":"…","timestamp":"…","details":null}` |
| Private profile visible to friend | alice (following) → bob | `GET /api/profile/bob` | 200 `{"userId":bob,"accountVisibility":"PRIVATE","lifecycleStatus":"ACTIVE"}` |
| Relationship invalidation on a private view | alice | unfollow, then `GET /api/profile/bob` | 404 exact GATEWAY_404 envelope; the previously readable private view is gone |
| Block | alice → bob | `POST /api/relationships/blocks/bob` | 200 `{"blockedByViewer":true,"blockedViewer":false,"following":false,"mutedByViewer":false}`; `user_blocks` row; friend invalidation journal 0→1 `FRIEND_RELATIONSHIP_CHANGED` |
| Block observed by target | bob → alice | `GET /api/relationships/alice` | 200 `{"blockedByViewer":false,"blockedViewer":true,"following":false,"mutedByViewer":false}` |
| Blocked follow denied | alice → bob | `POST /api/follows` | 403 exact `{"code":"GATEWAY_403","message":"Forbidden","traceId":"…","timestamp":"…","details":null}` |
| Blocked profile denied (both directions) | alice ↔ bob | `GET /api/profile/{other}` | 404 exact GATEWAY_404 envelope in both directions |
| Concert feed baseline | carol | `GET /api/concert/events/{event}/feed?limit=1` | 200; Bob's post present; DELETED seed absent |
| Deleted post read | carol | `GET …/posts/{deleted_post}` | 404 exact `{"code":"CONCERT_404","message":"Concert request unavailable","traceId":"…","timestamp":"…","details":{"reason":"RESOURCE_UNAVAILABLE"}}` |
| Blocked author disappears from feed | alice after blocking bob | `GET …/feed?limit=1` | 200 `{"items":[]}`; Bob's post absent |
| Blocked post read | alice | `GET …/posts/{bob_post}` | 404 exact CONCERT_404 `RESOURCE_UNAVAILABLE` |
| Wall member read | alice | `GET …/mural/snapshot` | 200; revision 1 |
| Wall non-participant read | outsider | `GET …/mural/snapshot` | 404 exact CONCERT_404 `RESOURCE_UNAVAILABLE` |
| Unblock | alice → bob | `DELETE /api/relationships/blocks/bob` | 200 `{"blockedByViewer":false,"blockedViewer":false,"following":false,"mutedByViewer":false}`; `user_blocks.deleted_at` set |
| Profile after unblock | alice → bob | `GET /api/profile/bob` | 200 (public profile restored) |
| Denied payload audit | all denials | exact key set `{code,message,traceId,timestamp,details}` | 11 denial cases; **0 private metadata leaks** across user ids, emails, handles, tokens and passwords |

Every successful follow/accept/reject/unfollow/block/unblock was also read back
from the owned PostgreSQL databases (`mcr_friend`, `mcr_profile`,
`mcr_post`, `mcr_concert`).

## Relationship-invalidation evidence

- `friend_world_invalidation_journal` held 1
  `FRIEND_RELATIONSHIP_CHANGED` row after the block and 2 after the unblock
  (both directions of the pair, exact `user_id`/`peer_id`).
- The already-loaded private profile and the concert feed both changed on the
  next request after the relationship write; no cached allow survived.
- The feed's own author fence re-observed the pair and dropped the blocked
  author's post; the direct post read returned the closed 404 body.

## Request-scoped feed seeding

The Concert feed **write** routes have no composed Concert producer in this
checkout (`Operation` has no `createConcertPost`/`createConcertComment`
variant), so the gate inserts three `post_concert_posts` rows and one
`post_concert_comments` row directly into the owned disposable
`mcr_post` database. Every read above is the real Concert/Post producer.
This is content provisioning for a read-fence gate, not a feed-write
qualification and not a client substitute.

## NOT_RUN

| Case | Missing input or blocking consumer | Observed |
| --- | --- | --- |
| `venue_meeting_reads` | No composed `Operation::Meeting` ProductOwner; `venue_meeting_admission.rs` states the bilateral friend read does not exist, so the route fails closed at 503. Consumer: `backend/services/concert-service/src/product_rpc/venue_meeting_admission.rs`. | Not executed; no venue privacy claim |
| `venue_location_snapshot` | `PgVenueLocationAuthority::friend_in` returns `PolicyUnavailable` and no `VenueLocation` product producer is composed. Consumer: `backend/services/concert-service/src/venue_location/mod.rs`. | Not executed |
| `social_block_list`, `social_block`, `social_unblock`, `unverified_age_denial` | The Social command transaction registration (`RuntimeHooks::register` → `CanonicalOwnerFence::track_transaction`) does not complete inside the 2 s product budget on this stack. `GET …/me/blocks` returned `CONCERT_503 DEPENDENCY_UNAVAILABLE`; no access was granted. Friend block/unblock and the feed/profile fences did execute. | 503 exact body recorded |
| `comments_read` | The comments route (`feed/content.rs`) did not return data within the composed authority window: observed `CONCERT_504 DEADLINE_EXCEEDED`; earlier runs returned `CONCERT_401 AUTHENTICATION_REQUIRED` and `CONCERT_503 DEPENDENCY_UNAVAILABLE`. The seeded comment row is `PUBLISHED` in `post_concert_comments`; the route returned no data and no private metadata. | Fail-closed 401/503/504 recorded |
| `blocked_comment_author_filter` | Same comments-route budget limitation with a parent author and a blocked comment author in one read. | Not executed |
| `age_provider_positive` | Missing input: an authorized age-verification provider with signed verified claims. Social consent requires `concert_age_owners` plus verified `concert_age_claims` for both participants and a configured sharing verifier; no provider credential or provenance exists. | NOT_RUN |
| `social_reveal_revoke` | Depends on that same provider/claim input; a real grant cannot be created without verified claims, so approved/unrelated/expired/revoked/blocked reveal denials cannot be executed against a real grant. | NOT_RUN |
| `external_oauth_provider` | Missing input: external OAuth/social provider credentials, callback registration and provider test account. | NOT_RUN |
| `external_mail_or_sms` | Missing input: external mail/SMS provider. Only the owned local HTTP mail sink was exercised. | NOT_RUN |
| `client_cache_and_ws_frames` | No browser client ran in this backend gate. No client-cache or world-socket frame claim is made; the server invalidation journal and read-time fences are the evidence. | NOT_RUN |

## Findings and limits

1. The server-side privacy fences for profile, concert feed, post detail and the
   Wall are enforced and return closed exact bodies with no private metadata.
2. The comments read and the Social command registration do not fit the
   composed 2 s product/authority budget on this development stack. They fail
   closed (401/503/504) rather than leak; they are **not** recorded as passes.
3. The venue meeting/location producers are not composed; IN03 cannot claim a
   venue privacy pass until that producer exists.
4. The provider-dependent reveal/revoke and age-positive cases remain
   `NOT_RUN` with the missing inputs named above.
5. No release, QA08, capacity, device, visual or client-cache qualification is
   claimed.

## Cleanup receipt

- Driver `finally`: `stack.close()` → `cleanup = PASS`; owned PostgreSQL,
  Redis, mail sink, TLS material and temp root removed.
- No push or merge was performed; the commit is local to
  `codex/mcr-exec-w5-in03`.
