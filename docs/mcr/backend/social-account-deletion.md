# BE42 Sharing deletion owner

Status: **IN_PROGRESS — component qualification PASS; normal Root publication pending**. Rust 1.88 build, two lightweight cases, actual SQLx migration initialization, four native behavior cases and replay after a physical PostgreSQL restart pass at source `9523b0baa01f752bafceb53739dec69aaf005b32`. No full BE42, mounted public reveal/WS/cache or production Auth-to-Concert transport acceptance is claimed.

## Ownership and source boundaries

BE42-SHARING-01 continues the existing unfinished BE28–30/BE37 relationship/privacy objective in the same exclusive backend worktree. The historical goal remains `BLOCKED`; it is not reset or falsely completed. IN03 helper ownership was stopped and returned to Root before this serial task. Branch `codex/mcr-be-BE42-sharing-deletion` starts at published `10bde5f3f33438740f2fa8696becf4ae1aa1a09c`; the helper branch remains preserved.

The exact authored scope is:

- New `src/social_live/account_deletion.rs`, `tests/social_account_deletion.rs`, `migrations/0054_social_account_deletion.sql` under `backend/services/concert-service/`.
- Existing `src/social_live/producer.rs`: permanent deleted-user write/reveal guards only.
- Existing `src/social_live/invalidation.rs`: source verification and bounded draining of sealed account-deletion signals only.
- This new document.

Root separately granted own-worktree-only dependency composition: byte-exact common `src/world/account_deletion.rs` and `0052_world_account_deletion.sql`, plus exactly `pub mod account_deletion;` in `world/mod.rs` and `social_live/mod.rs`. These are isolated dependency commits `fa650038` and export-placement correction `5fe6792f`, not a transfer of common ownership. Root must omit both local composition commits from the leaf publication and use the common owner's qualified handoff and Root-owned export disposition. No other World, Auth, proto, config, main, manifest, lockfile, frontend, migration 0051 or source archive is modified. Developers own validation and fixes; there is no king/SL review or acknowledgment gate.

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

Leaf receipts and audience source identities cannot be changed, deleted or truncated; completed receipts are immutable. Pending deletion signals cannot be cascaded away, and deleting aggregate sources or consent bindings involving an incomplete fenced user is refused, including event/participant cascades. Statement-level guards also refuse TRUNCATE while common-fenced Sharing work or deletion publication history exists; row guards alone do not cover TRUNCATE. This preserves source discovery for parallel deletion targets. Completed-event pruning cannot erase the separate durable audience completion receipt. Other cleanup owners retain their retention duties.

## Test plan and runtime boundary

There are two lightweight cases and six explicit native setup/qualification cases. All eight pass in the allocated component qualification:

- Raw/missing authority, wrong target and expired/extended original dispatch refuse before touching a database.
- Actual Auth-created users and persisted deletion saga feed the labelled component test transport.
- Two events with 70 incoming/outgoing requests each exercise pagination, revocation, real AES-GCM opaque material removal, retained metadata/bindings and exact replay.
- Publisher retry, unknown/empty registry, cancellation and pending-source tampering/cascade attempts keep completion false; nonempty component private-view tuples must actually withdraw before their fixture publisher acknowledges.
- Common user-lock waits expire under the original deadline without creating partial leaf state; mismatched actor, subject, saga, target and sub-microsecond retention fail. Loss of the actual Auth source at the pre-commit recheck rolls back the first round and common fence.
- An explicit postmaster-restart phase reopens durable receipts and the permanent fence without new publication.

Social aggregate SQL setup is explicitly a storage fixture using the actual canonical Auth IDs. A short current storage deadline permits that setup; it is not an acquired Participant authority or deletion source. It creates no fake user, age claim, ordinary command receipt/outbox, expired signal, privileged grant or forced owner settlement. The component LoginUseCase uses its explicit local JWT test configuration; tokens are not supplied as canonical deletion proof. Recording fixture private views do not certify production HTTP/WS or actual World caches.

The native initializer invokes actual SQLx Auth and Concert runtime migrators over the exact copied/frozen migration directories, producing real migration ledgers; there is no manually forged ledger. The retained workspace enables `migrate` but not SQLx macros, so no manifest change is necessary. The runner must create only fresh marker/inode-owned PG16 databases `mcr_world_deletion_auth` and `mcr_world_deletion_concert`, Unix socket only, using the existing Cluster helper. `BE42_OWNER_ROOT`, identity/token, the two explicit DB URLs and `BE42_MIGRATIONS_ROOT` identify the owned fixture and frozen inputs. All six native cases are ignored by default and require explicit qualification invocation; initializer and restart each require their own named phase. The restart phase additionally requires `BE42_SHARING_PHASE=AFTER_RESTART` and an actual prior `BE42_SHARING_BEFORE_POSTMASTER` observation. Merely invoking it without physical restart fails.

