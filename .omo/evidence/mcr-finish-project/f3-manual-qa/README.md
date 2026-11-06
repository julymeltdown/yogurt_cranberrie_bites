# F3 - Real manual QA on the final candidate

- Candidate: `71e54aa735e6fbd7c51f0c7b6115fa71a9704048` (branch `codex/mcr-exec-f3`,
  base `71e54aa73`; tracked tree clean at build time).
- Machine report: [`report.json`](report.json) (assembled by `assemble_report.py`
  from the raw driver reports in `raw/`; every status, row and log excerpt below is
  copied from those reports).
- Scope: IS-1 meetup/pickup, IS-2 mural, IS-3 admin closure/archive, IS-4 venue
  invitation/notes, and the role x surface privacy spot-check.
- Banner: **development release bundle, NOT_ADMITTED**. QA08 capacity, IN03 cohort
  admission and release qualification are **NOT_CLAIMED**.

## Verdict summary

| Scenario | Verdict | Measured checks | Key statuses |
| --- | --- | --- | --- |
| IS-1 two-client meetup and pickup | **PASS** | 15 / 15 | capacity 200 then 409 VERSION_CONFLICT then 409 PARTY_FULL; stock 200 then 409 VERSION_CONFLICT then 409 GIVEAWAY_SOLD_OUT; claim 200, same-key replay 200 with the same receipt, new-key same-code replay 403 PICKUP_CODE_INVALID |
| IS-2 mural convergence and moderation | **PASS** | 39 / 39; 2 NOT_RUN sub-items | cooldown 429 MURAL_COOLDOWN; fill >256 409 MURAL_FILL_TOO_LARGE; budget 429 MURAL_BUDGET_EXCEEDED; undo guard 409 MURAL_UNDO_NOT_LATEST; eligible undo 200; pause 200 + write 409 MURAL_PAUSED; freeze 200 + write 409, frozen terminal |
| IS-3 admin closure and archive | **PASS at the reached READ_ONLY phase** | 51 true, 2 null (archive 200/fields) | operator ACK 200; member 403 PERMISSION_DENIED; repeat closure 409 INVALID_TRANSITION; post-closure feed write 409 EVENT_READ_ONLY; authenticated and anonymous archive 404 RESOURCE_UNAVAILABLE |
| IS-4 venue invitation and notes | **PASS** | 28 / 28 venue checks | invitation 200 -> accepted 200; message send 200 + read 200 with the body; note create 201, reply 201, withdraw 200; stranger replies/invitations/messages 404, foreign replies/withdraw 403 |
| Privacy spot-check | **FAIL** | IN03 27 / 27 PASS; QA04 aborted | QA04 measured 96 cells before the abort (74 PASS, 22 NOT_RUN, 0 FAIL), then aborted twice at its own state-restore unblock with 504 |

## Environment (per scenario, fresh)

- Seven native binaries built from this checkout with
  `cargo build -p concert-service -p auth-service -p friend-service -p profile-service
  -p post-service -p notification-service -p api-gateway -j 6` (exit 0) into
  `/Users/lhs/Desktop/mcr-build/exec-f3`, copied to `exec-f3-bins`.
  Binary SHA-256 values are pinned in `report.json` under `binaries`.
- Owned disposable PostgreSQL 16.14 cluster, owned Redis, owned local mail sink and
  real per-run mTLS PKI; loopback-only ports; a private `/private/tmp/mcr-pg-*` root
  per run, removed by the driver (`cleanup: PASS` in every run).
- Enrollment policy is explicitly nonproduction:
  `productionApproval=false`, `approvalReference=development-not-admitted`.
  Revisions: IS-1 `1147bdca-edae-4c01-a0f4-6a0e9ac6f82e`,
  IS-3/IS-4 `042e8bef-2d3f-4f60-9c49-d42f67124842`.
- Gateway budget: the IS-1/IS-3/IS-4 drivers set
  `GATEWAY_REQUEST_TIMEOUT_MS=5000` so the transport does not pre-empt the product's
  own 2000 ms command budget. IS-2 and the final IN03 run use the driver default.
  The QA04 rerun applied the same 5000 ms budget through
  `run_privacy_with_gateway_budget.py`; the outcome did not change.

