# BE33 Notification target authority

Status: `IN_PROGRESS`. The two exact artifacts compile and the four pure cases plus one real PostgreSQL/mTLS lifetime/recovery case pass. Provider publication and production Root composition remain pending. This document does not accept full BE33, IN03 or a public Gateway journey.

## Contract and composition

Source baseline: `145a8e939fb7bede7e46931d6559adb2c51f62a7`. Dependency commit `50a5003370b4dfab9cb6483c8689a8378446f9a0` appends the private `ConcertNotificationTargetAuthority.Authorize` RPC in `backend/proto/notification/v1/concert_inbox.proto` without modifying existing ABI and supplies `notification-service/src/concert/target_client.rs`. The Notification startup owner separately exports and constructs `RootTargetClient::connect(endpoint, credential, &TlsMaterial)`; its integration is not implied by this source commit.

Root composition must construct `TargetReadSettler::new(pool.clone(), instance)?` before the existing canonical core, and pass it to `CompositeChildIntentSettler::with_target`. Mount `notification_live::target_authority::TargetAuthorityService::new(pool, core.registry()).server()` under its exported `POLICY`, `guard_without_delegation`, mandatory mutual TLS, the Notification credential and `CallDeadlineLayer`. Reuse the actual configured pool, producer instance and registry. This source assignment does not modify production composition/main/listener files.

The callback requires the authenticated Notification workload, exact original opaque RootReadView, parent Auth/Friend views and binding, original operation/method/HTTP tuple, actor/event/family/request/digest, unchanged expiry and ordered typed candidate snapshot. It admits only the four Inbox operations. It retains RootReadPermit across actual canonical Auth/Friend enrollment, source observation, durable intent, native PREPARE and guard attachment. A digest covers the entire callback request including its call nonce. A one-MiB bound refuses the entire snapshot; it never truncates candidates or computes partial counts.

Even an empty count/list snapshot enrolls the canonical actor and SHARE-locks the actual event, admission and participant revision. Six local target policies reuse `notification_target_policy::observe_local_target` inside the same SQLx-tracked borrowed transaction. Visible targets additionally enroll actual owner/applicant/recipient IDs extracted from already-locked source rows. Negative canonical source decisions hide affected candidates; unknown authority fails the operation.

Post, Report and missing targets remain `Unavailable`: no actual Post owner or operation-bound EventRoleFence is installed. Missing owner rows never prove deletion. No synthetic positive provider, narrowed required scope or full acceptance claim is supplied.

## Native lifetime and recovery

Migration `0050_notification_target_child_intents.sql` adds the closed `TARGET_READ` kind while preserving all five published kinds. Its private encrypted intent captures actual PostgreSQL system identifier, database OID, full XID, backend PID/start, producer instance, Root scope, original binding, the exact linked NotificationInbox intent/reference digest and request snapshot digest. Intent persistence precedes PREPARE. The fixed GID derives only from its typed nonce, never arbitrary SQL or caller-selected GID text.

The callback retains its actual native read transaction through Notification native completion and Root outer EOF/recovery. Before-byte checks require the same prepared transaction under the original deadline. Recovery releases only after durable RootFinished and exact linked Notification completion, checks its own immutable intent digest and configured instance/database identity, rolls back the exact prepared transaction, and requires `pg_xact_status(full_xid) = 'aborted'`. Missing GID, expiry, lost memory, renewed authority or a later callback do not prove settlement. PublicProfile recovery explicitly refuses TargetRead; a missing composite target settler also refuses it.

## Provenance and validation

No `docs/source/` file is changed. Newly authored rendering is outside this backend scope. Existing local policy code is reused unchanged at baseline `145a8e9`; its only module exposure is in concert-service/lib.rs. Native proof/capacity mechanics are adapted from `src/adapters/out/public_profile/local.rs` and `src/social_live/read_child.rs` at that same revision, with a separate closed target kind/GID and additional durable dependency checks. Runtime dependencies are existing SQLx, tonic/prost, platform deadline/mTLS/workload guards, SHA-256, serde, chrono and UUID; no manifest or lockfile changes.

Tests reuse the existing CF01 owned environment/capture and owner fixture source at `145a8e9`; the copied owner fixture adds the actual Notification producer settlement endpoint to the exact producer map. Its negative completion fault refuses before forwarding; positive authority and settlement come only from production services. This is an internal transport fixture, not a new process/controller harness or public Gateway proof.

