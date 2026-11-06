# BE37 consumer adoption status — 2026-11-06 execution checkpoint (attempt 2)

Status: **IN_PROGRESS / partial**. No acceptance claim. Branch
`codex/mcr-exec-w1-relationship`; base `501710d293e5309e862997a353714e5753a76da0`.
The central progress register was reverted to task 47 ownership (`ab57da67f`);
this file is the only BE37 record in this branch.

## Delivered in attempt 2

### Real PostgreSQL: Party read projection removes blocked and suspended actors

`backend/services/concert-service/tests/party_live_native.rs` now drives the
actual Party list projection (`dispatch_guarded_for_test` -> `local_subject_in`
+ `local_pair_in`) against a real owned PostgreSQL cluster:

1. unblocked control list returns both parties;
2. a committed canonical `concert_relationship_pairs` row with
   `low_blocks_high = true` removes the owner from the list;
3. clearing the pair restores both parties (unblock is not a hidden grant);
4. a committed `concert_event_admissions.state='SUSPENDED'` removes the owner;
5. restoring `ACTIVE` re-admits the control.

Command and result:

```
python3 backend/scripts/mcr/party_live_native_smoke.py \
  --test-binary /private/tmp/be37-native/party_live_native \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /tmp/be37-party-<epoch>.json
# -> {"status": "PASS", "cleanup": "PASS", "failure": null}
```

### Two-client World socket: block removal and unblocked control

`backend/services/concert-service/tests/world_ws.rs` keeps the existing
`recipient_block_removes_the_subject_from_actual_socket_deltas` and now also
clears the same authoritative block under the owner fence, then requires the
peer (the unblocked control) to reappear as an `upserts` entry on the same
socket within the next invalidation. The test fails if the peer is not restored.

```
cargo +1.88.0 test -p concert-service --test world_ws
# -> exit 0; 20 passed; 0 failed; 4 ignored; block test ok
```

## Re-verified suites (fresh private PostgreSQL 16.14)

| Check | Command | Result |
| --- | --- | --- |
| World WS | `cargo +1.88.0 test -p concert-service --test world_ws` | exit 0; 20 passed, 0 failed, 4 ignored |
| Relationship authority | `cargo +1.88.0 test -p concert-service --test relationship_authority` | exit 0; 3 passed, 1 ignored |
| Social live | `cargo +1.88.0 test -p concert-service --test social_live` | exit 0; 8 passed, 22 ignored |
| Relationship producer (real PG) | `python3 backend/scripts/mcr/relationship_smoke.py --pg-bin /opt/homebrew/opt/postgresql@16/bin --repository-test <relationship_policy>` | PASS; 22/22 producer tests, runtime migrate/verify/reapply, restart persistence, cleanup PASS |
| Party native (real PG) | `party_live_native_smoke.py --test-binary <frozen party_live_native>` | PASS; block/suspension/control assertions included |

The private clusters are marker/inode-owned under `/private/tmp`, socket-only,
and removed at the end of each run (reports record `cleanup: PASS`).

## Existing real coverage for the other consumers (read, not re-executed)

- Presence: `tests/world_recipient_owners/mod.rs` drives the actual
  `WorldRecipientReader` with real Auth/Friend/Concert databases and asserts
  Friend block, local pair block, admission suspension and Auth suspension make
  the candidate invisible.
- Intro: `tests/intro_product.rs::canonical_block_departure_suspension_and_profile_outage`
  drives the actual `IntroRuntime` list/detail with real owner clients.
- Feed/public profile: the `public_profile` native suite drives the shared
  `PublicProfileReader::policy` child that feed consumes, including
  `blocked-low`/`blocked-high` and `suspended` denials.
- Gift: `gift_live/private/locks.rs::metadata_in` is the actual private read and
  reads both pair directions plus admission state; its native tests are
  exclusion-focused and do not currently assert projection removal.
- Report/notification: `adapters/out/notification/routing.rs::route_in`
  suppresses both pair directions and `SUSPENDED`; `notification_target_policy`
  covers current-owner target policy.

## Not delivered — exact blockers

1. **Single six-entry real-PG test.** The six read paths live in different
   modules under different authority regimes. Five of six require an
   `OwnerGuardScope` built from real Auth/Profile/Friend mTLS clients; presence
   has a two-database gRPC harness; party has a test binding; gift requires a
   full giveaway/reservation aggregate. A single test would have to embed all
   six harnesses or add `cfg(test)` seams that return internal snapshots. That is
   an architecture decision (which seam shape to accept), not a one-line fix;
   it needs the owner's direction before code is written.
2. **Route-level 403/404 for blocked actors.** `RouteHarness` in
   `api-gateway/tests/unit/http_support.rs` exposes only the fail-closed
   transport router; there is no in-process registered product runtime with a
   stub Concert server returning `NOT_FOUND` for a blocked subject. The QA04
   seven-principal plan remains absent.
3. **Two-client nearby/party/feed QA.** The World socket half is now proven.
   Party is proven at producer level. Feed/nearby need the registered runtime
   and two real accounts; neither exists in this worktree.

No test was skipped, weakened or allowed to pass vacuously; the two new
assertion blocks are unconditional.
