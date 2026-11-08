# WP-07 — Personal room and posters

Status: **INTEGRATED**. Tasks T-601 … T-612 of
[task-breakdown.md §11](../integration/task-breakdown.md#11-wp-07--room-and-poster-tasks)
are implemented, tested against a real PostgreSQL 16 cluster and the real frontend
suite, and committed on `codex/yc-wp07-room` (base `ff45c37d9`).

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-601 | Room layout value and validation | `backend/crates/concert-domain/src/profile_room.rs` |
| T-602 | Room persistence | `backend/services/concert-service/src/profile_room/repository.rs` |
| T-603 | Room visibility | `backend/services/concert-service/src/profile_room/visibility.rs` |
| T-604 | Poster upload | `backend/services/concert-service/src/profile_room/repository.rs` (`upload_poster`, `validate_png`) |
| T-605 | Room routes | `backend/services/concert-service/src/profile_room/http.rs` |
| T-606 | Pixel conversion | `frontend/src/features/profile-room/pixelize.ts`, `worker/` |
| T-607 | Poster importer | `frontend/src/features/profile-room/PosterImporter.tsx` |
| T-608 | Room scene runtime | `frontend/src/features/profile-room/scene/room-scene.ts` |
| T-609 | Room editor | `frontend/src/features/profile-room/RoomEditor.tsx`, `RoomPage.tsx`, `model/` |
| T-610 | Room entry from the profile | `frontend/src/features/profile/RoomEntry.tsx` |
| T-611 | Room persistence across sessions | `frontend/src/features/profile-room/__tests__/persistence.test.tsx` |
| T-612 | Package verification | `docs/mcr/integration/evidence/v08-room.json`, `v10-room.json` |

The room is a profile-owned row (`profile_rooms`, WP-02 T-107) whose layout is
replaced wholesale by a save. There is no new migration: the table, its revision
guard and its account-deletion guard already exist and are used unchanged.

## 2. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| 5 posters rejected | `concert-domain/tests/profile_room.rs::five_posters_are_rejected` |
| Two furniture items on the same cell in the same layer rejected | `profile_room.rs::two_furniture_items_on_one_cell_in_one_layer_are_rejected` (and the same cell in a different layer is admitted) |
| Unknown wall style rejected | `profile_room.rs::an_unknown_wall_style_is_rejected` |
| Two concurrent saves at one revision: one succeeds, one 409 with the current layout | `profile_room_persistence.rs::concurrent_saves_at_the_same_revision_are_one_success_and_one_conflict`, `::many_concurrent_savers_at_one_revision_commit_exactly_once` |
| A non-owner save writes nothing | `profile_room_persistence.rs::a_non_owner_save_writes_nothing` (asserts `profile_rooms_owner_immutable`), `::a_save_that_names_its_own_owner_is_refused_and_writes_nothing` |
| Room visibility never reads or writes an intro audience | `profile_room_visibility.rs::changing_room_visibility_leaves_every_intro_audience_unchanged`, `::changing_an_intro_audience_leaves_room_visibility_unchanged`, `::the_visibility_module_never_names_an_intro_table` |
| A JPEG poster is rejected | `profile_room_posters.rs::a_jpeg_poster_is_rejected` |
| A PNG whose declared dimensions differ from actual is rejected | `profile_room_posters.rs::a_png_whose_declared_dimensions_differ_from_the_actual_ones_is_rejected` |
| Pixel conversion is deterministic | `pixelize.test.ts::two_runs_on_the_same_input_produce_byte_identical_pngs` |
| Leaving the room destroys the scene and releases textures; re-entering restores the layout | `scene-lifecycle.test.ts::leaving_the_room_destroys_the_scene_and_releases_textures`, `::re_entering_the_room_restores_the_saved_layout_exactly` |
| No presence or "online" affordance in the room | `scene-lifecycle.test.ts::no_presence_or_online_affordance_exists_anywhere_in_the_room`, `::the scene exposes no network, session or presence method` |
| No intro post component imports from `features/profile-room/` | `frontend/src/features/profile/__tests__/RoomEntry.test.tsx::no_intro_post_component_renders_a_room_preview` |

Every negative database case asserts the **specific constraint name** through
`sqlx::Error::Database::constraint()`, never "some error occurred".

## 3. The five frozen operations

| # | operationId | Method | Path |
| --- | --- | --- | --- |
| 2.4.1 | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` |
| 2.4.2 | `getOwnRoom` | GET | `/api/me/room` |
| 2.4.3 | `saveOwnRoom` | PUT | `/api/me/room` |
| 2.4.4 | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` |
| 2.4.5 | `uploadRoomPoster` | POST | `/api/me/room/posters` |

The route table is closed to exactly those five. Any target containing an event
segment **or** an intro segment — including the percent-encoded spellings — is
refused with `400 INVALID_INPUT` before the table is consulted, and the test
asserts that nothing was written. A body over the bound is `413` before it is
parsed. A stale write is `409` with `details.reason = VERSION_CONFLICT` and
`details.current` carrying the current `social.OwnRoom`.

A profile that has never saved reads the default room at revision 1 and
`PUBLIC` visibility. A read never creates a row, so a read cannot authorize or
fabricate a write.

## 4. Verification commands and exit codes

Run from the repository root unless stated. The PostgreSQL cluster is the private,
marker-owned `mcr-pg-*` cluster created by
`backend/scripts/mcr/wp07_room_cluster.py` in `/tmp`; no test attaches to a foreign
server.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | — |
| 2 | `python3 backend/scripts/mcr/wp07_room_cluster.py --report … --run-cmd 'cargo +1.88.0 test --locked -p concert-domain --test profile_room'` | **0** | 17 passed |
| 3 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_room_persistence'` | **0** | 15 passed |
| 4 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_room_visibility'` | **0** | 8 passed |
| 5 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_room_posters'` | **0** | 10 passed |
| 6 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_room_routes'` | **0** | 12 passed |
| 7 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | — |
| 8 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/profile-room` | **0** | 5 files, 57 passed |
| 9 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/profile-room src/features/profile/__tests__/RoomEntry.test.tsx` | **0** | 6 files, 64 passed |

62 backend tests and 64 frontend tests are new in this package.

## 5. Deviations from the task document

1. **No new migration.** T-601 … T-605 were expected to sit on top of WP-02's
   `profile_rooms` (0081) and `profile_media` (0080). Both tables already carry
   everything the room needs — the revision guard, the closed visibility set, the
   closed `ROOM_POSTER` purpose, the state machine and the account-deletion guard —
   so this package adds no migration and edits none.
2. **The default visibility is `PUBLIC`, not the column default `PRIVATE`.**
   http-contract.md §2.4 declares `public_read` for `getProfileRoom` and
   product-plan.md §6 says authorized visitors see the saved room while the owner
   is offline. The column default is only what a row gets when a writer states no
   choice; every write in this package states the visibility explicitly, and a
   profile that has never chosen is projected as `PUBLIC`.
3. **`Domain_feed_Media.purpose` is projected as `POST`.** The frozen
   `Domain_feed_Media` enum has no `ROOM_POSTER` member, while the profile-scoped
   purpose is `ROOM_POSTER` in `profile_media` (whose closed set does include it).
   The projection is a render hint, not an authorization; reading the bytes is
   re-checked against the media row.
4. **The poster preset is enforced server-side.** The conversion contract fixes
   the output size per preset (64×64 fine, 32×32 chunky), so a poster that declares
   any other size was not produced by the contract's conversion and is refused. The
   declared size is still checked against a real decode, so the rule does not
   replace the signature/dimension validation.
5. **The room scene is not `MapScene`.** `MapScene` is a tilemap world with a
   camera, controls, avatars, murals and a skin-replacement protocol. A room is a
   fixed pixel-art space with a handful of objects. The scene reuses the world
   renderer's `sceneryDepth` rule and its texture discipline (register cleanup
   before upload, `FilterMode.NEAREST`, release on failure) rather than the whole
   engine, and it acquires its own Phaser game so it can never share the world's
   lease.
