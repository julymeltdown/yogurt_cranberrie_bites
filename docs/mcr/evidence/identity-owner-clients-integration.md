# BE04 Profile owner and Gateway Auth client integration

This reviewed increment adds a minimal Profile owner read and the signed Gateway Auth owner client. It leaves BE04 IN_PROGRESS; it does not accept a new task or activate a public route. The Profile self RPC and its Gateway client are being developed separately against a frozen internal interface.

## Delivered behavior and review

Profile worker `81665a04b579d8709fa6cbe018c46b01db4edce0`, base `3fdb977`, is integrated byte-identically as `92fb4ef`. The parent reviewed every one of its seven paths, two exact module exports, source ancestry, consumer validation and PostgreSQL harness. Its single parameterized statement projects only user UUID, nickname, bio and statement observation, excluding soft deletion and the owning tombstone. It is uncached, nonserialized content, not authentication or admission. Private self content can be read only through a separately guarded producer once that producer is qualified.

Parent review caught a transfer-bound defect before this handoff: unrestricted TEXT could reach SQLx before application length rejection. The final SQL rejects nickname above 160 bytes/40 scalars and bio above 640 bytes/160 scalars before transfer, without truncation. Real repository-boundary tests seed independent 2 MiB fields, excessive ASCII and four-byte text; no record escapes and stored sizes remain unchanged. Valid four-byte maxima return exactly. PostgreSQL may still decompress or inspect data to evaluate those predicates; this is not a database CPU/IO bound.

Auth client worker `868d02c` plus correction `77890b9acc0edf90a7a9f253eade6a7c0816587d` is integrated as `7106e76` / `9dceb49`. All four cumulative file hashes match. It borrows the actually verified request context, uses the configured service identity/tracing/actor signer and generated channel, bounds response decoding to 128 bytes, validates the exact optional provider-binding shape, and returns private context-checked observations. Negative account/session/binding facts remain explicit; no portable admission grant, retry, cache or broad identity DTO is introduced.

The parent found that the initial client bypassed the retained global GatewayAuth limiter and its metrics. The fix acquires the real shared permit before generated readiness/signing/dispatch and retains it through completion, error, timeout or cancellation. The same absolute stop remains inside the helper's accounting, including late completion. CapacityExhausted is a safe projection of local refusal or peer overload; its variant alone does not prove no dispatch.

Independent QA then found a cancellation test could consume a stale Notify permit and abort an unpolled task. The final process-isolated case uses a fresh harness and observes exactly one active owner call, one held permit and an unfinished client before abort. It verifies all 256 permits return, cleanup and subsequent reuse. Exact counters prove one saturation refusal, one peer deadline and one shorter outer deadline, with no cancellation increment. The child repeats the same test case and is not counted as additional coverage.

[Independent QA](../qa/gateway-auth-owner-client-review.md), worker reports `ad4e963` / `f0ed0a2`, is integrated as `7594ec7` / `6d91e61`. Both findings are resolved. The initial missed finding, false-positive test draft, compiler failures and inaccurate historical 253 count remain explicitly historical rather than being overwritten by the final result.

## Validation and durable evidence

On combined candidate `9dceb49`, parent sequential commands all exit 0 using Rust 1.88.0, locked/offline dependencies, two jobs, and BOTH dev/test debug information zero:

- Gateway: 252 top-level tests (17 admission transport, 13 strict context, 209 retained unit, 13 verification transport), six compile-fail doctests and binary build. The isolated capacity child executes the same case once more.
- Profile: 16 application tests, one compile-fail doctest, one architecture test and library check.

Profile's exact worker candidate additionally passes seven distinct real PostgreSQL tests and one successful recovery repeat: 25 distinct tests total when combined with its application/doc/architecture tests. Parent inspected the native harness and hash-verified its source, binary-bound receipt, all logs and cleanup; parent did not rerun PostgreSQL. Every backend input was unchanged from worker base `3fdb977` to preceding published `3c32eb6`; only execution-plan prose differed among the pinned inputs. The new Auth patch does not alter Profile or shared dependencies.

The PostgreSQL evidence uses an owned native 16.14 UTF-8 private Unix socket cluster, isolated MCR database, exact seven retained migrations, restricted SELECT-only reader, private-column/write refusal, coherent content/fence snapshots, actual stopped-server failure, recovery and exact cleanup. Direct SQL migration application is not service startup or SQLx migration-ledger proof. The client transport evidence uses the real generated client/guard with a controlled loopback owner; it is not live Auth database or production TLS evidence.

[Machine record](identity-owner-clients/result.json) pins delivered ancestry and source hashes. Byte-preserved parent logs, exact worker receipts, native Profile evidence and Auth correction history are adjacent. The worker's absolute local evidence paths are retained historical provenance; copied artifacts are resolved through this record's relative artifact map. Planning consistency and the exact 12-path backend boundary check pass. Parent reviewed module exports and cross-directory imports; there are no frontend, legacy art, source archive, manifest, lock or public contract changes.

Raw log whitespace is preserved to keep source hashes exact. The publication check records any raw-log-only whitespace diagnostics separately and requires clean code/document diffs. It does not waive source formatting or failing application checks.

Remaining work includes trusted HTTP deadline capture and key refresh/withdrawal across awaits, qualified real Profile RPC and consumer, current event participation, signed Concert admission, revocation/stream invalidation, real multiuser and release gates. No public accepting policy is enabled by these exports. The earlier unrelated Docker-only Auth privilege/schema fixture failure remains separate historical evidence.
