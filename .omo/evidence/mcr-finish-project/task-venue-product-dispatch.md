# Venue product dispatch over HTTP

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w3-dispatch`
Branch: `codex/mcr-exec-w3-dispatch`
Base: `a68edaadf` fast-forwarded to current `origin/master` `7562f3b96` (the
bilateral friend authority `c4f220ce9` is not an ancestor of the stated base).
Build target: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w3-dispatch`, `--jobs 3`.
Date: 2026-11-06

## Cause traced

`concert_product` mounted venue location, notes and meeting templates, and the
L/M/N application adapters existed, but:

1. `product_rpc::operation` had no venue location/notes parser, so those targets
   fell through to `Unsupported product operation` (403 at the product edge).
2. `Operation::Meeting` was classified, but the generic query gate rejected
   every meeting selection query before `venue_meeting_admission::request`.
3. No producer was composed for either family, so even classified meeting
   targets stopped at `Product producer unavailable`.
4. The C04 status whitelists in `ProductRpcService` and the Gateway omitted
   `201`, so a committed place-note create/reply was converted to 503 after its
   effect.

## Delivery

- `product_rpc/venue_admission.rs`: closed L/N route decoding, query/body/key
  validation and typed `VenueRequest` reconstruction from the exact signed
  target/body.
- `product_rpc/venue_product.rs`: `ScopedProductProducer` for location snapshot,
  sharing and fix; notes list/thread/replies/create/reply/withdraw; meeting
  context/inbox/invitation/detail/decision/messages/send/status. Location and
  note authority reuse `VENUE_FRIEND_V1` (`VenueFriendAuthority`) under the held
  `OwnerGuardScope`; note authors use the real `PublicProfileReader`.
- `config/venue_runtime.rs`: explicit `CONCERT_VENUE_MODE=live` composition over
  the shared canonical owners. `CONCERT_VENUE_POLICY_JSON` supplies the
  source-identified location/meeting policy; no default exists and `off` keeps
  every venue operation unavailable. The reviewed-landmark provider is the
  existing `concert_events_map_releases` / `concert_map_release_places` catalog
  restricted to `review_state='REVIEWED'`, `verification_state='VERIFIED'`,
  `place_kind='PHYSICAL'` and current `closure_state='OPEN'`.
- `config/canonical_runtime.rs`: the shared composition retains one
  `GrpcRelationshipAuthority` for request-bound venue friend reads.
- `venue_notes`: durable BE15-style keyed receipts for note create/reply/withdraw
  (`0078_place_note_receipts.sql`); same key + same canonical payload replays the
  stored resource, same key + changed payload returns 409 `VERSION_CONFLICT`, and
  no receipt/body snapshot is stored. Receipt-only create/reply paths perform the
  same event/place/friend guards without projecting the author profile.
- Place-note reads now fail closed with 503 `DEPENDENCY_UNAVAILABLE`: the N01
  prepared native read-retention path does not exist (see Limits).
- `product_rpc/mod.rs` + Gateway: `201` is an admitted product status so a
  committed place-note create/reply is not converted to 503 after its effect.
- Gateway `feature_routes.rs`: mounted `GET` list/thread/replies for place
  notes and closed their `limit`/`before`, `expected_revision` and location
  command query validation.

## Classification table

