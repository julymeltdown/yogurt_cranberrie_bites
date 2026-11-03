# BE41 event lifecycle — implementation record

Status: IN_PROGRESS. Base `cd7192febf9c2764169b55d63a70d7e00d13fd08`, branch
`codex/mcr-be-BE41`, isolated checkout `/Users/lhs/Desktop/mcr-worktrees/be-event-lifecycle`.
Persistent goal is ACTIVE, unbudgeted. Source milestones are not BE41 acceptance.

## Provenance and contract

Newly authored Rust modules; no Bigyeting rendering or source code copied.
Consulted BE15 command/receipt/outbox implementation at base above, C04
`contracts/domain/v1/{admin,mural,common}.schema.json`, `operations.json` and
`policy.json`. Archive is `admin.Archive`, not a separate archive schema.
The seven public steps are READ_ONLY, FREEZE_MURAL, SNAPSHOT, EXPIRE_GRANTS,
EXPIRE_BEACONS, EXPORT, CLEANUP. BE38 owner source consulted at its worker HEAD;
shared migrations/exports/composition remain Delivery-owned. Source archives unchanged.

## Proposed explicit retry amendment (not frozen)

Existing `closeConcertEvent` POST route only. START retains existing
`{expected_revision:event,confirmation:CLOSE_EVENT,reason}`. RESUME is
`{job_id,expected_revision:job,confirmation:RESUME_CLOSURE,reason}` with a new
idempotency key, current initiating actor/OPERATOR/Auth/event-policy checks and
same bound job in READ_ONLY. Exact same-key/body transport replay returns the
previous command acceptance; it never repeats job effects. GET status never
resumes work. Failed step CAS and stable owner operation keys protect recovery.
A receipt SUCCEEDED means durable local admission actually committed, not that
all downstream steps succeeded. BE15 prohibits SUCCEEDED/202; use existing
success status 200 with the job reference and project current authorized job.
202 remains pending/uncertain only. Delivery must register/generate the amendment.

## Durable persistence increment

`src/event_lifecycle/mod.rs` implements the BE15 producer, atomic event lock +
READ_ONLY + job + local admission-barrier receipt, current authority fences,
exact actor/event/job/key binding and explicit failed-boundary CAS resume.
One owner step runs per PostgreSQL transaction; owner SQL and applied evidence
commit together. A savepoint rolls back partial mutations before recording
FAILED. Completed steps cannot regress. READ_ONLY is not treated as proof that
already-admitted world/party/reservation flows were invalidated: the actual local
owner barrier is mandatory in START. Worker ticks do not auto-resume failed jobs.
Retention is reviewed configuration, pinned by policy revision and due timestamp;
CLEANUP is not executed before due. ARCHIVED is committed only with final success.

`event_lifecycle/schema.sql` is a candidate inside the assigned scope, NOT an
installed migration. Delivery must reserve/copy it into the canonical migration
sequence and wire domain/application/service exports, cleanup worker and routes.
No root/manifests/contracts/migrations have been edited by this worker.

The archive adapter returns only the exact anonymous C04 fields. It never stores
Post bodies or accepts protected-media fallback. Current Post/deletion/moderation,
semantic privacy review and public-image qualification must be enforced by the
public owning adapter on EVERY page, with a final revision fence. Pagination is
strict descending `(created_at,post_id)` and encrypted/authenticated to event,
archive revision, upper bound, last tuple, limit and 15-minute expiry. Public
projection changes bump archive revision; a stale cursor conflicts. Final mural
is immutable. The HTTP parser validates the generated C04 schema: RESUME remains
rejected there until Delivery freezes/regenerates the proposed amendment.

## Executed checks

- `rustfmt +1.88.0 --edition 2021 --check` on all new Rust roots: exit 0.
- `git diff --check`: exit 0.
- `python3 -m py_compile backend/scripts/mcr/event_lifecycle_pg.py`: exit 0.
- `python3 scripts/mcr/check-plan.py`: exit 0; planning/provenance only.
- `python3 backend/scripts/mcr/event_lifecycle_pg.py --output /tmp/mcr-be41-sql-evidence.json`:
  exit 0, 23 actual isolated PostgreSQL SQL checks. Fixture uses a uniquely owned
  data directory and Unix socket, no TCP listener or existing database. It tests
  phase/job atomicity, unique job, false completion and missing receipt denial,
  authority deadlines, rollback, actual backend-process death before commit,
  successful effect/progress commit, partial failure, PostgreSQL restart,
  completed-step immutability, resume, actor/retention binding, immutable final mural, archive/source revision advancement, actual social/grant/beacon expiry SQL with foreign-event isolation, and append-only evidence/audit.
  The temporary cluster is stopped and removed. The JSON identifies source hashes.
  This is SQL persistence/restart evidence, NOT execution of the Rust adapters or
  production owner integrations. Earlier fixture iterations caught/corrected
  PL/pgSQL CASE parsing and generic-trigger field access before this green run.

