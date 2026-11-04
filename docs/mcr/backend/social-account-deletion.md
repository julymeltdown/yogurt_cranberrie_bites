# BE42 Sharing deletion owner

Status: **IN_PROGRESS**. Source and test authoring checkpoint. Rust 1.88 formatting and whitespace checks pass; Cargo, SQLx migration execution, native tests and physical PostgreSQL restart are **NOT_RUN** at this checkpoint. No full BE42, public reveal/WS/cache or production Auth-to-Concert transport acceptance is claimed.

## Ownership and source boundaries

BE42-SHARING-01 continues the existing unfinished BE28–30/BE37 relationship/privacy objective in the same exclusive backend worktree. The historical goal remains `BLOCKED`; it is not reset or falsely completed. IN03 helper ownership was stopped and returned to Root before this serial task. Branch `codex/mcr-be-BE42-sharing-deletion` starts at published `10bde5f3f33438740f2fa8696becf4ae1aa1a09c`; the helper branch remains preserved.

The exact authored scope is:

- New `src/social_live/account_deletion.rs`, `tests/social_account_deletion.rs`, `migrations/0054_social_account_deletion.sql` under `backend/services/concert-service/`.
- Existing `src/social_live/producer.rs`: permanent deleted-user write/reveal guards only.
- Existing `src/social_live/invalidation.rs`: source verification and bounded draining of sealed account-deletion signals only.
- This new document.

Root separately granted own-worktree-only dependency composition: byte-exact common `src/world/account_deletion.rs` and `0052_world_account_deletion.sql`, plus exactly `pub mod account_deletion;` in `world/mod.rs` and `social_live/mod.rs`. This is isolated dependency commit `fa650038`, not a transfer of common ownership. Root must omit that local composition commit from the leaf publication and use the common owner's qualified handoff and Root-owned export disposition. No other World, Auth, proto, config, main, manifest, lockfile, frontend, migration 0051 or source archive is modified. Developers own validation and fixes; there is no king/SL review or acknowledgment gate.

## Common identity and reuse provenance

The sole common owner is the ticket owner. Frozen contract `2c61aa79`, additive PgConnection helper `f57e7299`, private database-name fix `991f5655`, and restart-test checkpoint `9bbd33e98e4ab6b9240181b2d5311af2121722e9` define the shared source. The imported common file SHA-256 is `9682fce1433f9de0e2a3af52f22d69ced6eeeea8bafb51e65cd68711e69902aa`; migration 0052 SHA-256 is `214e7db3cd1c506d810fd768a247bf85f777f08fb03232a509f38fb02ebd9b52`. Both remain byte exact.

`WorldDeletionCommand` mirrors Auth's `ConcertSharingDeletionPort` command: saga, user, target `Sharing`, actor and exact seconds-plus-nanoseconds retention. The common identity hash excludes target; the command hash includes its discriminator. Sharing uses the ONE immutable `concert_world_account_deletion_fences` identity. It creates no second identity table or competing tombstone. Target progress/completion is separate.

The common mandatory `DeletionAuthority`, private-field `VerifiedDeletionSource`, `verify_source`, `establish_fence_in`, `bound_transaction_in` and fixed advisory-lock ABI are reused. No default positive source exists. Every complete attempt is bounded by the original Tokio deadline, at most five seconds; configured Auth authority is rechecked, with the same source fingerprint, before each commit. Raw UUIDs/commands are not verified sources. Root owns the actual canonical Auth saga/private RPC bridge; its absence remains explicit.

Test Auth helpers are adapted from `tests/world_account_deletion.rs` at `9bbd33e9`, full source SHA-256 `6b93d9a2608a25c8dd75838ba462a50c7fddb916073bff45f3d29c324b245bcb`, with only the target mapped to `ConcertSharing`. They perform actual SignupUseCase, verification, LoginUseCase, DeleteUserUseCase and PgUserDeletionSagaRepository checks. The component transport verifies persisted deleted status and the exact canonical saga/step inventory before issuing a test-transport-labelled source fingerprint. It is not the missing production transport. Auth code, migrations and dependencies are read-only. Existing async-trait, chrono, ring, sha2, sqlx, tokio, uuid and Auth service test dependency suffice; no dependency changes are needed.

## Durable bounded behavior

`SharingDeletion::new(pool, authority, invalidation)` requires injected canonical source verification and the existing actual `SocialInvalidationSink`. `apply(command, original_deadline)` accepts only `Sharing`; mismatched subjects, actors, sagas, nanosecond retention or source fingerprints fail closed. It returns the common exact `DeletionAcknowledgement` only after durable completion; `Incomplete` and publisher failures are never acknowledgments.

The common user lock is first, before receipt, event, request and generation locks. The immutable common fence and exact leaf receipt commit with the round's effects. A round visits one event and at most 32 requests, over incoming and outgoing history across **all** events. Cursors and source bindings survive uncertain replies/restart. Handle ciphertext, nonce and key ID are removed while retaining revision/tombstone metadata. Active grants and pending/approved requests become revoked; consent bindings, immutable command receipts, outbox/audit integrity and aggregate metadata are retained. New user-leading indexes support discovery and final remaining-work checks. The common fence prevents newly inserted earlier cursor work from appearing after traversal.