| Contract operation | Method and path | `Operation` |
| --- | --- | --- |
| getVenueLocationSnapshot | GET `/venue/location` | `Venue(LocationSnapshot)` |
| setVenueLocationSharing | PUT `/venue/location/sharing` | `Venue(SetLocationSharing)` |
| publishVenueLocationFix | PUT `/venue/location/fix` | `Venue(PublishLocationFix)` |
| getVenueLocationCommand | GET `/venue/location/commands/{key}?operation=` | `Venue(LocationCommand(key))` |
| listVenuePlaceNotes | GET `/venue/landmarks/{id}/notes` | `Venue(ListNotes)` |
| createVenuePlaceNote | POST `/venue/landmarks/{id}/notes` | `Venue(CreateNote)` |
| getVenuePlaceNote | GET `/venue/landmarks/{id}/notes/{note}` | `Venue(GetNote(note))` |
| listVenuePlaceReplies | GET `/venue/landmarks/{id}/notes/{note}/replies` | `Venue(ListReplies(note))` |
| createVenuePlaceReply | POST `/venue/landmarks/{id}/notes/{note}/replies` | `Venue(CreateReply(note))` |
| withdrawVenuePlaceNote | DELETE `/venue/landmarks/{id}/notes/{note}` | `Venue(WithdrawNote(note))` |
| resolveVenueMeetingContext | GET `/venue-meeting/context` | `Meeting(Context)` |
| listVenueMeetingInvitations | GET `/venue-meeting/invitations` | `Meeting(Invitations)` |
| createVenueMeetingInvitation | POST `/venue-meeting/invitations` | `Meeting(CreateInvitation)` |
| getVenueMeetingInvitation | GET `/venue-meeting/invitations/{id}` | `Meeting(GetInvitation(id))` |
| decideVenueMeetingInvitation | POST `/venue-meeting/invitations/{id}/decision` | `Meeting(DecideInvitation(id))` |
| listVenueMeetingMessages | GET `/venue-meeting/messages` | `Meeting(Messages)` |
| sendVenueMeetingMessage | POST `/venue-meeting/messages` | `Meeting(SendMessage)` |
| getVenueMeetingCommand | GET `/venue-meeting/commands/{key}` | `Meeting(Status(key))` |

`tests/venue_product_routes.rs` asserts every mounted row above, plus closed
rejection for unknown, unconverted and malformed targets. The mutation oracle
below proves the test fails when one row is altered.

## Live composed stack

Command:

```
python3 backend/scripts/mcr/closure_vertical_stack.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w3-dispatch/debug \
  --report /Users/lhs/Desktop/mcr-build/exec-w3-venue-live.json --venue-only
```

Report: `/Users/lhs/Desktop/mcr-build/exec-w3-venue-live-receipts.json` -
`result=PASS`, `cleanup=PASS`, 28/28 venue checks true. The stack uses its own
private PostgreSQL 16 cluster, ephemeral loopback ports and an owned temporary
root; the enrollment policy and reviewed place are explicitly nonproduction
development fixtures. The run is labelled `NOT_ADMITTED`; it claims no IN03
cohort or QA08 qualification.

| Step | HTTP | Durable/observed result |
| --- | --- | --- |
| member `PUT /venue/location/sharing` ACTIVE to operator | 200 | `setVenueLocationSharing` receipt |
| member `PUT /venue/location/fix` | 200 | `publishVenueLocationFix` PHONE_FIX receipt |
| operator `GET /venue/location` | 200 | one friend row for member with latitude 37.5665 |
| member `GET /venue/location` without a reciprocal grant | 200 | `friends=[]`, own position only |
| blocked friend `GET /venue/location` | 200 | `friends=[]` (no coordinates) |
| unauthenticated `GET /venue/location` | 401 | `AUTHENTICATION_REQUIRED` |
| member `POST .../notes` FRIENDS | 201 | `SUCCEEDED` receipt, resource id/revision |
| same key/body `POST .../notes` | 201 | same resource id/revision, no new row |
| same key/changed body `POST .../notes` | 409 | `VERSION_CONFLICT` |
| friend `GET .../notes`, thread, replies | 503 | `DEPENDENCY_UNAVAILABLE` (missing prepared read retention) |
| operator reply / same-key replay | 201 / 201 | same reply id; changed body 409 |
| stranger reply | 403 | `PERMISSION_DENIED` |
| non-author withdraw | 403 | `PERMISSION_DENIED` |
| author withdraw / same-key replay | 200 / 200 | same revision; changed expected revision 409 |
| friend `GET /venue-meeting/context` | 200 | `canRead=true`, `canSend=true` |
| stranger context / create invitation / detail / decision / send / status | 404 | `RESOURCE_UNAVAILABLE` |
| friend list invitations | 200 | page (empty or bounded) |
| friend create invitation | 200 | `APPLIED` invitation with id |
| friend invitation detail | 200 | exact invitation id |
| recipient decide invitation | 200 | `APPLIED` |
| friend send message / list messages | 200 / 200 | `APPLIED` / exact body |
| friend command status | 200 | `UNKNOWN` for an unused key |