Eight ignored native adapter tests are authored in `tests/event_lifecycle.rs` and `tests/event_lifecycle_archive.rs` with
explicitly fixture-only authority/effects under `tests/support/event_lifecycle/`.
The runner refuses empty/incomplete/duplicated suites and requires both binaries. It compares an owned-source SHA-256 compiled into each binary with the current sources; the restart seed and recovery tests run in separate processes with a PostgreSQL restart between them. Native build/run is
NOT_RUN pending root exports and an explicit shared Cargo resource handoff.

## Required remaining implementation and evidence

Production current fenced authority; actual owner
freeze, archive projection, grant/beacon + reservation/party/world expiry;
configured ownership-aware retention; strict anonymous archive reads with current
Post/media removal fences, immutable mural snapshot and revision-bound cursors.
No fake applied owner receipt. ARCHIVED only after required cleanup succeeds.

Rust 1.88.0 locked/offline, jobs=2, debug=0. Shared target is Delivery-owned;
no Cargo command run. Domain/application roots not exported yet. Unit compilation,
real owned PostgreSQL persistence/restart/partial-failure, HTTP integration and
acceptance are NOT_RUN at this source milestone.


## Privacy components and remaining owner integration

`privacy.rs` contains actual Concert SQL to revoke ACTIVE grants, PENDING/APPROVED
social requests and ACTIVE beacons, plus due-only event-owned privacy retention.
Retention deletes terminal Concert requests/bindings/grants and revoked beacons,
scrubs retained handle ciphertext, preserves command/audit tombstones, and checks
pinned policy revision and exact timestamps. These functions do not touch Auth or
Post tables. SOCIAL_EXPIRY has a durable applied receipt; BEACON_EXPIRY and
PRIVACY_RETENTION return deliberately narrower component receipts which cannot
certify world/community or whole cleanup success.

Full-base migration execution exposed the BE37 relationship deadline guard. The
production component requires BOTH an actual lifecycle authority fence and the
canonical relationship owner's independently pinned authority deadline; it never
manufactures the BE37 fence. Native fixtures pin synthetic authority only inside
test-owned adapters/seed transactions. The successful SQL command explicitly
qualifies raw SQL and foreign-event isolation, not the uncompiled Rust adapter.

Concrete unresolved owners: canonical Auth/operator/event-role policy plus
reviewed retention configuration; BE37 owner fence; final mural freeze/snapshot;
public Post/deletion/moderation/semantic review + independently public media;
community party/reservation cancellation; world admission/lease invalidation;
remaining owner retention. No production default/mock implementation supplies
these. Required module exports, canonical migration number, C04 RESUME amendment
and generated producer/consumer checks, Gateway mounting and cleanup-loop wiring
remain Delivery-owned. Shared Cargo target still belongs to Delivery; no Cargo
command or duplicate full target has been used by BE41.

Native compile request (after exact exports and resource transfer), backend cwd:
`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test --locked --offline -p concert-service --test event_lifecycle --test event_lifecycle_archive --no-run`.
Then run the owned PG script with both explicitly qualified native binaries.
Domain/application unit tests and service archive-cursor unit tests are also
NOT_RUN until this handoff. Full BE41 goal remains ACTIVE and required acceptance
is not claimed. Commits 077f01cf and 2a8bfb4 are source milestones sent to Delivery.


## Targeted review fixes

A bounded independent read-only review of archive/http at e2b150e9 found a P2:
response validator outages were collapsed to InvalidState and returned misleading
HTTP 409 conflicts. The adapter now preserves dependency outages as 503 and
separates malformed server projections as 500, both no-store. A focused Rust
regression is authored (NOT_RUN pending the same compile handoff).

Local retention review also removed per-row updated/created-time filters from
event-close privacy cleanup: the reviewed event-close deadline cannot be silently
renewed by a late write. Cleanup verifies no event-owned private rows/ciphertext
remain; the native test includes an unexpected late handle update. This does not
authorize cross-event or cross-service cleanup, and composite-owner obligations
remain required.
