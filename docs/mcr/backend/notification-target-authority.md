# BE33 Notification target authority

Status: `IN_PROGRESS`. Source implementation and internal qualification cases are authored; Cargo, database and mTLS executions are `NOT_RUN` pending fresh named resource allocation. This document does not accept BE33, IN03 or a public Gateway journey.

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
| `cargo test -p concert-service --test notification_target_authority` | `NOT_RUN` |
| `cargo test -p notification-service --test concert_target_client` | `NOT_RUN` |
| Ignored owned PG16/mTLS lifetime/recovery case | `NOT_RUN` |
| Production main mount / combined Root and Notification startup | `NOT_RUN`, integrator-owned composition |
| Public Gateway flow / complete BE33 / IN03 | `NOT_RUN` |

The ignored native case exercises a real empty inbox callback and source-lock retention through a refused Notification completion, finished Root, original deadline expiry and subsequent system recovery. Six-kind native positive callbacks, nonempty masks/counts, Post/Report owners, broader adversarial callback replay and the final public vertical flow remain required qualification/acceptance work; local policy unit/component evidence does not certify these flows. All executed commands and delivered ancestry must be appended after qualification, without replacing `NOT_RUN` with inferred success.


## Bounded resource prerequisite

Before requesting compilation, owner207 retired only its three completed clean STOPPED old CF01/BE15-QA09/CR07 source worktrees under the explicit sustainable-headroom assignment. All retained branches/history, tracked blobs/modes and 15 source-original entries per checkout were verified. Unique ignored files and complete per-worktree Git administration, numeric custody, ACLs and xattrs were privately archived and independently extracted/verified before normal non-force removal. Root supplied a factual no-active/frozen-old-path-consumer statement. No target, dependency, other checkout, active provider source, original proof archive or selected artwork was removed.

Private custody receipt basename: `mcr-207-retirement-hrtkjjvw`; manifest SHA256 `c3c4d2302e22365872a08898f7a12d6e57e363d7652d6c3a8df1491db786d641`. It records exact command attempts/exits, all three archive hashes and original baked-path reconstruction instructions. Observed shared-volume free space was 17,204,813,824 bytes before and 20,440,555,520 after (net +3,235,741,696; concurrent volume activity prevents exclusive attribution). Resource recovery is not compilation/native evidence and does not itself authorize a build: a fresh named allocation and fresh floor checks remain required.


## Notification startup owner report

The separate delivery/startup owner reported 11 passing checks against this assignment's frozen `50a50033` wire/client: four routing/client cases, three mTLS ingress cases and four runtime cases, including explicit live-mode mounting and graceful Notification restart. The agreed `RootTargetClient::connect` constructor, module export and both service build integrations reportedly compiled without changes to the owned dependency. That owner's resource allocation is fully released and its normal publication handoff is pending.

This is attributed owner-reported dependency evidence, awaiting that owner's final committed qualification receipt and publication readback. It does not upgrade this provider's unexecuted tests, establish a positive Root target decision, qualify retained target native policy or accept BE33/IN03. Actual combined Root composition remains a separate obligation.