The completed developer verification used finite 21d allocation `BE42-SHARING-BACKEND-202611041433Z`: one exact Rust 1.88 locked/offline jobs-two build, freeze/copy the Cargo-JSON-selected executable and source/SQL manifests, release the physical target before copied tests, run the two lightweight cases, initialize owned schemas, run four native pre-restart cases, physically stop/start the same owned PG, run the explicit post-restart case, verify zero prepared transactions and stop/remove only owned resources. Allocation deadlines, fair queue, historical byte carry, process/PG limits, disk floors and cleanup reserve remain binding. The target was released before copied tests, followed by full resource release; no access to the target occurred after its release.

Source checks: `rustup run 1.88.0 rustfmt --edition 2021` on the four authored Rust files and `git diff --check`, both exit 0. These are syntax/style evidence only. Exact commands, source/binary/SQL hashes, failures/fixes, restart and complete release evidence follow. The original full BE42 bridge, actual reveal/reconnect denial and multi-target end-to-end saga remain open.


## Qualified source, commands and evidence

The exact qualified source is `9523b0baa01f752bafceb53739dec69aaf005b32`. The later handoff changes this document only. Frozen readset: 2,385 source/contract files, complete source archive and copied SQL inputs outside the mutable target. No frozen target executable was used in place.

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p concert-service --test social_account_deletion --no-run --message-format=json
# Exit 0. Dev/test debug=0, incremental=0, both Rust flags unset.

$FROZEN_EXE --test-threads=1
# Exit 0: 2 passed, 6 deliberately ignored native cases.
$FROZEN_EXE --ignored --exact native_initialize_owned_schemas --test-threads=1 --nocapture
# Exit 0: 1 passed. Actual SQLx: Auth 18 migrations, Concert 42 migrations;
# every successful ledger checksum matches the frozen SQL source.
$FROZEN_EXE --ignored native_ --skip native_initialize_owned_schemas \
  --skip native_replay_after_external_physical_postmaster_restart --test-threads=1 --nocapture
# Exit 0: 4 passed, including actual Auth source-loss pre-commit rollback.
# Physical owned postmaster stop/start, then explicit AFTER_RESTART environment:
$FROZEN_EXE --ignored --exact native_replay_after_external_physical_postmaster_restart \
  --test-threads=1 --nocapture
