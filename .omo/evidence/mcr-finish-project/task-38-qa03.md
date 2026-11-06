# Task 38 - QA03 database race suite

Status: PASS for the QA03 database race gate on the current candidate. This is
task-local PostgreSQL evidence; it does not certify canonical HTTP, Auth/Friend
authority, IN02, IN04, or the release.

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w7-qa03`
- Branch: `codex/mcr-exec-w7-qa03`
- Base: master `25b2d356f`
- PostgreSQL: 16.14 (Homebrew), private Unix-socket cluster owned by the QA03 controller
- Build target: `/Users/lhs/Desktop/mcr-build/exec-w7-qa03`
- Rust: 1.88.0 pinned by `backend/rust-toolchain.toml`; locked/offline, `-j 3`

## Delivered change

The existing QA03 controller and eleven native cases already covered party last
slot, stock 1, claim/cancel, expiry, idempotency, Wall and restart. This task
adds the missing release/re-reservation and mid-flight receipt restart cases,
records observed DB rows, and captures the observed lock graph for each
interleaving.

Changed paths:

- `backend/services/concert-service/tests/qa03_races_support/gifts.rs`
- `backend/services/concert-service/tests/qa03_races_support/party.rs`
- `backend/services/concert-service/tests/qa03_races_support/mod.rs`
- `backend/scripts/mcr/qa03_races/qualify.py`
- `docs/mcr/backend/qa03-database-races.md`
- this evidence file

No production guard was changed.

## Cases and invariants

All synchronization uses a held PostgreSQL row lock (`FOR UPDATE`), the real
`pg_blocking_pids` wait graph, or backend termination. The polling loop only
observes that graph under a ten-second deadline; it does not order the race.

| Case | Forced interleaving | Asserted invariant | Observed blocker count |
|---|---|---|---|
| `party::last_slot_and_identical_replay_are_forced` | Two approvals behind held party row; then two identical replays behind held root | Exactly one approval succeeds, other is VersionConflict; active members = capacity (2); owner retained; replay returns same receipt and adds no outbox; changed body/key conflicts | 2, 2 |
| `gifts::five_buyers_forced_one_unit_and_duplicate_replay` | Five reservations behind held giveaway row | Exactly one hold; `reserved_unclaimed=1`, `quantity_claimed=0`, `quantity_total=1`; aggregate matches index; exact replay stable; no-stock retry SoldOut | 5 |
| `gifts::claim_cancel_forced_terminal_transition_once` | Claim and cancel behind held giveaway row | Exactly one terminal transition; one stock disposition; no pickup secret retained; successful replay stable | 2 |
| `gifts::duplicate_claim_and_negative_actors_codes` | Same-key claim pair behind held giveaway row; wrong code/actor; later distinct-key claim | One receipt and one consumed unit; wrong code/actor rejected; code issuance never re-prints plaintext; a distinct claim key cannot mint a second receipt | 2 |
| `gifts::expiry_cancel_claim_release_once` | Expired claim/cancel behind held root while sweep runs with SKIP LOCKED | Expired claim cannot succeed; release happens once; repeat sweep emits nothing; secret removed | 2 |
| `gifts::expiry_skip_locked_pending_worker_restart_seed` | Blocked claim/cancel aborted; sweep skips held root | Durable reservation remains; no partial command or stock change; due work survives for the restart phase | 2 |
| `gifts::reservation_release_and_re_reservation_forced` | Cancel and stale-revision reserve behind held giveaway row | Valid release commits; stale reserve cannot take the unit (VersionConflict or SoldOut); fresh-revision reserve takes the released unit exactly once; one active reservation; total 1 | 2 |
| `gifts::interrupted_claim_rolls_back_receipt_survives_restart` | Claim blocks on pickup-secret row after admission and receipt insert; blocked backend terminated | Claim transaction holds a write XID and the receipt relation lock; uncommitted receipt rolls back; admission remains durable; reservation still unclaimed; no partial stock | 1 |
| `wall::identical_idempotency_and_cooldown_wait_on_real_lock` | Two same-key writes, then three distinct-key writes behind held mural row | One operation/revision/limit/outbox; identical result; later writes hit cooldown | 2, 3 |
| `wall::storage_failure_rolls_back_all_effects_then_same_key_recovers` | Event-specific outbox constraint fails after pixel effects | Pixel/revision/limit/receipt/budget all roll back; same key succeeds after constraint removal | none (constraint, not row wait) |
| `party::restart_preserves_approval_receipt` | Fresh process after actual postmaster restart | Same approval receipt; no repeated outbox; capacity still full | post-restart |
| `gifts::restarted_worker_drains_expiry_exactly_once` | New sweep service after actual postmaster restart | Due work drains once; second sweep empty; expiry secret removed | post-restart |
| `gifts::restarted_claim_completes_once` | Fresh process after actual postmaster restart | Interrupted admission replays to exactly one claim and one SUCCEEDED receipt; replay returns same receipt; stock claimed once | post-restart |
| `wall::restart_retains_operation_receipt_and_cooldown` | Fresh process after actual postmaster restart | Same operation receipt; no repeated outbox; original cooldown remains enforced | post-restart |

Result: `PASS`, 14/14 exact cases, each process exit 0. PostgreSQL postmaster
PID changed `63956` -> `64110`. Both SQLx ledgers matched all 63 migration
SHA-384 values. Prepared transactions and PENDING receipts were ZERO. Owned
cleanup PASS.

Native report SHA-256: `42a0662a645838338e89dae769ed832c9decedbe78fef816308c1e7859d7c648`.
Copied binary SHA-256: `9e0c6b1ab8f67fb01d5113a398550c58bedd619adbcdaa0a836908a1a577e349`.
Frozen receipt SHA-256: `80be47f6cca4fa07e235461e38f69bd9e98a7938a02811f88f43a56fc4c5b4b8`.

## Observed DB rows

Rows are printed by the suite as `QA03_STATE` lines and captured in the native
report. Representative before/after rows:

```
party_capacity_before {"active_members":1,"capacity":2,"indexed_member_count":1,"revision":1,"state":"OPEN","succeeded_receipts":3}
party_capacity_after  {"active_members":2,"capacity":2,"indexed_member_count":2,"revision":2,"state":"OPEN","succeeded_receipts":4}