## IS-1 - meetup and pickup

Driver: `backend/scripts/mcr/closure_vertical_stack.py --in02` (exit 0,
`result: PASS`, `cleanup: PASS`). Event `f1fd7b85-d393-5410-8bc3-5ea6b71aeb77`.

Preconditions measured in the report: event `OPEN`; 3 ACTIVE admissions and 3
participants for host/alice/bob; `parties=true, giveaways=true`; one reviewed
development physical place published to the event's bound map. Principals were
created through the real development enrollment -> mail verification -> login ->
admission flow and initialized through the public profile route.

| Step | Request | Measured |
| --- | --- | --- |
| party create | host `POST /parties`, capacity 2 | 200 |
| two applications | alice and bob `POST /parties/{id}/applications` | 200 / 200, PENDING |
| concurrent approvals | two approvals in parallel | one 200, one 409 CONCERT_409 VERSION_CONFLICT |
| retry at current revision | loser approval | 409 CONCERT_409 PARTY_FULL |
| party read | host `GET /parties/{id}` | 200, `member_count=2` |
| giveaway | host `POST /giveaways`, stock 1 | 200, available 1 |
| pickup meeting | `PUT /giveaways/{id}/pickup-meeting` | 200, revision 2 |
| open | `PATCH /giveaways/{id}` OPEN | 200, revision 3 |
| concurrent reservations | alice and bob in parallel | alice 200 READY, bob 409 GATEWAY_409 VERSION_CONFLICT |
| third reservation | current revision | 409 GATEWAY_409 GIVEAWAY_SOLD_OUT |
| pickup code | alice `POST /reservations/{id}/pickup-code` | 200, reservation revision 2 |
| wrong code | claim with a wrong code | 403 GATEWAY_403 PICKUP_CODE_INVALID |
| claim | claim with the issued code | 200 CLAIMED |
| same-key replay | same idempotency key | 200, same receipt and same `claimed_at` |
| new-key same-code replay | fresh key, consumed code | 403 GATEWAY_403 PICKUP_CODE_INVALID |

Durable rows (exact driver strings):

```
party:        OPEN|2|2
members:      e917e52b-...|t|ACTIVE  2c868e28-...|f|ACTIVE
applications: 2faf8a8a-...|PENDING|1  b87f07d4-...|APPROVED|2
giveaway:     OPEN|4|1|1|0  ->  OPEN|5|1|0|1
reservation:  CLAIMED|3|<claimed_at>
pickup secret: "" (single-use secret consumed)
receipts:     decide FAILED PARTY_FULL, reserve FAILED GIVEAWAY_SOLD_OUT,
              claim FAILED PICKUP_CODE_INVALID, claim SUCCEEDED, ...
```

Log excerpt (concert-service, `report.json -> scenarios.IS-1.steps`): every step
carries its own excerpt or the explicit `{"note": "no new child log bytes"}`.
Scenario-level excerpt: `CONCERT_HEALTH_LISTENER=127.0.0.1:65090 | CONCERT_WORLD_LISTENER=127.0.0.1:65091 | child_recovery attempted=3 completed=3 pending=0 timed_out=0`.

## IS-2 - mural convergence and moderation

Driver: `backend/scripts/mcr/in04_wall_b_gate.py` (exit 0, `result: PASS`,
`cleanup: PASS`, 3 principals on attempt 1). Wall pack manifest
`3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce`.

