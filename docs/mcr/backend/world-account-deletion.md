# BE42 World ticket deletion

Status: IN_PROGRESS. Source contract checkpoint; native checks NOT_RUN. This is an internal World primitive, not a mounted production Auth deletion port or full-saga acceptance.

## Ownership and provenance

Owner branch: `codex/mcr-be-BE42-world-tickets`, exclusive existing checkout `be-avatar-motion-v2`, base `89f26e6edf916cfd5bcdae481c24091597027448`. The earlier C06 branch and frozen evidence remain intact. Root granted only this document, `world/account_deletion.rs`, its module export and ticket issue/consume guards in `world/mod.rs`, `tests/world_account_deletion.rs`, and migration `0052_world_account_deletion.sql`. Root publishes validated handoffs. Developers own validation and fixes; there is no king or SL review gate.

The raw command mirrors `backend/crates/auth-application/src/concert_deletion/mod.rs` at the base revision. That source, World ticket repository and migration 0005 were read as provenance; no rendering, copied dependency, Auth implementation or historical source was imported or modified. Existing async-trait, chrono, sha2, sqlx, tokio and uuid dependencies suffice. No manifest or lockfile changes.

## Shared frozen API

`world::account_deletion` exports `WorldDeletionCommand { saga_id, user_id, target, actor_id, retention_until }` and `WorldDeletionTarget::{Tickets,Sessions,Sharing,PrivateData}`. UUIDs are typed and nonnil. `identity_binding()` hashes the exact saga/user/actor/full seconds-and-nanoseconds retention; `binding()` additionally binds a fixed target discriminator. The v1 hash domains are part of the contract. Dates are not rounded to PostgreSQL microseconds before hashing. Raw commands, proof and acknowledgements redact Debug output.

`DeletionAuthority::verify(&command, original_deadline)` is mandatory configuration. A trusted adapter must check actual canonical Auth deletion repository/transport provenance before constructing `VerifiedDeletionSource::from_verified_auth_source`. That constructor binds a checked source fingerprint, exact command and original Tokio monotonic deadline; it does not authenticate an incoming request. There is no default positive authority. `verify_source` checks and bounds the authority call. Proof accessors: `check(command, original_deadline)`, `source_binding()`, `original_deadline()`.

All leaves must wrap their entire attempt with `timeout_at(original_deadline, ...)`, retain the original maximum five-second deadline through all awaits, and bound SQL waits with `bound_transaction_in`. They must recheck the configured authority before commit. A timeout during commit has an uncertain result; exact replay resolves it, never a fabricated acknowledgement.

`establish_fence_in(tx, command, verified)` takes the shared advisory lock first and returns `FenceReceipt { user_id, saga_id, revision }`. `lock_user(tx,user)` uses `hashtextextended('mcr-world-account-deletion:' || user::uuid::text,0)`, including when no fence row exists. Every producer must acquire this before event, pair, row or digest locks, then use `is_fenced_in`. Hash collisions only add serialization. Reentrant acquisition in one transaction is allowed; acquiring this after another lock violates the order.

Migration 0052 owns ONE `public.concert_world_account_deletion_fences`: user primary key, globally unique saga, immutable identity binding and positive revision 1. Update, delete and truncate are rejected. Another saga for that user or that saga for another user fails closed. This fence is common across every event and all four targets; it certifies no target completion. Sessions 0053 and Sharing 0054 must reference it and own separate immutable command/source-bound progress and completion receipts. No independent identity or replacement tombstone.

## Ticket behavior

`TicketDeletion::new(pool, Arc<dyn DeletionAuthority>)` has no authority fallback. `apply(command, original_deadline)` accepts only Tickets. It verifies authority, establishes the permanent fence, checks its exact command/source receipt and deletes at most 128 outstanding (including expired, unconsumed) tickets across all events in one transaction. Consumed ticket rows remain unchanged. A partial page commits its fence, cursor, count and progress atomically, then returns `Incomplete`; acknowledgement requires no outstanding user tickets and a durable completion receipt. Replay reads the immutable completed receipt. Completed receipts cannot be changed, removed or truncated. The pending-user digest index bounds each page; checking remaining work uses indexed EXISTS. The cursor is monotonic evidence, not a reason to skip an outstanding lower digest.

Actual ticket issuance and consumption acquire the same user lock before event/digest locks and reject a permanent fence. Consumption rechecks the immutable ticket subject after acquiring its row lock. Missing revoked digests return Unknown; an extant consumed digest of a fenced user is denied before AlreadyConsumed. The first issuance cannot race an absent fence. Unrelated users use distinct locks except harmless advisory hash collisions.

A ticket claim consumed before the fence may already be in an admission path. Sessions/admission/reconnect fencing is owned by the Sessions leaf, not this ticket scope. The canonical Auth dispatcher/source bridge and runtime mounting are Root-owned and currently missing; this primitive alone cannot certify public Auth-to-World deletion, the complete saga or active session removal.

## Validation record

- `rustfmt +1.88.0 --edition 2021 backend/services/concert-service/src/world/account_deletion.rs`: exit 0 at initial source checkpoint.
- Cargo, native PostgreSQL, actual Auth journey, races and restart/replay: NOT_RUN at this checkpoint; require fresh 21d allocation before resource use.
- Native fixtures must exercise actual Auth signup, verification, login and deletion repository source through explicitly labeled test transport. No forged JWT, fabricated Auth saga or synthetic user row is accepted as that evidence.

The next handoff will append exact commands, exit codes, source pins, limitations and committed publication ancestry. This checkpoint does not claim acceptance.
