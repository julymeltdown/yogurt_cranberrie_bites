# Discovery projection writer

Status: **INTEGRATED** at the writer and source-read level. Closes the gap
recorded in the WP-03 handoff: `discovery_profiles`,
`discovery_favorite_items` and `discovery_blocks` were read and tested but never
populated, so production discovery returned an empty list.

## 1. What was built

| Piece | Path |
| --- | --- |
| The mechanism decision and its tradeoffs | `docs/mcr/integration/discovery-projection.md` |
| The profile service's internal source read | `backend/proto/profile/v1/discovery_source.proto`, `backend/services/profile-service/src/discovery_source.rs` |
| The projection writer | `backend/services/concert-service/src/discovery/projection.rs` |
| Block mirroring | `backend/services/concert-service/src/discovery/blocks.rs` |
| Writer bookkeeping migration | `backend/services/concert-service/migrations/0091_discovery_projection_writer.sql` |
| Favorites-change visibility migration | `backend/services/profile-service/migrations/0017_profile_favorites_touch.sql` |
| The two-database test harness | `backend/scripts/mcr/discovery_projection_cluster.py` |
| The tests | `backend/services/concert-service/tests/discovery_projection.rs` |

Migration heads: profile-service **17** (was 16), concert-service **91** (was 90).

## 2. The mechanism: a bounded incremental reconciliation job

The three candidates and their tradeoffs are recorded in
`discovery-projection.md`. The decision is **(b)**, and the two decisive
arguments are:

1. **Ownership.** `discovery_*` are concert-service tables, so the concert
   service must be their only writer (`domain-contract.md` §1.1,
   `profile-producer-coverage.md` §2). An outbox (a) and a synchronous call (c)
   both put write pressure on the profile service for tables it does not own.
2. **Completeness.** The projection has three sources written by three
   services — profile facts and favorites in the profile service, blocks in the
   friend service. One pull read covers all three; an outbox would need three
   producers.

The cost is bounded staleness, which is acceptable for a surface that is not
real-time: a person appears in someone's discovery list a moment after they pick
a favorite.

### How it stays correct

* **Idempotent.** Every write is an upsert keyed by `profile_id`, and a
  profile's favorites are replaced wholesale in the same transaction as its
  facts.
* **No lost updates.** The watermark advances only after a page is committed,
  and only to the cursor the *source returned*.
* **Bounded.** A pass applies at most `MAX_PAGES_PER_PASS` pages of
  `PAGE_SIZE` profiles.
* **Honest on failure.** A source failure writes nothing and records the
  failure. It never clears the projection, because an empty projection is
  indistinguishable from "nobody has favorites".

## 3. The bug the tests found

The first version of the source read used `profile_identities.updated_at` as the
watermark column. That is wrong: a favorites change does not touch the profile
row, so a favorite added or removed after a pass caught up would **never** be
seen again. `an_update_after_a_pass_is_not_lost` failed on exactly that.

The fix is migration `0017_profile_favorites_touch`: an `after insert or delete`
trigger on `profile_favorites` bumps the owning profile's `revision`, which the
identity trigger turns into a fresh `updated_at`. A removal now leaves a trace,
which `created_at` alone could not provide.

This is worth calling out because it is the class of bug the assignment asked
about: without a test that changes the source *after* convergence, the writer
looks correct and silently stops tracking picks.

## 4. How idempotency and no-lost-updates were proved

**Idempotency** (`the_writer_is_idempotent`,
`applying_the_same_page_twice_is_a_no_op`): the test converges the projection,
records the rows, rewinds the watermark to the beginning, converges again, and
requires the row set and the profile count to be identical. A replay of the whole
source stream is therefore a no-op.

**No lost updates** (`an_update_after_a_pass_is_not_lost`,
`a_visibility_change_reaches_the_projection`,
`concurrent_writers_converge_without_losing_updates`):

* the first two change the source *after* the writer has caught up, which is the
  case a naive watermark misses;
* the third runs two writers concurrently on the same projection and requires
  every subject present and no duplicate rows.

`the_watermark_advances_only_to_the_source_cursor` additionally asserts the
watermark names a real source row, so a watermark cannot drift to a value the
writer invented.

## 5. Verification commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | clean |
| 2 | `python3 backend/scripts/mcr/wp03_discovery_tests.py --concert-test discovery` | **0** | 17 passed — the existing discovery tests are unchanged |
| 3 | `python3 backend/scripts/mcr/discovery_projection_cluster.py --test discovery_projection` | **0** | 19 passed |
| 4 | `python3 backend/scripts/mcr/wp02_migration_ledger.py --report /tmp/proj-ledger.json` | **0** | profile head 17, concert head 91 |

### The two-database harness

`discovery_projection_cluster.py` creates **both** databases on one private
cluster, because the correctness argument is about two services that do not
share one:

* `mcr_projection_profile` — profile-service migrations, holding
  `profile_identities`, `profile_favorites` and `catalog_items`;
* `mcr_projection_concert` — concert-service migrations, holding the
  `discovery_*` tables.

The test reads the source through the **real** profile-service query
(`profile_service::discovery_source::read_changed`) and writes through the
**real** writer. Nothing joins across the two databases.

## 6. Deviations and limitations

1. **Two migrations, not one.** The assignment anticipated one. The writer needs
   bookkeeping (`0091`), and the no-lost-updates property needs the favorites
   trigger (`0017`). Both are additive and both were run through the ledger.
2. **The block source is a port.** `blocks.rs` turns a resolved `BlockState`
   into a projection row and is tested; resolving a `FRIEND_RELATIONSHIP_CHANGED`
   pair signal to that state through the relationship authority is the caller's
   job, and is not wired into a running worker here.
3. **The job is not scheduled.** `ProjectionWriter::run_pass` is the unit of work
   and is fully tested; mounting it on a periodic worker in `concert-service`
   `main` is a composition step.
4. **The gRPC source is implemented but not live-tested.** `GrpcDiscoverySource`
   compiles and is wired into both build scripts; the tests exercise the same
   code path minus the transport.
5. **`interaction_style` is always null.** `profile_identities` has no such
   column. The projection stores null and the discovery card omits the line,
   which is the correct "no fabricated fact" behaviour rather than a gap.
