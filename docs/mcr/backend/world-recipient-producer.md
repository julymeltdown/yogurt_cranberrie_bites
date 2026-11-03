# World recipient read producer

Status: `IN_PROGRESS`. Scope: the BE13/BE14 recipient-read prerequisite. IN01, live multiuser acceptance, runtime composition, publication fencing, and IN05 remain `NOT_RUN` here.

## Ownership and provenance

- Assigned checkout: `be-world-recipient-producer` (isolated assigned worktree).
- Assigned branch: `codex/mcr-be-world-recipient-producer`.
- Base: `1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`.
- New producer: `backend/services/concert-service/src/world/recipient_owners.rs` and `recipient_owners/local.rs`.
- New native evidence source: `backend/services/concert-service/tests/world_recipient_owners/mod.rs` and `native_smoke.py`.
- Registration-only grant: one module export in `backend/services/concert-service/src/world/mod.rs`.
- No manifest, lockfile, proto, runtime, infrastructure, frontend, or original source archive changes.

The producer is newly authored. It calls the existing signed `GrpcRelationshipAuthority` and `relationship::lock_state` local lock protocol at the base revision. It does not copy Bigyeting rendering or query Auth/Friend databases directly. Native fixture support is adapted from `tests/relationship_authority.rs`, `src/adapters/out/relationship_authority/world_auth/tests.rs`, and `tests/world_auth_read/native_smoke.py` at that revision. It retains the marker/inode/private-directory/socket/database checks and guarded owning-service reads. Dependencies are the retained workspace versions of sqlx, tokio, tonic, chrono, uuid, platform, concert-application/domain, and the existing Auth/Friend test service dependencies; no dependency changes are required.

The separately owned sealed-context source dependency is `353e7f8ebe96bac3c90134e6ca60e627589decb5`, cherry-picked with explicit Main authorization as `4a73d53`. Its qualification is separately owned; its source commit alone is not publication acceptance. The declared interface is `relationship_authority::world_projection::WorldPresenceProjection`, constructed through a `WorldProjectionParent` retained from a checked `WorldOwnerRequest` and `WorldAuthObservation`. This producer has not yet been type-checked against that committed dependency. Compilation against an uncommitted other checkout is prohibited.

## Behavior

`WorldRecipientReader::new(PgPool, GrpcRelationshipAuthority)` has no permissive authority fallback. Its asynchronous `read(&WorldPresenceProjection)` accepts one server-scoped candidate. A caller cannot construct an observation from browser identity or a raw `WorldSubject`.

Before any I/O, the reader validates the sealed context, converts it to the exact Presence `RequestBinding`, and fallibly reserves both Auth and Friend attempts. Nested attempt guards and an outer timeout retain the original monotonic stop, immutable credential/source expiry, and shared cancellation. Same-target derivation cannot mint new attempts; distinct room targets remain bounded by the parent context's fanout and original stop. There are no retries or newly minted dependency budgets.

Remote I/O completes before local database locks. Local reads reuse the event/pair/participant locking protocol and independently check:

- Presence-only actor/event/subject scope; same room, process owner/epoch and complete resource pin.
- Event phase, participation, active actor admission, subject suspension and both event-local block directions.
- Current exact session, connection, membership owner/epoch, live owner lease, membership and immutable access expiry, disconnect state, room availability, and current room map/avatar pins.

Missing authority, expired leases, owner drift, malformed state, pool/SQL failures and elapsed operation stops fail closed. A disallowed actor fails the read. A blocked, deleted, suspended or otherwise disallowed subject produces `visible=false` with independent observed revisions where the local rows still exist. No identity/profile/private handle data is returned by this producer.

Local SQL reserves a data transaction and a cancellation-control transaction before taking policy locks. An owned per-reader semaphore permit travels with the SQL task through both rollbacks, capping these two-connection operations at half the pool maximum; other pool users can still cause fail-closed acquisition expiry. An owning task retains the data connection until cancellation and rollback drain. Dropping the outer request signals that task, which cancels the exact captured PostgreSQL backend through the reserved control connection and rolls back before recycling it. Transaction-local statement and lock limits are clipped to the remaining original budget with a 100ms safety ceiling. The control query has a 250ms cleanup limit and grants no new read or publication authority. These limits disappear at rollback and do not alter serving defaults. Reads slower than the conservative SQL ceiling fail closed.

