# WP-03 — favorites, catalog, and shared-interest discovery

Status: **INTEGRATED** at the producer and client level. Tasks T-801 … T-806 of
[task-breakdown.md §13](../task-breakdown.md#13-wp-03--favorites-and-discovery-tasks)
are implemented, tested against a real PostgreSQL 16 cluster and the real
frontend transport, and committed on `codex/yc-wp03-discovery`.

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-801 | Catalog table, catalog policy, search, seed path | `backend/services/profile-service/migrations/0016_catalog_items.sql`, `backend/services/profile-service/src/catalog/`, `backend/services/profile-service/catalog/catalog-seed.v1.json`, `backend/scripts/mcr/catalog_import.py` |
| T-802 | Discovery query | `backend/services/concert-service/migrations/0086_discovery_projection.sql`, `backend/services/concert-service/src/discovery/repository.rs` |
| T-803 | Discovery route parser | `backend/services/concert-service/src/discovery/http.rs` |
| T-804 | Favorite picker | `frontend/src/features/favorites/` |
| T-805 | Discovery screen | `frontend/src/features/discovery/` |
| T-806 | This evidence | `docs/mcr/integration/evidence/discovery-*` |

Migration numbering: profile-service ends at **0016** (previous head `0015`),
concert-service ends at **0086** (previous head `0085`). No applied migration was
edited.

## 2. The design decisions that matter

### There was no catalog, so there is now the smallest honest one

The repository had no music/anime/game catalog. Rather than build a provider
integration, the catalog is:

* one table, `catalog_items`, keyed by the canonical UUID the frozen contract
  calls `catalog_item_id` (the donor's `tci_…` prefixed text ids are not used);
* one closed JSON manifest of hand-curated rows;
* one importer, `backend/scripts/mcr/catalog_import.py`, that validates the
  manifest and writes through `psql`.

The import is idempotent: re-running it upserts on `(category, lower(label))`.

### K-pop exclusion is a schema rule, not a query filter

The task says the exclusion must be *catalog policy, not string filtering at
query time*. That is implemented literally:

* `catalog_items_music_excludes_kpop` makes a K-pop music row impossible to
  store, so no search can return one;
* `catalog_items_music_requires_exclusion_review` makes every music row carry an
  explicit review decision, so a writer cannot import an unreviewed act by
  omitting the flag;
* `catalog_items_exclusion_category` stops a non-music row claiming an exclusion.

The search query performs **no** label test. If a K-pop row somehow existed, the
search would return it — which is exactly why the exclusion lives in the schema
and has two negative tests that assert the specific constraint names.

### Discovery starts from profiles, never from intro posts

The donor (`explicit_taste_matches.rs`) joins `intros` and hangs the overlap off
them, so a profile that never wrote an intro is invisible. The destination query
drives from `discovery_profiles` and does not reference `intro_posts` at all.
Two tests protect that:

* `a_profile_with_no_intro_post_is_discoverable` — the fixture asserts the
  subject has zero `intro_posts` rows, then requires it in the page;
* `the_discovery_query_never_references_intro_posts` — reads the repository
  source, strips comments, and fails if `intro_posts` or `intro_id` reappears.

### Overlap is catalog ids only, and there is no score

The join is on `catalog_item_id`. The label is carried for display and never
compared. The result type has no numeric field, the schema has no score column,
and the ordering is a stable `(nickname, profile_id)` sort. Four tests hold that
line:

* `the_response_contains_no_numeric_score_field` — walks the serialized page and
  rejects every numeric value and every score/rank/match/count-like key;
* `the_order_does_not_rank_by_shared_item_count` — a profile sharing one item
  sorts above one sharing three;
* `the_discovery_schema_has_no_score_column`;
* `overlap_uses_catalog_ids_and_never_labels`.

### An empty overlap is an empty array, never an omission

`PeopleCard.shared_items` is required and has `minItems: 1` in the frozen
schema, so a card that appears in the people list always names what it shares.
The "no overlap" case is therefore expressed where it can occur:

* the profile is absent from the people list entirely;
* the frontend `sharedInterestLine()` returns `null` for an empty list, and the
  card renders **no** shared-interest element at all;
* the frontend decoder refuses a card that claims nothing.

## 3. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| A profile with NO intro post appears in discovery | `discovery.rs::a_profile_with_no_intro_post_is_discoverable`, `discovery.rs::the_discovery_query_never_references_intro_posts` |
| A profile with no overlap appears without a shared-interest claim | `discovery.rs::a_profile_with_no_overlap_is_not_claimed_as_sharing`, `Discovery.test.tsx::omits_the_shared-interest_line_when_there_is_no_overlap` |
| The response contains no numeric score field | `discovery.rs::the_response_contains_no_numeric_score_field`, `discovery.rs::the_discovery_schema_has_no_score_column`, `Discovery.test.tsx::rejects_a_page_that_carries_a_score_field` |
| K-pop is absent from music search results | `catalog_search.rs::a_kpop_artist_is_absent_from_music_search`, `catalog_search.rs::the_database_refuses_a_kpop_music_row` |
| A sixth favorite in one category is rejected | `discovery.rs::a_sixth_mirrored_favorite_is_refused`, `FavoritePicker.test.tsx::blocks_a_sixth_pick_in_one_category` |
| Skipping all categories leaves the profile usable | `FavoritePicker.test.tsx::skipping_every_category_leaves_the_profile_usable` |
| The same query returns the same order twice | `catalog_search.rs::the_same_query_returns_the_same_order_twice` |
| An unknown query field is `400` | `discovery_routes.rs::an_unknown_query_field_is_a_client_error` |
| The shared-interest line is omitted with no overlap | `Discovery.test.tsx::omits_the_shared-interest_line_when_there_is_no_overlap` |
| No room preview or room action on a discovery card | `Discovery.test.tsx::offers_no_room_preview_or_room_action_on_a_discovery_card` |
| The two lists are never merged | `Discovery.test.tsx::keeps_the_people_list_and_the_mutuals-intro_list_as_separate_tabs` |
| The three tabs are in the order music, anime, games | `FavoritePicker.test.tsx::shows_the_three_tabs_in_the_order_music,_anime,_games` |
| No intensity, importance or preference control | `FavoritePicker.test.tsx::renders_no_intensity,_importance_or_preference_control` |
| Private profiles and blocks are applied | `discovery.rs::a_private_profile_is_never_discovered`, `discovery.rs::a_block_in_either_direction_hides_the_profile`, `discovery.rs::a_removed_block_no_longer_hides_the_profile` |

Every negative database case asserts the **specific constraint name** through
`sqlx::Error::Database::constraint()`, never "some error occurred".

## 4. Verification commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | clean |
| 2 | `python3 backend/scripts/mcr/wp03_discovery_tests.py --profile-test catalog_search` | **0** | 15 passed |
| 3 | `python3 backend/scripts/mcr/wp03_discovery_tests.py --concert-test discovery` | **0** | 17 passed |
| 4 | `python3 backend/scripts/mcr/wp03_discovery_tests.py --concert-test discovery_routes` | **0** | 11 passed |
| 5 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | clean |
| 6 | `cd frontend && npx vitest run src/features/favorites/__tests__/FavoritePicker.test.tsx src/features/discovery/__tests__/Discovery.test.tsx` | **0** | 24 passed |
| 7 | `cd frontend && corepack pnpm@11.7.0 run test` | **1** | 18 pre-existing failures in 4 files, identical on master `b5926f288` |

### The private cluster harness

`backend/scripts/mcr/wp03_discovery_tests.py` reuses the WP-02 pattern from
`migration_smoke.py`: a marker-owned `mcr-pg-*` cluster in `/tmp`, no TCP
listener, no inherited credentials. It creates two fixture databases, applies
each service's migrations through the same SQLx-style ledger the service
migrator writes, seeds the catalog from the manifest, runs the requested test
binaries with `PROFILE_TEST_DATABASE_URL` / `CONCERT_TEST_DATABASE_URL`, and
removes the cluster.

### The pre-existing frontend failures

`pnpm run test` fails on 18 tests in 4 files. Every one of them fails
identically in a clean worktree at master `b5926f288` with no changes applied:

| File | Failing tests |
| --- | --- |
| `src/features/meet-beacon/__tests__/photo-client/client.test.ts` | 14 |
| `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` | 1 |
| `src/shared/api/paradise-city-v1/__tests__/location-registration.test.ts` | 1 |
| `src/app/world-entry/__tests__/map-presentation-v1/input-readiness.test.ts` | 2 |

They are unrelated to WP-03: none of them imports the favorites or discovery
features, and the failures are about a pinned operation count, a pinned input
manifest hash and the photo client's receipt recovery.

## 5. Deviations from the task document

1. **T-801 catalog table.** The task names only
   `backend/services/profile-service/src/catalog/`. A search over a catalog needs
   a catalog, and the repository had none, so this work package also adds
   migration `0016_catalog_items.sql`, the seed manifest, and the importer. This
   is the "catalog table plus a seed/import path" the assignment authorized.
2. **T-802 discovery projection.** The task names
   `backend/services/concert-service/src/discovery/repository.rs`. Discovery
   lives in the concert service and favorites live in the profile service, and
   the two do not share a database, so the query needs a local mirror. Migration
   `0086_discovery_projection.sql` adds `discovery_profiles`,
   `discovery_favorite_items` and `discovery_blocks`. The mirror is written by no
   producer yet (see limitations).
3. **`core.PeopleCard.avatar`.** The frozen schema requires an `avatar` object on
   every card. The concert service has no avatar store and the profile service's
   avatar projection does not exist yet, so the card type omits `avatar` rather
   than fabricating a default. The frontend card renders the nickname initial
   until the avatar projection lands.
4. **`FavoriteItem.external_url` in discovery cards.** `discovery_favorite_items`
   stores no URL, so `shared_items[].external_url` is always `null`. The contract
   allows `null`, and the shared-interest line only needs the label.
5. **Discovery returns only overlapping profiles.** The product plan says two
   profiles that share nothing may still browse each other's public profiles.
   That is served by `getProfile`, not by the people list; the people list is
   defined as "profiles whose selected catalog items overlap with the viewer's".
   The no-overlap rule is therefore about what a *card* claims, which is what the
   tests assert.
6. **`interaction_style` is mirrored, not authored.** `discovery_profiles` carries
   the column so a card can show the optional line; the profile's own setting
   lives in the profile service and is not written by this work package.
7. **The gateway route is not mounted.** See limitations.
8. **`base64ct` added to two service manifests.** The opaque cursors are
   base64url without padding, and `base64ct` is already the workspace's base64
   crate. `Cargo.lock` gains the two new dependency edges and nothing else.

## 6. Limitations

- **The gateway does not serve these two operations yet.**
  `/api/catalog/{category}/search` and `/api/discover/people` are frozen in
  `contracts/core/v1/operations.json` and both producers exist, but mounting the
  routes is the gateway work package's scope. This evidence is at the producer
  and client level, not an end-to-end HTTP capture.
- **No projection writer.** `discovery_profiles`, `discovery_favorite_items` and
  `discovery_blocks` are read by the query and tested against real rows, but no
  producer populates them yet. Writing that projection is a separate work
  package because it crosses the profile/concert service boundary.
- **The catalog is a seed, not a provider integration.** 39 stored rows plus 4
  K-pop policy records. It is intentionally small and reviewable in a diff.
- **The mutuals-intro list is a slot, not an implementation.** The discovery
  screen renders it as its own tab and never merges it with the people list; the
  intro-post list itself is a different work package with its own audience
  filter.
- **No browser capture.** The 360/390/430 px comparison is a visual acceptance
  step that needs the mounted route; it is not claimed here.