6. **`clampCell` normalizes `-0`.** A move to the left edge clamps to `-0`, which
   `Object.is` treats as distinct from `0`; the first version of the reducer
   therefore reported a move and then dropped it as a no-op. `clampCell` adds `0`
   and `sameCell` compares normalized integers. The keyboard test covers it.
7. **The pre-existing typecheck error in `src/app/errors/__tests__/connectivity.test.ts`
   is fixed.** The file compared a typed event-map key against `'visibilitychange'`
   with `===`, which TypeScript 6 rejects; the comparison is now on the string
   spelling. Without this the required `typecheck` command cannot exit 0.
8. **The room route is registered in the shell router.** `panelRoutes` gains
   `/people/:userId/room` (a subordinate screen of the profile) and the router
   mounts `RoomRoutePanel`. `src/app/__tests__/routes.test.ts` records the addition
   in its `authorizedPanelAdditions` list, exactly as it records the two invitation
   routes granted earlier.

## 6. Limitations

- No live HTTP server was started. The route layer is exercised through its pure
  parser and its handler against a real database, not over a socket. Gateway
  dispatch for the five room operations is a gateway/profile-service composition
  change outside this package's Writes scope.
- The scene tests replace Phaser with a recording double, because Phaser needs a
  real canvas and frame loop. The assertions cover ownership, release, draw order
  and texture filtering; Phaser's own rendering is not exercised.
- The 360/390/430 px browser captures were not taken (no browser session against a
  live stack), so the room screen's visual acceptance remains open. Recorded as
  `NOT_RUN` in `v10-room.json`.
- The shell's profile screen does not yet compose the `RoomEntry` section; the
  entry component and its route are delivered and tested, and the composition is
  the profile screen owner's change.
- V-08-a (deleting an intro post leaves the room unchanged) belongs to WP-05's
  intro deletion; the room table has no intro reference, so a delete cannot reach
  it.