| Step | Measured |
| --- | --- |
| initial convergence | alice and bob 200, revision 1, identical snapshot id and pixel SHA-256 |
| pencil | alice 200 revision 2; bob sync delta 1->2, `reset_required=false`, 1 operation |
| opaque erase | bob 200 revision 3, `changed_cells=1`; alice requires reset then revision 3 |
| four-connected fill | bob 200 revision 4, 244 changed cells, exact component |
| cooldown | second write inside 1200 ms 429 MURAL_COOLDOWN, revision unchanged |
| fill ceiling | 11616-cell fill 409 MURAL_FILL_TOO_LARGE, revision and pixel hash unchanged |
| undo guard | foreign write revision 7, undo of revision 6 -> 409 MURAL_UNDO_NOT_LATEST |
| eligible undo | erase revision 8, undo 200 revision 9, exact pre-erase grid restored |
| shared budget | 429 MURAL_BUDGET_EXCEEDED, `remaining_cells=20`, state unchanged |
| uncertain outcome | discarded response recovered 200, same key 200 revision 13, no duplicate charge |
| moderation | member admin read 403, member pause 403 PERMISSION_DENIED; operator pause 200; write 409 MURAL_PAUSED; resume 200; lock 409; restore advertised snapshot 200; freeze 200 and writes 409, terminal |
| world socket | alice and bob each received a `snapshot` frame through `/world-ws` |

DB evidence: `concert_mural_state.state = {"state":"FROZEN","revision":26,...}`,
26 revisions, per-actor `concert_mural_cell_charges`, tool actions, receipts and
outbox rows (full values in `report.json -> scenarios.IS-2.db`).

Log excerpt: each step records its own excerpt; moderation steps are
`{"note": "no new child log bytes"}` (the effect is the status and the DB row).

## IS-3 - admin report and closure

Driver: `backend/scripts/mcr/closure_vertical_stack.py` (exit 2 = PARTIAL).
The PARTIAL is exactly the two `null` archive checks; all 51 executed checks are true.

| Step | Measured |
| --- | --- |
| operator role row | `OPERATOR` revision 1, not revoked |
| operator closure | `POST /api/concert/events/{event}/admin/closure` -> 200, job `59d9453b-...`, revision 1, state PENDING, steps `[READ_ONLY SUCCEEDED, FREEZE_MURAL..CLEANUP PENDING]` |
| member closure | same route with a member token -> 403 CONCERT_403 PERMISSION_DENIED |
| post-closure supported write | fresh-key closure -> 409 GATEWAY_409 INVALID_TRANSITION |
| post-closure feed write | `POST /posts` -> 409 CONCERT_409 EVENT_READ_ONLY, row counts identical before/after (7/6/7/2/6/6) |
| archive authenticated | 200-token member -> 404 GATEWAY_404 RESOURCE_UNAVAILABLE |
| archive anonymous | no token -> 404 GATEWAY_404 RESOURCE_UNAVAILABLE (the contract declares the archive public) |
| forged token | -> 401 CONCERT_401 AUTHENTICATION_REQUIRED |
| expired RS256 token | genuinely signed, exp 60 s past -> 401 CONCERT_401 AUTHENTICATION_REQUIRED |
| world survives closure | concert process alive, no component failure after the 1 s authority tick |

Durable rows:

```
concert_events:           READ_ONLY|2
concert_closure_jobs:     59d9453b-...|0b7b9f78-...|1|PENDING
concert_command_receipts: SUCCEEDED|200   FAILED| (the fresh-key retry)
concert_closure_effects:  READ_ONLY|1
concert_closure_archives: 0
post_concert_posts:       2
```

NOT_RUN: an archived 200 with the public projection. `PgPublicArchive::page`
requires phase `ARCHIVED` and a `concert_closure_archives` row; the composed
lifecycle stops at `READ_ONLY` because `FREEZE_MURAL`..`CLEANUP` have no composed
`LifecycleOwners` (`ClosureProducer::UnavailableLater`). The reached-phase archive
behaviour is the 404 above, not a 200.

Log excerpt: `CONCERT_HEALTH_LISTENER=127.0.0.1:49195 | CONCERT_WORLD_LISTENER=127.0.0.1:49196`;
the closure steps report `{"note": "no new child log bytes"}`; the gateway
carries the unrelated scheduled `signing key fetch failed; keeping the installed keys` warning.

## IS-4 - venue invitation and notes

Driver: same closure run. All 28 venue checks true.

