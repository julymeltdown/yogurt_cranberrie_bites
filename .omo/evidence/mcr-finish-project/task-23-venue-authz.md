# Task 23 — venue authorization and privacy matrix

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-authz`
- Branch: `codex/mcr-exec-w3-authz`
- Base: `cc8ad1b5b` (`master`)
- Delivered commit: `08bcfba8d test(venue): cover the authorization and privacy matrix`
- Scope inputs read: `.omo/plans/mcr-finish-project.md` rows 18–23,
  `docs/mcr/venue-feature-amendment-2026-11-05.md`,
  `docs/mcr/contracts/paradise-city-v1/policy-freeze.md`, venue OpenAPI 3.1.0.

## Delivered

| Path | Role |
| --- | --- |
| `backend/services/concert-service/tests/venue_authz_matrix.rs` | 46 real-PostgreSQL matrix cells + contract/admission anchors |
| `backend/services/api-gateway/tests/venue_authz_http.rs` | Real Gateway HTTP feature route, error envelope and cache-control cells |
| `backend/scripts/mcr/venue_authz_mutation_oracle.py` | Throwaway-worktree mutation oracle (minimum three flipped cells) |

No production source, migration, register, lockfile or `docs/source/` byte was
changed.

## Surface map and proof boundary

- **Application guards (real PostgreSQL, production adapters).**
  `PgVenueLocation`, `PgVenueMeeting` and `PgPlaceNotes` were driven directly.
  Friend/Auth/Place/Profile/Moderation owners are test-only PostgreSQL tables
  because the production bilateral `VENUE_FRIEND_V1` read does not exist; the
  production ports remain fail-closed (`PolicyUnavailable`/503) and are not
  replaced by this test.
- **HTTP.** The real Gateway `concert_product` router and `venue-meeting`
  feature routes were exercised with real signed browser identities. Every
  identity reaches the honest composed result on this base: `503
  DEPENDENCY_UNAVAILABLE` (no production product client), with the exact
  canonical error envelope and no-store headers. HTTP-level origin/intent/key
  and target/body guards also produced their concrete `403`/`400` cells.
- **Cache layer.** Venue responses pass the Gateway protected-response layer:
  `cache-control: no-store`, `pragma: no-cache`, `nosniff`, and no
  `ETag`/`Last-Modified` on every asserted response. Adapter reads were repeated
  on the same service instance after pause/revoke/block/hide/expiry; no stale
  private row was disclosed.
- **Location and note HTTP mounting is not present on base `cc8ad1b5b`.**
  Row 22 (`exec-w3-pg`) is adding those Gateway routes in parallel. On this
  base, a product-RPC `venue/location` target is refused as an unsupported
  operation (`PermissionDenied`), which the matrix asserts; therefore L/N HTTP
  authorization is proven at the application/authorization layer, not claimed
  as an HTTP 200/403 result. This is a real dependency gap for rows 29/31, not
  a client-side substitute.

## Matrix summary (46 tests, all pass)

Outcome is the exact server result; the status is the frozen OpenAPI class.
Location hides denial with 404 (its contract declares no 403); meeting and notes
declare 403 for explicit denial.

### Location — `PgVenueLocation` (real PG)

| Cell | Actor | Outcome | Status |
| --- | --- | --- | --- |
| own snapshot | owner | `ACTIVE`, own PHONE position | 200 |
| selected viewer | friend | position visible, `physical_arrival_verified=false` | 200 |
| unselected viewer | stranger | `Denied`; snapshot friends empty, no coordinates | 404 |
| blocked selected viewer | blocked | `Denied`; snapshot friends empty | 404 |
| suspended viewer | suspended | `Denied`; friend read denied | 404 |
| expired lease | friend | `NotFound`; owner snapshot `EXPIRED`, no position | 404 |
| operator not selected | admin | `Denied`; snapshot friends empty | 404 |
| pause | owner | viewers=0, fixes=0; partner read `NotFound` | 200 |
| revoke | friend | immediate `NotFound` on same instance; no cached fix | 404 |
| non-participant | outsider | `Denied` | 404 |

### Invitations — `PgVenueMeeting` (real PG)

| Cell | Actor | Outcome | Status |
| --- | --- | --- | --- |
| create with reviewed landmark | friend pair | `APPLIED`, `PENDING`, pair-only fields | 200 |
| create | stranger | `ResourceUnavailable` | 404 |
| create | blocked | `ResourceUnavailable` | 404 |
| create | suspended | `ResourceUnavailable` | 404 |
| create | admin | `ResourceUnavailable` | 404 |
| stale landmark revision | friend pair | `ResourceUnavailable` | 404 |
| accept then sender cancel | recipient/sender | `ACCEPTED` revision 2 → `CANCELLED` revision 3 | 200 |
| decision after cancel | recipient | `ResourceUnavailable` | 404 |
| expired invitation decision | recipient | `InvalidState`; detail actions empty | 409 |
| foreign detail + inbox | stranger | `ResourceUnavailable`; inbox 0 items, no ids echoed | 404 / 200 |

### Conversation — `PgVenueMeeting` (real PG)

| Cell | Actor | Outcome | Status |
| --- | --- | --- | --- |
| context | friend | `canRead=true`, `canSend=true`, no landmark → `canInvite=false` | 200 |
| context | stranger | `ResourceUnavailable` | 404 |
| context | blocked | `ResourceUnavailable` | 404 |
| context | suspended | `ResourceUnavailable` | 404 |
| context | admin | `ResourceUnavailable` | 404 |
| message round trip | friend pair | one message, exact body/sender | 200 |
| message list | stranger | `ResourceUnavailable` | 404 |
| expired message | friend | empty page; body absent | 200 |
| block after message | friend | `ResourceUnavailable`; history retained but not disclosed | 404 |
| stranger send with `invitationId` | stranger | `ResourceUnavailable` (link grants nothing) | 404 |
| context write side effect | friend | context/conversation counts unchanged (0,0) | 200 |

### Place notes — `PgPlaceNotes` (real PG)

| Cell | Actor | Outcome | Status |
| --- | --- | --- | --- |
| FRIENDS note | friend | visible with exact text | 200 |
| FRIENDS note | stranger | absent; thread `NotFound`; no text in payload | 200 / 404 |
| FRIENDS note after block | blocked | absent; thread `NotFound` | 200 / 404 |
| suspended actor (not event-eligible) | suspended | list/create `Denied` | 403 |
| FRIENDS note | admin (moderator, not friend) | absent; thread `NotFound` | 200 / 404 |
| EVERYONE note | stranger, blocked, admin | visible to current participants | 200 |
| withdraw by non-author | friend | `Denied` | 403 |
| reply to FRIENDS note | friend / stranger | friend reply visible, count 1; stranger `Denied` | 403 |
| expired note | stranger | absent; thread `NotFound` | 200 / 404 |
| moderator hide | admin | root + reply hidden, bodies purged to `""` | 200 |
| Meet Here | friend / stranger | friend entry only; 0 invitation/0 reply rows written | 200 |
| unreviewed landmark | owner | `NotFound` | 404 |
| block after read | friend | note disappears immediately on the same instance | 200 / 404 |

### HTTP — real Gateway `venue-meeting` routes

| Cell | Actors | Outcome |
| --- | --- | --- |
| `GET context?selectedFriendId=…` | friend, stranger, blocked, suspended, expired, admin | `503 DEPENDENCY_UNAVAILABLE`, exact `{code,message,traceId,timestamp,details:{reason}}` body, id not echoed |
| `POST invitations` without origin | owner | `403 ORIGIN_NOT_ALLOWED` |
| `POST invitations` without intent | owner | `403 BROWSER_INTENT_REQUIRED` |
| `POST invitations` without key | owner | `400 INVALID_INPUT` |
| `POST invitations` with all mutation headers | owner | `503 DEPENDENCY_UNAVAILABLE` |
| malformed context/messages/invitations targets | owner | `400 INVALID_INPUT` before any owner read |

Every HTTP response asserted `cache-control: no-store`, `pragma: no-cache`,
`x-content-type-options: nosniff`, and absence of `ETag`/`Last-Modified`.

The contract anchor test reads the three frozen OpenAPI files and asserts every
status used above is declared for that operation, that the error schemas have
`additionalProperties=false` with exactly `code/message/traceId/timestamp/details`,
and that `details.reason` remains a closed enum.

## Commands and exit codes

| Command | Exit | Decisive output |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-authz CARGO_BUILD_JOBS=3 cargo +1.88.0 check -p concert-service --test venue_authz_matrix` | 0 | `Finished dev profile` |
| `… cargo +1.88.0 test -p concert-service --test venue_authz_matrix -- --test-threads=1` (private PG) | 0 | `46 passed; 0 failed` in 0.65s |
| `… cargo +1.88.0 test -p api-gateway --test venue_authz_http -- --test-threads=1` | 0 | `4 passed; 0 failed` in 6.49s |
| `python3 backend/scripts/mcr/venue_authz_mutation_oracle.py --target-dir …` | 0 | `"result": "PASS"` (3 / 10 / 4 flips) |
| `rustfmt +1.88.0 --edition 2021` on both test files | 0 | formatted |
| `git diff --check` | 0 | clean |
| `GIT_MASTER=1 git status --short` after commit | 0 | empty |
| `GIT_MASTER=1 git log -1 --oneline` | 0 | `08bcfba8d test(venue): cover the authorization and privacy matrix` |

