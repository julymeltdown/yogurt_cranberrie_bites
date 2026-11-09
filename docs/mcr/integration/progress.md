# Implementation progress

Status snapshot for the Yogurt Cranberrie Bites integration. This document is
updated as work lands; it is the single place to read current progress.

| Field | Value |
| --- | --- |
| Snapshot | 2026-11-09 16:30 Asia/Seoul |
| Delivery branch | `julymeltdown/yogurt_cranberrie_bites` `master` |
| Head at snapshot | `1e014e6b2` |
| Task register | 110 tasks in [task-breakdown.md](task-breakdown.md) |
| Tasks integrated | **94 / 110** |
| Tasks in flight | 0 |
| Tasks not started | 16 |
| Release gate | **INCOMPLETE** — see §6 |

Progress is counted per task, not per package. A task counts as integrated only
when its code is merged to `master`, its tests pass with a recorded command and
exit code, and its acceptance statements are demonstrated.

---

## 1. Summary by work package

| Package | Tasks | Integrated | State |
| --- | --- | --- | --- |
| WP-01 Contract freeze | 8 | **8** | Complete |
| WP-02 Schema and migrations | 12 | **12** | Complete |
| WP-03 Favorites and discovery | 6 | **6** | Complete |
| WP-04 Claims and visibility policy | 12 | **6** | Claim adapter in; like/media/link/likers surfaces open |
| WP-04b NAVER verification | 12 | **12** | Complete |
| WP-05 Intro posts and hearts | 16 | **16** | Complete |
| WP-06 Profile posts and media | 12 | **12** | Complete |
| WP-07 Room and posters | 12 | **12** | Complete |
| WP-08 Avatar re-key | 6 | **6** | Complete |
| WP-09 Navigation and screens | 8 | **8** | Complete |
| WP-10 Integration and evidence | 6 | 2 | Partial |

Two packages outside the original register also landed and are counted inside
their owning packages above: the profile/social visual layer (inside WP-09) and
the profile producer composition (inside WP-02).

---

## 2. Feature elements

Each row is a product element, its state, and the evidence that supports it.

### 2.1 Identity and verification

| Element | State | Evidence |
| --- | --- | --- |
| NAVER OAuth start with one-use state | Integrated | `identity_start` — 8 tests |
| Provider profile decoding (`F/M/U`, `birthyear`, `birthday`) | Integrated | `naver_profile` — 9 tests |
| Provider client with token redaction | Integrated | `naver_provider` — 9 tests |
| Production config refuses placeholders | Integrated | `naver_config` — 15 tests |
| Callback: validate → derive age → store | Integrated | `identity_callback` — 25 tests |
| Claim supersede on re-verification | Integrated | included in `identity_callback` |
| Status read with canonical claim ordering | Integrated | `identity_status` — 11 tests |
| Disconnect (idempotent revoke) | Integrated | `identity_disconnect` — 8 tests |
| Anonymous callback route with 303 | Integrated | `identity_routes` — 12 tests |
| Identity status UI | Integrated | `src/features/identity` — 20 tests |
| **Live NAVER console verification** | **Blocked** | No test NAVER application exists in this environment |

### 2.2 Profiles and favorites

| Element | State | Evidence |
| --- | --- | --- |
| `profile_identities` table with deterministic backfill | Integrated | `profile_identity_schema` — 19 tests |
| `profile_favorites` with 5-per-category cap | Integrated | `profile_identity_schema` |
| `catalog_items` with K-pop exclusion as a schema constraint | Integrated | `catalog_search` — 15 tests |
| Catalog search served end to end | Integrated | `profile_producer_coverage` — 8 tests |
| Favorite picker UI (three tabs, max 5, skipping allowed) | Integrated | `FavoritePicker.test.tsx` |
| Own profile read/write | Partial | `getOwnProfile` and `getProfile` served; `updateOwnProfile` and `setOwnFavorites` are NOT_IMPLEMENTED |
| Visitor profile page | Partial | `PublicProfilePage.test.tsx` — 17 tests; the route still mounts the concert component because `getProfile` returns four fields and the view model needs three it does not produce |

### 2.3 Discovery

| Element | State | Evidence |
| --- | --- | --- |
| Discovery query starting from profiles, not intro posts | Integrated | `discovery` — 17 tests |
| Overlap computed from catalog item ids only | Integrated | `discovery` |
| No score, rank, or weight column anywhere | Integrated | `discovery` |
| Discovery route mounted in the gateway | Integrated | `profile_product_routes` — 11 tests |
| Discovery UI (people list separate from intro list) | Integrated | `Discovery.test.tsx` |
| Discovery projection writer | Integrated | `discovery_projection` — 19 tests; bounded incremental reconciliation, idempotent, no lost updates |
| Projection worker scheduled | Integrated | `discovery_worker` — 12 tests; never exits on a round failure, stops without leaking a task |
| Block source consumer | Integrated (not on the worker loop) | `discovery_block_consumer` — 11 tests; needs the friend invalidation endpoint in startup config |
| gRPC discovery source | Integrated (loopback) | `discovery_source_live` — 5 tests; one process, not two containers |

