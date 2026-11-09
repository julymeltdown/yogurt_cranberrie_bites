# Implementation progress

Status snapshot for the Yogurt Cranberrie Bites integration. This document is
updated as work lands; it is the single place to read current progress.

| Field | Value |
| --- | --- |
| Snapshot | 2026-11-09 09:55 Asia/Seoul |
| Delivery branch | `julymeltdown/yogurt_cranberrie_bites` `master` |
| Head at snapshot | `588a8a0a9` |
| Task register | 110 tasks in [task-breakdown.md](task-breakdown.md) |
| Tasks integrated | **82 / 110** |
| Tasks in flight | 12 (WP-04) |
| Tasks not started | 16 (blocked on WP-04) |
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
| WP-04 Claims and visibility policy | 12 | 0 | In flight |
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
| Visitor profile page | Integrated (view model) | `PublicProfilePage.test.tsx` — 17 tests; route still mounts the concert profile pending the profile-service read |

### 2.3 Discovery

| Element | State | Evidence |
| --- | --- | --- |
| Discovery query starting from profiles, not intro posts | Integrated | `discovery` — 17 tests |
| Overlap computed from catalog item ids only | Integrated | `discovery` |
| No score, rank, or weight column anywhere | Integrated | `discovery` |
| Discovery route mounted in the gateway | Integrated | `profile_product_routes` — 11 tests |
| Discovery UI (people list separate from intro list) | Integrated | `Discovery.test.tsx` |
| **Discovery projection writer** | **Not implemented** | `discovery_profiles` has no producer; discovery returns empty in production |

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
| Intro routes on the profile transport | Integrated | `intro_routes` |
| Composer, audience fields, heart button, likers panel, activity feed | Integrated | `src/features/intros` — 171 tests |
| **Real `can_view_intro` wired into likes and reads** | **Not wired** | WP-05 wired a fail-closed `DenyRestricted` port; the real policy lands with WP-04 |
| **Intro list/detail HTTP routes registered** | **Not registered** | The producer parses them; no gateway route is mounted |

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
| **Profile screen composes the room entry** | **Not wired** | `RoomEntry` and the route exist; the shell profile screen does not render the section |

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
| **Operations served** | **21 / 43** | 6 by profile-service, 15 by concert-service |
| **Operations NOT_IMPLEMENTED** | **22 / 43** | WP-04 and WP-05 surfaces, plus profile write commands |

---

## 3. Verification matrix

| ID | Subject | State |
| --- | --- | --- |
| V-01 | Age boundary 18/29 allowed, 17/30 denied | Not run (WP-04) |
| V-02 | Unknown, expired, revoked attributes fail closed | Not run (WP-04) |
| V-03 | One decision across every surface | Blocked at the gate |
| V-04 | Like removal discloses nothing | Integrated (unit) |
| V-05 | Idempotent likes, one notification | Integrated (unit) |
| V-06 | Attachment isolation | Integrated (unit) |
| V-07 | Ordinary posts independent and unconverted | Integrated (unit) |
| V-08 | Lifecycle independence | Partial — intro half not exercised |
| V-09 | Ownership and account switching | Partial — `ACCOUNT_CONTEXT_STALE` never emitted |
| V-10 | Failure handling and resource cleanup | Integrated (unit) |
| V-11 | End-to-end three-identity scenario | **Blocked** |
| V-12 | NAVER verification produces a real claim | 15 of 16 cases pass; console case blocked |
| V-13 | Unrestricted posts need no verification | Not run (WP-04) |

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
| Discovery against real data | The projection has no writer |
| `ACCOUNT_CONTEXT_STALE` | The reason does not exist anywhere in `backend/`; the docs named a header the code never had |
| Performance | Deferred by the earlier MCR decision; not a gate for this integration |
| Licensing | Unresolved; recorded in [source-register.md](source-register.md) §6 |

---

## 8. Next

1. Land WP-04 and wire the real `can_view_intro` into likes and reads, replacing
   the fail-closed placeholder.
2. Register the intro list and detail gateway routes.
3. Re-run the integration gate unchanged; every step must be `PASS`.
4. Compose the room entry into the profile screen.
5. Add the discovery projection writer.
6. Run browser captures at the three widths.
