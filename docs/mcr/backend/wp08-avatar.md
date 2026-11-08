# WP-08 — Avatar re-keying to profile ownership

Status: **INTEGRATED**. Tasks T-701 … T-706 of
[task-breakdown.md §12](../integration/task-breakdown.md#12-wp-08--avatar-tasks) are
implemented, tested against a real PostgreSQL 16 cluster and the real frontend
suite, and committed on `codex/yc-wp08-avatar` (base `b5926f288`).

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-701 | Profile avatar domain | `backend/crates/concert-domain/src/profile_avatar.rs` |
| T-702 | Profile avatar persistence | `backend/services/concert-service/src/profile_avatar/repository.rs` |
| — | `profile_avatars` table | `backend/services/concert-service/migrations/0086_profile_avatars.sql` |
| T-703 | Avatar routes | `backend/services/concert-service/src/profile_avatar/http.rs` |
| T-704 | Avatar editor migration | `frontend/src/features/profile-avatar/`, `frontend/src/entities/emo-avatar/` |
| T-705 | Avatar cache isolation | `frontend/src/features/profile-avatar/model/cache.ts` |
| T-706 | Package verification | `docs/mcr/integration/evidence/v09-avatar.json` |

The emo character used to be stored per concert participant, keyed by
`(event_id, user_id)`. A personal profile outlives any concert, so the avatar is
re-keyed to profile ownership. The concert table and its operations are
untouched; the profile implementation is additive.

## 2. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| No function signature accepts an event id or participant key | `the_module_source_has_no_event_or_participant_coupling`, `the_route_source_has_no_event_parameter_or_event_path`, `the editor has no event gate` — each inspects the module source for `ParticipantKey`, `participant`, `event_id`, `EventId`, `eventId`, `/events/` |
| No event id appears in any request | `reads and saves through /api/me/avatar with no event id in any request` — every recorded path and body is asserted free of the event id; `the persistence path is the profile path in every request` |
| No fallback character art; `allow_legacy_character_fallback` stays `false` | `allow_legacy_character_fallback stays false and the profile entity references no fallback path` — reads the source pack and scans every profile entity and feature source for fallback identifiers and paths; `refuses to render a character when the artwork cannot be verified, with no substitute` |
| A non-owner write changes nothing (403) | `a_non_owner_write_cannot_reach_another_profiles_avatar` (asserts `profile_avatars_owner_immutable`), `a_write_that_names_its_own_owner_is_refused`, `two_profiles_never_share_an_avatar`, `the_http_projection_of_a_denied_write_is_403` |
| Concurrent writes at the same revision: one success, one conflict | `concurrent_writes_at_the_same_revision_are_one_success_and_one_conflict`, `many_concurrent_writers_at_one_revision_commit_exactly_once` (8 writers → 1 success, 7 conflicts) |
| The source pack and the atlas path/hash contract are preserved | `the pinned atlas path and hash contract is preserved`, `validates the real pinned artwork through the profile entity`; `git diff` on `character-source-pack.json` and `frontend/public/world/avatars/derived-v1/motion-candidates-v1/` is empty |

Every negative database case asserts the **specific constraint name** through
`sqlx::Error::Database::constraint()`, never "some error occurred".

## 3. The two profile operations

| # | operationId | Method | Path | Request | Response |
| --- | --- | --- | --- | --- | --- |
| 2.5.1 | `getOwnProfileAvatar` | GET | `/api/me/avatar` | — | `core.AvatarState` |
| 2.5.2 | `setOwnProfileAvatar` | PUT | `/api/me/avatar` | `core.SetAvatarRequest` | `core.AvatarState` |

The route table is closed to exactly those two entries. Any target containing
`/events/` — including the percent-encoded spellings `%2Fevents%2F` — is refused
with `400 INVALID_INPUT` **before** the table is consulted, and the test asserts
that nothing was written. A stale write is `409` with
`details.reason = VERSION_CONFLICT` and `details.current` carrying the current
`core.AvatarState`, so the client reconciles without a second round trip.

A profile that has never saved reads the release default at revision 1. A read
never creates a row, so a read cannot authorize or fabricate a write.

## 4. Verification commands and exit codes

Run from the repository root unless stated. The PostgreSQL cluster is the
private, marker-owned `mcr-pg-*` cluster created by the
`backend/scripts/mcr/migration_smoke.py` harness pattern in `/tmp`; no test
attaches to a foreign server.

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | — |
| 2 | `python3 backend/scripts/mcr/wp08_avatar_cluster.py --report … --run-cmd 'cargo +1.88.0 test --locked -p concert-domain --test profile_avatar'` | **0** | 9 passed |
| 3 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_avatar_persistence'` | **0** | 13 passed |
| 4 | `… --run-cmd 'cargo +1.88.0 test --locked -p concert-service --test profile_avatar_routes'` | **0** | 12 passed |
| 5 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | — |
| 6 | `cd frontend && corepack pnpm@11.7.0 run test` | **1** | 5776 passed, 19 failed, 14 skipped |

The 19 failures in command 6 are all in four files that fail **identically at the
base commit** with this branch's changes removed:

- `src/app/world-entry/__tests__/map-presentation-v1/input-readiness.test.ts`
- `src/features/meet-beacon/__tests__/photo-client/client.test.ts`
- `src/shared/api/__tests__/c07-social-client/compatibility.test.ts`
- `src/shared/api/paradise-city-v1/__tests__/location-registration.test.ts`

They are pre-existing and unrelated to WP-08. Every WP-08 test file passes:
`editor.test.tsx` (12), `account-switch.test.tsx` (13),
`profile-avatar-panel.test.tsx` (2).

## 5. V-09 disposition

| Case | Status | Note |
| --- | --- | --- |
| V-09-a | NOT_IN_SCOPE | `PUT /api/me/room` belongs to WP-07. |
| V-09-b | **PASS** | `PUT /api/me/avatar` cannot target another profile. |
| V-09-c | **PASS** | The avatar cache and draft are cleared on an account switch. Pending uploads and the room draft belong to other packages. |
| V-09-d | **PASS** | After switching back, the previous account's draft is not visible. |
| V-09-e | **PARTIAL** | The client recognizes `ACCOUNT_CONTEXT_STALE`, clears the cache and writes nothing. The server does not emit the reason yet — the string does not exist anywhere in `backend/`. |

V-09-e is recorded as `PARTIAL`, not `PASS`. WP-02 recorded the same gap: the
foundation has no `account_context_version`, so the client half is correct and
waiting, and the server half belongs to whichever package introduces the version.

## 6. Deviations from the task document

1. **`profile_avatars` migration.** WP-02's task list named `profile_avatars`, but
   the schema it delivered (`0080`–`0085`) does not contain it. WP-08 authors the
   table as `0086_profile_avatars.sql`, strictly greater than the `0085` head,
   with no applied migration edited.
2. **`config` column shape.** `domain-contract.md` §4 names `config jsonb` and
   leaves the shape to the avatar catalog. The column stores the canonical
   `core.AvatarConfig` object; the catalog validates its contents and the column
   only fixes `jsonb_typeof = 'object'`.
3. **The reviewed manifest source.** The concert authority fetched
   `/api/concert/events/{eventId}/bootstrap` to learn the reviewed manifest. A
   profile avatar has no event, so the profile authority takes the pinned
   manifest from the shell's own release read and issues no event-scoped request.
   `admitProfileAvatar` checks the account, not an event.
4. **`MotionAuthority` qualification union.** `entities/emo-avatar/runtime.ts`
   widens `MotionAuthority` to a union of `TEST_ONLY` and `REVIEWED_PRODUCTION`.
   The save gate reads `canSave`, which only the production variant carries, so a
   fixture authority cannot satisfy it.
5. **The concert editor is not deleted.** `features/avatar/motion-adoption/` and
   `entities/avatar/runtime/motion-adoption/` remain for the concert product,
   which still uses the event-scoped avatar. WP-08 adds the profile
   implementation beside them and switches the shell's avatar view to it.
6. **Whole-character and layered editors.** The shell still falls back to
   `WholeCharacterEditor` or `AvatarEditor` when the release is not the reviewed
   motion character. Those are different admitted representations of the same
   profile avatar, not character fallbacks; the reviewed-motion path never
   substitutes one for a failed art load.

## 7. Limitations

- No live HTTP server was started. The route layer is exercised through its pure
  parser and its handler against a real database, not over a socket. Gateway
  dispatch for `/api/me/avatar` already exists in the frozen profile route table
  (WP-02), but the profile producer does not yet serve the avatar operation;
  wiring it is a gateway/profile-service composition change outside this
  package's Writes scope.
- V-09-a (room) and the media/room halves of V-09-c belong to other packages.
