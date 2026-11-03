# C05 Auth invalidation RPC

Status: **IN_PROGRESS**. Source implementation is authored; compilation and
native execution are **NOT_RUN**, awaiting an explicitly released build/PG slot.
The persistent C05-AUTH-INVALIDATION-RPC goal remains active through validation,
committed handoff and concrete fixes. This is not combined World acceptance.

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

## Qualification plan and limits

Source checkpoint precedes Cargo. After an explicit exclusive lease: Rust1.88,
locked/offline, jobs<=2, dev/test debug0, incremental0, >=8GiB free, assigned
existing target only. Freeze exact Cargo JSON-selected Auth binary, RPC test,
relationship test and unit executables with clean source hashes outside targets;
close target handles before private PG. No customer database or third target.

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

Rust compilation, native RPC, existing regressions and process cleanup are
**NOT_RUN** at this checkpoint. Source/format/Python safety checks are recorded
with the source handoff. The planned gate compiles Auth startup composition; the
entire production binary is not started with provider workers. Private transport
uses the exact retained registration function and TLS configuration. Combined
World invalidation, real two-user removal, send races, deployment and release
qualification remain **NOT_RUN** even after an owner-local transport pass.

Source-only checks: two new Python runner safety tests passed (exit0);
Rust1.88 rustfmt check on the new adapter/test/support passed (exit0), and
`git diff --check 68daf583` passed. The boundary command delegates only the
recognized central composition path `backend/services/auth-service/src/main.rs`;
the assigned gRPC module export is an ordinary backend path checked manually.
An initial invocation incorrectly listed that export as a composition path and
the checker refused the invocation before checking files; no permission or
ownership changed. The corrected boundary command is recorded at source handoff.
