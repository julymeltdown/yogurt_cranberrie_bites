# Task 8 — BE15 outbox and command-receipt production adoption

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w1-receipts`
Branch: `codex/mcr-exec-w1-receipts` (base `501710d293e5309e862997a353714e5753a76da0`)
Date: 2026-11-06

## Change

- `backend/services/concert-service/src/workers/receipts.rs` (new): supervised
  `ReceiptMaintenance` worker. Each round calls the retained bounded repository
  methods `PgCommandRepository::retire_expired(1000)` and `PgOutbox::prune(1000)`
  on the database clock; a 30-second skipped-miss tick loop tolerates global
  shutdown. Storage failure stops the component instead of reporting success.
- `backend/services/concert-service/src/workers/outbox.rs`: added
  `OutboxWorker` — a owned sink + `PgOutbox` (30-second DB-clock leases, 8-attempt
  ceiling, retry backoff retained by the repository) with `run_round` and a
  500 ms `run_until` supervisor loop. Claim limit is validated to 1..=100.
- `backend/services/concert-service/src/main.rs`: the receipt-maintenance worker
  is started as a supervised component in every service mode and requests shared
  shutdown on unexpected failure.
- `backend/services/concert-service/tests/command_receipts.rs`: three new real-PG
  regressions and no weakened tests.
- `backend/scripts/mcr/command_receipts_smoke.py`: expected test count 17 -> 20
  and contract pins refreshed to the committed C04 inputs.

Producers already execute through `PgCommandRepository` with injected
`FingerprintKeys` (party_live, community/gift, intro, mural, report,
relationship/social, event lifecycle, age provider, media, feed). This task adds
the missing runtime worker startup rather than duplicating producer wiring.
Family-scoped outbox delivery remains owned by `SocialDeliveryRuntime`
(social/block/unblock sealed families). No global claimant is introduced: the
Social delivery contract explicitly forbids claiming families whose consumer
legs are not composed, and uncovered rows must retain their state/attempt budget.

## Acceptance evidence

### 1. Receipt integration suite on real PostgreSQL 16 (private Unix cluster)

Command:

```sh
cd backend
python3 -B scripts/mcr/command_receipts_smoke.py \
  --binary /Users/lhs/Desktop/mcr-build/exec-w1-rec/debug/concert-service \
  --repository-test /Users/lhs/Desktop/mcr-build/exec-w1-rec/debug/deps/command_receipts-2db1e6be94543255 \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /Users/lhs/Desktop/mcr-build/exec-w1-rec/be15-report.json
```

Exit code: 0

```json
{"result": "PASS", "failure": null, "cleanup": "PASS",
 "real_postgres_tests": 20, "restart_persistence": "PASS"}
```

Report: `/Users/lhs/Desktop/mcr-build/exec-w1-rec/be15-report.json`
- owned cluster `/private/tmp/mcr-pg-9rudx0ci` (created, migrated, restarted, removed)
- service binary SHA256 `4ffdbb08107dec934b9f0883ce1bd73b3d8cbb53785983a0fb255a13ee9ea9f8`
- test binary SHA256 `c211169d11d67ec5430cb68835b20e19048d7a89dece62dad8762785230c1028`
- migrations applied/reapplied/verified; failed-late-DDL rollback; damaged-checksum refusal;
  actual PostgreSQL server restart preserves receipt and outbox counts.
- cleanup: PASS (owned directory removed, no listener/imported data).

Contract pins used (committed C04 inputs):
common `cc34c0a1...f258`, operations `e6fd3cb0...458fb`, policy `55deb76b...e3ef`.

### 2. Duplicate command replay (party, one row + identical response)

Command:

```sh
cd backend
python3 -B scripts/mcr/party_live_native_smoke.py \
  --test-binary /Users/lhs/Desktop/exec-w1-rec-artifacts/party_live_native \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /Users/lhs/Desktop/exec-w1-rec-artifacts/party-replay.json
```

Exit code: 0; `status: PASS`, `native_postgres: PASS`, `restart_durability: PASS`,
`cleanup: PASS`.

The executed native test replays `createConcertParty` with the same idempotency
key and asserts: identical receipt row count, one party row, one outbox row, and
a repeated 200 response (see `party_live_native.rs` replay assertions).
Post-restart snapshot row counts: `concert_command_receipts=22`,
`concert_outbox=18`, `concert_community_parties=3`; snapshot SHA identical before
and after restart.

### 3. Killing a worker mid-claim resumes without duplicate effects

New test `supervised_worker_resumes_after_midclaim_kill_without_duplicate_effect`
(runs inside the 20-test suite above): the supervised worker claims the row and
the sink commits its deduplicated effect, then the task is aborted before
acknowledgement. The row stays `LEASED`; only lease expiry releases it. A
restarted worker claims attempt 2, acknowledges, and the consumer effects total
remains `1` with `applied = 1`.

### 4. Mid-flight receipt deletion stays safely retryable

New test `aborted_flight_leaves_no_receipt_and_retries_exactly_once`: an aborted
producer transaction leaves zero receipts, zero outbox rows and no aggregate
revision. The same authenticated command then executes once (one receipt, one
outbox row, revision 1) instead of replaying a phantom result.

### 5. Production maintenance worker round

New test `production_maintenance_round_retires_expired_and_prunes_terminal_rows`:
one maintenance round retires the expired receipt tombstone and prunes the
terminal outbox diagnostic while preserving the receipt tombstone.

## Commands

| Command | Exit |
| --- | --- |
| `cargo +1.88.0 check --locked -p concert-service --bins` (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w1-rec) | 0 |
| `cargo +1.88.0 test --locked -p concert-application` | 0 |
| `cargo +1.88.0 test --locked -p concert-service --test command_receipts --no-run` | 0 |
| `command_receipts_smoke.py ...` | 0 |
| `party_live_native_smoke.py ...` | 0 |

## Risks / limits

- No global outbox claimant was added. Operations outside the Social/runtime
  sealed families have no composed consumer in this branch; their rows keep
  their state and attempt budget. Adding a blanket claimant would burn attempts
  and could acknowledge without the required consumer legs.
- The generic `OutboxWorker` loop is compiled and regression-tested but production
  startup composes the maintenance worker; each family delivery worker keeps its
  own sealed claim (Social delivery today).
- Public HTTP/WS routes and their receipt responses belong to task 12; this task
  does not compose new public producers.
- Real Auth/provider policy remains out of scope; tests use the owned fixture
  authority already present in the suite.