| Step | Measured |
| --- | --- |
| sharing/fix | location owner (member principal) 200; fix 200 |
| friend location | selected friend (operator principal) 200; no-consent view empty; blocked view empty; unauthenticated 401 |
| note create | member principal 201, revision 1; same-key replay 201 with the same id/revision; changed body with the same key 409 |
| note replies read | 503 CONCERT_503 DEPENDENCY_UNAVAILABLE (N01 read-retention path not composed) |
| reply | friend (operator principal) 201, revision 1; replay stable; conflicting revision 409 |
| stranger reply | 403 CONCERT_403 PERMISSION_DENIED |
| foreign withdraw | friend (operator) 403 CONCERT_403 PERMISSION_DENIED |
| own withdraw | note author (member) 200 revision 3; replay stable; stale revision 409 |
| meeting context | friend (operator) 200 `canRead=true`; stranger 404 |
| message round trip | operator sends to member 200 APPLIED MESSAGE; member reads 200 with the exact body |
| invitation | operator sends to member 200 APPLIED, state PENDING, revision 1; detail 200; member accepts 200 state ACCEPTED revision 2 |
| invitation stranger denials | detail/create/decision/messages/command all 404 |

Durable rows: `sharing|fixes|notes|replies|messages|receipts = 1|1|2|1|1|3`.

NOT_RUN: the blocked place-note **read** denial (reads are 503 for every role, so no
blocked denial status exists to assert). Blocked venue-meeting denial (404) and
blocked location (200 with empty friends) are measured by the QA04 matrix cells.

Log excerpt: `CONCERT_HEALTH_LISTENER=127.0.0.1:49195 | CONCERT_WORLD_LISTENER=127.0.0.1:49196`;
every venue step is `{"note": "no new child log bytes"}` and its effect is the status plus the row.

## Privacy spot-check - FAIL (QA04 abort), IN03 PASS

Surface roles: owner, member, stranger, blocked, plus suspended, expired and admin
where the matrix defines them.

**IN03 social/privacy gate (`raw/privacy-in03.json`, exit 0, 27 / 27 checks):**
private profile 404, blocked follow 403, blocked profiles 404, blocked post 404,
wall outsider 404, blocked author absent from feed, journal invalidation, unblock
restores the profile. The denial audit covered 10 denied cases; every body has
exactly `{code,message,traceId,timestamp,details}` and no private values
(`no_denied_private_metadata: true`).