## Failure oracle (throwaway worktree)

The script creates `git worktree add --detach HEAD`, copies only the uncommitted
matrix test, removes one guard, runs the same real-PG suite, reverts, and
removes the worktree. Each mutant must flip at least three cells.

| Mutant | Guard removed | Failed cells | Flipped |
| --- | --- | --- | --- |
| `location_visibility` | viewer-scope + bilateral friend check in `visible_for` | 3 | stranger, blocked friend, admin |
| `meeting_friendship` | `require_friend` authority call | 10 | invitation strangers/blocked/suspended/admin; conversation stranger/blocked/suspended/admin/read/link |
| `notes_audience` | FRIENDS audience predicate in `audience_ok` | 4 | stranger, blocked friend, admin FRIENDS read, block-after-read |

Raw result:

```json
{
  "result": "PASS",
  "mutants": [
    {"mutant": "location_visibility", "passed": 43, "failed": 3,
     "flipped": ["location_admin_without_selection_denied",
                 "location_blocked_friend_denied_owner_location",
                 "location_stranger_denied_owner_location"]},
    {"mutant": "meeting_friendship", "passed": 36, "failed": 10,
     "flipped": ["conversation_admin_context_denied",
                 "conversation_block_revokes_read_access",
                 "conversation_blocked_context_denied",
                 "conversation_stranger_cannot_read_messages",
                 "conversation_stranger_context_denied",
                 "conversation_suspended_context_denied",
                 "invitation_admin_create_denied",
                 "invitation_blocked_create_denied",
                 "invitation_stranger_create_denied",
                 "invitation_suspended_create_denied"]},
    {"mutant": "notes_audience", "passed": 42, "failed": 4,
     "flipped": ["notes_admin_cannot_read_friends_note",
                 "notes_block_removes_friends_note_immediately",
                 "notes_blocked_friend_cannot_see_friends_note",
                 "notes_stranger_cannot_see_friends_note"]}
  ]
}
```