| Check | Result |
| --- | --- |
| Rust 1.88 formatting of authored Rust files | Exit 0 |
| `git diff --check` | Exit 0 |
| Exact concert-service `notification_target_authority` artifact | `--no-run` exit 0; copied pure run exit 0, 3 passed / 1 ignored; separate native case below |
| Exact notification-service `concert_target_client` artifact | `--no-run` exit 0; copied run exit 0, 1 passed |
| Ignored owned PG16/mTLS lifetime/recovery case | Explicit copied exact-case run exit 0; 1 passed, 5.37 seconds |
| Production main mount / combined Root and Notification startup | `NOT_RUN`, integrator-owned composition |
| Public Gateway flow / complete BE33 / IN03 | `NOT_RUN` |

The ignored native case exercises a real empty inbox callback and source-lock retention through a refused Notification completion, finished Root, original deadline expiry and subsequent system recovery. Six-kind native positive callbacks, nonempty masks/counts, Post/Report owners, broader adversarial callback replay and the final public vertical flow remain required qualification/acceptance work; local policy unit/component evidence does not certify these flows. All executed commands and delivered ancestry must be appended after qualification, without replacing `NOT_RUN` with inferred success.


## Bounded resource prerequisite

Before requesting compilation, owner207 retired only its three completed clean STOPPED old CF01/BE15-QA09/CR07 source worktrees under the explicit sustainable-headroom assignment. All retained branches/history, tracked blobs/modes and 15 source-original entries per checkout were verified. Unique ignored files and complete per-worktree Git administration, numeric custody, ACLs and xattrs were privately archived and independently extracted/verified before normal non-force removal. Root supplied a factual no-active/frozen-old-path-consumer statement. No target, dependency, other checkout, active provider source, original proof archive or selected artwork was removed.

Private custody receipt basename: `mcr-207-retirement-hrtkjjvw`; manifest SHA256 `c3c4d2302e22365872a08898f7a12d6e57e363d7652d6c3a8df1491db786d641`. It records exact command attempts/exits, all three archive hashes and original baked-path reconstruction instructions. Observed shared-volume free space was 17,204,813,824 bytes before and 20,440,555,520 after (net +3,235,741,696; concurrent volume activity prevents exclusive attribution). Resource recovery is not compilation/native evidence and does not itself authorize a build: a fresh named allocation and fresh floor checks remain required.


## Notification startup owner report

The separate delivery/startup owner reported 11 passing checks against this assignment's frozen `50a50033` wire/client: four routing/client cases, three mTLS ingress cases and four runtime cases, including explicit live-mode mounting and graceful Notification restart. The agreed `RootTargetClient::connect` constructor, module export and both service build integrations reportedly compiled without changes to the owned dependency. That owner's resource allocation is fully released. Its leaf was subsequently published at `c0ed4243862cd4ec110fa591921e070eabd79c38`.

The 11-case outcome is attributed owner evidence. A fresh independent remote readback confirmed that publication and byte-identical postimages for both owned dependency files: `50a50033` maps through `f7e215eb` to delivered `55a52ce266982ecdc8af73a596fd38ab4e41836a`. Provider qualification below is a separate execution, and actual combined production Root composition remains a separate obligation.


## Executed provider qualification

