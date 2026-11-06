# Task 29 — IN02 real meetup and pickup gate

- Plan row: 29 (`Pass IN02 real meetup and pickup`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w5-in02`
- Branch: `codex/mcr-exec-w5-in02`
- Base: `2f2f11b61` (`master`)
- Gate driver: `backend/scripts/mcr/closure_vertical_stack.py --in02`
- Result: **NOT_RUN / FAIL**. `POST /parties` returns `503 DEPENDENCY_UNAVAILABLE`
  before any party row is written. No capacity, stock or pickup assertion was
  reached, and this document does not claim PASS.
- Machine report: `task-29-in02.json` (this directory)
- Cleanup: **PASS**

Banner: this is one owned development stack with a development enrollment
policy (`productionApproval: false`, labelled development, not legal approval).
It is not release qualification, not QA08 capacity, and not a production claim.

## Why the gate cannot complete at this base

The base mounts the C04 party and gift HTTP routes and the Concert product
transport classifies `createConcertParty`, but the serving `ProductOwners` chain
has no Party or Gift producer. The request therefore reaches the product
boundary and is answered with:

```
HTTP/1.1 503
{"code":"CONCERT_503","details":{"reason":"DEPENDENCY_UNAVAILABLE"}}
```

No `concert_community_parties`, `concert_community_giveaways` or
`concert_command_receipts` row is written. Every later journey step is
`NOT_RUN` on this exact blocker. The base also has no composed physical beacon
or rally producer; those steps would be separate `NOT_RUN` operations.

## Stack and principals

- Seven native services built from this checkout:
  `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in02 cargo +1.88.0
  build -p concert-service -p auth-service -p friend-service -p profile-service
  -p post-service -p notification-service -p api-gateway -j 3` (exit 0).
- Owned private PG16 cluster from `dev_stack`, owned Redis, owned mail sink,
  real mTLS/PKI and a private temp root. No foreign database or shared writable
  checkout was used.
- Principals: host, alice and bob were created through the real development
  enrollment attempt → mail verify → login → admission flow, and each profile
  was initialized through the public profile HTTP route.
- Event provisioning for this gate: `concert_event_admissions` 3 (`ACTIVE`),
  `concert_participants` 3, `concert_avatar_settings` 3, event settings
  `parties=true, giveaways=true`, one reviewed development physical place
  (`map_in02_dev`, revision 1) and one active event map binding.

### Provisioned durable rows

| Query | Observed |
| --- | --- |
| `concert_events.phase` | `OPEN` |
| `concert_participants` for event | `3` |
| `concert_event_admissions` `ACTIVE` | `3` |
| `concert_event_settings.parties,giveaways` | `t\|t` |
| `concert_map_release_places` for `map_in02_dev` revision 1 | `1` |
| `concert_community_parties` for event | `0` |
| `concert_community_giveaways` for event | `0` |
| `concert_command_receipts` for event | empty |

## Steps

Statuses are from the authoritative base run in `task-29-in02.json`.

| Step | Request / effect | Status and exact evidence |
| --- | --- | --- |
| 1. Host creates meetup with capacity 2 and one stock unit | `POST /api/concert/events/{event}/parties` with capacity 2, physical place and keyed idempotency | **503 `CONCERT_503` / `DEPENDENCY_UNAVAILABLE`**; no party, member, application or receipt row |
| 2. Alice and bob apply | `POST /parties/{id}/applications` | **NOT_RUN** — blocker: step 1 wrote no party id |
| 3. Two concurrent approvals / capacity invariant | `POST /party-applications/{id}/decision` ×2 | **NOT_RUN** — same blocker |
| 4. Giveaway create with stock 1 | `POST /giveaways` | **NOT_RUN** — no Party/Gift producer composed |
| 5. Two concurrent reservations / stock invariant | `POST /giveaways/{id}/reservations` ×2 | **NOT_RUN** — same blocker |
| 6. Issue and redeem pickup code exactly once | `POST /reservations/{id}/pickup-code`, `POST .../claim` | **NOT_RUN** — same blocker |
| 7. Replay same code | `POST .../claim` with a new key | **NOT_RUN** — no issued code exists |
| 8. Physical beacon / rally | C04 beacon and rally routes | **NOT_RUN** — no composed beacon/rally producer at this base |

`HTTP statuses`: only step 1 was reachable; it returned `503`. All later steps
are `NOT_RUN`, never `PASS`.

## Commands and exit codes

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo +1.88.0 build -p concert-service -p api-gateway -j 3` with `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w5-in02`, jobs 3 | 0 | both binaries rebuilt from this checkout |
| `python3 -m py_compile backend/scripts/mcr/closure_vertical_stack.py` | 0 | driver syntax valid |
| `python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir .../debug --report task-29-in02.json --in02` | 1 | gate reached step 1 only; blocker recorded; cleanup PASS |
| `python3 scripts/mcr/check-plan.py` | NOT_RUN | not run in this round |

## Excluded experiment (not committed)

A local composition experiment (Party startup + Gift admission + Party native
resolver + gift route admission) compiled and moved the failure past route
classification, but the first Party create then failed inside the canonical
owner path:

```
PARTY_PROFILE_ERR DependencyUnavailable
PROFILE_LOCAL_STATUS The service is currently unavailable Product scope lifecycle unavailable
SCOPE_LEDGER_SQL_ERR error returned from database: canceling statement due to lock timeout
```

The Party command's prepared transaction and the owner PublicProfile child
intent contend on the same owner-scope/relationship locks; both pre-command and
post-command profile reads deadlock. This experiment was reverted and is **not
part of this commit**, because it changes security-sensitive composition without
a passing gate. The committed tree remains the base product plus this driver and
evidence.

## Limits

- No capacity, stock, pickup-code, one-time replay, beacon or rally invariant was
  executed. Calling any of them PASS would be false.
- The development physical place and event settings are synthetic development
  inputs, not reviewed venue or legal approval.
- No browser, device, visual or 200-user capacity claim is made.
- This is not IN02 acceptance. The blocking consumer for steps 2–8 is the
  missing Party/Gift/beacon product composition in the serving Concert owner
  chain, with the first observable status `503 DEPENDENCY_UNAVAILABLE`.
