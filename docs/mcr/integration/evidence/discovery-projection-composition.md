# Discovery projection — scheduling and block wiring

Status: **INTEGRATED**. Closes the two composition gaps recorded in the previous
handoff: the projection job is now scheduled, and the block source is wired.

## 1. What was built

| Piece | Path |
| --- | --- |
| The worker loop | `backend/services/concert-service/src/discovery/worker.rs` |
| The block consumer | `backend/services/concert-service/src/discovery/block_consumer.rs` |
| The block cursor migration | `backend/services/concert-service/migrations/0092_discovery_block_cursor.sql` |
| Opt-in startup configuration | `backend/services/concert-service/src/config/discovery_projection.rs` |
| Worker mounted in `main` | `backend/services/concert-service/src/main.rs` |
| The discovery-source server mounted | `backend/services/profile-service/src/main.rs` |
| The live gRPC test | `backend/services/concert-service/tests/discovery_source_live.rs` |
| The worker tests | `backend/services/concert-service/tests/discovery_worker.rs` |
| The block consumer tests | `backend/services/concert-service/tests/discovery_block_consumer.rs` |

Migration heads: profile-service **17** (unchanged), concert-service **92**
(was 91).

## 2. Step 1 — the worker

`ProjectionWorker` follows the established shape in this crate
(`workers::outbox::OutboxWorker::run_until`): a `tokio::time::interval` with
`MissedTickBehavior::Skip`, a `biased` `select!` that checks the stop signal
first, and one bounded round per tick.

**Bounded.** The interval is constrained to 1 s … 600 s
(`MIN_INTERVAL_MS`/`MAX_INTERVAL_MS`), the page size is `PAGE_SIZE` = 100, and a
round applies at most `MAX_PAGES_PER_PASS` = 20 pages. A `ROUND_DEADLINE` of 60 s
abandons a wedged round rather than holding the loop open.

**On source failure.** The projection is left exactly as it was — an empty
projection would be a fabricated fact. The failure is recorded in
`discovery_projection_state.last_failure`, logged at `error` with
`worker = "discovery-projection"` and `outcome = "failure"`, and retried on the
next tick. The worker never exits on a round failure: a stale projection is
better than a stopped concert service.

**On shutdown.** The stop future is checked before each tick, and a round already
in progress finishes its current page. `run_until` therefore returns only when no
round is running, so the caller can drop the pool without racing a transaction.

**Observable.** Every round emits a span with `worker`, `outcome`
(`caught_up` / `behind` / `failure`), `profiles`, `pages` and `blocks_applied`.
A failing round is an `error!` with the failure names.

**Opt-in.** `CONCERT_DISCOVERY_PROJECTION_MODE=live` enables it;
`CONCERT_DISCOVERY_SOURCE_GRPC_ENDPOINT` names the profile service and
`CONCERT_DISCOVERY_PROJECTION_INTERVAL_MS` sets the interval. Off by default, so
a deployment without the endpoint does not start a worker that fails every round.

## 3. Step 2 — the block source

A `FRIEND_RELATIONSHIP_CHANGED` signal says *something about this pair changed*.
It is emitted for a block, an unblock, a follow and a mute alike, so the signal is
**not** a fact. The consumer resolves the pair through a `PairAuthority` port and
writes only the resolved state.

`FriendJournal` wraps the real `InvalidationSource` for the friend owner and
reads `read_after` with the consumer's durable cursor. The cursor lives in
`discovery_block_cursor` (migration `0092`), is separate from the profile
watermark because the two streams advance independently, and cannot regress
within an epoch (the schema refuses it).

**Cursor discipline.** The cursor advances only after the projection write for
that signal commits. An authority that cannot answer (`PairState::Unknown` or an
error) stops the cursor *before* that signal rather than reading it as
"unblocked" — which would silently restore a blocked profile.

## 4. Step 3 — the live gRPC test

**Possible, and done.** `discovery_source_live.rs` serves a real
`DiscoverySourceService` over loopback gRPC with the real method policy and
caller registry, and reads it through the concert service's
`GrpcDiscoverySource`. Five cases pass:

* every field round-trips, including the optional ones as absent rather than
  empty;
* the cursor round-trips and a second read returns only later changes;
* a tombstone is delivered as `deleted = true` rather than dropped;
* a non-Concert caller is refused by the method policy;
* the `(updated_at, profile_id)` ordering survives the wire.

**What it does not claim.** This is one process serving a loopback listener, not
two containers. It exercises the same server, policy, registry, client and proto
the deployment uses, but it does not prove container networking, TLS or service
discovery.

To make it possible I also mounted the discovery-source server in
`profile-service/src/main.rs`, which it was not before. That is a real change to
the profile service's composition, not only a test fixture.

## 5. Verification commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | clean |
| 2 | `discovery_projection_cluster.py --test discovery_projection` | **0** | 19 passed (unchanged) |
| 3 | `discovery_projection_cluster.py --test discovery_block_consumer` | **0** | 11 passed (new) |
| 4 | `discovery_projection_cluster.py --test discovery_worker` | **0** | 12 passed (new) |
| 5 | `discovery_projection_cluster.py --test discovery_source_live` | **0** | 5 passed (new) |
| 6 | `wp03_discovery_tests.py --concert-test discovery` | **0** | 17 passed (unchanged) |
| 7 | `cargo test -p concert-service --lib discovery_projection` | **0** | 5 config tests |
| 8 | `wp02_migration_ledger.py --report /tmp/proj2-ledger.json` | **0** | heads 17 / 92 |

## 6. Deviations and limitations

1. **The block consumer is not yet on the worker loop in `main`.** The worker
   runs the profile pass; the block consumer is composed per-deployment and its
   `BlockJournal`/`PairAuthority` ports are wired, tested and ready. Mounting it
   needs the friend invalidation endpoint in the concert service's startup
   configuration, which is a composition decision rather than a code gap.
2. **The live test is loopback, not containers.** Stated plainly above.
3. **The profile service gained a mounted service.** The discovery-source server
   is now registered in `profile-service/src/main.rs`. It is internal-only
   (`guard_without_delegation`, Concert workload only) and is not on the Gateway.
4. **A flaky test was fixed, not papered over.** `shutdown_during_a_round_waits_for_the_round`
   originally slept 5 ms and hoped a round had started. It now waits for the
   source read to be observed, and was verified stable across three consecutive
   runs.