### 2.4 Mutuals-intro posts and hearts

| Element | State | Evidence |
| --- | --- | --- |
| Draft / publish / delete commands | Integrated | `intro_post_commands` |
| Favorites snapshot frozen at publish | Integrated | `intro_snapshot` |
| Attachment rules (max 4, ownership, purpose) | Integrated | `intro_attachments` |
| Comments and replies gated by post audience | Integrated | `intro_comments` |
| Heart toggle, one row per (post, viewer) | Integrated | `intro_likes` |
| Like removal returns null count when access lost | Integrated | `intro_likes` |
| Author-only liker list | Integrated | `intro_likes` |
| `INTRO_LIKED` notification with dedupe | Integrated | `0087_intro_like_notifications` |
| Intro operation parser (13 operations, closed) | Integrated | `intro_routes` |
| Intro mutation stores (draft, publish, delete, like, comment) | Integrated | `intro_post_commands`, `intro_likes`, `intro_comments` |
| Composer, audience fields, heart button, likers panel, activity feed | Integrated | `src/features/intros` — 171 tests |
| `can_view_intro` pure decision (12-row table) | Integrated | `intro_visibility` — 14 tests |
| SQL predicate, proven identical to the pure function | Integrated | `intro_visibility_contract` — 8 tests, shared case table compiled into both binaries |
| Page and detail queries with the predicate inside | Integrated | `intro_posts_page` — 15 tests, `intro_cursor` — 7 tests |
| Claim adapter over the profile service | Integrated | `claims_adapter` — 11 tests |
| Real `can_view_intro` wired into the `Visibility` port | Integrated | `ComposedPolicy`; the fail-closed default remains only when the endpoint is unset |
| **Audience feature end to end** | **Works** | A live matching claim sees a restricted post; expired, absent, and wrong-gender claims do not; unrestricted is visible to all |
| Intro operations served over HTTP | Integrated (11 of 13) | `concert_profile_product_routes` — 12 tests |
| Intro composer mounted at `/discover/intro` | Integrated | `router.test.tsx` |
| `listIntroPosts` / `getIntroPost` | Integrated | `intro_reads` — 11 tests; predicate inside the query, viewer-bound cursor |

### 2.5 Photographs and artwork

| Element | State | Evidence |
| --- | --- | --- |
| Profile media upload lifecycle | Integrated | `profile_media_upload` — 16 tests |
| Profile post create/delete | Integrated | `profile_posts` — 16 tests |
| Visitor-facing reads gated by post visibility | Integrated | `profile_post_read` — 11 tests |
| Media byte serving with no-store and nosniff | Integrated | `profile_media_routes` — 10 tests |
| Attachment isolation (no intro→gallery leak) | Integrated | `v06-attachment-isolation.json` |
| Composer and gallery UI | Integrated | `src/features/profile-media` — 55 tests |
| **No pixel conversion in the photo path** | Integrated | structural test: no image code in the command module |

### 2.6 Personal room

| Element | State | Evidence |
| --- | --- | --- |
| Room layout value with grid and poster bounds | Integrated | `profile_room` — 17 tests |
| Revision conflict returns the current room | Integrated | `profile_room_persistence` — 15 tests |
| Room visibility independent of intro audience | Integrated | `profile_room_visibility` — 8 tests |
| Poster upload (PNG, ≤128px, ≤1MiB, declared vs actual) | Integrated | `profile_room_posters` — 10 tests |
| Room routes | Integrated | `profile_room_routes` — 12 tests |
| Deterministic pixel conversion (64 / 32, nearest neighbour) | Integrated | `pixelize.test.ts` |
| Three-step poster importer | Integrated | `PosterImporter.test.tsx` |
| Scoped Phaser scene with release on unmount | Integrated | `scene-lifecycle.test.ts` |
| Room editor with keyboard placement | Integrated | `RoomPage.test.tsx` |
| Profile screen composes the room entry | Integrated | `ProfilePage` renders `RoomEntry`; `ProfilePage.test.tsx` asserts the link |

### 2.7 Avatar

| Element | State | Evidence |
| --- | --- | --- |
| Profile-owned avatar domain (no participant key) | Integrated | `profile_avatar` — 9 tests |
| Persistence with revision conflict | Integrated | `profile_avatar_persistence` — 13 tests |
| `GET`/`PUT /api/me/avatar` routes | Integrated | `profile_avatar_routes` — 12 tests |
| Editor migrated from `MotionEditor` | Integrated | `editor.test.tsx` — 12 tests |
| Cache isolation on account switch | Integrated | `account-switch.test.tsx` — 13 tests |
| Source pack and atlas hashes preserved | Integrated | `git diff` empty on both |