Producer hooks derive known request subjects from immutable stored bindings, acquire common user locks in sorted order before event locks and reject any permanent fence. This is a guard, not authorization from parsed IDs. SQL guards cover handle/request/grant/binding creation and mutation. Removal transactions may only clear encrypted fields or revoke states under the exact verified deletion context. The fallback trigger uses nonblocking common-lock acquisition so a misplaced event-first writer cannot create a late waiting lock-order cycle. Verified deletion never waits for peer user locks after its event lock. The mounted Social runtime enforces future write/reveal refusal; raw database SELECT permission is not a reveal capability.

The original relationship deadline function retains its ordinary path. Migration 0054 adds only a closed removal-context branch tied to the exact leaf/common fence and original deadline; it never fabricates an ordinary Participant command, receipt, outbox or expiry source. Its database cutoff uses an actual server-clock sample and the original monotonic remainder, conservatively subtracting the sampling round trip. No application clock is changed and no fresh five-second window is created after waits.

Each affected event/actor receives a monotonically advanced policy generation and durable audience record. New explicit deletion-source columns and the three-way closed CHECK distinguish ordinary outbox, real deadline and account-deletion provenance. Foreign keys bind deletion signals to the exact immutable operation/source/fence and audience revision. Before constructing the existing sealed `VerifiedSocialInvalidation`, the kernel joins and verifies that durable source and common identity. Only the existing publisher receives `CANONICAL_POLICY_CHANGED`; there is no unsealed or alternate publisher call.

Audience publication is recorded in the same transaction as successful existing-publisher delivery. A retryable/unknown/held publisher keeps the leaf incomplete. Completion requires traversal finished, no active material/requests/grants and no pending audience. The publisher contract requires each matching stream/private view to be withdrawn or durably fenced, with reconnect using the durable baseline. A completed receipt replays without a new publication; an empty registry cannot complete an unfinished operation. The real World/private-cache publisher remains Root-owned and must be injected and qualified in production composition.

Leaf receipts and audience source identities cannot be changed, deleted or truncated; completed receipts are immutable. Pending deletion signals cannot be cascaded away, and deleting aggregate sources involving an incomplete fenced user is refused, including event/participant cascades. This preserves source discovery for parallel deletion targets. Completed-event pruning cannot erase the separate durable audience completion receipt. Other cleanup owners retain their retention duties.

## Test plan and runtime boundary

There are two lightweight cases and six explicit native setup/qualification cases. They are authored, not executed at this source checkpoint:

- Raw/missing authority, wrong target and expired/extended original dispatch refuse before touching a database.
- Actual Auth-created users and persisted deletion saga feed the labelled component test transport.
- Two events with 70 incoming/outgoing requests each exercise pagination, revocation, real AES-GCM opaque material removal, retained metadata/bindings and exact replay.
- Publisher retry, unknown/empty registry, cancellation and pending-source tampering/cascade attempts keep completion false; nonempty component private-view tuples must actually withdraw before their fixture publisher acknowledges.
- Common user-lock waits expire under the original deadline without creating partial leaf state; mismatched actor, subject, saga, target and sub-microsecond retention fail.
- An explicit postmaster-restart phase reopens durable receipts and the permanent fence without new publication.

Social aggregate SQL setup is explicitly a storage fixture using the actual canonical Auth IDs. A short current storage deadline permits that setup; it is not an acquired Participant authority or deletion source. It creates no fake user, age claim, ordinary command receipt/outbox, expired signal, privileged grant or forced owner settlement. The component LoginUseCase uses its explicit local JWT test configuration; tokens are not supplied as canonical deletion proof. Recording fixture private views do not certify production HTTP/WS or actual World caches.

The native initializer invokes actual SQLx Auth and Concert migrators embedded in the frozen test executable, producing real migration ledgers; there is no manually forged ledger. The runner must create only fresh marker/inode-owned PG16 databases `mcr_world_deletion_auth` and `mcr_world_deletion_concert`, Unix socket only, using the existing Cluster helper. `BE42_OWNER_ROOT`, identity/token, and the two explicit DB URLs must pass the copied custody checks. Initializer and restart phases are ignored by default and require explicit named invocation. The restart phase additionally requires `BE42_SHARING_PHASE=AFTER_RESTART` and an actual prior `BE42_SHARING_BEFORE_POSTMASTER` observation. Merely invoking it without physical restart fails.

Expected developer verification requires a fresh finite 21d allocation: one exact Rust 1.88 locked/offline jobs-two build, freeze/copy the Cargo-JSON-selected executable and source/SQL manifests, release the physical target before copied tests, run the two lightweight cases, initialize owned schemas, run four native pre-restart cases, physically stop/start the same owned PG, run the explicit post-restart case, verify zero prepared transactions and stop/remove only owned resources. Allocation deadlines, fair queue, historical byte carry, process/PG limits, disk floors and cleanup reserve remain binding. No native resource or target access has occurred at this checkpoint.

Source checks: `rustup run 1.88.0 rustfmt --edition 2021` on the four authored Rust files and `git diff --check`, both exit 0. These are syntax/style evidence only. Exact Cargo/native commands, exit codes, frozen source/binary/SQL hashes, failures/fixes, physical restart and complete release evidence will be appended after the responsible developer's allocated qualification. The original full BE42 bridge, actual reveal/reconnect denial and multi-target end-to-end saga remain open.
