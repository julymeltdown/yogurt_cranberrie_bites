# C05 Auth invalidation RPC

Owner-local RPC: **CONTRACT_READY**. Implementation, startup composition and
native qualification are complete; delivery ancestry is verified in the final
handoff. Parent C05 remains **IN_PROGRESS**. Combined World and deployed-service
acceptance are **NOT_RUN**.

## Assignment and provenance

Isolated branch `codex/mcr-be-auth-invalidation-rpc`, base
`68daf58309241d00cefeaf6b9bc0483a5708dc9a`. Write ownership is exactly the new
gRPC adapter, its export, adjacent guarded construction/registration in Auth
`main.rs`, new RPC test/support files, new runner/safety tests and this document.
The qualified Auth journal, migrations, platform guards, canonical protobuf,
domain/application, manifests and locks are read-only and retained unchanged.

Requirements are the implementation specification, project assessment/execution
plan, C05 realtime authorization rules, and
[`world-invalidation-contract.md`](../world-invalidation-contract.md) revision 1
at `9204e1e57524badddfc7d16431ad44dcd290e967`. The existing imported port-rust
foundation is `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`.

The new adapter reuses the single `concert_relationship::proto` generated module
and the prior `PgInvalidationJournal`; it does not copy producer logic or types.
`grpc_method_policy::guard_without_delegation`, `CallerRegistry`, `TlsMaterial`
and `CallDeadlineLayer` remain the retained authentication/transport mechanisms.
Dependencies are existing SQLx/Tonic/Prost/Tokio/UUID/platform; no install or new
manifest resolution. The native runner follows the previously qualified
`auth_invalidation_smoke.py` frozen-executable guard and `migration_smoke.Cluster`
ownership/restart/cleanup implementation in place, without modifying them.

## Security and bounded operations

Only verified `Workload::Concert` may invoke either explicit method. The policy
has no actor fields and does not use delegation; the handler independently
checks `VerifiedCaller`. A forged consumer UUID, public JWT or actor proof is
not authentication. TLS binds the workload certificate to the same registry
credential. There is no deleted/suspended person's JWT prerequisite.

Startup adds only adjacent construction and optional registration. The service
is omitted without retained TLS material; no plaintext/development replay route
is supplied. Existing Auth already accepts optional Concert credentials from
`CONCERT_SERVICE_CREDENTIAL` (and the retained previous-credential rotation
mechanism). Missing/mismatched credentials fail closed. No central credential
registry, new configuration flag, actor `ReadCurrent` or platform policy change
is needed. Production operations still must provision legitimate credentials
and trusted certificates; test-generated PKI is not deployed configuration.

One ingress operation stop begins before credential/method guard and generated
decoding. It is at most one second and only shortens an inherited/propagated
deadline. The same absolute monotonic stop encloses pool readiness, journal SQL,
mapping, response validation and handler return; no retry or new operation
budget exists. The retained journal's SQL timer rounds up at most one millisecond
to avoid its floor masking expiry as Unavailable; outer absolute timers still
return by the original stop. On HTTP/2 cancellation the future and read-only
transaction drop. SQLx queues rollback; an in-flight read retains its bounded
original statement timeout rather than promising instantaneous server abortion.
The World client must separately enclose channel readiness and response
validation in its same original deadline; a receiving server cannot account for
time already spent before arrival.

Requests are capped at 4096 decoded bytes. All identifiers must be canonical,
nonzero UUIDs; cursor is 0..9007199254740991, and explicit page limit is 1..256.
Malformed structure is INVALID_ARGUMENT; changed epoch/future committed position
is FAILED_PRECONDITION. Missing storage is UNAVAILABLE, deadline expiry is
DEADLINE_EXCEEDED, workload mismatch is PERMISSION_DENIED and missing/forged
credential is UNAUTHENTICATED. A cancelled transport has no success response.
Errors are fixed/redacted and contain no raw SQL, identifiers or private data.

Responses echo the exact complete query and use no-store. ReadAfter takes one
consistent journal snapshot, preserves the last returned `next` separately from
the committed head, validates contiguous sequences, stable unique signal IDs,
Auth-only closed reasons and exclusive user/family scope, and caps the encoded
message at 128 KiB. No grants, expiry extension, acknowledgments or pruning are
introduced. Bootstrap/application/checkpoint obligations remain with World.

## Qualification and delivery evidence

Source checkpoint `d901b13309095e1886bc3e9827b6bf6028eaa6c1` preceded Cargo.
The exclusive assigned MEDIA lease used Rust 1.88, locked/offline, jobs 2,
dev/test debug 0 and incremental 0. Fresh free-space and handle checks preceded
builds; when capacity fell below 8 GiB the rebuild was refused before Cargo and
resumed only after coordinated space recovery. Only the assigned existing target
was used; no third target, install or customer database was involved.

The runner requires an exact frozen artifact inventory, checks clean source and
digests before starting, uses only a fresh marker/inode-owned socket-only PG16
cluster, exercises actual retained mutual-TLS transport and guards, validates
all migration receipts, physically restarts PostgreSQL and removes its own root.
Its tests cover owning session store logout/family revocation, credential update,
owning suspension and physical deletion SQL, rollback with no signal, replay
past256, stable duplicate/restart replay, public/Gateway/sibling/forged caller
denials, malformed/truncated/oversized frames, missing storage, cancellation,
shared pool-plus-SQL deadline, and the original one-second cap. Retained actor
relationship guard/native tests and Auth unit tests remain separate regressions.

