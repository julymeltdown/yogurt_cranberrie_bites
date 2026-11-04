# Social production startup composition

Status: **INTEGRATED** for the compiled startup composition and its scoped component checks. This source increment connects the qualified Social producer and SQL policy sink to the existing Product and World owners. It does not certify public HTTP, real age verification, multiuser consent/reveal, or IN03. The scoped qualification is recorded in `social-main-build.json`; full product acceptance remains open.

## Source and ownership

Root owns `src/main.rs`, `src/config/{mod,social_runtime,social_startup,social_policy_runtime,world_runtime}.rs`, `src/adapters/in/world_ws/policy.rs` connection-coverage check, the actual event accessor in `src/world/invalidation/mod.rs`, `tests/social_startup.rs`, and the narrow World socket harness extension in `tests/world_ws.rs` under `backend/services/concert-service/`. The existing Social invalidation kernel remains the delivered twelve-commit series ending at `c4895848e9076b419467b76010410e953d793980`, integrated at `cb944f7210f6c55fdf1d0e897c96ea7ef0ba8325`. Its isolated SQL/provider qualification is recorded separately; it is not production startup evidence.

The related SOCIAL-DELIVERY-01 owner holds exactly the new `src/config/social_delivery.rs`, `tests/social_delivery.rs`, and `docs/mcr/backend/social-delivery-production.md`. Root exports and mounts that committed implementation. The original Social nine-path handoff and all migrations remain unchanged. OPS-PRODUCT-01 returned its separate nine-path lease after scoped qualification; its Report admission and producer were independently published through `e966739b`. This startup increment does not activate Report or modify its business logic.

All new composition is authored for MCR. Existing source adapters are called through their public interfaces; no old rendering or second identity implementation is copied. No manifest, lockfile, schema, SQL, generated contract or frontend path is changed.

## Explicit configuration

`CONCERT_SOCIAL_MODE=off` (also the default) ignores all optional Social inputs. `live` requires the same guarded shared Intro/Participant core, the existing World listener, live Notification inbox plus its actual target authority, and actual parsed mTLS material. These checks run before database connection/listener binding.

Live mode requires these private, operator-supplied inputs; no production defaults are generated:

- `CONCERT_SOCIAL_HANDLE_KEYS_JSON` and `CONCERT_SOCIAL_COMMAND_KEYS_JSON`: closed JSON arrays with `id` and `secret_hex`; first version active, at most 31 retained versions. Existing managed-key constructors validate identifiers and duplicate versions.
- `CONCERT_SOCIAL_CURSOR_KEY_HEX`: independent 32-byte, nonzero lowercase hexadecimal material. Every Social active/retained handle, command and cursor secret must be distinct.
- `CONCERT_SOCIAL_REQUEST_TTL_SECONDS`: canonical integer seconds, 1 through 86400.
- Existing `CONCERT_NOTIFICATION_GRPC_ENDPOINT` and `CONCERT_SERVICE_CREDENTIAL`: the same explicit HTTPS Notification endpoint and Concert workload credential as the canonical owner composition. URL credentials, missing ports, paths, fragments and queries are refused. The actual `NotificationDeliveryClient` uses the existing TLS identity.
- Optional `CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER` and `CONCERT_SOCIAL_ACCEPTED_CLAIM_REVISION`: supplied together as the immutable accepted provenance snapshot. Absence uses the existing unavailable provenance implementation. These settings do not issue, verify or seed any claim. Configuring accepted persisted provenance does not provide a real verification-start adapter.

The real verification provider/protocol/private configuration remain unconfigured. Sharing requiring verified claims remains unavailable without genuine qualifying persisted claims. Unrestricted Intro discovery and ordinary World movement retain their independent rules.

## One canonical composition and one socket

`SocialRuntimeConfig::compose_shared` attaches only `Operation::Social` dispatch over the exact existing canonical fence and delivery publisher. Other Product operations delegate to the already composed owner. The existing canonical core retains its Social read-child settler and native resolver; this path creates no replacement registry, ledger, scope acquisition or listener.

`SocialWorldPolicy` creates one `SocketTransport` and takes its registry-only publisher before attaching the actual Social SQL baseline adapter. This order avoids an Arc ownership cycle. A real held SQL generation guard survives the short local stream installation and is consumed by its real commit. The already qualified World transport requires the matching snapshot handoff and successful guard commit before emitting a baseline. An unavailable database or failed commit never grants a private baseline; ordinary World movement remains independent.

Before acknowledging a publication, the sealed bridge reads actual active durable World membership connection IDs for that actor/event and requires coverage by this incumbent transport. A foreign active connection returns retryable rather than treating an empty local registry as complete. New/reconnecting streams read the already committed generation through the actual held baseline.

Only `VerifiedSocialInvalidation`, constructed inside the locked source-verifying Social sink, reaches the World metadata publisher. The bridge maps the closed five reasons exactly and forwards the actual actor/event/revision internally. Socket frames retain only the public metadata schema; no actor ID, signal/receipt/outbox ID, handle, body or credential is added.