Accepted offer: `BE33-TARGET-AUTHORITY-BACKEND-202611041114Z`. Compiled source was `ef0f0785145fcf44f9a46a7160fb0d96f62dc5da`, whose Rust source is the `59ced660` checkpoint plus a documentation-only successor. No Rust fix or build/native retry was needed. The exact sequential commands were:

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test notification_target_authority --no-run --message-format=json
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p notification-service --test concert_target_client --no-run --message-format=json
```

Both exited 0 (Cargo elapsed 25.72 and 7.54 seconds; guarded compile/freeze wrapper 36.58 seconds), with debug information and incremental compilation disabled. Only the two test executables were copied: 35,846,064 and 11,249,872 bytes, total 47,095,936. Automatically produced mains were accounted as target growth and were neither explicitly built nor copied/run.

Full source/runtime/fixture/migration pins and Cargo JSON were frozen privately. The build target was unconditionally released with fresh zero-handle/absent-compiler proof at 20:23:11 KST on 2026-11-04, before **all** test execution. Subsequent checks accessed only the frozen copies and source/pins; there was no later target access.

The copied Root artifact passed three pure cases, with its native case accurately ignored in that run. The client artifact passed its one pure refusal case. The existing `canonical_owner_composition_support/qualify.py` controller was then configured with exactly `original_root_target_callback_retains_policy_until_notification_settles` and its existing 90-second case timeout; that explicit native run passed in 5.37 seconds. Its historical artifact-key ABI was explicitly aliased to the actual `notification_target_authority` executable without making another copy or running any CF01 test. Resource observation only added owned-root/process registration and bounded PostgreSQL settings; no new application/controller harness or helper chat was introduced.

One marker/inode-owned, socket-only PostgreSQL 16 cluster created exactly `mcr_canonical_auth`, `mcr_canonical_friend`, `mcr_canonical_concert`, `mcr_canonical_profile` and `mcr_canonical_notification`. The four used stores had exact source/checksum migration receipts: Auth 18, Friend 18, Concert 39, Notification 6. Profile was intentionally unused and empty; its schema/migration execution is explicitly `NOT_RUN`, not a five-owner migration success claim. Actual Auth/Friend/Root/Notification mTLS services shared the one test PID and used four in-process listeners (cap eight).

The native case exercised:

- Actual callback refusals for Post, Report and absent targets, a foreign actor, a changed original Root secret and an invalid Notification credential; none created a target child.
- A successful real empty unread-count operation through the actual Notification client, Root registry, canonical Auth/Friend guards and retained target transaction; it created distinct Inbox and Target intents.
- Actual actor-admission writers refused by retained native locks before Root completion and after the original deadline expired.
- A negative-only pre-forward Notification completion refusal left a finished Root, pending Inbox and still-prepared Target child. The system sweeper could not release it while Inbox remained pending.
- After the real Notification completion was allowed, the typed worker settled Inbox and then Target using actual native abort evidence. All pending child intents and all five databases' prepared counts were zero **before** cluster teardown.

These are real internal callback/native facts; they do not establish a nonempty six-kind inbox, all before-byte/foreign/replay adversaries, a Post owner, EventRoleFence, production Root mount, external Gateway EOF or complete BE33/IN03 acceptance.

The absolute original offer clock was conservatively bounded to 20:34 KST, with compile/freeze cutoff 20:27:30, copied-job cutoff 20:32 and at least 120 seconds reserved for cleanup. Actual full release was 20:25:52 KST. Fresh initial free space was 22,469,337,088 bytes; the guarded 8-GiB stop floor held. Conservative whole carry was 1,799,565,312 bytes, including the full active source checkout, entire prior CF02 peak including failed attempts, retirement custody and original restore evidence. Maximum sampled whole allocation including fresh target/evidence/PG/PKI/TMP growth was 2,083,287,040 bytes, below the 3-GiB cap. Positive target growth remained charged after release. Owned postmaster, foreground test/build process groups, cluster, TMP and PKI were absent at full release. Teardown itself is never counted as native settlement.

Private receipt basename: `mcr-be33-target-xj2uuc3t`. The following hashes identify the frozen evidence without publishing credentials or local private contents.

| Evidence | SHA256 |
| --- | --- |
| `offer-acceptance.json` | `4c00e31f3f4150a3e869d50a5bd184f5df1ff98c1d2753b26b6747ee0926849b` |
| `checkpoint.json` | `fcb7ec4f70cbde56411b9bfc7a6e68ff187f9e389807bdfd2af8442031c875f3` |
| `runtime-pins.json` | `26da35c3f60792bfeba110427ac3f2056f1470343657d98fe5d7e55c7ad5ecf6` |
| `target-release.json` | `abb3e4e61832ad2083ffbd02c0ba9af678d922d64185912ff7949757afe43202` |
| `runtime-guard.json` | `c304903e7541649859dba0c23266f2798e44f39cd888de7f487e1c910c84b70a` |
| `native.json` | `351c783bcf1248c0a00fc616960ac723d433f894f3db24c7ea293589880f44eb` |
| `full-release.json` | `de9c5ee2c4f29ed7f6dd5b430580fd467c27fe9c37ea02accfceedb71d0434bf` |
| `dependency-publication-readback.json` | `7493f1998e5e8ac6d25120e77a49804c5a3f61193fd2e92efddd57991f9aeb56` |
| `notification_target_authority-tests` | `92376ea50b41427a1ce7756fa3d42593ed307f9659ffe21796195b6e368b4acb` |
| `concert_target_client-tests` | `79b2108bbf69e593a94be4c9b8c51dca8cecdf008a3c51fbb5d9922a207964ed` |

Delivered ancestry begins at `145a8e939fb7bede7e46931d6559adb2c51f62a7`: `50a50033` (already independently published dependency), `4394b409` (provider/typed recovery/migration/tests), `59ced660` (native refusal cases and custody), and `ef0f0785` (dependency attribution only). Subsequent evidence-only commits do not change compiled Rust. Integrator must compare all 16 owned paths and source pins, intake serially, supply the existing shared core's target settler and callback mount, run the relevant combined checks and publish normally. This worker retains required review fixes; no technical approval/ACK gate is introduced. The original persistent goal remains unfinished.