The first build found the retained interceptor's `ResponseBody` type mismatch;
`d46e19cc` normalized its body in the operation wrapper. Compilation then passed.
The first native run found a test comparing full responses containing fresh
query IDs; `9411c5e93c1d4304754c1cb90132a0e3c97b197e` corrects that assertion to
compare stable signals/positions while separately verifying exact query echo on
each call. The final clean build at that commit took4.311s (exit0), followed by
the complete fresh native gate (exit 0). Earlier failed reports are retained.

Evidence root: `/private/tmp/mcr-auth-invalidation-rpc-_7aenthl` (local artifacts,
not published CI). `binaries-3.json` records the clean source checkpoint, all
source/contract inputs, four exact Cargo JSON-selected executables and digests.
The binaries were frozen outside the target; fresh lsof found no target handles
before private PG. The regular Auth executable was emitted by the test build,
so no separate binary build was necessary.

| Command / gate | Exit / result |
| --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p auth-service --test concert_invalidation_rpc --test concert_relationship --test unit --no-run --message-format=json` | 0; `build-3.jsonl`, `build-result-3.json`; Auth startup binary and all three test executables compiled |
| `python3 backend/scripts/mcr/auth_invalidation_rpc_smoke.py --binaries-json /private/tmp/mcr-auth-invalidation-rpc-_7aenthl/binaries-3.json --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /private/tmp/mcr-auth-invalidation-rpc-_7aenthl/native-report-3.json` | 0; PASS; 134 Rust tests: 4 pure RPC, 3 native RPC, 2 relationship guards, 1 native relationship, 124 Auth unit |
| `python3 -m unittest discover -s backend/scripts/mcr -p test_auth_invalidation_rpc_smoke.py -v` | 0; 2 safety tests passed |
| `rustup run 1.88.0 rustfmt --check --edition 2021 backend/services/auth-service/src/adapters/in/grpc/concert_invalidation.rs backend/services/auth-service/tests/concert_invalidation_rpc.rs backend/services/auth-service/tests/concert_invalidation_rpc_support/native.rs` | 0 |
| `python3 scripts/mcr/check-plan.py` | 0; PASS, planning provenance only |
| `python3 scripts/mcr/check-boundaries.py --base af7310c8c742416171ef66b2a02fc23e9ee0c138 --lane backend --allow-composition backend/services/auth-service/src/main.rs --allow-doc docs/mcr/backend/auth-invalidation-rpc.md --allow-harness backend/scripts/mcr/auth_invalidation_rpc_smoke.py --allow-harness backend/scripts/mcr/test_auth_invalidation_rpc_smoke.py` | 0; exactly 8 assigned paths; gRPC module export/imports manually inspected |
| `git diff --check af7310c8c742416171ef66b2a02fc23e9ee0c138` | 0 |

The native report records PostgreSQL 16.14, all 16 exact migration receipts in
both private databases, 313 committed contiguous records, actual postmaster
restart PID 31155 to 34219, byte-identical replay, and owned-root cleanup.
Both failed and passing run roots were removed. `qualification-audit.json`
checks source pins, frozen executable digests, absent roots, and no matching
process or open MEDIA/frozen-artifact handle. The standing MEDIA lease was
unconditionally released after qualification; no further Cargo is authorized
without a new lease.

| Artifact | SHA256 |
| --- | --- |
| `native-report-3.json` | `e01a5a2bcd67052b3d0884634db3aa3a2d31a0f09a4e1bb055c86883bc73e98e` |
| `binaries-3.json` | `605b5b5132002f5d833acd4c2d8d0da84d827b035ead8b91c15e2186d4d3b00c` |
| `build-3.jsonl` | `2988645b6bef4421bbe8d6651d460a895dd929af76b146412e11e591011e617b` |
| RPC test executable | `aa97ec290c305c365b825db6e4b1d6d6436a565485f2079da0a1ace452ab654c` |
| Regular Auth executable | `3cd1433836476891c3cc3dd55dd2cd6871a545a03bebd7f7adb546ec8d4b270d` |

Current remote master was fetched/reconciled serially at `d8f9f9ac` and
`af7310c8c742416171ef66b2a02fc23e9ee0c138`; both merges were conflict-free.
The qualified source/contract input hashes remain byte-identical after peer
integration. Only the exact 8 assigned Auth RPC paths differ from the reconciled
remote; producer migrations, journal, platform, shared protobuf, manifests/locks
and `docs/source/` remain unchanged by this work. The final evidence commit
changes this document only. Latest human policy authorizes direct non-force
publication after qualification; the final handoff records delivered SHA and
verified remote ancestry. No additional king/SL approval is required.

The entire production binary was not started with provider workers; private
transport used the exact retained registration function and TLS configuration.
Combined World invalidation, real two-user removal, send races, deployment and
release qualification remain **NOT_RUN**. All concrete qualification fixes are
resolved; deployment credentials and consumer wiring remain separate ownership.
