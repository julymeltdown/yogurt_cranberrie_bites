# BE37 consumer adoption — 2026-11-06 execution checkpoint (narrowed scope)

Status: **PASS for the narrowed task-7 scope**. Route-level 403/404 and the
two-client feed/nearby QA are explicitly **deferred** to tasks 39 (QA04 privacy
matrix), 30 (IN03) and 32 (IN01 deltas), which own those surfaces. Branch
`codex/mcr-exec-w1-relationship`; base `501710d293e5309e862997a353714e5753a76da0`.
The central progress register is untouched (reverted in `ab57da67f`; byte-identical
to the base).

## Six-entry real-PostgreSQL proof (new)

One lib test drives the six read entries against a fresh owned PostgreSQL 16.14
cluster after a committed canonical block and after a committed suspension:

`backend/services/concert-service/src/relationship/consumer_probes.rs`
`relationship::consumer_probes::committed_block_and_suspension_remove_all_six_read_entries_and_reread_after_revision`

| Entry | Probe | Production function called |
| --- | --- | --- |
| presence | `world::recipient_owners::probe_presence_visible_in` | `local::local_visibility_in` (the predicate used by `read_in`) |
| party | `party_live::probe_visible_in` | `local_subject_in` + `local_pair_in` (the list filter) |
| gift private | `gift_live::private::probe_gift_visible_in` | `locks::local_visibility_in` (the `metadata_in` predicate) |
| feed/public profile | `adapters::out::public_profile::probe_visible_in` | `local_pair_and_members_in` (the `acquire_local` predicate) |
| intro | `intro_live::probe_intro_visible_in` | `authority::local_visible_in` (the `row_in` predicate) |
| notification/report routing | `notification::routing::probe_recipients_in` | `SocialProjectionRouting::route_in` |

All probes are `#[cfg(test)] pub(crate)`. The three inline predicates were
extracted into named functions and are now the only implementation used by the
production read paths; no SQL is duplicated in the test. `cargo build -p
concert-service` exits 0, so there is no production surface change.

Sequence asserted by the test:

1. baseline: all six entries visible;
2. committed `concert_relationship_pairs.low_blocks_high = true` (pair revision
   advances, which is the cache-invalidation signal): all six removed;
3. unblock with a further revision bump: all six visible again;
4. committed admission `SUSPENDED`: all six removed;
5. restored `ACTIVE`: all six visible again.

Command and result (fresh marker/inode-owned PG16, socket-only, cleaned up):

```
python3 backend/scripts/mcr/relationship_consumer_probe.py \
  --test-binary <concert-service lib test binary> \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /tmp/be37-consumer-probe-committed.json
# -> {"status": "PASS", "exit": 0, "cleanup": "PASS"}
```

## Retained party and World-socket proofs

- `party_live_native.rs` drives the actual Party list projection on owned PG16:
  control visible -> committed canonical pair block removes the owner -> unblock
  restores -> committed `SUSPENDED` removes -> `ACTIVE` restores.
  `party_live_native_smoke.py` -> `{"status":"PASS","cleanup":"PASS"}`.
- `world_ws.rs::recipient_block_removes_the_subject_from_actual_socket_deltas`
  removes the blocked peer and, after clearing the same authoritative block,
  requires the unblocked peer to reappear in `upserts` within the next
  invalidation. Full suite 20 passed / 0 failed / 4 ignored.

## Re-run receipts

| Check | Result |
| --- | --- |
| `cargo +1.88.0 test -p concert-service --test world_ws` | exit 0; 20 passed, 0 failed, 4 ignored |
| `cargo +1.88.0 test -p concert-service --test relationship_authority` | exit 0; 3 passed, 1 ignored |
| `cargo +1.88.0 test -p concert-service --test social_live` | exit 0; 8 passed, 22 ignored |
| six-entry consumer probe (real PG16) | PASS; 1 passed, 0 failed |
| `backend/scripts/mcr/relationship_smoke.py` | PASS; 22/22 producer tests, runtime migrations, restart persistence, cleanup |
| `cargo +1.88.0 build -p concert-service` | exit 0 |

## Deferred (reassigned, not claimed here)

- 403/404 route matrix for blocked actors without private metadata -> task 39
  (QA04 privacy matrix), with the seven-principal seed plan.
- Two-client nearby/party/feed browser QA through the registered runtime and the
  IN01 deltas -> tasks 30 (IN03) and 32 (IN01 deltas). The World socket half is
  proven here; the HTTP/party/feed surface is not.

No test was skipped, weakened or allowed to pass vacuously. The only new
production-code changes are semantics-preserving extractions of the existing
inline visibility predicates into named functions, each called from both the
original read path and its `cfg(test)` probe.
