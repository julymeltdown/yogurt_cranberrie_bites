# BE15 bounded command receipts and outbox

Status: IN_PROGRESS; bounded implementation is ready for parent code review. Internal foundation only; no public producer, canonical identity
adapter, HTTP receipt endpoint or runtime worker is composed. This does not accept BE15
or freeze C04. Parent integration and concrete review fixes remain required.

Assigned worktree: `/Users/lhs/Desktop/mcr-worktrees/be-commands`.
Branch: `codex/mcr-be-BE15`. Assignment base: `376cf21`.
Command/outbox logic is newly authored; no legacy outbox implementation copied. Read-only helper
reuse is `backend/scripts/mcr/migration_smoke.py` and `concert_core_smoke.py` at that base.
The `owned_pool` test helper is copied/adapted from BE05
`backend/services/concert-service/tests/postgres.rs` (source SHA256
`7d8d1400be9d436c00cd34f9c735c6071b521c7902babbe8b5421a737801461c`)
into `tests/command_receipts.rs`; it restricts the database to
`mcr_concert_be_commands` and preserves marker/inode/socket validation. Helper source hashes:
`migration_smoke.py` = `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`;
`concert_core_smoke.py` = `967f206689b036ec2b9619079508f3f790c20f87d9777180ecfb0a82e4e62c09`.
Retained dependencies: async-trait, chrono, SQLx, Tokio, UUID and SHA2; no upgrades.

## Candidate contract inputs

C04 domain v0.1 is REVIEW_CANDIDATE, not a full freeze. SHA256:

| File under contracts/domain/v1 | SHA256 |
| --- | --- |
| common.schema.json | 29e89618ae0258f8793407ab67a26b88ccd4383f513ea1869b416b0739c3ee4f |
| operations.json | 0acd5e1c68b5136916d1560dfbb2eef7e59410c81987bf0ea24b0d605c11083e |
| policy.json | 58d136b8f067848bd724d2870d6596bddff82e7b8f61bc2dbd75e47d7392a66d |

The 59 registered command IDs are a pinned internal candidate registry, not new HTTP
or proto authority. C03/C05 stay unchanged. Application values are internal seams,
not independently serialized public response DTOs. Public receipt has PENDING,
SUCCEEDED, FAILED and RETIRED; expired retention is an internal conflict outcome,
not another invented wire state. Public adapter mapping remains NOT_RUN.

## Design and composition request

Application exports `commands`; normal UUID/SHA2 dependency edges required. Service
exports `adapters::out::repository::{receipt,outbox}` and `workers::outbox`. These
central edits belong to the integrator. No automatic startup is requested.

Scope is canonical actor UUID, event UUID, registered operation and key UUID. New
commands carry a canonical target UUID. The producer resolves identity; a Scope is
correlation only. Canonical digest binds registry method, constructed canonical path,
sorted decoded validated query values/defaults and canonical JSON (or exact binary length/SHA256 for the registered binary command). Length framing
prevents concatenation ambiguity; closed JSON algebra rejects duplicate object keys,
unsafe integers, floats, oversized collections/depth/body. Transport parsing must reject
duplicate JSON keys before constructing this algebra, enforce each operation schema,
and validate query names/types/defaults. ABSENT, JSON null, JSON primitive types and
array order are distinct; object/query ordering is canonical. Binary and JSON body
entrypoints cannot be exchanged; the binary helper has an internal 10 MiB hard ceiling,
not a product upload-size entitlement. The upload producer must enforce its exact schema. Secret inputs exist only transiently for hashing;
no Debug implementation on input/hash. No generic request/result JSON storage exists.

The target UUID identifies the created/changed resource, including a server-chosen
new-resource UUID when relevant. Result and invalidation UUIDs must equal it; SQL
constraints bind outbox event/resource to its receipt as well. Parent-resource and
cross-resource invalidation protocols require a later reviewed extension.

One SQLx transaction reserves the receipt, applies local aggregate guards/mutation,
records UUID/revision safe result and enqueues metadata-only invalidation. A current
transactional producer authorizes the stored target for replay/status and the proposed
target for new writes. It must lock/fence policy until commit, fail closed on authority
outage, and enforce actor/event/operation/target/role/revision and feature semantics.
No policy implementation is supplied for production; test authority is explicitly local
synthetic test data. Err rolls back all effects. A deliberate Failed completion rolls
back aggregate/outbox effects to a savepoint and retains only a safe failure reason.
Pending does not fabricate success or replay automatically. Future async completion
needs a separately reviewed owning-service protocol; no generic completion endpoint.

A global DB admission lock serializes new commands and duplicates. Identical hashes
return only currently authorized receipts. Changed input conflicts; expired keys never
rerun. TTL is exactly 86400 seconds by DB time. Bounded retirement clears result fields,
retains scope/hash/target tombstones, and does not delete/reuse keys. There is no
unreviewed cleanup or automatic replenishment: global lifetime cap is 1,000,000 receipts,
per actor/event cap 1000, active outbox cap 10,000. Exhaustion rolls back the whole
command. These conservative internal limits need product/operations review before use.
The global lock favors correctness over throughput; no load qualification is claimed.

