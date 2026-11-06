# Task 29 — IN02 real meetup and pickup gate

- Plan row: 29 (`Pass IN02 real meetup and pickup`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in02`
- Branch: `codex/mcr-exec-w5-in02`
- Base: `2f2f11b61` (`master`); master `d21de138a` merged into this branch as
  `cb3929e17` (`Merge branch 'master' into codex/mcr-exec-w5-in02`), and the
  cargo, route-parity and IN02 gate rows were re-run on that merged tree
- Driver: `backend/scripts/mcr/closure_vertical_stack.py --in02`
- Result: **PASS** on the merged tree: 15/15 checks true, cleanup **PASS**.
  The machine report is `task-29-in02.json` in this directory.
  The beacon/rally producer remains absent and is recorded as `NOT_RUN` in the
  report's machine-readable `not_run` list.

Banner: one owned development stack with a development enrollment policy
(`productionApproval: false`, labelled development, not legal approval). This is
not release qualification, not QA08 capacity, not IN02 acceptance and not a
production claim.

## Master merge (2026-11-06)

Master `d21de138a` merged as `cb3929e17`. Two files conflicted and both were
resolved keep-both, and two merge-time collisions that git did not mark are
fixed because each side was individually correct but mutually exclusive.

- `backend/services/concert-service/src/product_rpc/mod.rs`: the venue product
  module and `VenueOperation` re-export stay beside the Party/Gift modules with a
  single `mod operations_admission`; the Gift route parser and the Venue route
  parser both run (their prefixes are disjoint); the query-accepting `matches!`
  and the feature-decoder `matches!` lists union the Party reads, `Gift`,
  `ServicePolicy`, `Meeting`, `Venue` and `FeedCommand` arms.
- `backend/scripts/mcr/closure_vertical_stack.py`: `--in02` and `--venue-only`
  are both accepted; the concert-service environment keeps the Party/Gift live
  settings and adds the venue policy and Post/Feed settings, followed by the Post
  environment block; the retrying enrollment helper serves all three principals
  with `stranger = third`; `--in02` runs the IN02 provision and gate and returns,
  and every other invocation runs the venue/feed/closure flow.
- Both branches added a concert migration `0077`. Master already owns
  `0077_event_service_policy.sql` and `0078_place_note_receipts.sql`, so this
  branch's child-intent lock migration is renumbered to
  `0079_product_child_intent_shared_lock.sql`. Before the rename the driver
  preflight refused the merged set with `invalid migration set: concert-service`.
- One event has exactly one map binding. Master's venue seed binds
  `closure_dev_map` before the IN02 provisioner runs, so an IN02-owned
  `map_in02_dev` binding was skipped by `ON CONFLICT` and the fixture place was
  unselectable (party create returned `403 PERMISSION_DENIED`). `in02_provision`
  now publishes its reviewed fixture place into the event's existing bound map
  and only creates and binds `map_in02_dev` when the event has no binding.

## Lock-timeout diagnosis and fix

The earlier failure was real, not a timeout that needed to be raised. Captured
PostgreSQL 16 lock-wait logs (`log_lock_waits=on`, `log_statement=all`) showed
four independent self-lock cycles inside one request:

1. **Scope row, command vs child intent.** The Party native-witness insert takes
   `FOR KEY SHARE` on `concert_product_owner_scopes` through its foreign key and
   guard trigger. The same request then registered its public-Profile child; the
   child-intent ledger and trigger asked for `FOR UPDATE` on that row, while the
   command could not commit until the child returned:

   ```
   process 29204 still waiting for ShareLock on transaction 1460 after 101 ms
   Process holding the lock: 29177
   CONTEXT: while locking tuple (1,5) in relation "concert_product_owner_scopes"
   STATEMENT: SELECT binding_digest,phase FROM concert_product_owner_scopes ... FOR UPDATE
   ERROR: canceling statement due to lock timeout
   ```

   Fix: child-intent **admission** uses `FOR SHARE` in both
   `PgScopeLedger::register_child_intent` and migration
   `0079_product_child_intent_shared_lock.sql`. Sealing, completion and every
   other lifecycle transition keep `FOR UPDATE`, so a seal still waits for an
   admitted child and a child cannot register after sealing.

2. **Relationship pair, command insert vs Profile child insert.** The Party
   command (and its post-command projection) inserted `concert_relationship_pairs`
   rows in its own transaction; the Profile child's `INSERT ... ON CONFLICT DO
   NOTHING` waited for that uncommitted tuple:

   ```
   [73999] while inserting index tuple (0,5) in relation "concert_relationship_pairs"
   STATEMENT: insert into public.concert_relationship_pairs(...) on conflict do nothing
   ERROR: canceling statement due to statement timeout
   ```

   Fix: the Party product producer now commits the exact pair rows a request
   needs before starting the command transaction: actor self, the party owner
   (including every owner in a list), the decision applicant, and the removed
   member. It pins `mcr.relationship_authority_until` to the original deadline,
   so the writes satisfy the relationship deadline guard. The child then takes
   `FOR SHARE` on already committed rows.

3. **Relationship authorization `FOR UPDATE` vs child `FOR SHARE`.** Ordinary
   authorization took `FOR UPDATE` on the same pair the retained Profile child
   holds `FOR SHARE`. Fix: normal `authorize_in`/`authorize_actor_in` now take
   `FOR SHARE`; only the safety/block mutation path (`safety_authority`, which
   actually updates the pair) keeps `FOR UPDATE`. A block still waits for the
   read and vice versa.

4. **Wrong parent digest comparison.** `PartyLiveService::dispatch_for_scope`
   compared SHA-256 of the unframed protobuf message with
   `parent_body_digest`, which covers the gRPC frame. Every request became
   `400 INVALID_INPUT`. The root `ProductService::authorize` already proves the
   exact parent message and `PartyLiveRequest::from_owner_scope` checks every
   HTTP/scope field, so the duplicate framed/unframed comparison was removed.

The gateway's default 1,000 ms request timeout also cut the product's own 2,000 ms
budget during two concurrent approvals. The IN02 driver sets
`GATEWAY_REQUEST_TIMEOUT_MS=5000` for its development stack so the transport does
not pre-empt the product budget. No product or owner timeout was raised.

## Composition

| Family | Producer composed | Admission/classification | Native settlement |
| --- | --- | --- | --- |
| Party | `PartyRuntimeSettings` builds `ScopedPartyProductProducer` with `PgPartyResources`, `FingerprintKeys`, `PartyCursorCodec` and the shared `PublicProfileReader` | Product router classifies all 16 contract Party paths; the eight implemented operations dispatch, the other eight return `503 Party operation producer unavailable` instead of `Unsupported product operation` | `PartyNativeResolver` in `NativeResolverUnion` |
| Gift | `GiftRuntimeSettings::compose` wraps `GiftRuntime` in `ScopedProductOwner` over the same fence/delivery publisher and private pickup keys | `GiftOperation::route` classifies and the composed `GiftRuntime` serves all 13 frozen Gift paths; the gateway mounts all 13 GET and mutation routes; Product validation defers the envelope to `GiftRequest::from_verified` | `GiftNativeResolver` in `NativeResolverUnion` |
| Admission | `ProductSettings` still rejects Feed/media opt-ins; Party/Gift require guarded gRPC plus the shared canonical owner composition | `CONCERT_PARTY_MODE=live`, `CONCERT_GIFT_MODE=live` with independent cursor/receipt keys | scope ledger + delivery publisher shared |

Capacity, stock, one-time pickup code, owner/recipient eligibility and
application-pair privacy remain enforced on the server by the existing
`PgCommunity`/`PgGifts` producers. `PgPartyResources` checks event phase, the
`parties` feature switch and the reviewed physical place; `PgGiftResources`
already checks phase, `giveaways`, media and pickup meeting.

**Gift surface correction (2026-11-06).** Gift does not return the Party
producer-unavailable `503` on any path. All thirteen frozen Gift operations
(`listConcertGiveaways`, `getConcertGiveaway`, `createConcertGiveaway`,
`updateConcertGiveaway`, `getConcertGiveawayManagement`,
`setConcertPickupMeeting`, `reserveConcertGiveaway`, `getConcertGiftActivity`,
`getConcertReservation`, `cancelConcertReservation`,
`claimConcertReservation`, `getConcertPickup`, `issueConcertPickupCode`) are
classified by `GiftOperation::route`, accepted by `GiftRequest::from_verified`,
and dispatched by the composed `GiftRuntime`; verification of the merged
candidate observed `200`/`400`/`404`/`409` across them and no `503`. Any earlier
statement that five Gift paths return `503` is withdrawn: the eight `503`
producer-unavailable paths are Party operations, not Gift. The machine-readable
form is `gift_surface` in `task-29-in02.json`.

## Stack and principals

- Seven native services rebuilt from the merged tree with
  `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in02` and `-j 3`
  (exit 0).
- Owned private PG16 cluster (`/private/tmp/mcr-pg-wxvkk38h` for the
  authoritative merged-tree run), owned Redis, mail sink, real mTLS/PKI and a
  private temp root. No foreign database or shared writable checkout was used.
- Host, alice and bob were created through the real development enrollment
  attempt → mail verification → login → admission flow; each profile was
  initialized through the public profile route.
- Provisioning: event `OPEN`, 3 `ACTIVE` admissions, 3 participants, 3 avatar
  settings, `parties=true, giveaways=true`, and one reviewed development
  physical place (`11111111-1111-4111-8111-111111111111`) published into the
  event's single bound map (`closure_dev_map`, revision 1 on this run) with an
  `OPEN` place state.

## Steps and exact statuses

| Step | Request | Status / body |
| --- | --- | --- |
| 1 | Host `POST /parties` capacity 2 | `200`, party `d75ff4fc-6b58-4a67-b894-87f71d643fc9`, `member_count=1` |
| 2 | Alice apply | `200`, application `a00129fd-9806-40ef-83a4-ae2f4a7b552d`, `PENDING` |
| 3 | Bob apply | `200`, application `bbcb7296-c4d7-46f4-854c-e6516fb083b5`, `PENDING` |
| 4 | Concurrent approve Alice/Bob | Bob `200`; Alice `409 CONCERT_409 VERSION_CONFLICT` |
| 5 | Retry loser approval with current party revision | `409 CONCERT_409 PARTY_FULL` |
| 6 | Host `GET /parties/{id}` | `200`, `member_count=2` |
| 7 | Host `POST /giveaways` stock 1 | `200`, giveaway `5b20aec2-6c91-4cae-b316-7e36ecc7e149`, `available=1` |
| 8 | `PUT /giveaways/{id}/pickup-meeting` | `200`, revision 2, map 1 / place revision 1 |
| 9 | `PATCH /giveaways/{id}` state OPEN | `200`, revision 3 |
| 10 | Concurrent reserve Alice/Bob | Alice `200 READY`; Bob `409 GATEWAY_409 VERSION_CONFLICT` |
| 11 | Third reservation attempt at current revision | `409 GATEWAY_409 GIVEAWAY_SOLD_OUT` |
| 12 | `POST /reservations/{id}/pickup-code` | `200`, code `063349`, reservation revision 2 |
| 13 | Wrong code claim | `403 GATEWAY_403 PICKUP_CODE_INVALID` |
| 14 | Valid claim | `200 CLAIMED`, receipt `759f024f-57e4-4996-afe2-dd44dabf1cdf` |
| 15 | Same key replay | `200`, same receipt and same `claimed_at` (`2026-11-06T14:48:23.121319Z`) |
| 16 | Same code, new key replay | `403 GATEWAY_403 PICKUP_CODE_INVALID` |

### Durable rows

| Query | Observed (merged-tree run) |
| --- | --- |
| `concert_community_parties` | `OPEN\|2\|2` (state, revision, capacity) |
| `concert_community_members` | owner `ACTIVE`, one approved member `ACTIVE` |
| `concert_community_applications` | one `PENDING`, one `APPROVED` |
| `concert_community_giveaways` after reservation | `OPEN\|4\|1\|1\|0` (state, revision, total, reserved, claimed) |
| `concert_community_reservations` after reservation | one `READY\|1` |
| pickup secret at issue | generation `1`, 32-byte keyed hash, expiry issued |
| giveaway after claim | `OPEN\|5\|1\|0\|1` |
| reservation after claim | `CLAIMED\|3` |
| pickup secret after claim | empty (single-use secret consumed) |
| `concert_command_receipts` | create/apply/decide/giveaway/meeting/open/reserve/issue/claim receipts; failed `VERSION_CONFLICT`, `PARTY_FULL`, `GIVEAWAY_SOLD_OUT` and `PICKUP_CODE_INVALID` rows retained |
| `concert_outbox` | `10` rows |

## Tests and commands (pre-merge IN02 tree at `6e0d9e252`)

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo +1.88.0 build -p concert-service -p api-gateway -j 3` (target `exec-w5-in02`) | 0 | both binaries rebuilt |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | registry 69, contract commands 63, templates 241, commands 70, missing 0 |
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_party_live_smoke.py -p test_gift_public_controls.py` | 0 | 10 tests passed |
| `cargo +1.88.0 test -p concert-service --test product_runtime` | 1 | 7 passed; the pre-existing `only_gateway_may_dispatch_with_a_signed_field_one_actor` assertion expects 2 RPCs while the base proto already has `Dispatch`, `GetArchive`, `CompleteResponse` |
| `cargo +1.88.0 test -p api-gateway --test concert_product_http` | 0 | 7 passed; every party/gift path authenticates and reaches the Concert boundary |
| `cargo +1.88.0 test -p concert-service --test gift_live_protocol --test party_live_wire --test party_live_owner --test party_live_cursor` | 0 | 8 + 8 + 4 + 3 passed |
| `python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir .../debug --report task-29-in02.json --in02` | 0 | all 15 checks true, cleanup PASS |

The driver reached the real PostgreSQL stack for every step. `route-parity.py`
covers every registered command; the gateway boundary test also covers the
newly mounted Party/Gift reads. The IN02 driver is the real multiuser proof for
the capacity, stock and pickup invariants.

## Merged-tree verification (2026-11-06, `cb3929e17`)

Run with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in02` and
`-j 3`:

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo check -p concert-service -p api-gateway --all-targets --locked` | 0 | finished in 46.66s; warnings only (pre-existing dead code and unused imports in tests) |
| `cargo build --locked -p profile-service -p notification-service -p friend-service -p post-service -p concert-service -p auth-service -p api-gateway` | 0 | all seven service binaries rebuilt in 41.19s |
| `python3 -B scripts/mcr/contracts/route-parity.py` | 0 | registry=69 contract_commands=63 templates=245 commands=70 missing=0 |
| `python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir .../debug --report task-29-in02.json --in02` | 0 | 15/15 checks true, cleanup PASS |

## Limits

- **Beacon/rally is `NOT_RUN`.** Recorded in `task-29-in02.json` under
  `not_run` as ids `beacon` and `rally`. The gateway mounts the C04 beacon
  routes, but the Concert product router has no beacon arm, so a beacon write is
  refused as an unsupported product operation, and no beacon producer is
  composed. Party `GetPartyRally`/`SetPartyRally` are classified but the Party
  producer refuses them with `503 Party operation producer unavailable`.
- Party update, transfer, application-list/detail, member-list, rally and
  activity paths are **classified** but have no producer in this scope and
  return `503 Party operation producer unavailable`. They are not claimed as
  working. This is the only family with a producer-unavailable `503`; every Gift
  path is served.
- The `only_gateway_may_dispatch_with_a_signed_field_one_actor` assertion in
  `product_runtime` is pre-existing and unchanged by this work: base
  `product.proto` already declares `GetArchive`, while the test expects only
  `Dispatch` and `CompleteResponse`. The other seven product-runtime tests pass.
- The development place and event policy are synthetic development inputs, not
  reviewed venue or legal approval. No browser, device, visual or capacity claim
  is made. This is not IN02 acceptance or release qualification.
