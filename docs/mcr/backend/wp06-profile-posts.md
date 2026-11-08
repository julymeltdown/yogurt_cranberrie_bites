# WP-06 — Profile posts and profile media

Status: **INTEGRATED**. Tasks T-501 … T-512 of
[task-breakdown.md §10](../integration/task-breakdown.md#10-wp-06--profile-post-and-media-tasks)
are implemented, tested against a real PostgreSQL 16 cluster, and committed on
`codex/yc-wp06-posts` (base `ff45c37d9`).

A profile post is an ordinary selfie, photograph or piece of original artwork.
It is independent of any mutuals-intro post and independent of the room: it has
its own visibility, its own lifecycle and no `intro_id`. Publishing one requires
no intro post and no decorated room, and deleting one touches neither.

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-501 | Profile media upload lifecycle | `backend/services/concert-service/src/profile_media/upload.rs` |
| T-501 | `Purpose` extended with the four profile purposes | `backend/crates/concert-application/src/media/mod.rs` |
| T-502 | Profile post create and delete | `backend/services/concert-service/src/profile_posts/commands.rs` |
| T-503 | Profile post reads | `backend/services/concert-service/src/profile_posts/read.rs` |
| T-504 | Closed route parser (three post + five media operations) | `backend/services/concert-service/src/profile_posts/http.rs` |
| T-505 | Byte serving and the media read decision | `backend/services/concert-service/src/profile_media/authorization.rs`, `backend/services/api-gateway/src/adapters/in/http/profile_product/mod.rs` |
| T-506 | Composer UI | `frontend/src/features/profile-media/ProfileMediaComposer.tsx`, `controller.ts`, `upload.ts` |
| T-507 | Gallery | `frontend/src/features/profile-media/ProfileMediaGallery.tsx` |
| T-508 | Alt text and description validation | `frontend/src/features/profile-media/validation.ts` |
| T-509 | Attachment isolation evidence | `docs/mcr/integration/evidence/v06-attachment-isolation.json` |
| T-510 | Upload failure and cancellation | `frontend/src/features/profile-media/upload.ts`, `controller.ts` |
| T-511 | Deletion independence | `backend/services/concert-service/tests/profile_post_lifecycle.rs` |
| T-512 | Package verification | `backend/scripts/mcr/wp06_profile_posts_verify.py`, `docs/mcr/integration/evidence/v07-profile-posts.json` |

The private PostgreSQL harness is
`backend/scripts/mcr/wp06_profile_posts_cluster.py` (the
`migration_smoke.py` pattern reused verbatim, as `wp08_avatar_cluster.py` does).

## 2. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| A post with an `INTRO_POST` media id is **rejected** | `profile_posts.rs::a_post_with_an_intro_post_media_id_is_rejected` — the command refuses it, and the database trigger `profile_post_attachments_purpose` refuses the same association independently |
| The stored bytes are the **normalized photograph**, not a pixel conversion | `profile_posts.rs::the_stored_bytes_are_the_normalized_photograph_not_a_conversion` (source pixels and dimensions preserved), `profile_media_upload.rs::the_stored_bytes_are_the_normalized_photograph_not_a_conversion`, `profile_posts.rs::the_command_module_contains_no_pixel_conversion` (structural) |
| Publishing works with a **default room and no intro post** | `profile_posts.rs::publishing_works_with_a_default_room_and_no_intro_post`, `ProfileMediaComposer.test.tsx::publishes_a_photo_post_with_a_default_room_and_no_intro_post` |
| A private post is **absent from a visitor's list** and **404 on direct read**; the author sees it | `profile_post_read.rs::a_private_post_is_absent_from_a_visitors_list_and_404_on_direct_read` |
| Deleting a profile post leaves **room, avatar, intro posts and favorites** untouched | `profile_post_lifecycle.rs::deleting_a_profile_post_leaves_the_room_avatar_intro_and_favorites_untouched` (full snapshot before/after), `::deleting_an_intro_post_leaves_profile_posts_untouched` (the other direction) |
| A **foreign media id is rejected without revealing whether it exists** | `profile_posts.rs::a_foreign_media_id_is_rejected_without_revealing_whether_it_exists` — the foreign and absent answers are compared for equality |
| Only **JPEG/PNG/WebP**; 10 MiB; 4096 px; 16 M pixels. No GIF, no video | `profile_media_upload.rs::a_gif_is_refused_and_never_becomes_ready`, `::a_body_over_ten_mebibytes_is_refused_before_any_row`, `::a_photograph_wider_than_4096_pixels_is_refused_at_finalize`, `::a_photograph_over_sixteen_megapixels_is_refused_at_finalize`, `::the_profile_media_table_refuses_a_concert_purpose_and_a_gif`; `profile_post_routes.rs::only_the_three_image_types_are_accepted_for_raw_bytes`; `profile_media_routes.rs::a_bytes_upload_accepts_only_the_image_allowlist_and_requires_the_envelope` |
| A **cancelled upload leaves no orphan** | `profile_media_upload.rs::a_cancelled_upload_leaves_no_orphan_once_the_window_passes`; `failure.test.tsx::cancelling_before_finalize_never_publishes_and_never_creates_a_post`, `::leaving_the_screen_mid_upload_cancels_cleanly` |

## 3. Verification commands and exit codes

Run from the repository root unless stated. The PostgreSQL cluster is the
private, marker-owned `mcr-pg-*` cluster created by
`backend/scripts/mcr/wp06_profile_posts_cluster.py` in `/tmp`; no test attaches to
a foreign server.

| # | Command | Exit |
| --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** |
| 2 | `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_media_upload` | **0** (16 passed) |
| 3 | `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_posts` | **0** (16 passed) |
| 4 | `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_post_read` | **0** (11 passed) |
| 5 | `cargo +1.88.0 test --locked -p concert-service --test profile_post_routes` | **0** (14 passed) |
| 6 | `CONCERT_TEST_DATABASE_URL=… cargo +1.88.0 test --locked -p concert-service --test profile_post_lifecycle` | **0** (4 passed) |
| 7 | `cargo +1.88.0 test --locked -p concert-service --lib profile_` | **0** (17 passed, 3 ignored) |
| 8 | `cargo +1.88.0 test --locked -p api-gateway --test profile_media_routes` | **0** (10 passed) |
| 9 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** |
| 10 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/profile-media` | **0** (55 passed, 4 files) |
| 11 | `python3 backend/scripts/mcr/wp06_profile_posts_verify.py --report docs/mcr/integration/evidence/v07-profile-posts-run.json` | **0** |

Command 11 is the package runner: it creates the cluster, runs 1–10 against it,
writes one JSON report with every command and exit code, and removes the cluster.
Its report is `docs/mcr/integration/evidence/v07-profile-posts-run.json`.

## 4. Design notes

### One media domain, two scopes

`Purpose` in `concert-application/src/media/mod.rs` gained the four profile
purposes rather than a second enum. What keeps the two scopes apart is
`Purpose::scope()` and, at the database, the two tables' CHECK constraints: a
concert purpose is refused by `profile_media` and a profile purpose is refused by
`concert_media`. The byte, dimension and pixel limits are the same constants for
both, so they cannot drift.

### Purpose/context mismatch

`UploadPurpose` is the closed set of purposes a *context* admits, and
`UploadPurpose::admits` is the only way to ask. The profile-post upload path
admits only `PROFILE_POST`, so a `ROOM_POSTER` asset cannot be finalized through
it — refused at init, at put and at finalize, and independently by the stored
row's purpose. `profile_media_upload.rs::a_room_poster_cannot_be_finalized_through_the_profile_post_path`
proves all three refusals and then finalizes the same asset in the room context,
so the refusal is about the context and not the bytes.

### No pixel conversion anywhere

The post command module contains no image code at all, and its test asserts that
structurally. The only transformation a profile photograph undergoes is the
shared `BoundedImageSanitizer`: decode, verify the format and single frame,
re-encode without metadata. The pixels and dimensions are the source's, which the
byte-for-byte tests assert. Poster conversion exists only for room posters
(product-plan.md §2 hard rule 5) and is not reachable from this surface.

### Pagination inside the query

`VISIBLE` is one SQL expression used by both the list and the direct read, and it
is part of the list's `where` clause together with the keyset cursor. A page is
cut from the visible set, never post-filtered; `a_page_is_full_even_when_most_posts_are_private`
asserts that a visitor asking for 5 gets 5 even when 20 of the author's 25 posts
are private. The cursor is a `(created_at, post_id)` pair because
`clock_timestamp()` is not unique; `posts_created_in_the_same_microsecond_are_not_skipped`
forces five identical timestamps and asserts a total walk.

### Foreign ids are not an existence oracle

"Not yours" and "does not exist" are both `404 RESOURCE_UNAVAILABLE`, and a test
compares the two answers for equality. The same rule applies to a media read by
id: a non-owner gets the absent-id answer for both metadata and content.

### The media read decision is one pure function

`can_view_media` takes the facts a query established and returns a decision. It
is deny-by-default, requires every fact, refuses a purpose that does not match
the surface it was found through, and is tested exhaustively without a database.
The metadata read, the byte read and the gallery projection all use it, so they
cannot disagree.

### Cancellation and the orphan window

The frozen §2.3 contract has no delete-media operation, so the client cannot
remove a `PENDING` row. The guarantee is therefore about what the client *sends*:
a cancelled upload never reaches `finalize` and never creates a post, so the only
row it can leave is a `PENDING` intent with no bytes. The server erases that after
the foundation's 24-hour orphan window, and never erases a `READY` asset or one a
live post still references. Both halves are tested.

## 5. Deviations from the task document

1. **`RevisionQuery` on `deleteProfilePost`.** The frozen operation is a bodyless
   `DELETE` whose request schema is `common.RevisionQuery`, but no
   `RevisionQuery` definition exists in `contracts/domain/v1/common.schema.json`
   (only `RevisionRequest`, which is a body). The revision is therefore read from
   the query string (`?expected_revision=N`) by `delete_revision`, which requires
   exactly one in-range field. The contract writer should either declare
   `RevisionQuery` or change the operation to `RevisionRequest`; WP-06 does not
   edit `contracts/`.
2. **`feed.Media.purpose` lists the concert purposes.** The frozen
   `feed.Media` object is shared with the concert feed, so its `purpose` enum
   still names `POST`/`GIVEAWAY`/`REPORT`/`BEACON` and its `read_path` pattern
   still requires an event segment. A profile media projection cannot satisfy
   that pattern. The profile surface therefore projects `feed.Media` with the
   profile purpose set and the profile read path (`/api/media/{id}/content`), and
   the frontend decoder enforces both. This is the same shared-contract gap WP-02
   recorded for the profile room; it needs a contract decision (a profile media
   object, or a widened `feed.Media`), not a code change on one side.
3. **The author projection is the profile id only.** `social.ProfilePost.author`
   is `core.PublicProfileV2`, which the concert service cannot build without a
   cross-service call. The concert service projects `{ profile_id }` and the
   profile service fills in the rest when it composes a response. The frontend
   decoder accepts that subset.
4. **`0086` is carried by two migration files.** `0086_discovery_projection.sql`
   (WP-03) and `0086_profile_avatars.sql` (WP-08) share a number. They touch
   disjoint tables, so a single ordered pass applies both and reaches the same
   head schema, and the WP-06 harness applies both with a recorded warning.
   `profile_social_schema.rs::new_concert_migrations_follow_the_previous_head`
   was widened from "exactly 0080…0085" to "the WP-02 block is present, ordered
   and contiguous" so it stays true as later packages add migrations. Renumbering
   is an integrator task; WP-06 adds no migration.
5. **The composer's upload client is wired through the shared HTTP client.** The
   shared client only accepted a raw binary body for `putConcertUploadBytes`. It
   now accepts `putProfileMediaBytes` through the same `rawUploadOperations` set,
   with the same Blob/type/size checks. This is a one-line widening of an
   existing allowlist, not a second upload path.
6. **`cleanup_owner_orphans` is a test seam with product value.** The bounded
   `cleanup_orphans` is the maintenance entry point. A per-owner variant exists so
   the orphan rule can be proved for one account without a bounded maintenance run
   in a shared test database being starved by another test's rows; it applies the
   same window and the same reference checks.

## 6. Not in this branch

- `V-06-e` (a room poster referencing an intro attachment) is the WP-07 room
  upload route. WP-06 supplies the prohibition it depends on.
- `V-10-c`, `V-10-d`, `V-10-e` are the room save race and the room scene
  lifecycle, both WP-07.
- The profile service's product producer does not yet route the eight §2.3
  operations: the concert-service functions, the closed parser and the gateway
  route table are in place, and the producer wiring is the integration step that
  joins the two services. Every operation is reachable through the gateway's
  closed route table and refused or served by the concert-service functions in
  this branch.
