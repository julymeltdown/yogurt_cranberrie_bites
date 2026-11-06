# Task 39 — QA04 privacy and authorization matrix

- Plan row: 39 (`Pass QA04 privacy and authorization matrix`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w7-qa04`
- Branch: `codex/mcr-exec-w7-qa04`
- Base: `25b2d356f` (`master`)
- Date: 2026-11-06
- Result: **FAIL** — the matrix executes but exposes a real WebSocket authorization gap.
- Delivered: test harness, IN03 extension hook and this evidence. No product source
  is changed; the two exploratory admission probes were reverted before commit.

## Scope

The matrix runs only against the real seven-service development stack started by
`backend/scripts/mcr/in03_social_gate.py` on an owned PostgreSQL 16 cluster with
real mTLS, real Auth/Friend/Profile/Post/Concert/Gateway processes and an explicit
nonproduction enrollment policy (`approvalReference=development-not-admitted`).
No frontend mock, synthetic gateway client or client-side hiding is used.

| Path | Role |
| --- | --- |
| `backend/services/concert-service/tests/authorization/qa04_matrix.py` | Role × surface matrix executed after the IN03 stack and principals exist |
| `backend/services/concert-service/tests/authorization/qa04_matrix.rs` | Ignored Rust entry point that invokes the real-stack run |
| `backend/scripts/mcr/in03_social_gate.py` | Optional `--qa04-report` hook plus gift/media/venue stack composition |

## Matrix result (110 cells)

| Result | Count |
| --- | --- |
| PASS | 83 |
| NOT_RUN | 23 |
| FAIL | 4 |

Surfaces executed: feed, wall snapshot, wall history, gift list/detail/management,
gift media metadata/content, profile, venue location/notes/meeting, admin mural,
world-ticket issuance, world-socket subscription and ticket replay.

Roles: owner (alice), member (carol), stranger (outsider, no event admission),
blocked (bob, blocked by the owner), suspended (canonical Auth
`users.status=SUSPENDED` with active admission rows), expired (member of a
separate `ARCHIVED` event with a past cutoff) and admin (event `OPERATOR` with no
event admission).

## WebSocket failures (the gate is not passed)

| Cell | Expected | Actual |
| --- | --- | --- |
| stranger ticket issuance | 403 (no event admission) | 200, a ticket is issued |
| admin ticket issuance | 403 (operator role, no admission) | 200, a ticket is issued |
| stranger subscription | rejected | `snapshot` frame received |
| admin subscription | rejected | `snapshot` frame received |

Owner, member and blocked members receive a `snapshot`; owner ticket replay is
rejected by the one-use ticket path. The four failures above are a real
authorization defect: event admission is not enforced on the world ticket /
subscription path used by the composed stack. Two candidate checks (ticket
issuance in `world_rpc.rs` and admission in `world/mod.rs`) were compiled and
exercised; both left the observed 200/snapshot behavior unchanged, so they were
reverted rather than shipped unverified.

## Denial envelope audit

49 denied cells asserted the exact five-key envelope
`{code,message,traceId,timestamp,details}`, a UUID trace and a parseable
timestamp. The only nested value allowed is `details.reason` (or null for a plain
Gateway denial). A byte-level audit checked 11 private values (actor UUIDs,
tokens, coordinates, note text, media id and place id) and found no private
metadata in any denied body. Private fields are absent by construction because
the key set is closed to the envelope.

## Cache eviction

The profile path passed: the blocked member's profile was set PRIVATE, the member
followed the owner and the owner accepted, the warm `GET /api/profile/{blocked}`
returned 200, and after unfollow the next read returned the documented
`GATEWAY_404`. The durable
`mcr_friend.friend_world_invalidation_journal` rows are recorded in
`cache_eviction.profile.journal_rows`.

The feed eviction cell is `NOT_RUN`: the warm owner feed contained no removable
blocked-author post, although the block still produced a journal row
(`journal_before=0`, `journal_after=1`). No stale-content claim is made for feed
eviction.

## NOT_RUN cells (23)

- Media upload and read (14 cells): the composed public uploader-media mutation
  returns `CONCERT_403 PERMISSION_DENIED` for the resource owner before any object
  exists, and the read path closes with a truncated Gateway body. No upload/link
  or media-read authorization is claimed.
- Venue place-note reads (7 cells): the N01 prepared native read-retention path is
  not composed, so list/thread/replies return 503 for every role. The 503 is
  asserted; no note-read privacy pass is claimed.
- Media upload probe (1 cell): same composition closure.
- Feed cache eviction (1 cell): warm feed had no removable post.

## Commands

```sh
cd /Users/lhs/Desktop/mcr-worktrees/exec-w7-qa04/backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa04 \
  cargo build -j 3 -p api-gateway -p auth-service -p concert-service \
    -p friend-service -p notification-service -p post-service -p profile-service
# exit 0

python3 -B backend/scripts/mcr/in03_social_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w7-qa04-bins \
  --qa04-report /private/tmp/qa04-run39.json \
  --report /private/tmp/in03-qa04-run39.json
# exit 1: QA04 FAIL 83 PASS / 23 NOT_RUN / 4 FAIL

python3 scripts/mcr/contracts/route-parity.py
# exit 0: registry=69 contract_commands=63 templates=245 commands=70 missing=0

CONCERT_TEST_DATABASE_URL=postgres://mcr_owner@127.0.0.1:55439/mcr_venue_authz \
CONCERT_TEST_OWNER_ROOT=/private/tmp/qa04-pg \
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa04 \
  cargo test -j 3 -p concert-service --test venue_authz_matrix -- --test-threads=1
# exit 1: 45 passed; 1 failed (pre-existing)
```

The one venue-matrix failure is pre-existing and unrelated to QA04:
`meeting_route_admission_is_exact_and_closed` at
`backend/services/concert-service/tests/venue_authz_matrix.rs:2107` still expects
the product RPC to refuse `venue/location`, but the current master classifies it
as `Venue(LocationSnapshot)`. The test is stale against the merged venue
dispatch; it was left untouched.

## Cleanup

The IN03 stack closed its owned cluster, Redis and seven processes after the run;
`route-parity.py` and the venue matrix used separate owned PostgreSQL 16
resources under `/private/tmp/qa04-pg`, removed after validation.

## 2026-11-06 — world ticket/subscription authorization correction

Status: **PASS for the 110-cell QA04 matrix** on the real seven-service stack
(87 PASS, 23 NOT_RUN, 0 FAIL). The four failing cells were not a missing ticket
gate. The probe itself admitted the stranger and the unadmitted operator to the
event before issuing their tickets.

### Measured root cause

The matrix's `GET .../bootstrap` is the canonical event-join write. On OPEN/LIVE
events `BootstrapService::bootstrap` -> `BootstrapPostgresRepository::bootstrap_with_checks`
inserts the ACTIVE admission, participant and avatar defaults; the native
`bootstrap.rs` suite asserts that exact creation. `PgWorldCanonicalAuthority::subject`
in `world_rpc.rs` calls `BootstrapService::world_admission`, which reads both the
admission and participant rows and denies when either is absent. The prior
candidate checks in `world_rpc.rs` and `world/mod.rs` were therefore redundant
and changed nothing: by ticket time the probe's own bootstrap had created the rows
they checked.

An instrumented run recorded direct `mcr_concert` reads at the start of the
websocket phase (`/private/tmp/qa04-run42.json`, `world_probe`):

| Role | Before bootstrap | After bootstrap | Bootstrap | Rooms | Ticket |
| --- | --- | --- | --- | --- | --- |
| stranger | admission ABSENT, no participant, no avatar | ACTIVE, participant, avatar | 200 | 200 | 200 |
| admin (OPERATOR) | admission ABSENT, no participant, no avatar | ACTIVE, participant, avatar | 200 | 200 | 200 |
| owner | ACTIVE, participant, avatar | unchanged | 200 | 200 | 200 |

After the probe stopped bootstrapping the two unadmitted roles, the same ticket
call returned 403 through the existing gate. That exposed a second defect in the
C05 gateway error rewriter: it emitted only `{code,message,details}`, so the
matrix failed its documented five-key denial envelope (`qa04-error-envelope` at
98 cells in `/private/tmp/qa04-run43.json`).

### Delivered changes

| Path | Change |
| --- | --- |
| `backend/services/concert-service/tests/authorization/qa04_matrix.py` | Derives release pins from the admitted owner; never calls bootstrap for `stranger`/`admin`; records per-role admission state; attempts the ticket with canonical pins; presents a well-formed but never-issued ticket to `/world-ws` and requires close 4401; restores the owner block and both visibility values for later IN03 checks. |
| `backend/services/api-gateway/src/adapters/in/http/concert_world.rs` | C05 denial responses now contain `code,message,traceId,timestamp,details`; added a unit test for the exact key set and parseable trace/time. |
| `backend/services/concert-service/tests/world_admission.rs` | Native owned-PostgreSQL gate test: stranger, event OPERATOR without admission, admission-without-participant and participant-without-admission are denied; owner/member with both are admitted; SUSPENDED admission is denied. |
| `backend/services/concert-service/tests/world_ws.rs` | Native well-formed-but-unissued ticket is closed with 4401 and receives no snapshot. |

`world_rpc.rs` and `world/mod.rs` are unchanged. The previous candidate checks
would not have altered the observed rows.

### Matrix result, before and after

| Cell | Pre-fix `/private/tmp/qa04-run41.json` | Post-fix `/private/tmp/qa04-run45.json` |
| --- | --- | --- |
| stranger ticket issuance | 200, FAIL | 403, PASS |
| admin ticket issuance | 200, FAIL | 403, PASS |
| stranger subscription | `["snapshot"]`, FAIL | `["rejected:4401"]`, PASS |
| admin subscription | `["snapshot"]`, FAIL | `["rejected:4401"]`, PASS |
| owner/member/blocked subscription | `["snapshot"]`, PASS | `["snapshot"]`, PASS |
| owner ticket replay | closed, PASS | closed, PASS |

Counts: pre-fix 83 PASS / 23 NOT_RUN / 4 FAIL; post-fix **87 PASS / 23 NOT_RUN /
0 FAIL**. The post-fix report records `stranger` and `admin` with admission
ABSENT before and after the skipped bootstrap, `rooms=403`, and no ticket.

### Mutation proof

Reverting only the probe precondition (`unadmitted = set()`, restoring the
bootstrap call) and rebuilding nothing else reproduced the four failures:
`83 PASS / 23 NOT_RUN / 4 FAIL`, with the same 200/`snapshot` observations
(`/private/tmp/qa04-mutation.json`). The check was restored before the final
commit; `/private/tmp/qa04-run45.json` is the final-code run.

The gateway-envelope mutation is `/private/tmp/qa04-run43.json`: with the
corrected probe and the pre-fix gateway binary, the first unadmitted ticket was
403 but the run aborted at 98 cells with `qa04-error-envelope`, because the body
had only `code/message/details`.

### Verification commands

```sh
cd /Users/lhs/Desktop/mcr-worktrees/exec-w7-qa04
python3 -B backend/scripts/mcr/in03_social_gate.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/exec-w7-qa04-bins \
  --qa04-report /private/tmp/qa04-run45.json \
  --report /private/tmp/in03-qa04-run45.json
# QA04 report: PASS, 110 cells, 87 PASS / 23 NOT_RUN / 0 FAIL
# wrapper exit 1 at the later IN03 check feed-carol-503 (CONCERT_503
# DEPENDENCY_UNAVAILABLE); that check is after QA04 and is not claimed PASS

cd backend
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa04 \
  cargo test -j 3 -p concert-service --test world_rpc_authority \
  -- --test-threads=1
# exit 0: 4 passed

CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa04 \
  cargo test -j 3 -p concert-service --test world_ws -- --test-threads=1
# exit 0: 21 passed; 5 ignored

python3 -B backend/scripts/mcr/world_admission_smoke.py \
  --binary /Users/lhs/Desktop/mcr-build/exec-w7-qa04/debug/deps/world_admission-d44391759398674a \
  --report /private/tmp/world-admission-run3.json
# exit 0: PASS, 7 tests, restart persistence PASS, cleanup PASS

CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa04 \
  cargo test -j 3 -p api-gateway --lib 'concert_world::'
# exit 0: 4 passed (147 filtered), including the denial-envelope shape test

cd /Users/lhs/Desktop/mcr-worktrees/exec-w7-qa04
python3 scripts/mcr/contracts/route-parity.py
# exit 0: registry=69 contract_commands=63 templates=245 commands=70 missing=0
```

### Stale venue test (recorded, not changed here)

`backend/services/concert-service/tests/venue_authz_matrix.rs:2107`
(`meeting_route_admission_is_exact_and_closed`) still expects
`operation(event, "GET", "/venue/location")` to return `PermissionDenied`.
The current product dispatch classifies `GET /venue/location` as
`Operation::Venue(VenueOperation::LocationSnapshot)` in
`backend/services/concert-service/src/product_rpc/venue_admission.rs:159`.
The owner of that suite should update the assertion to the dispatched
`Venue(LocationSnapshot)` operation (or remove the stale negative if the HTTP
application-layer authorization test is the intended proof). It is a stale test,
not a product refusal, and was left untouched for its owner.

### Limits

- The native `world_admission` gate test is a regression guard for the gate that
already existed; it passes on the pre-fix source. The failures that prove the
change are the QA04 cells (probe admitted its own stranger) and the pre-fix C05
envelope (run43). No new product admission check was claimed.
- The combined IN03 runner still exits 1 after QA04 at `feed-carol-503`; the QA04
report itself is PASS. That later feed check was not diagnosed here.
- The binaries used by the real stack were rebuilt under
`/Users/lhs/Desktop/mcr-build/exec-w7-qa04-bins`; they are outside the checkout
and are not part of the commit. This remains development-stack evidence, not
release qualification or a browser claim.
- No push was performed.