Database rows after the run:
`sharing|fixes|notes|replies|messages|receipts = 1|1|2|1|1|3` (one root note row
and one reply row remain, bodies scrubbed by withdrawal; three receipts remain:
create, reply, withdraw).

## Tests

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 test -p concert-service --test venue_product_routes` | 0 | 2 passed; all 18 mounted venue rows asserted |
| `cargo +1.88.0 test -p api-gateway --test venue_authz_http` | 0 | 7 passed, including the six missing meeting rows and their authenticated 503 cells |
| `python3 backend/scripts/mcr/venue_postgres_smoke.py --target-dir ...` | 0 | venue L/M/N migrations, restart, full ledger and 4+3+2 venue tests pass (receipt replay/conflict included) |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=234 commands=70 missing=0` |
| `cargo +1.88.0 check -p concert-service -p api-gateway --all-targets` | 0 | all targets typecheck |
| Mutation oracle (one classification arm altered) | 101 | route test fails on the altered row; restored and re-run 0 |

### Durable receipt proof

The real-PostgreSQL test
`note_receipts_replay_and_conflict_without_new_rows` creates a root, replies,
and withdraws with fixed keys. Same key + same payload returns the original
id/revision (root, reply, withdrawal); same key + changed text (root/reply) or
changed `expected_revision` (withdraw) returns `PlaceNoteError::Conflict`. After
the three commands the database still holds exactly one root, one reply and
three receipt rows. Over HTTP the same-key replay returns 201/201/200 with the
identical resource id/revision and the conflicts return 409
`VERSION_CONFLICT`.

Note reads (list, thread, replies) return 503 `DEPENDENCY_UNAVAILABLE` before
any content because the prepared read-retention path is absent (see Limits).

Mutation oracle: changing only the `GET /venue/landmarks/{id}/notes` arm from
`VenueOperation::ListNotes` to `VenueOperation::GetNote(Uuid::nil())` makes
`venue_product_routes` fail with `left: "getVenuePlaceNote" right:
"listVenuePlaceNotes"` at the list row (exit 101). The arm was restored and the
test re-run green. The mutated source was touched after restoration so Cargo
could not reuse the mutated artifact.

## Limits

- `getVenueLocationCommand` is classified but the producer returns 503: migration
  `0073_venue_location.sql` has no location receipt table, and the note receipt
  table is keyed to the three note operations. VENU-BE-L01 must supply the
  location command receipt/read adapter; this row records it as mandatory.
- `requestVenueLocation`, `decideVenueLocationRequest`,
  `setVenueLocationCheckIn`, `listVenueLandmarks` and `getVenueLandmark` remain
  unmounted and unclassified: no request/check-in persistence and no
  `LandmarkGrant` list/detail projection exist. VENU-BE-L01 owns these five.
- Place-note reads deliberately return 503. The frozen N01 contract requires
  prepared native read retention through an `Arc<OwnerGuardScope>`, event,
  landmark and database time, with durable child intent/ACK/rollback and a
  disclosure ceiling through HTTP EOF. That infrastructure does not exist for
  notes: `ChildRecoveryReference` has no Notes kind; `CompositeChildIntentSettler`,
  `NativeResolverUnion`, `VenueFriendAuthority` and `PgPlaceNotes` have no note-read
  child/settler/resolver; and the current list/thread/replies use ordinary read
  transactions that release locks at commit. Returning 200 would silently
  violate the contract, so the producer refuses with 503 before any content.
- The note receipt is a durable keyed actor/event/operation/digest/result row,
  but it is not the BE15 outbox/audit transport; no outbox publication or
  background receipt retirement exists. Receipt `expires_at` is the note/parent
  expiry, never an extension.
- Meeting invitation create/detail/decision and message send/status are
  classified and exercised over HTTP; the stack's reviewed place/policy are
  explicitly nonproduction development fixtures.
- With `CONCERT_VENUE_MODE` unset/off, every venue operation remains unavailable
  (503); no default policy, place, friend or landmark is invented.
