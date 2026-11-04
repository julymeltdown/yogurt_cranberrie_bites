# BE42 World ticket deletion

Status: IN_PROGRESS. Tickets component implementation qualified at `9bbd33e98e4ab6b9240181b2d5311af2121722e9`; additional granted admission guards and publication remain pending. This is an internal World primitive, not a mounted production Auth deletion port or full-saga acceptance.

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

## First component qualification, 2026-11-04

Fresh allocation `BE42-WORLD-TICKETS-BACKEND-202611041409Z` was accepted at 14:10:34 UTC with original fixed deadlines. Clean source `9bbd33e98e4ab6b9240181b2d5311af2121722e9` froze 2,385 files (backend, contracts, root AGENTS and this document) and the complete selected source archive. Its parent checkpoints are `2c61aa79` (common contract/schema), `f57e7299` (actual Auth test source), and `991f5655` (private database names). The source archive is 5,410,594 bytes. `before.json`, `source.tar.gz`, `tool-readset.json`, Cargo JSON, stderr, exact executable and checksummed receipts reside at `/Users/lhs/Desktop/mcr-tools/world-tickets-20261104T1412Z`. The prefix label is not the acceptance timestamp.

The command, run from backend with the allocated literal target, was:

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -j 1 -p concert-service --test world_account_deletion --no-run --message-format=json
```

Both Rust flags variables were unset. No profile variant, installation, manifest edit, cache cleanup or new target was needed. Cargo exited 0 on the first attempt in 84 seconds. It emitted the exact integration test executable and an automatic non-test Concert service binary; only the 16,725,536-byte test executable was copied and executed. All emitted executables are recorded in `freeze.json`.

An independent time/disk/process guardian began before compilation. Physical carry counted the entire tools directory (including retained C06 copies), physical checkout, shared source Git, and matching older avatar/motion temporary outputs: 2,729,132,032 bytes before new work. A subsequent accounting reconciliation explicitly added retained C06 positive target growth, reported historically as 338.2 MiB and conservatively rounded up to 339 MiB (355,467,264 bytes). Corrected starting carry is 3,084,599,296 bytes; including the first ticket round positive target growth and measured final outside outputs, corrected whole usage is 3,131,940,864 bytes, below 6 GiB. The initially messaged 3,131,904,000-byte figure was a transcription error; `carry-reconciliation.json` preserves the measured calculation. Original receipts remain unchanged; no released target was reread and no cap breach occurred. Peak positive target growth was 23,441,408 bytes; outside output at measured FULL was 23,900,160 bytes, below the 384 MiB cap; minimum observed free space was 28,777,209,856 bytes, above the 8 GiB live floor. The whole amount stayed below 6 GiB. No cap breach occurred.

`release.json` records unconditional shared-target release at 14:14:34.503 UTC, lsof exit 1/zero handles and zero compiler processes, before every copied test. The guardian stopped reading the target before release. There was no subsequent target access. Frozen binary commands:

```sh
<artifact-prefix>/world_account_deletion --test-threads=1 --nocapture --skip native_
<artifact-prefix>/world_account_deletion --test-threads=1 --nocapture --ignored --exact native_actual_auth_bounded_replay_rollback_and_ticket_races
<artifact-prefix>/world_account_deletion --test-threads=1 --nocapture --ignored --exact native_actual_postmaster_restart_replays_durable_receipts
```

All three commands exited 0: three lightweight tests, one actual Auth/component test and one postmaster-restart test. The native controller applied all 59 Auth and Concert migration files present in the frozen readset to two fresh private databases using the unchanged marker/inode-owned `migration_smoke.Cluster`, installed PostgreSQL 16.14, private Unix sockets and no TCP listener. Each migration returned zero. The exact controller, readsets, tools, migration hashes and command/log hashes are retained with `native-report.json`.

Actual Auth signup, generated email verification, password login with stored refresh session, and DeleteUserUseCase produced the canonical deletion saga read by PgUserDeletionSagaRepository. The explicitly labeled test transport checked exact saga/user/actor/retention and Tickets inventory. Unknown source and missing authority failed closed. This did not fabricate a user row or JWT and did not mount the production deletion port.

The component test proved: 129 outstanding tickets across two events (including an expired ticket) revoke as 128 plus 1 with durable Incomplete progress; rejected authority after effects rolls back the fence and page; exact replay survives pool/service reconstruction; one consumed ticket row remains byte-equivalent JSON; unrelated tickets remain unchanged; modified actor, nanosecond retention, saga and target fail; fence and completed receipts reject update/delete/truncate; first issuance and an existing digest consume wait behind the common deletion lock and deny afterward; an 80 ms original deadline expires behind the lock without being renewed. The runner stopped and restarted the actual postmaster (82453 to 84782); four receipts then replayed with revision 1 and future issuance stayed denied.

Native work and cleanup finished in 16.923 seconds. `full-measured.json` at 14:15:55 UTC confirms all four owned postmaster/native/guardian PIDs absent, no fixture directory, and 80.591 seconds from target release to measured FULL. No resources remain held. `python3 scripts/mcr/check-plan.py` and `git diff --check` also exited 0; the planning check is not application acceptance.

Root subsequently granted only additional `world/mod.rs` admit/switch/renew user-fence guards and the exact Sessions module export when its leaf exists. Those changes are outside this tested pin and require a new source pin and meaningful validation. `renew_owner` has no Auth user identity; membership lease extension belongs to the Sessions admission bridge. No result here certifies Sessions/Sharing cleanup, the canonical production Auth source bridge or the full public deletion saga.