## Private PostgreSQL provenance and cleanup

- PostgreSQL 16.14 (Homebrew), `listen_addresses=''`, Unix socket only at
  `/Users/lhs/Desktop/mcr-build/exec-w3-authz-pg/sock`, port 55447,
  `max_prepared_transactions=256`.
- Root `/Users/lhs/Desktop/mcr-build/exec-w3-authz-pg`, mode `0700`, owner uid
  501 (the runner uid).
- Applied `0001_concert_core.sql`, `0073_venue_location.sql`,
  `0074_venue_invitations.sql`, `0075_place_notes.sql`; 18 public tables,
  including all nine `concert_venue*`/`concert_place_notes` tables.
- Cleanup receipt: `pg_ctl … -m fast stop` exit 0 (`server stopped`), root
  removed (`ROOT_EXISTS False`), no oracle temp worktree or temp directory
  remains.

## Limitations and risks

- **L/N HTTP composition is absent on this base.** Row 22's parallel Gateway
  mount is required before location/note HTTP statuses can be asserted; the
  product RPC also refuses these targets today. The matrix proves the server
  guards and exact outcomes, not an L/N HTTP 200/403.
- **Production friend authority is unavailable.** HTTP `venue-meeting` is
  therefore `503` for every role on this base; the role matrix uses the same
  production adapters with test-only owner evidence. No production
  friend/block/suspension stack is claimed.
- **Suspension model for notes.** `PgPlaceNotes` has no account-state port; the
  suspended note actor is modelled as no longer event-eligible (the existing
  `concert_participants` guard), which returns `Denied`/403. Location and
  meeting use the explicit suspension owner fixture.
- **Blocked `EVERYONE` notes** remain visible to blocked participants, matching
  policy N2 (block is the FRIENDS audience relation); FRIENDS notes are hidden.
- Location denial is 404 because its frozen OpenAPI declares no 403.
- Real Friend/Auth service wiring, two-client live flow, and device/browser
  evidence remain with rows 29/31 and are not claimed here.
