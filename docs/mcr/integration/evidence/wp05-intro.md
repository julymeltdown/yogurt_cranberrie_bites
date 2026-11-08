# WP-05 — mutuals-intro posts, hearts, likers, notifications, comments

Status: **INTEGRATED** at the producer and client level. Tasks T-401 … T-416 of
[task-breakdown.md §9](../task-breakdown.md#9-wp-05--intro-post-and-hearts-tasks)
are implemented, tested against a real PostgreSQL 16 cluster and the real
frontend transport, and committed on `codex/yc-wp05-intro`.

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-401 | Draft save, publish, delete | `backend/crates/concert-application/src/intro_posts/`, `backend/services/concert-service/src/intro_posts/commands.rs` |
| T-402 | Favorites snapshot | `intro_posts/snapshot.rs` (pure), `concert-service/src/intro_posts/snapshot.rs` (store) |
| T-403 | Attachment rules | `intro_posts/attachments.rs` (pure), `concert-service/src/intro_posts/attachments.rs` (store) |
| T-404 | Comments and replies | `intro_posts/comments.rs` (pure), `concert-service/src/intro_posts/comments.rs` (store) |
| T-405 | Closed intro routes | `concert-service/src/intro_posts/http.rs` |
| T-406 | Composer reducer | `frontend/src/features/intros/composer-reducer.ts` |
| T-407 | Audience fields | `frontend/src/features/intros/IntroAudienceFields.tsx` |
| T-408 | Composer screen | `frontend/src/features/intros/IntroComposer.tsx` |
| T-409 | Media picker and preparation | `frontend/src/features/media/preparation.ts` |
| T-410 | Profile media upload client | `frontend/src/features/media/api/profile-media.ts` |
| T-411 | Heart button | `frontend/src/features/intros/IntroLikeButton.tsx`, `api/intro-likes.ts` |
| T-412 | Liker list panel | `frontend/src/features/intros/IntroLikersPanel.tsx` |
| T-413 | Post card and detail | `frontend/src/features/intros/IntroPostCard.tsx`, `IntroPostDetail.tsx` |
| T-414 | Draft recovery | `frontend/src/features/intros/draft-recovery.ts` |
| T-415 | Activity feed | `frontend/src/features/activity/` |
| T-416 | This evidence | `docs/mcr/integration/evidence/wp05-*`, `v04-*`, `v05-*`, `v08-*` |

Plus the like store and its notification table (T-411 backend) in
`concert-service/src/intro_posts/likes.rs` and migration
`0087_intro_like_notifications.sql`.

Migration numbering: concert-service ends at **0087** (previous head `0086`).
No applied migration was edited.

## 2. The design decisions that matter

### The rules are pure; the store applies them

Every rule lives in `concert_application::intro_posts`, which has no database
handle, no clock and no I/O. The concert-service store calls those functions
inside a transaction. That split is why the rules can be tested exhaustively
(27 unit tests) and why the SQL layer cannot quietly disagree with them.

### A foreign media id is not an existence oracle

`AttachmentError::Unavailable` is the single answer for *every* reason an asset
cannot be attached: absent, foreign, not `READY`, wrong purpose, deleted. The
test asserts that a real asset owned by somebody else and an id that does not
exist produce **the same value**, so no caller can tell them apart.

### The snapshot is a value, not a view

`save_draft` writes `favorites_snapshot` **only on creation**. An update
preserves the stored column regardless of what the caller sent. The test edits
the author's profile favorites for real after publishing and requires the
snapshot to be byte-identical.

### Delete is a soft delete with a bounded cascade

`delete` sets `state = 'DELETED'`, clears `published_at` (the
`intro_posts_published_shape` constraint requires it), and removes exactly
`intro_likes`, `intro_comments` and `intro_attachments`. The test snapshots
`profile_rooms`, `profile_posts`, `discovery_favorite_items`, `profile_media`
and `discovery_profiles` before and after and requires every count unchanged.

### Two PUTs, one row, one notification

The row is the primary key `(intro_id, actor_profile_id)`. The notification is
deduped by `(recipient, intro, actor)`. Both use `on conflict … do nothing`, so a
replay cannot produce a second one. A trigger removes the notification when the
like is withdrawn, so the author never keeps an entry for a like that is gone.

### `like_count: null` renders as an em dash

`likeCountLabel(null)` returns `—`. The button also does not move a null count
optimistically: there is no number to move.

### The audience summary is server-derived, and the client cannot compute one

`OwnIntroPost.audience_summary` is a required string produced by
`Audience::summary()`. The composer renders it verbatim. A test feeds a
deliberately wrong summary (`"Server decided: everyone"`) and requires it to be
rendered as given, proving the client does not override it with its own reading
of the bounds.

### The visibility decision is a port, not a copy

WP-04 owns `can_view_intro`. This package defines the `Visibility` port and
wires `DenyRestricted` as the fail-closed default: an unrestricted post is
visible to everyone, a restricted one only to its author. When WP-04 lands, the
composition swaps the implementation and nothing here changes.

## 3. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| Deleting an intro post removes its likes and comments and NOTHING else | `intro_post_commands.rs::deleting_a_post_removes_likes_and_comments_and_nothing_else` |
| Changing favorites after publishing leaves the post snapshot unchanged | `intro_snapshot.rs::changing_favorites_after_publishing_leaves_the_snapshot_unchanged` |
| A foreign media id is rejected without revealing whether it exists | `intro_attachments.rs::a_foreign_media_id_is_indistinguishable_from_a_missing_one` |
| A fifth attachment is rejected | `intro_attachments.rs::a_fifth_attachment_is_rejected`, `::the_database_refuses_a_fifth_attachment_row` |
| Two PUT likes → one row, one notification | `intro_likes.rs::two_puts_produce_one_row_and_one_notification` |
| DELETE by a viewer who lost access → `like_count: null` | `intro_likes.rs::delete_by_a_viewer_who_lost_access_returns_a_null_count` |
| Non-author liker list → 403 | `intro_likes.rs::a_non_author_liker_list_is_403` |
| The audience heading is exactly "Who can view my mutuals-intro post" | `IntroAudienceFields.test.tsx::uses_the_exact_heading_text` |
| The review step never computes the summary locally | `IntroComposer.test.tsx::shows_the_server-derived_summary_and_never_computes_one`, `::renders_a_server_summary_verbatim_even_when_it_looks_unexpected`, `::does_not_import_a_client-side_summary_helper` |
| Neither IntroPostCard nor IntroPostDetail imports from features/profile-room/ | `IntroPost.test.tsx::neither_IntroPostCard_nor_IntroPostDetail_imports_features_profile-room` |
| No approval/acceptance/pending affordance anywhere | `ActivityFeed.test.tsx::contains_no_approval_affordance_and_no_pending_state`, `IntroLikersPanel.test.tsx::offers_no_acceptance_or_room_affordance` |

Also covered: draft with an empty body saves but does not publish; contact-bypass
body fails to publish; a comment on a restricted post by an unauthorized viewer
is 404; a cross-post parent is rejected; deleting a comment keeps its replies;
an unknown query field is 400; a method mismatch is 400; double-tapping the heart
sends one request; a failed like reverts; a 404 shows the neutral prompt; a GIF
is rejected client-side with the server message; cancelling mid-preparation
leaves nothing; a failed finalize cleans up; an account switch aborts the
upload; a draft from account A is invisible to account B; a recovered draft does
not auto-publish; 18–29 sends `minimum_age: 18, maximum_age_exclusive: 30`.

## 4. Verification commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | clean |
| 2 | `python3 backend/scripts/mcr/wp05_intro_cluster.py --test intro_post_commands --test intro_snapshot --test intro_attachments --test intro_comments --test intro_likes --test intro_routes` | **0** | 52 passed |
| 3 | `cd backend && cargo +1.88.0 test --locked -p concert-application --test intro_post_rules` | **0** | 27 passed |
| 4 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | clean |
| 5 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/intros src/features/media src/features/activity` | **0** | 171 passed in 15 files |
| 6 | `cd frontend && corepack pnpm@11.7.0 run test` (full suite) | **1** | 19 pre-existing failures in 5 files |

### The private cluster harness

`backend/scripts/mcr/wp05_intro_cluster.py` reuses the `migration_smoke.py`
pattern (as `wp06_profile_posts_cluster.py` does): a marker-owned `mcr-pg-*`
cluster in `/tmp`, no TCP listener, no inherited credentials. It applies the
concert-service migrations in order and runs the requested test binaries with
`CONCERT_TEST_DATABASE_URL` set.

### The pre-existing frontend failures

`pnpm run test` fails on 19 tests in 5 files. Each fails identically in a clean
worktree at `ff45c37d9` with no changes applied:

| File | Failing tests |
| --- | --- |
| `src/features/meet-beacon/__tests__/photo-client/client.test.ts` | 14 |
| `src/app/world-entry/__tests__/map-presentation-v1/input-readiness.test.ts` | 2 |
| `src/app/operations-composition/__tests__/reports-production/composition.test.tsx` | 1 |
| `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` | 1 |
| `src/shared/api/paradise-city-v1/__tests__/location-registration.test.ts` | 1 |

None imports the intros, media or activity features.

## 5. Deviations from the task document

1. **The visibility decision is a port with a fail-closed default.** WP-04's
   `can_view_intro` and `visibility_sql` are being written concurrently and are
   not in this worktree. I did not write those files. `intro_posts::visibility`
   defines the `Visibility` port and wires `DenyRestricted`, which denies every
   restricted post to non-authors — strictly stronger than the real policy.
   Swapping in WP-04's implementation is one line in the composition.
2. **The like notification has its own table.** WP-02 deliberately put no trigger
   on `intro_likes`, and the notification-service's table is a different
   service's. Migration `0087` adds `intro_like_notifications` in the concert
   service, deduped as the task requires. Projecting it into the notification
   service is a separate composition step.
3. **`intro_posts` gained no column.** The existing WP-02 table already carries
   every field the commands need, so this package adds one migration (the
   notification table) and edits none.
4. **The delete clears `published_at`.** The WP-02 constraint
   `intro_posts_published_shape` requires a non-published row to carry no
   publication instant. The delete sets it to null; the post's identity, author
   and creation time stay immutable.
5. **`GET /api/intro-posts/{id}/like` is not an operation.** The frozen contract
   has `PUT` and `DELETE` on that path only. The parser refuses `GET` rather than
   letting a read reach a mutation handler; a test asserts it.
6. **`concert-application` gained a `serde_json` dependency.** The audience and
   snapshot wire projections need it. `Cargo.lock` gains one dependency edge.
7. **The media picker uses explicit move-earlier/move-later rather than drag.**
   The task says "no dnd-kit"; the controls are buttons, so reordering is
   keyboard-operable.
8. **`deleteStaged` is a documented no-op.** The frozen register has no
   delete-media operation, so the client cannot delete a staged asset. The asset
   is unreferenced and the retention worker erases it. The function exists so a
   future delete operation has one place to land.

## 6. Limitations

- **The gateway does not serve these 13 operations yet.** The producer and the
  closed parser exist and are tested; mounting the routes is the gateway work
  package's scope.
- **The intro list and detail projections are not built.** The stores expose
  `read_own`, `list` and `media`; a `social.IntroPostDetail` assembler that joins
  the author's public profile, the like state and the comment count is part of
  the gateway/projection composition.
- **`shared_with_author` on a liker row is empty.** The join needs the catalog,
  which lives in the profile service. The field is present and the panel omits
  the line when it is empty, which is the correct behaviour for "no overlap".
- **V-08-c and V-08-d are not exercised here.** They require the WP-07 room
  module, which is not in this worktree. The intro side of the independence is
  proven; the room-to-intro direction is WP-07's evidence.
- **No browser capture.** The 360/390/430 px comparison needs the mounted route.