### 2.8 Navigation and presentation

| Element | State | Evidence |
| --- | --- | --- |
| Three primary sections (Discover, Activity, My Profile) | Integrated | `navigation.test.tsx` — 19 tests |
| Legacy path redirects | Integrated | `router.test.tsx` — 19 tests |
| Owner and visitor profile screens | Integrated | `ProfilePage.test.tsx` — 10, `PublicProfilePage.test.tsx` — 17 |
| Identity settings screen | Integrated | `IdentitySettings.test.tsx` — 9 tests |
| Accessibility pass | Integrated | `accessibility.test.tsx` — 26 tests |
| Single visual token system across six screens | Integrated | `profile-social-primitives.test.tsx` — 28 tests |
| Left-accent-bar prohibition enforced by reading CSS | Integrated | same suite |
| **360/390/430 browser captures** | **Not run** | No browser session against a live stack |

### 2.9 Transport and composition

| Element | State | Evidence |
| --- | --- | --- |
| Event-free profile product transport | Integrated | `profile_product_transport` — 8 tests |
| Event-free concert profile transport | Integrated | `concert_profile_product_routes` — 9 tests |
| Routing rule: the service that owns the tables | Integrated | `profile_routing` — 7 tests |
| Coverage register over all 43 profile operations | Integrated | `profile_producer_coverage` — 8 tests |
| **Operations served** | **32 / 43** | 6 by profile-service, 26 by concert-service |
| **Operations NOT_IMPLEMENTED** | **11 / 43** | `listIntroPosts`, `getIntroPost`, and the profile write commands |

---

## 3. Verification matrix

| ID | Subject | State |
| --- | --- | --- |
| V-01 | Age boundary 18/29 allowed, 17/30 denied | Integrated (unit) — shared case table |
| V-02 | Unknown, expired, revoked attributes fail closed | Integrated (unit) — shared case table |
| V-03 | One decision across every surface | Partial — reads and likes wired; media and shared links open |
| V-04 | Like removal discloses nothing | Integrated (unit) |
| V-05 | Idempotent likes, one notification | Integrated (unit) |
| V-06 | Attachment isolation | Integrated (unit) |
| V-07 | Ordinary posts independent and unconverted | Integrated (unit) |
| V-08 | Lifecycle independence | Partial — intro half not exercised |
| V-09 | Ownership and account switching | Partial — `ACCOUNT_CONTEXT_STALE` never emitted |
| V-10 | Failure handling and resource cleanup | Integrated (unit) |
| V-11 | End-to-end three-identity scenario | Not yet re-run since the audience feature became satisfiable |
| V-12 | NAVER verification produces a real claim | 15 of 16 cases pass; console case blocked |
| V-13 | Unrestricted posts need no verification | Integrated (unit) — the unrestricted path reads no claim |

---

## 4. Test counts recorded per package

| Package | Backend | Frontend | Notes |
| --- | --- | --- | --- |
| WP-01 | — | — | 35 core + 61 domain + 17 bundle contract tests |
| WP-02 | 54 | — | against real PostgreSQL 16 |
| WP-03 | 43 | 24 | |
| WP-04b | 85 | 20 | |
| WP-05 | 79 | 171 | 27 pure rules + 52 database |
| WP-06 | 61 | 55 | |
| WP-07 | 62 | 64 | |
| WP-08 | 34 | 27 | |
| WP-09 | — | 112 | seven new suites: navigation 19, router 19, shell 12, accessibility 26, profile 10, public profile 17, identity settings 9 |
| Art layer | — | 28 | CSS-reading guard suite |
| Design round 2 | — | 39 | guard suite grew 28 -> 39; 812 scoped tests pass |
| Discovery projection | 35 | — | 19 projection + 11 block consumer + 5 live source |
| Discovery worker | 12 | — | shutdown and failure behaviour |
| Frontend refactor | — | 6 | query-key audit; net -358 lines across 49 files |
| WP-04 visibility | 74 | — | 10 audience + 9 claims + 14 pure + 8 SQL parity + 7 cursor + 15 page + 11 claim adapter |
| Intro reads | 11 | — | predicate inside the query, viewer-bound cursor |
| Backend refactor | — | — | test counts identical before and after: 342 / 1449 / 737 |
| Producer routing | 16 | — | 7 gateway + 9 concert transport |

Frontend full-suite baseline: **19 tests fail on `master` before any of this
work**, in five files unrelated to the profile product. They are recorded as
pre-existing, not as passes.

---

## 5. Defects found and fixed during integration

