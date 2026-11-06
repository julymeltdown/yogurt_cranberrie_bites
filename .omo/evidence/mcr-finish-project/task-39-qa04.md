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