# Exit 0: 1 passed. Four exact completed saga receipts replay without publication.
```

The two retained compile failures are part of the evidence: the local one-line exports initially preceded inner module documentation (`E0753`, fixed only in dependency commit `5fe6792f`); the initializer initially used SQLx macros disabled by the retained dependency (`E0433`, replaced with its existing runtime migrator and exact frozen SQL copies). Both failed commands exited 101. No manifest/lockfile, common-source byte, SQL migration history or build cache was altered to conceal a failure. The final build exited 0 in 1.29 seconds; all original deadlines and failure outputs were retained.

Native behavior proves one 32-request page leaves exactly 108 of the 140 requests active before retry; eventual completion revokes all 140 requests and 280 grants, removes only the deleted user's encrypted material and preserves 140 consent bindings and two handle tombstones. Empty/held publishers cannot complete an unfinished operation. Actual source loss rolls the first fence back. Direct binding deletion, TRUNCATE, source mutation, event cascade and new-platform recreation are refused under the permanent fence. These are component/database assertions; real retained guarded read/reconnect behavior through Root remains unqualified here.

Physical restart changed postmaster PID `63960` to `64847` and start time from `2026-11-04 23:39:14.59525+09` to `2026-11-04 23:39:32.757863+09`. Counts of common fences, completed leaf receipts, audience rows and deletion signals remained `4|4|9|9`. Prepared transactions were zero before the restart stop, after restart, before the final stop and in teardown.

Local evidence directory: `/private/tmp/mcr-sharing-deletion-rg8zi9hw`. All previous Social outputs and both compile failures remain preserved. The frozen executable is 16,973,984 bytes, SHA-256 `32cb3633aed296c3481f0623e708b519490e9ae57af4e269554b17c5e092978c`; artifact/source manifest SHA-256 `bbb1aef96aa44e0c33218b945f1407b232946a24697507955400445bac73abd1`; full-source archive SHA-256 `c1d2442936269d816b2899229b1ac57a8bf1c3c5a8536a71e5567e4854f16659`. Native report SHA-256 `72fad579c1ddd66adc9bdd0ecdcf93a2209bb165005bff429894d220d139c4c6`; full-release receipt SHA-256 `4e09aecf93c1ae3d4f9f75533daf9d1c08fb489dc9505b18af1128d10fca9a04`.

Qualified authored source SHA-256:

| Concert-service path | SHA-256 |
| --- | --- |
| `migrations/0054_social_account_deletion.sql` | `06615a4ef00102ac26e735616490afb810f4b80c4d0e87c6184122255e383ad0` |
| `src/social_live/account_deletion.rs` | `fed95c3407274d6e51f5e5446e40c2d189ce680f3cf2baa982261aab312783cd` |
| `src/social_live/producer.rs` | `128f5cdb7b9ce0ad429d706b79b2c7a5a9596592d9f738035090d03ddf0732a9` |
| `src/social_live/invalidation.rs` | `ee5d992f2f4ca91db8aab41c98a7cd2b9386dfab15947e9f0a9548e3f140c6e5` |
| `tests/social_account_deletion.rs` | `e59fee93de180b4e44e439a4cea4382dcf2d3d361da13da2c4767b7a138b35df` |

## Resource release and publication routing

Allocation accepted at `2026-11-04T14:34:53.673768Z`. Original compile/work/full cutoffs were `14:44:52.764924Z`, `14:47:52.764924Z`, `14:50:52.764924Z`. An independent watchdog existed before compiler/runtime children and enforced disk/deadline cleanup. Full initial carry was 4,120,969,216 bytes including physical checkout, shared source Git, historical native copies and 221,478,912 bytes of prior target growth. All 6 GiB whole, 1 GiB new-target, 384 MiB new-outside, executable/process/disk bounds remained satisfied; the parent observed peak was 4,253,192,192 bytes, with an independent physical-source-growth check as well. New target growth was 24,776,704 bytes, so known historical growth for future rounds is 246,255,616 bytes, without a reset or history deletion.

The literal assigned backend target was unconditionally released with zero handles/compiler processes at `14:38:39.666940Z`, before any copied executable or PG startup. The native round took 20.33 seconds. Full release at `14:40:12.382204Z` was 92.72 seconds after target release, inside the shortened 180-second work/360-second full window. Owned PG root, native/controller processes and watchdog are absent; no target read/rebuild occurred after release. The read-only slot observation was zero actual postmasters; 21d subsequently confirmed usable capacity under the three-native aggregate limit. No PKI, application services, installation, new target or new worktree was used.

Root is the named publishing developer. Publish the six authored paths using leaf commits `22a17d5e`, `483e2da6`, `922ffc64`, `10711bcd`, `9523b0ba` plus this documentation handoff, preserving their provenance. Omit local-only dependency commits `fa650038` and `5fe6792f`; use the sole common owner's qualified handoff and place the Root-owned exports after existing inner module documentation. The Sharing writer stops those six paths at handoff; no king/SL inspection or acknowledgment is a prerequisite. Root owns non-fast-forward reconciliation and combined composition validation; this implementation owner verifies remote postimages and mapped ancestry after normal publication.

Remaining original product gates: actual canonical Auth private transport into the required typed verifier; Root's actual sealed publisher/private-cache withdrawal and guarded reveal/reconnect denial; multi-target saga completion and required real vertical flow. No default positive verifier, empty-registry fallback or component fixture is offered as production composition. The broader historical persistent goal is not falsely completed by this prerequisite handoff.

Exact migrated SQL filename/SHA-384/source-commit/ownership disposition inventory is retained in `/private/tmp/mcr-sharing-deletion-rg8zi9hw/migration-readset.json`, SHA-256 `9fe3e68dc3a9998857c4ef40010135987152ba74b4bc9c31e5b57bff86409bbc`. Concert has exactly **42** migrated files: 40 unchanged files through 0051 from published base `10bde5f3`, sole-owner common 0052 and Sharing-owned 0054. Earlier coordination prose saying 43 was a counting error; the native report already contains the correct 42. No 0053, 0055, foreign prerequisite fixture or additional dependency SQL was present. The separate 18-file Auth ledger is read-only fixture input, not an Auth publication handoff.
