# Discovery projection writer — mechanism decision

Status: **DECIDED**. This note records *why* the projection is written the way it
is, because the three candidate mechanisms have genuinely different tradeoffs and
a future reader deserves the reasoning rather than only the code.

## The problem

`discovery_profiles`, `discovery_favorite_items` and `discovery_blocks` are
concert-service tables (migration `0086`). The data they mirror is
profile-owned and lives in `mcr_profile` (`profile_identities`,
`profile_favorites`, `catalog_items`) and in the friend service
(`user_blocks`). `domain-contract.md` §1.1 forbids a cross-database join, and the
two services are separate processes with separate databases
(`infra/mcr/compose.yml`).

Nothing populated the projection, so production discovery returned an empty
list. This document and the code beside it close that gap.

## The three candidates

### (a) Outbox event from the profile service, consumed by the concert service

The profile service writes an event row in the same transaction as the favorites
change; the concert service claims and applies it.

* **For.** Exactly the pattern the codebase already uses for invalidation
  (`concert-service/src/workers/outbox.rs`, `friend-service` invalidation). It is
  transactional with the source write, so no update is lost at the boundary. It
  is push-based, so latency is low.
* **Against.** It requires a producer inside the **profile** service that emits
  an event for a table the **concert** service owns. That is a new cross-service
  contract, a new consumer, and a delivery-failure surface. More importantly, it
  only covers mutations made *through* the profile service. The projection also
  has to reflect blocks, which are written by the **friend** service, and profile
  deletion, which is its own saga. An outbox would therefore need three
  producers in three services to be complete.

### (b) Bounded reconciliation job that reads the profile service's owner API

A concert-service worker periodically asks the profile service for the current
state of profiles and applies it.

* **For.** It has **one** writer, in the service that owns the tables, so the
  projection cannot be written by a service that does not own it. It is
  naturally self-healing: a missed update is corrected on the next pass rather
  than being lost forever. It needs no new event contract, and it covers every
  source (favorites, visibility, deletion) through one read path.
* **Against.** It is pull-based, so a change is visible only after the next pass.
  A full pass over every profile does not scale, so the job must be **bounded**
  and **incremental**.

### (c) Synchronous call at the mutation site

The profile service calls the concert service when favorites change.

* **For.** Lowest latency; the projection is current when the mutation returns.
* **Against.** It makes the profile mutation depend on the concert service being
  up. A favorites save would fail or block because a *projection* is down, which
  inverts the dependency: discovery is a derived read, and a derived read must
  never be able to fail the source write. It also cannot cover blocks (written by
  the friend service) or deletion without the same fan-out problem as (a).

## Decision: (b), a bounded incremental reconciliation job

**Chosen because it is the only option with a single writer in the owning
service that also covers every source.**

The decisive argument is ownership. `domain-contract.md` §1.1 and
`profile-producer-coverage.md` §2 both say the service that owns the tables
answers the operation. `discovery_*` are concert tables, so the concert service
must be the writer. Options (a) and (c) both put write pressure on the profile
service for tables it does not own; (b) keeps every write in the concert service.

The second argument is completeness. The projection has three sources — profile
facts, favorites, and blocks — and they are written by three different
services. A reconciliation read covers all three through one path; an outbox
would need three producers.

The cost — staleness — is bounded and acceptable for this feature. Discovery is
not a real-time surface: a person appears in someone's discovery list a moment
after they pick a favorite, not instantly. The job's interval is a configuration
value, and the tests assert convergence rather than an instant.

### How the bounded job works

The job does **not** scan every profile. It reads a **watermark** and asks the
profile service only for profiles whose source row changed since that watermark.
The profile service exposes one internal read for exactly this purpose
(`profile.discovery_source.v1`), which returns a page of profile facts ordered by
`(updated_at, profile_id)` and a cursor. The job applies each page and advances
the watermark.

Three properties make it correct:

1. **Idempotent.** Every write is an upsert keyed by `profile_id`, and the
   favorites are replaced wholesale inside one transaction. Applying the same
   page twice produces the same rows.
2. **No lost updates.** The watermark only advances past a page that was
   committed. A crash mid-page re-reads the same page. Because the read is
   ordered by `(updated_at, profile_id)` and the write is transactional, a
   concurrent source change either lands before the read (and is included) or
   after it (and moves the row's `updated_at`, so the next pass sees it).
3. **Deletions propagate.** A profile that is soft-deleted in the source is
   returned by the read with `deleted = true`, and the job removes its projection
   rows. A profile that disappears entirely is caught by the same mechanism
   because the read reports tombstones.

### Why the projection can be empty-but-correct

If the profile service is unreachable, the job writes nothing and the projection
keeps its last consistent state. It does **not** clear the tables: an empty
projection would be indistinguishable from "nobody has favorites", which is a
fabricated fact. A stale-but-consistent projection is the honest failure, and the
job reports it as unavailable rather than as success.

### Blocks

Blocks are written by the friend service, which the concert service already
reaches for relationship authority. The job mirrors an active block as a row in
`discovery_blocks`; a lifted block sets `deleted_at`. The discovery query already
filters on `deleted_at is null`, so a lifted block restores the profile with no
query change.

## What this decision deliberately does not do

* It does not add a score, rank or count column. The discovery query's "no score
  column" property is asserted by `tests/discovery.rs`, and this work adds no
  column to any discovery table.
* It does not fabricate a profile fact. A source field that is absent is written
  as absent (`bio = null`, `interaction_style = null`), never as a default
  string.
* It does not make a profile mutation depend on the projection.
