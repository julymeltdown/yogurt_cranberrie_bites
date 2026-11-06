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
- `venue_notes`: added receipt-only create/reply paths that perform the same
  event/place/friend guards and insert, but skip the author-profile projection
  the command receipt does not need.
- `product_rpc/mod.rs` + Gateway: `201` is now an admitted product status.
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

Report: `/Users/lhs/Desktop/mcr-build/exec-w3-venue-live.json` -
`result=PASS`, `cleanup=PASS`, 20/20 venue checks true. The stack uses its own
private PostgreSQL 16 cluster, ephemeral loopback ports and an owned temporary
root; the enrollment policy and reviewed place are explicitly nonproduction
development fixtures. The run is labelled `NOT_ADMITTED`; it claims no IN03
cohort or QA08 qualification.

| Step | HTTP | Durable/observed result |
| --- | --- | --- |
| member `PUT /venue/location/sharing` ACTIVE to operator | 200 | `setVenueLocationSharing` receipt |
| member `PUT /venue/location/fix` | 200 | `publishVenueLocationFix` PHONE_FIX receipt |
| operator `GET /venue/location` | 200 | one friend row for member with latitude 37.5665 |
| member `GET /venue/location` without a reciprocal grant | 200 | `friends=[]`, own position only (missing consent yields no location) |
| member blocks operator through the Friend API, operator reads | 200 | `friends=[]` (no coordinates) |
| unauthenticated `GET /venue/location` | 401 | `AUTHENTICATION_REQUIRED` |
| member `POST .../notes` FRIENDS | 201 | `SUCCEEDED` `createVenuePlaceNote` receipt |
| operator `GET .../notes` | 200 | note visible with exact text |
| stranger `GET .../notes` | 200 | `items=[]` |
| stranger `GET .../notes/{note}` | 404 | `RESOURCE_UNAVAILABLE` |
| operator `POST .../notes/{note}/replies` | 201 | `SUCCEEDED` `createVenuePlaceReply` receipt |
| member `GET .../notes/{note}` | 200 | one reply with exact text |
| stranger reply | 403 | `PERMISSION_DENIED` |
| non-author withdraw | 403 | `PERMISSION_DENIED` |
| author `DELETE .../notes/{note}?expected_revision=2` | 200 | `SUCCEEDED` `withdrawVenuePlaceNote` receipt |
| `GET .../notes` after withdrawal | 200 | withdrawn note absent |
| friend `GET /venue-meeting/context?selectedFriendId=` | 200 | `canRead=true`, `canSend=true` |
| stranger context | 404 | `RESOURCE_UNAVAILABLE` |
| operator `POST /venue-meeting/messages` | 200 | `state=APPLIED`, `kind=MESSAGE` |
| member `GET /venue-meeting/messages` | 200 | exact message body present |

Database rows after withdrawal:
`sharing|fixes|notes|replies|messages = 1|1|2|1|1` (one root note row retained
with scrubbed body, one reply row retained with scrubbed body, one meeting
message).

## Tests

| Command | Exit | Result |
| --- | --- | --- |
| `cargo +1.88.0 test -p concert-service --test venue_product_routes` | 0 | 2 passed |
| `cargo +1.88.0 test -p api-gateway --test venue_authz_http` | 0 | 6 passed, including the new notes/location mount and note-mutation transport cells |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=234 commands=70 missing=0` |
| `python3 backend/scripts/mcr/venue_postgres_smoke.py --target-dir ...` | 0 | venue L/M/N migrations, restart, full ledger and 4+3+1 venue tests pass |
| `cargo +1.88.0 check -p concert-service -p api-gateway --all-targets` | 0 | all targets typecheck |
| Mutation oracle (one classification arm altered) | 101 | route test fails on the altered row; restored and re-run 0 |

Mutation oracle: changing only the `GET /venue/landmarks/{id}/notes` arm from
`VenueOperation::ListNotes` to `VenueOperation::GetNote(Uuid::nil())` makes
`venue_product_routes` fail with `left: "getVenuePlaceNote" right:
"listVenuePlaceNotes"` at the list row (exit 101). The arm was restored and the
test re-run green. The mutated source was touched after restoration so Cargo
could not reuse the mutated artifact.

## Limits

- `getVenueLocationCommand` is classified but the producer returns 503: migration
  `0073_venue_location.sql` has no location receipt table, and no adapter can
  read `?operation=` receipts. No fake success is returned.
- `requestVenueLocation`, `decideVenueLocationRequest` and
  `setVenueLocationCheckIn` remain unmounted and unclassified: the L adapter
  implements no request inbox or check-in persistence.
- `listVenueLandmarks` and `getVenueLandmark` remain unmounted and unclassified:
  no `LandmarkGrant`/list projection is implemented. The reviewed place catalog
  is used internally for note/meeting landmark review only.
- Place-note idempotency is not durable. The adapter still discards the
  `Idempotency-Key`; the producer returns a synthesized `NoteCommand` receipt
  with `state=SUCCEEDED` and the resource id/revision. Same-key replay and
  changed-body 409, and the contract's BE15 receipt/outbox, are not implemented.
- Place-note reads do not provide the contract's prepared native read retention;
  list/thread/replies use ordinary read transactions.
- Meeting invitation create/detail/decision are classified and wired through the
  same friend/policy/landmark authorities, but this run proves context and
  message read/send over HTTP only.
- The live policy and reviewed place are operator-supplied development inputs
  (`CONCERT_VENUE_POLICY_JSON`, seeded map release). With `CONCERT_VENUE_MODE`
  unset/off, every venue operation remains unavailable (503) and no default
  policy, place, friend or landmark is invented.
- The full closure/feed portion of the same driver was not part of the
  `--venue-only` PASS verdict. Earlier full runs on this loaded host hit
  PostgreSQL statement timeouts in feed queries; those are outside this task's
  venue scope and are not claimed as passing here.