gift_stock_before {"active_reservations":0,"quantity_total":1,"reserved_unclaimed":0,"quantity_claimed":0,"revision":3,"succeeded_receipts":3}
gift_stock_after  {"active_reservations":1,"quantity_total":1,"reserved_unclaimed":1,"quantity_claimed":0,"revision":4,"succeeded_receipts":4}

gift_release_before {"active_reservations":1,"quantity_total":1,"reserved_unclaimed":1,"quantity_claimed":0,"revision":4,"succeeded_receipts":4}
gift_release_after  {"active_reservations":1,"quantity_total":1,"reserved_unclaimed":1,"quantity_claimed":0,"revision":6,"succeeded_receipts":6}

gift_claim_interrupted_before {"active_reservations":1,"reserved_unclaimed":1,"quantity_claimed":0,"revision":4,"succeeded_receipts":5}
gift_claim_blocked_locks {"backend_pid":64091,"receipt_relation_row_exclusive":true,"write_transaction":true}
gift_claim_interrupted_after  {"active_reservations":1,"reserved_unclaimed":1,"quantity_claimed":0,"revision":4,"succeeded_receipts":5}

gift_claim_restarted_after {"active_reservations":0,"quantity_total":1,"reserved_unclaimed":0,"quantity_claimed":1,"revision":5,"succeeded_receipts":6}
```

The interrupted claim had already committed its admission and was blocked
inside the uncommitted receipt transaction (write XID plus
`RowExclusiveLock` on `concert_command_receipts`). Termination rolled the
receipt back; the post-restart replay produced one SUCCEEDED receipt and one
claimed unit.

## Mutation check

Temporary mutation in
`backend/services/concert-service/src/adapters/out/repository/receipt.rs`:

```diff
-                Ok(false) => return Err(CommandError::Conflict),
+                Ok(false) => {}
```

This weakens the receipt digest comparison so a changed request can reuse an
existing receipt. Recompiled and rerun with the same controller:

```
{"status": "FAIL", "failure": "exact native case failed: party::last_slot_and_identical_replay_are_forced", "cleanup": "PASS"}
assertion left == right failed
  left: Ok(SafeReceipt { ... })
 right: Err(Command(Conflict))
```

The mutant report SHA-256 is
`e1d4edc5d9ab2cf0471bcf4e2ca9b981874a7078e83fb2595400eace96720933`.
The guard was restored exactly; the restored suite then passed 14/14 again
(report above). The mutation run used the same owned PG16 controller and
cleaned up its cluster.

## Commands and results

```
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa03 cargo test \
  --locked --offline -j 3 -p concert-service --test qa03_races \
  --test community_party --test community_giveaway --no-run --message-format=json
exit 0

python3 backend/scripts/mcr/qa03_races/qualify.py run ... (14 cases, restart, cleanup)
exit 0, status PASS

python3 backend/scripts/mcr/community_smoke.py --repository-test <community_party> ...
exit 0, result PASS, restart_replay PASS, cleanup PASS

python3 backend/scripts/mcr/community_smoke.py --repository-test <community_giveaway> --giveaway ...
exit 0, result PASS, 6/6 cases pass; restart exact 1 passed; cleanup PASS

python3 scripts/mcr/contracts/route-parity.py
exit 0: registry=69 contract_commands=63 templates=245 commands=70 missing=0
```

The initial gift smoke run exposed a stale expectation, not a product failure:
`community_gift_code_rotation_throttle_and_one_time_pickup` expected a
distinct-key claim after success to return `Ok` with a `Failed` receipt object.
The current contract is `Err(Gift(CodeInvalid))` / 403 `PICKUP_CODE_INVALID`
while persisting the FAILED receipt row. The test now asserts that error and
queries `concert_command_receipts` for `state='FAILED'` and
`failure_reason='PICKUP_CODE_INVALID'`. Re-run: 6/6 pass, restart replay passes,
cleanup PASS (report SHA-256
`1ed2039f0bb80c3eb372247f16878ac2130cb8cdc0dd80f18bd4290e47caae91`). The
expectation, not the product, was stale; production code was unchanged.

## Honest limits

- The suite uses the real production PostgreSQL adapters and migrations with an
  explicitly isolated test ACL. It does not certify canonical Auth/Friend RPC,
  Gateway, public HTTP, private response settlement, IN02 or IN04.
- Pickup codes and trusted IP buckets are synthetic fixture inputs.
- The mid-flight case interrupts a claim transaction after admission and
  receipt insertion but before commit; it does not interrupt a sweep commit or
  an outbox publisher.
- No load, capacity, long-running soak or real browser flow is claimed; QA08
  remains deferred by user direction.
- The Wall cases are part of the retained QA03 matrix but the current task's
  primary product scope is party/gift/pickup; all fourteen cases are reported
  together for the gate.