The local transaction is explicitly rolled back. The shared lock helper can provision a neutral pair when none exists; this provisional row is never committed by the reader. Its initial local revision describes the observed neutral state and is not a durable mutation receipt. No fabricated `FreshAuthority`, `PolicyDecision`, scalar stamp or `FencedRelationshipAuthority` is introduced.

The observation contains the independent Auth and Friend `(epoch, revision)` values plus separate local event, pair, participant and restriction revisions. Its expiry is the minimum of owner observation expiry, actual/captured local lease bounds, original source/credential expiry and original operation deadline. Debug output is redacted.

`check_scope(context)` checks the exact captured request/family, actor and candidate membership snapshots, cancellation and expiry. It does **not** re-observe revocation. Neither comparing a vector with itself nor passing this check proves current distributed authorization.

## Runtime handoff boundary

The runtime owner must retain the original operation context. Initial ticket-backed parents are for self Profile only. Presence requires the authoritative post-admission current membership and checked Auth observation under the same original operation deadline; no reconsume, ticket promotion, raw-subject parent construction or fresh timeout is allowed.

This patch intentionally does not implement the existing `LiveWorldOwners::visible`, whose current inputs lack that sealed parent. Runtime publication/invalidation remains separately owned: issue bounded candidate reads outside the publication fence, compare/install exact owner-scoped results, invalidate on known Auth/Friend/local/connection/owner changes, and deny expired/unavailable views. A read completed before a subsequent remote revocation is an observation, not a distributed fence over later frame enqueue/publication. No real listener, product navigation or two-user flow is certified by this patch.

## Validation

Executed in the assigned checkout:

| Command | Exit | Meaning |
| --- | --- | --- |
| `rustfmt +1.88.0 --edition 2021 --config skip_children=true` on the owned Rust sources | 0 | Source parses and formatting completes; not a type check. |
| `python3 -m py_compile backend/services/concert-service/tests/world_recipient_owners/native_smoke.py` | 0 | Native runner syntax only. Generated bytecode removed. |
| `git diff --check` | 0 | Tracked patch whitespace only; recheck staged new files before handoff. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance/structure/export regression only; 13 archive checksums preserved. |

Native build and execution: `NOT_RUN`. Targeted peer review identified a dropped-future SQL lock-wait risk; the owning-task/cancel/rollback fix and a real blocked-query cancellation case are authored. Follow-up review also found premature semaphore release and a test that could pass through SQL timeout alone; the permit now remains with cleanup, and test-only exact-backend evidence requires an explicit cancel acknowledgement followed by both successful rollbacks. These fixes and await native verification. Both existing Cargo targets are leased to other owners; no third target or dependency install is authorized. The receiving handoff must record exact dependency pin, Rust/toolchain/locked build commands and exits, Cargo-JSON artifact hashes, native report, review fixes and resulting commits before claiming native validation.

The owned native runner requires a Cargo JSON receipt containing the Concert library test executable and Auth/Friend/Concert migration binaries from this checkout. It pins/copies exact artifacts before target lease release, checks marker/inode and database identity, migrates three isolated PostgreSQL 16.14 databases, invokes actual guarded Auth/Friend services, runs the exact ignored native scenario without skipped tests, checks persistent state after a private PostgreSQL restart, and cleans up only its owned cluster. It does not start the product World listener.

The native scenario covers signed positive Presence, repeated-attempt denial, non-persistence of neutral pairs, inherited reverse block and independent Friend revision advance, local restriction revision advance, local block, independent Auth subject restriction, disconnect, room-pin and owner-epoch drift, distinct-target fanout under one original request, cancellation during an observed database lock wait, original deadline expiry, and actor family revocation. These are authored cases, not passing evidence until the runner succeeds.

## Persistent goal

The existing BE13/BE14 persistent goal remains unfinished. At inspection its tool status was historical `blocked`; the exposed goal tool cannot resume it. This continuation is working on the authorized new producer without falsely completing or replacing that goal. The earlier recovery-fixture correction also has no fresh native rerun in this checkout. No goal completion, real IN01 acceptance or release readiness is claimed.
