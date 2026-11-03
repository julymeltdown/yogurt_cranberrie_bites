# World admission tickets and room membership

State: **IN_PROGRESS**. BE10/BE11 source is integrated in this isolated branch;
native qualification is incomplete. The application library tests (3) and
application integration tests (5) currently pass. The shared service test binary
compiles, but the latest pinned PostgreSQL run had two failing concurrency cases.
Public HTTP/WebSocket ingress, BE12 movement simulation and BE13 privacy projection
remain outside this implementation.

Closed C05 contract: `docs/mcr/realtime-contract.md`, schema
`contracts/realtime/v1/world.schema.json`, and operation policy
`contracts/realtime/v1/operations.json`. The source revision for this delivery is
based on `4d6fc85b710f69a73b6942374308941d8f7a7c38`.

## Application boundary

`concert_application::world_ticket` owns the admission rules and narrow ports:

- `WorldAuthority` obtains canonical actor, live session, event/participant
  admission, restriction state, access expiry, current map and current avatar
  manifest. It must fail closed when an owner dependency is unavailable.
- `TicketGenerator` is injected by composition. The application validates the
  resulting 32-byte secret and derives a SHA-256 digest.
- `TicketRepository` persists the digest-bound record and room catalog.
- `WorldRepository` atomically consumes tickets, fences room owners, admits,
  takes over, switches, disconnects, reauthorizes, leaves and prunes leases.

`AdmissionTicket` is the only raw-secret value and redacts itself in `Debug`.
Exactly 32 random bytes encode to 43 unpadded base64url characters; a strict
decode rejects alternate representation. The ticket digest and no raw ticket,
access token, URL or owner credential crosses persistence.

Ticket expiry is the earlier of issuance plus 30 seconds and verified access-JWT
expiry. Authority maps/avatars must equal the requested pins. Issuance rejects
unavailable/hidden and known-full rooms; that availability check is advisory and
never reserves a slot.

Consumption hashes the ticket, claims its digest in a committed database step,
then checks the immutable room/map/avatar binding and performs a fresh
owner-backed reauthorization. A denial, expiry, full room or owner fencing after
claiming consumes the ticket permanently. `Consumption` uses an update predicate
on `consumed_at IS NULL`; concurrent attempts produce one `Consumed` result and
`AlreadyConsumed` for the rest.

## PostgreSQL ownership

`concert_service::world::PgWorldRepository::new(PgPool)` is the durable adapter.
The service `src/lib.rs` export is intentionally not included here because the
delivery integrator owns root service composition; it must register the module
when composing the application and tests.
Migration `0005_world_admission.sql` creates four bounded tables:

| Table | Purpose |
| --- | --- |
| `concert_world_rooms` | Event-scoped room ID, label, capacity 1-50, availability and exact map/avatar pins; at most 16 rooms/event. |
| `concert_world_owner_leases` | One process/epoch lease per `(event_id,room_id)` with a deadline. |
| `concert_world_tickets` | 32-byte digest primary key and full C05 binding; raw secrets have no column. |
| `concert_world_memberships` | One primary key `(event_id,user_id)` and one connection ID; current room/session/pins plus deadline and disconnect flag. |

Admission takes a per-user/event advisory transaction lock, locks the room row,
then verifies the current owner lease and current resource pins before evaluating
live unexpired capacity. This serializes the user's event-wide membership while
allowing different users to enter different rooms concurrently. Capacity counts
unexpired rows only and excludes the user's own event membership, so takeover and
switching do not count a user twice.

Admission, takeover and switch all use the same user/event lock. Takeover replaces
the persisted connection and returns the old non-nil connection for
CONNECTION_REPLACED emission. A switch checks the current room, session,
connection and lease, validates the target room/map/avatar and available capacity,
then changes the one membership row in one transaction. Failure before commit
leaves the old room membership intact; the target ticket is already consumed.

Process identity is explicit (`owner_process_id` plus random `owner_epoch`). An
unexpired room lease belonging to a different owner cannot be claimed or renewed.
An expired lease may be taken over. Membership revalidation confirms the owning
lease is still current and unexpired. The membership lease is capped at five
seconds and at the ticket's access expiry. Disconnect marks the membership and
shortens its deadline within a caller-supplied grace interval; bounded pruning
removes expired memberships and tickets, up to 1,000 rows per call.

The room catalog contains only room ID, label, capacity and
`AVAILABLE`/`FULL`/`UNAVAILABLE`. It never includes a roster or exact occupancy.

## Composition obligations

The parent must inject a production `WorldAuthority` that obtains canonical Auth
live-session, event/participant/restriction and current asset authority. It must
also inject a CSPRNG-backed `TicketGenerator` producing exactly 32 bytes. No
permissive production authority or random fallback is included here.

The parent owns the HTTP route for `POST .../world-tickets`, same-origin/CSRF
checks, the `/world-ws` listener, three-second first-message deadline, 4,096-byte
client and 65,536-byte server message limits, frame/aggregate bounds, ordered
control messages, snapshot/delta sequences, `CONNECTION_REPLACED` delivery,
rate limits and safe close reasons. BE12 must provide movement/simulation. BE13
must provide privacy projection. This module exposes neither public route nor
WebSocket socket.

## Verification

Static/unit commands and native evidence are recorded below. The current receipt
is a failure receipt, not a native pass.

- Planning check: `python3 scripts/mcr/check-plan.py`.
- Application tests: `cargo test -p concert-application --test world_ticket`.
- Service tests: run the built `world_admission` test through
  `backend/scripts/mcr/world_admission_smoke.py`; direct execution without the
  marked owned cluster is intentionally rejected.
- Harness guards: `python3 -B -m unittest discover -s backend/scripts/mcr -p
  test_world_admission_smoke.py`.
- Boundary check: explicit harness assignments for both owned Python files, the
  two `lib.rs` module-export exceptions and this document.

The native harness uses a new socket-only, marker/inode-owned PostgreSQL cluster,
applies migrations by direct SQL, checks digest-only schema and constraints, runs
the concurrency suite, restarts the server, verifies persisted leases/consumption,
ensures credentials do not appear in captured output and removes the cluster only
after its identity and shutdown are checked. A real 50-room-capacity load test,
production multiuser flow, HTTP/WebSocket transport and release are **NOT_RUN**.

Observed on the latest pinned run:

- `check-plan.py`: PASS, exit 0.
- Application lib: 3/3 PASS, exit 0.
- Application `world_ticket` integration target: 5/5 PASS, exit 0.
- Service `world_admission`: compiled, exit 0; direct PostgreSQL tests 2/4 PASS.
- Owned PostgreSQL 16.14 direct migration/schema constraints: PASS.
- Native run result: **FAIL**; `ticket_digest_is_single_use_under_competing_connections`
  reached `WorldError::Conflict` and `persisted_capacity_and_switch_are_atomic_for_concurrent_users`
  failed to consume its switch fixture. The latest assertion was `requested digest missing`
  before later fixture-only corrections. The corrected source has not been rerun.
- Owned cluster cleanup: PASS. No native success or restart-persistence claim is
  made for the corrected source.