The same gate records 9 NOT_RUN items of its own: venue meeting/location reads
(this driver's venue setup, superseded here by the IS-3/IS-4 closure run), external
OAuth/mail/SMS providers, an unverified-age positive case, social reveal/revoke,
comments read, the blocked comment-author filter, and client-cache/WS frames
(no browser). None is counted as a PASS.

**QA04 role x surface matrix (`raw/attempt2-privacy-qa04.json`): FAIL.**
It measured 96 cells (74 PASS, 22 NOT_RUN, 0 FAIL) and then aborted twice, on the
default gateway budget and again with `GATEWAY_REQUEST_TIMEOUT_MS=5000`, at its own
state-restore step:

```
require(unblocked[0] in (200, 204), f"qa04-restore-unblock-{unblocked[0]}")
-> Qa04Failure: qa04-restore-unblock-504
route: DELETE /api/relationships/blocks/{target_user_id}
```

The gateway log at the abort records
`ERROR {"message":"downstream request timed out","target":"api_gateway::adapters::in::http"}`
(`2026-11-06T18:43:36.786475Z`; the first attempt at `18:41:11.236692Z`). The same
unblock route returns 200 in the standalone IN03 gate, so the abort is specific to
the matrix's preceding state and is not a global unblock outage.

Measured QA04 cells before the abort (per surface):

```
feed 7 PASS | wall snapshot 7 PASS | wall history 7 PASS | profile 7 PASS
venue location 7 PASS | venue note create 7 PASS | venue meeting 7 PASS
gift create 1 PASS | gift list 7 PASS | gift detail 7 PASS | gift management 7 PASS
admin mural 1 PASS positive / 1 PASS denied | profile cache eviction 1 PASS
media metadata 7 NOT_RUN | media content 7 NOT_RUN | venue notes read 7 NOT_RUN
feed cache eviction 1 NOT_RUN
```

- Media cells are NOT_RUN because the composed read path closes with a truncated
  Gateway body (`IncompleteRead`) and the upload mutation returns
  `CONCERT_403 PERMISSION_DENIED`; no media privacy claim is made.
- Venue note-read cells are NOT_RUN because the producer returns 503 for every role.
- The WS ticket/subscription/replay cells and the final denial audit were not reached.

## NOT_RUN

| Item | Exact blocker |
| --- | --- |
| IS-2 undo wall-clock expiry | No real >600 s wait this round; only the positive in-window undo and the revision guard executed. |
| IS-2 device/visual | Backend multiuser gate only; no browser or physical device. |
| IS-3 archive 200 | Phase stays `READ_ONLY`; later lifecycle owners are not composed, so `ARCHIVED` and a 200 archive are unreachable. |
| IS-4 blocked note-read denial | Note reads 503 `DEPENDENCY_UNAVAILABLE` for every role (N01 retention provider absent). |
| Privacy media metadata/content | 14 cells NOT_RUN by the matrix (truncated read body; upload 403). |
| Privacy venue note reads | 7 cells NOT_RUN by the matrix (503). |
| Privacy feed cache eviction | Warm feed had no removable blocked-author post; journal growth recorded. |
| Privacy WS ticket/subscription/replay | QA04 aborted before the WebSocket phase; no close-code assertion in this run. |
| Privacy QA04 denial audit | QA04 aborted before its final audit; the IN03 audit passed. |

## Limits

- Development release bundle, `NOT_ADMITTED`; QA08 capacity, IN03 cohort and
  release qualification are not claimed.
- Synthetic development authorities: event, place, wall pack and principals are
  fixtures; the enrollment policy is explicitly nonproduction.
- No physical device, browser, client cache or visual comparison; these are
  server-side HTTP/WS runs.
- No in-process restart or persistence replay during these scenarios; restart
  persistence is not re-certified here.
- Native process composition, not the container image; the compose path was not run.
- The QA04 matrix abort is reported as-is; its unmeasured surfaces are NOT_RUN, never PASS.

## Reproduction

```
cd /Users/lhs/Desktop/mcr-worktrees/exec-f3
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-f3 cargo build -p concert-service \
  -p auth-service -p friend-service -p profile-service -p post-service \
  -p notification-service -p api-gateway -j 6
mkdir -p /Users/lhs/Desktop/mcr-build/exec-f3-bins
cp /Users/lhs/Desktop/mcr-build/exec-f3/debug/{api-gateway,auth-service,concert-service,friend-service,notification-service,post-service,profile-service} \
  /Users/lhs/Desktop/mcr-build/exec-f3-bins/
BINS=/Users/lhs/Desktop/mcr-build/exec-f3-bins
EV=.omo/evidence/mcr-finish-project/f3-manual-qa
python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir "$BINS" --report "$EV/raw/is1-in02.json" --in02
python3 -B backend/scripts/mcr/in04_wall_b_gate.py --bin-dir "$BINS" --report "$EV/raw/is2-in04.json"
python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir "$BINS" --report "$EV/raw/is34-closure-venue.json"
python3 -B backend/scripts/mcr/in03_social_gate.py --bin-dir "$BINS" --report "$EV/raw/privacy-in03.json"
python3 -B .omo/evidence/mcr-finish-project/f3-manual-qa/run_privacy_with_gateway_budget.py \
  --bin-dir "$BINS" --report "$EV/raw/attempt2-privacy-in03.json" \
  --qa04-report "$EV/raw/attempt2-privacy-qa04.json"
python3 -B .omo/evidence/mcr-finish-project/f3-manual-qa/assemble_report.py
```

`--qa04-report` must be an absolute path that does not exist yet. Every run owns
its PostgreSQL/Redis/mail resources and removes them on exit (`cleanup: PASS`).