| Defect | Impact | Resolution |
| --- | --- | --- |
| Two migrations numbered `0086` | A real `--migrate` run refuses the packaged directory | Renumbered the avatar migration; ledger re-verified |
| Two migrations numbered `0087` | Same | Renumbered to `0089`/`0090`; added `test_migration_numbering.py` to `check.sh guards` so the class cannot recur |
| Cross-database producer gap | 15 tested producers unreachable; every profile post/room/avatar route returned 403 | Added an event-free concert profile transport and an explicit owner table |
| `-0` clamp in the room reducer | Left-edge furniture moves silently dropped | Fixed by the keyboard test |
| `GET /api/intro-posts/{id}/like` parsed as a mutation | A read could reach a mutation handler | Parser refuses it; test added |
| Stale typecheck error in an unrelated test | Blocked the required `typecheck` command | Fixed |
| Projection watermark used `profile_identities.updated_at` | A favorites change after a pass was never seen again | Added a `profile_favorites` touch trigger (migration `0017`); no-lost-updates test now mutates the source after convergence |
| Duplicate CSS rule blocks across 15 places | Drift between copies | Deduplicated to 0, verified in the built bundle |
| Three attachment decoders, two defaulting an unparseable purpose to `ProfilePost` | An intro asset could cross to the profile surface (V-06-d) | One shared decoder that defaults nothing that matters |
| `INVALID_PURPOSE` and `BYTES_MISMATCH` outside the closed error set | Gateway and client would drop a precise refusal into an opaque one | Both project to `INVALID_INPUT` |
| `seal_cursor` accepted windows `open_cursor` rejects | A cursor token no reader could ever accept | Fixed; cursor tests cover the round trip |
| Comment count filtered on `state = 'VISIBLE'` | The schema admits `PUBLISHED`; the count was always 0 | Found by the read test |
| Inner join on `discovery_profiles` in the detail query | Every post without a projection row became `Unavailable` | Left join; the "author required for rendering" decision moved to the transport |
| No claim adapter in the concert service | The audience feature was enforced but never satisfiable — nobody saw a restricted post | T-302 claim adapter; verified with live, expired, absent, and wrong-gender claims |

---

## 6. Release gate

The integration gate (`backend/scripts/mcr/wp10_profile_social.py`) was executed
against revision `6b38f3397`. Result: **INCOMPLETE**.

| Step | State | Reason |
| --- | --- | --- |
| 1 — profile posts | BLOCKED | Producer did not serve media init at the time of the run |
| 2 — intro post | BLOCKED | No intro route registered |
| 3 — liker navigation | BLOCKED | No hearts, notification, or likers surface |
| 4 — poster and delete | PARTIAL | Poster 403; intro delete 404 |
| 5 — outsider denial | BLOCKED | No audience decision to test |
| Cross-surface sweep | BLOCKED | Eight surfaces returned three different statuses |
| Lifecycle sweep | PARTIAL | Durable layer passes; intro half cannot run |

The producer half of steps 1 and 4 has since been closed by the routing work.
The gate has **not** been re-run since. It must be re-run unchanged before any
release claim, and step 5 must assert `404` for the post, its attachments, and
its bytes.

What the gate did prove on that revision: seven services start and become ready
on an owned private PostgreSQL 16 cluster; three identities are created through
the real enrollment flow; two complete the real NAVER verification flow and
receive `VERIFIED` claims; room visibility and avatar saves leave every other
feature's rows byte-identical.

---

## 7. What is not proven

| Item | Why |
| --- | --- |
| Live NAVER authorization | No console-issued client id, secret, or registered callback in this environment |
| Browser captures at 360/390/430 | No browser session against a live stack |
| End-to-end three-identity scenario | Blocked on WP-04 and on the intro routes |
| Discovery against real data | The writer and worker exist and are tested; the worker is opt-in and the block consumer is not on the worker loop, so a deployment must enable it |
| `ACCOUNT_CONTEXT_STALE` | The reason does not exist anywhere in `backend/`; the docs named a header the code never had |
| Performance | Deferred by the earlier MCR decision; not a gate for this integration |
| Licensing | Unresolved; recorded in [source-register.md](source-register.md) §6 |

---

## 8. Next

1. **Land WP-04** and wire the real `can_view_intro` into likes and reads,
   replacing the fail-closed placeholder. This is the last security blocker.
2. Write the two missing intro reads (`listIntroPosts`, `getIntroPost`). Both
   need the visibility-filtered query plus the viewer's like state.
3. Produce the three fields `PublicProfilePage` needs (`shared_items`,
   `room_available`, `gender_bucket`), then switch the visitor profile route.
4. Mount the block consumer on the discovery worker loop.
5. Re-run the integration gate unchanged; every step must be `PASS`.
6. Run browser captures at the three widths.
