# Task W3 — production bilateral venue friend authority

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-friend`
- Branch: `codex/mcr-exec-w3-friend`
- Base: `2f2f11b61ee2f33812ea848ba21c890d529e10b9` (`master`)
- Status: `IMPLEMENTED / DEVELOPER-VALIDATED`
- Delivered subject: `feat(venue): compose the bilateral friend authority`
- Publication: local branch only; no push or merge.

## Gap closed

`PgVenueLocationAuthority::friend_in` returned `PolicyUnavailable` for every
pair. The same gap made the meeting `VenueMeetingAuthority::friend` path
unreachable. The existing signed `relationship.v1.FriendConcertRelationshipService/ReadCurrent`
already read both blocks and both deletion tombstones in one MVCC snapshot, but
it did not report the two live follow directions that `VENUE_FRIEND_V1`
requires. A directional `CheckFollow` flag, a pending follow request, a shared
handle or a party membership is not friendship.

The chosen change extends that existing signed observation with two additive
fields (`actor_follows_subject = 10`, `subject_follows_actor = 11`) read from
`public.follows` in the same statement snapshot. No new service, RPC or
protocol was introduced. Calling `CheckFollow` twice was rejected because it is
Gateway-only, unsigned, non-atomic and carries no block/deletion snapshot; a
cross-owner database query was rejected because the Friend owner owns the graph.

## Authority table

| Layer | Fact | Authoritative read | Failure behavior |
| --- | --- | --- | --- |
| Friend | `(actor -> subject)` and `(subject -> actor)` live `follows` rows (`deleted_at IS NULL`) | signed `ReadCurrent` on `public.follows` | owner unavailable, malformed, stale or deadline exceeded: `503` |
| Friend | live block in either direction | signed `ReadCurrent` on `public.user_blocks` | any live block: deny |
| Friend | permanent account-deletion tombstone for either subject | signed `ReadCurrent` on `public.friend_user_deletion_tombstones` | either deleted: deny |
| Auth | actor active and the request's session family live; subject active | signed `ReadCurrent` on `public.users` and `public.refresh_tokens` | owner unavailable: `503`; inactive/suspended/deleted: deny |
| Concert | event phase `OPEN` or `LIVE`; both users are current `concert_participants` | local PostgreSQL in the location effect transaction | not eligible: deny |
| Venue location | explicit `ACTIVE` sharing lease selecting this viewer | `concert_venue_location_sharing` / `viewers` | no consent: no location (`NotFound`/`Denied`); friendship alone never discloses |

`VENUE_FRIEND_V1(a, b, event)` is allowed only when both live follow directions
exist, neither live block exists, both Auth accounts are current, neither
Friend tombstone exists, both are participants, and the event is `OPEN` or
`LIVE`. The location adapter additionally requires the owner's explicit
sharing lease selecting the viewer.

| Case | Mutual follows | Block | Account state | Event | Sharing selects viewer | Result |
| --- | --- | --- | --- | --- | --- | --- |
| mutual friends | yes | no | both `ACTIVE` | `OPEN` | yes | location visible (`Ok`) |
| one-way follow | one edge | no | both `ACTIVE` | `OPEN` | yes | denied; no location |
| blocked | yes | either direction | both `ACTIVE` | `OPEN` | yes | denied; no location |
| suspended viewer | yes | no | viewer `SUSPENDED` | `OPEN` | yes | denied at actor authority |
| suspended owner | yes | no | owner `SUSPENDED` | `OPEN` | yes | denied at actor authority |
| archived event | yes | no | both `ACTIVE` | `ARCHIVED` | yes | denied |
| no explicit consent | yes | no | both `ACTIVE` | `OPEN` | no | no friend location (`NotFound`) |
| Friend owner down | unknown | unknown | unknown | `OPEN` | unknown | `PolicyUnavailable` (`503`), never `200` |

## Request binding and composition

The authority is request-bound. A `RequestBinding` is built from the already
verified product request (`VerifiedProduct`: actor, event, session family,
request id, deadline), never from caller-supplied identity. Per pair it replaces
only the subject and field; the authenticated actor, session family, request id
and deadline remain fixed. `VenueFriendAuthority::actor` uses `SafetySelf` to
check the current actor's Auth session/account and Friend deletion tombstone.
`VenueFriendAuthority::pair_in` uses the location effect transaction for local
event scope; `pair` acquires its own connection for the meeting authority.

Composition entry points:

- `PgVenueLocationAuthority::from_binding` (tests / already verified binding) and
  `PgVenueLocationAuthority::from_product` (verified product).
- `PgVenueLocation::production(pool, policy, relationships, product, subject)`.
- `PgVenueMeeting::production(pool, relationships, product, subject)`.
- `VenueFriendAuthority` also implements `VenueMeetingAuthority`; its friend gate
  is real, while `reviewed_landmark` and `policy` remain honest
  `PolicyUnavailable` until separate providers are composed.

`PgVenueLocationAuthority::default()` remains the explicit fail-closed authority
for callers that have not composed the Friend read. The `friend_in` port now
names its first argument the authenticated `actor` and its second the `peer`;
`set_sharing` was calling it with the selected viewer first and is corrected.

## Changed paths

| Path | Change |
| --- | --- |
| `backend/proto/relationship/v1/relationship.proto` | additive fields 10/11 on `FriendRelationshipObservation` |
| `backend/crates/friend-application/src/concert_relationship.rs` | both follow facts in `CurrentRelationship`; `mutual_follow()` |
| `backend/crates/friend-application/tests/concert_relationship.rs` | mutual-follow and one-way unit coverage |
| `backend/services/friend-service/src/adapters/out/persistence/concert_relationship.rs` | same-snapshot `follows` reads |
| `backend/services/friend-service/src/adapters/in/grpc/concert_relationship.rs` | response mapping |
| `backend/services/friend-service/tests/concert_relationship.rs` | snapshot fixture fields |
| `backend/services/concert-service/src/adapters/out/relationship_authority/mod.rs` | verified binding helpers and `friend_mutual_follow` accessor |
| `backend/services/concert-service/src/adapters/out/relationship_authority/grpc.rs` | map both signed owner facts |
| `backend/services/concert-service/src/venue_authority.rs` | production `VENUE_FRIEND_V1` composition |
| `backend/services/concert-service/src/venue_location/mod.rs` | production authority, actor/peer order, `production` constructor |
| `backend/services/concert-service/src/venue_meeting/mod.rs` | meeting authority production constructor |
| `backend/services/concert-service/src/lib.rs` | export the authority module |
| `backend/services/concert-service/tests/relationship_authority.rs` | new owner observation fields |
| `backend/services/concert-service/tests/venue_location.rs` | fail-closed default constructor |
| `backend/services/concert-service/tests/venue_friend_authority.rs` | new real-PostgreSQL/real-owner suite |

No lockfile, manifest, shared registry, root router, frontend, `docs/source/` or
historical evidence file was changed.

## Commands and exit codes

| Command | Exit | Decisive output |
| --- | --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-friend CARGO_BUILD_JOBS=3 cargo +1.88.0 check --workspace --all-targets` | 0 | `Finished dev profile` |
| `cargo +1.88.0 test -p concert-service --test venue_friend_authority -- --test-threads=1` with the three private DB URLs | 0 | `8 passed; 0 failed; 0 ignored` in 0.45s |
| `python3 backend/scripts/mcr/venue_postgres_smoke.py --target-dir /Users/lhs/Desktop/mcr-build/exec-w3-friend --report …` | 0 | venue location `4 passed`, meeting `3 passed`, notes `1 passed`; restart `PASS`; failure oracle `dropped=1`, `mutated=1`; cleanup `PASS` |
| `CONCERT_TEST_DATABASE_URL=…/mcr_venue_authz CONCERT_TEST_OWNER_ROOT=… cargo +1.88.0 test -p concert-service --test venue_authz_matrix -- --test-threads=1` | 0 | `46 passed; 0 failed; 0 ignored` |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=230 commands=70 missing=0` |
| `python3 -m unittest scripts/mcr/contracts/test_route_parity.py` | 0 | `Ran 5 tests … OK` |
| `cargo +1.88.0 test -p friend-application -p friend-service -p concert-service --test concert_relationship --test relationship_authority` | 0 | `3 passed; 1 ignored` (adapter), `6 passed` (friend-application), `2 passed; 1 ignored` (friend-service) |
| `git diff --check` | 0 | clean |

### New suite

`backend/services/concert-service/tests/venue_friend_authority.rs` starts the
real production `RelationshipServer` handlers for Friend and Auth over real TCP
with the real method-policy guard and real PostgreSQL, builds a real
`GrpcRelationshipAuthority` with the signed actor-delegation chain, obtains a
`RequestBinding` through the guarded `ConcertService::Bootstrap` path, and
drives the production `PgVenueLocation`/`VenueFriendAuthority` composition.

| Test | Result |
| --- | --- |
| `mutual_friends_allow_location_only_after_explicit_consent` | PASS — no location before sharing; mutual friend visible after ACTIVE sharing/fix; stranger denied |
| `one_way_follow_is_not_friendship` | PASS — soft-deleted reverse edge denies snapshot/visible_for/set_sharing |
| `blocked_pair_is_denied` | PASS — live block denies even with both follow edges |
| `suspended_account_is_denied` | PASS — viewer suspension denies snapshot; owner suspension denies set_sharing |
| `archived_event_is_denied` | PASS — `ARCHIVED` phase denies snapshot and set_sharing |
| `friend_owner_outage_fails_closed` | PASS — closed Friend endpoint returns `PolicyUnavailable` for snapshot and set_sharing |
| `removing_the_friend_authority_flips_the_mutual_friend_outcome` | PASS — production `Ok(true)`/visible vs default authority `PolicyUnavailable` |
| `meeting_friend_gate_uses_the_same_bilateral_authority` | PASS — real meeting `friend` facts; one-way follow returns `ResourceUnavailable` |

## Mutation proof

Temporary mutation: `PgVenueLocationAuthority::friend_in` changed from the real
pair read to `Some(_) => Ok(true)`, i.e. the authority removed but the venue
adapter left in place. The new suite then produced:

```
test result: FAILED. 5 passed; 3 failed; 0 ignored
one_way_follow_is_not_friendship ... FAILED
blocked_pair_is_denied ... FAILED
mutual_friends_allow_location_only_after_explicit_consent ... FAILED
```

The third failure is the stranger-selection leg: with the authority removed the
owner could select a non-friend. The mutation was reverted; the final suite is
`8 passed; 0 failed`.

## Private PostgreSQL provenance

- PostgreSQL 16.14 (Homebrew), `listen_addresses=''`, Unix socket only at
  `/Users/lhs/Desktop/mcr-build/exec-w3-friend-pg/socket`, port 55448, owner
  `mcr_owner`, root mode 0700.
- New authority suite: `mcr_concert_venue_friend` (full 60 concert migrations),
  `mcr_friend_venue_friend` (full 19 Friend migrations), `mcr_auth_venue_friend`
  (full 19 Auth migrations). Fixture rows were created per test: an `OPEN`
  `concert_events` row, three `concert_participants`, three `users` +
  `refresh_tokens` families, and two `follows` edges. Mutations inserted
  `user_blocks`, soft-deleted one `follows` edge, set `users.status`, or advanced
  `concert_events.phase`/`revision`.
- Authorization matrix: `mcr_venue_authz` with `0001_concert_core.sql`,
  `0073_venue_location.sql`, `0074_venue_invitations.sql` and
  `0075_place_notes.sql`; its own `venue_authz_*` fixture tables.
- Canonical venue harness: its own disposable `venv`-independent PostgreSQL 16
  cluster, venue ledger + full ledger, physical restart, sentinel
  `ACTIVE|1` preserved, and cleanup `PASS`. Its report is
  `/Users/lhs/Desktop/mcr-build/exec-w3-friend-venue-report-final.json`
  (outside the repository).

## Limits and honest boundaries

- The venue location/meeting HTTP product dispatch is not present on this base;
  the authority is composed at the service constructors
  (`PgVenueLocation::production`, `PgVenueMeeting::production`) and proven by
  driving those production adapters with a real guarded bootstrap binding. This
  is not an HTTP `200` claim.
- The real owner servers in the new suite run in-process over plaintext TCP with
  the real handlers, guard, DB and signed delegation chain. They are not
  separate service binaries and the test does not exercise TLS; the production
  client path supports the configured TLS material.
- Meeting policy and reviewed-landmark providers are separate production inputs
  and remain absent. `PgVenueMeeting::production` therefore still fails closed
  at `policy`/`reviewed_landmark`; the friend gate itself is real and tested.
- `actor_in` now requires both Auth and Friend owner reachability for every
  venue operation, and location `set_sharing` performs one signed pair read per
  selected viewer. Each observation is bounded by the existing two-second owner
  window with no retry; QA08 performance qualification remains
  `DEFERRED_BY_USER`, not passed.
- The authority does not cache friendship. It re-reads both live edges on every
  call. The Friend coarse relationship revision is still bumped by blocks and
  tombstones, not by `follows`; `relationshipRevision` is therefore not a
  per-follow-edge counter, and the authority does not use it to cache an allow.
- The additive proto change is shared. Existing historical source-pin JSONs
  record the previous proto hash and were deliberately not rewritten; the new
  evidence and commit are the current pin.
- The Friend-outage test uses a real client pointed at a closed local endpoint,
  proving the `Unavailable -> PolicyUnavailable` mapping without killing a
  service process. The canonical harness separately proves migration/restart
  persistence.
- Rustfmt invoked on `lib.rs` recursed into unrelated modules during
  validation. Those files were restored byte-for-byte from HEAD before the final
  check; the committed diff contains only the paths listed above and
  `git diff --check` is clean.

## DoneClaim

The production `VENUE_FRIEND_V1` authority now answers from the Friend owner's
signed bilateral observation plus Auth account/session facts and Concert local
event scope. Mutual friends are allowed only with explicit location consent;
one-way follows, blocks, suspensions, deletions, missing participants and
archived events are denied; a Friend owner outage is `503`, never `200`. The
real-PostgreSQL/real-owner suite, the venue suites, the 46-cell authorization
matrix, route parity and the workspace check all pass. The honest limits are
listed above.