Outbox has one invalidation per receipt and only UUIDs, revision and fixed kind. Claim
uses DB time, SKIP LOCKED, <=100 entries, fixed 30-second leases and <=8 attempts.
Each claim rotates a fencing token. Expired leases recover after crashes; stale owners
cannot acknowledge or reschedule. Retry backoff is bounded (2..128 seconds); rejection
and exhausted attempts become DEAD, never DELIVERED. Terminal rows retain diagnostics
for seven days, then bounded pruning removes <=1000 per call, preserving receipts.
The explicit batch runner limits delivery to ten seconds and aborts owned tasks on
cancellation. No loop/startup is composed. SQL pool/statement timeouts remain composition
obligations; stale leases fail acknowledgement safely.

Delivery is at least once. Crash after external effect before acknowledgement causes
redelivery. Downstream ports must atomically deduplicate by outbox UUID with their local
effect and reauthorize any private projection. They must never cache or forward private
raw results, handles, tokens, age proof or pickup codes. No external exactly-once claim.
No distributed transaction or unsafe replay of an owning-service side effect is supplied.

Migration 0002 is forward-only: apply/reapply/verify use SQLx receipts. On failure,
transactional DDL rolls back. Production rollback means reviewed forward repair; no
live schema/data deletion script is provided. Owned test clusters are disposable.

## Validation and handoff

Validation uses Rust **1.88.0**, jobs **2**, retained lockfile and PostgreSQL **16.14**
in a new private Unix-socket cluster; no TCP listener, imported data or global service
manipulation. Build/evidence root: `/Users/lhs/Desktop/mcr-build/be15`.

| Command / check | Exit / evidence |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0; `plan-check.log` (planning only) |
| `cargo +1.88.0 test --locked --offline -p concert-domain -p concert-application --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/be15/target -j 2` | 0; `core-regression-final.log`: 15 tests (5 domain, 4 existing application, 6 commands) |
| `cargo +1.88.0 test --locked --offline -p concert-service --test config --test health --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/be15/target -j 2` | 0; `service-regression.log`: 3 tests |
| Scoped `concert-service --test command_receipts --no-run --message-format=json`, same locked/offline/toolchain/target/jobs arguments | 0; `postgres-build-final.jsonl`, `postgres-build-final.log` |
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_command_receipts_smoke.py` | 0; `python-test-final.log`: 6 tests |
| `rustfmt +1.88.0 --check --edition 2021` on all seven owned Rust files | 0 |
| `python3 -B backend/scripts/mcr/command_receipts_smoke.py --binary /Users/lhs/Desktop/mcr-build/be15/target/debug/concert-service --repository-test /Users/lhs/Desktop/mcr-build/be15/target/debug/deps/command_receipts-830a8bea0365f0ac --pg-bin /opt/homebrew/opt/postgresql@16/bin --report /Users/lhs/Desktop/mcr-build/be15/native-final.json` | Final clean committed candidate owned PG receipt; source/binary/helper/contract hashes and cleanup |

Thirteen real PG tests cover simultaneous same/different bodies, rollback without
orphans, deliberate Failed rollback, Pending distinction, wrong actor/event/operation/
target, current and concurrent role revocation, expiry/tombstones, changed-hash conflict,
lease expiry/reclaim and stale completion, durable deduplicated consumer effect after
lost acknowledgement, explicit runner, bounded backoff/exhaustion/pruning, actor/global/
active-queue admission rollback, safe reference binding and closed secret-free storage.
The lifecycle harness applies/reapplies/verifies both migrations, induces late migration
failure and proves no partial 0002 DDL/success receipt, forward-recovers, rejects damaged
checksums, restarts the actual server and proves receipt/outbox persistence, then
removes only the owned cluster after marker/inode/postmaster validation. Expected
negative CLI exits are 1 and recorded separately from successful checks (0).

Initial diagnostic attempts are retained: `native-01` refused an incorrectly specified
test executable before creating a cluster; `native-02` found an incorrect test-only
registered operation name. Both cleanup results PASS; later receipts contain the fix.
`native-03` (9 tests) and `native-04` (12 tests) were intermediate PASS candidates.
`native-05` (13 tests) also PASS. No failed receipt is substituted for final candidate evidence.

Integrator prerequisite: original `f74fb660d886e5c479f6b275ef7038c21b9f280b`,
cherry-picked here as `7ed63b434ea3f9d1dd7431705420be0696067478`. Its five
central files are integrator-owned and remain separate from the worker implementation
commit. There are no frontend imports/writes, shared contract/router/manifest writes
in the worker patch, no automatic runtime startup and no push. Final worker SHA and
clean-candidate evidence are sent to parent for serial review/integration. Remote master
ancestry remains NOT_RUN in this lane; only the integrator publishes it. Public
producer tests, real Auth policy changes, service routing, multiuser flows and release
acceptance are NOT_RUN. A test-only local authority cannot certify those obligations.