The World startup method receives that same configured transport. After native `handle.ready`, journal catch-up and the original authority startup check, it sends the actual local `WorldInvalidationConsumer` for the same admission bridge to the delivery worker, then opens the existing gRPC startup barrier. There is no second socket, Auth owner, listener or native world.

The Social delivery worker is the sixth existing shutdown-supervised component. Its actual source contract waits for the World consumer before claiming work. Closed supported source families require their real sealed Social leg and, where applicable, actual Notification or local World application before acknowledgement. Block selection, exhaustion and delivery classification are bound to the actual resolved World consumer event; foreign-event blocks retain their state and attempts. Other families remain untouched. Exact claim/fanout/deadline behavior and its qualification are owned by SOCIAL-DELIVERY-01; queue acceptance alone cannot certify fanout.

## Validation boundaries

The planning provenance checker and scoped rustfmt/diff checks are separate from Rust compilation and runtime validation. The new opt-in tests cover missing prerequisites/TLS, key aliases and rotation, trust pairs, TTL bounds, and retargetable delivery endpoints. The socket extension uses the actual SQL baseline adapter against a deliberately unavailable test database to check that basic World movement survives without a private baseline. This is a failure-boundary component test, not positive SQL/Auth/sharing evidence.

Pending: current combined Rust/main build, copied component checks under a fresh allocated resource slot, actual production main with the complete migration set, acquired canonical Auth/Friend scopes, public Gateway original requests, consent/reveal/revoke, block-triggered private withdrawal, deadline expiry, shutdown/restart and full IN03. Earlier component EXEs and QA05's retained World cohort do not certify this changed main or the newly composed feature stack.

## First combined qualification and retained fixes

The first allocated combined check accepted clean `e7d1f06bbb2a335f4b0c99d8d4cb2f94f40a0d8a` at 2026-11-04 13:30:53.003014 UTC. Its Rust compilation failed with E0308 in Root-owned World startup: the already shared `Arc<AdmissionBridge>` had been wrapped again. Fix `309b407320d50f74c8854f479a5bf000e80ab0fb` passed the same four locked/offline Rust 1.88 build commands without resetting the original clock or target-growth baseline. The failed Cargo JSON, full pinned source and acceptance receipt remain retained separately.

The corrected candidate froze 3,719 SHA256-pinned inputs and four Cargo-selected executables (three tests and the actual non-test main). The target was unconditionally released before copied execution, with zero handles and absent compiler group; cumulative target growth was 27,271,168 bytes. Copied `social_startup` then passed three cases and failed its endpoint-rejection case. Other copied components and actual-main startup refusals were **NOT_RUN** because the controller stopped at this failure. The full workflow released at 13:33:04.716938 UTC after 131.714 seconds, only 2.394 seconds after target release. Owned groups, PKI and temporary directories were absent; the conservative whole charge was 4,673,658,880 bytes. No PostgreSQL was launched.

Fix `389d8d48` rejects a raw fragment delimiter before HTTP URI normalization, adds a bare-fragment negative case and labels any unexpectedly accepted endpoint. This source fix requires a fresh allocated qualification; the earlier successful compilation does not qualify this changed source. All first-attempt evidence remains retained under the Root-owned private `mcr-root-social-main-j3vg0v_d` receipt directory.

## Corrected candidate qualification

Fresh offer `ROOT-SOCIAL-MAIN-BACKEND-202611041339Z` accepted clean `e1531f40bbc5c10b13bc3ad8074aa9f5c4ef52bb` at 13:39:54.578019 UTC. The four exact locked/offline Rust 1.88 commands passed: `social_startup`, `social_runtime_config`, `world_ws` with `--no-run`, then the actual non-test `concert-service` build. Three thousand seven hundred nineteen inputs were SHA256-pinned and the complete read set was frozen outside the target; public image files were pinned before/after and were not inputs to these component checks. Four copied binaries totaled less than 128 MiB.

After unconditional zero-handle target release, copied tests passed: Social startup 4/4, Social runtime configuration 5/5, and World socket 19/19 with four PostgreSQL-dependent cases explicitly ignored. The new socket case uses the actual Social SQL baseline adapter against an unavailable test database; it confirms snapshot and movement without a private baseline. All five actual-main startup refusals exited 1 with their expected configuration errors before database connection. These are expected refusals, not failed startup acceptance attempts.

The copied controller exited 0 in 11.866 seconds. Full release at 13:41:24.100314 UTC took 89.522 seconds from acceptance and 12.825 seconds from target release. The whole conservative charge was 4,830,060,544 bytes including all retained earlier attempts; fresh final free space was 30,828,404,736 bytes. No PostgreSQL was launched, owned groups and private PKI/TMP were absent, and target access ended permanently at its release receipt. `social-main-build.json` records commands, binary hashes, exact counts, receipt hashes and limits.

Still **NOT_RUN**: the changed production main against the complete live seven-service stack, public Gateway Social/Intro/Feed/Inbox requests and responses, positive policy fanout, actual multiuser block/private-cache withdrawal, production verifier and full IN03. Independently qualified Social delivery and Report adapters retain their narrower evidence. This qualification does not activate Report or supply missing age/operator/retention providers.
