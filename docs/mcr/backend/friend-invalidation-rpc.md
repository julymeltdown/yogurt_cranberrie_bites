# C05 Friend invalidation guarded RPC

## Assignment and current state

Task `C05-FRIEND-INVALIDATION-RPC` adds the canonical generated
`relationship.v1.FriendConcertInvalidationService` to the published Friend
producer. Base: `d8f9f9ac0b2aa60c7c861a2701bb62c37f9a0b8c`; isolated branch:
`codex/mcr-be-friend-invalidation-rpc`. Contract: revision 1,
`docs/mcr/world-invalidation-contract.md`, published at `9204e1e5`, and
`backend/proto/relationship/v1/relationship.proto`.

Status: `IN_PROGRESS`. Source and nonnative harness checks are authored;
compilation/native qualification wait for an explicitly released build slot
and at least 8 GiB free disk. No native acceptance follows from this document.
The broader BE13/BE14 persistent goal remains unfinished and historically
`blocked`; it is reused manually without reset, token budget, or false completion.

Exact owned paths: new `src/adapters/in/grpc/concert_invalidation.rs`,
`src/adapters/in/grpc/mod.rs` module export only, `src/main.rs` guarded-service
construction/registration only, new `tests/concert_invalidation_rpc.rs`,
`tests/concert_invalidation_rpc_support/**`, new
`backend/scripts/mcr/{friend_invalidation_rpc_smoke,test_friend_invalidation_rpc_smoke}.py`,
and this document. No Auth, platform, shared proto, journal, migration, manifest,
lockfile, frontend, or infrastructure changes are included.

## Authentication and startup

Both `ReadHead` and `ReadAfter` use the retained credential registry and an
explicit Concert-only method policy. The handler independently requires
`VerifiedCaller(Concert)`. Unknown methods deny. Consumer/request UUIDs identify
a query and are never credentials. There are no actor fields, delegation verifier,
actor JWT, or deleted-user token requirements for these system-consumer reads.
Existing actor-authorized relationship observations remain unchanged.

Startup constructs the guarded service only when retained TLS material is
present, then registers it on the same retained mutually authenticated TLS
server. Missing TLS omits this endpoint; invalid TLS material fails the existing
server configuration rather than downgrading. The production configuration
already requires Concert's current credential; the unchanged registry binds TLS
leaf workload identity to the presented credential. The wrapper constructor is
also used by in-process denial tests; it does not create its own listener or
constitute permission to mount an unguarded/plaintext product endpoint.

## One operation budget and bounded replies

The service wrapper starts one absolute monotonic deadline before caller/method
guards and generated protobuf frame decoding. It caps the operation at one
second, shortened by either retained ingress timeout header or ambient deadline.
There is no retry or reset. The handler requires that original budget; direct
calls without it fail closed. A cancel-on-drop token connects handler lifetime
to the existing Friend reader, whose pool acquisition, read-only SQL, consistent
snapshot, validation, and completion all retain that stop. A dropped HTTP/2
request cannot put its pending SQL connection back into the pool; server-side
statement timeout remains bounded by the original operation.

Generated request decoding is limited to 4096 bytes and response encoding to
128 KiB. Existing Friend constructors require canonical nonzero UUIDs, safe
sequences and limits 1–256. Existing typed page validation requires an exact
contiguous prefix, closed Friend reason/scope, stable signal UUIDs, consistent
epoch/head and next cursor. The adapter verifies exact query echo again, checks
encoded length and remaining time, and marks success `cache-control: no-store`.
Wrong epoch/future position is `FAILED_PRECONDITION`; invalid input is
`INVALID_ARGUMENT`; cancellation, deadline, unavailable owner and access denials
remain corresponding failures rather than successful empty catch-up.

This increment exposes existing owner-local facts; it does not change business
mutators or journal allocation. An invalidation grants no access. Production
World client readiness/response validation, cursor persistence, resync and native
handoff fencing remain separately owned and unaccepted here.

## Provenance and dependencies

New Friend adapter wrapper shape is adapted from Auth RPC at `d46e19cc`,
including normalization of the retained interceptor response body. Pure guard
and TLS fixture structure derive from Auth `d901b133`:
`services/auth-service/{src/adapters/in/grpc/concert_invalidation.rs,tests/concert_invalidation_rpc.rs,tests/concert_invalidation_rpc_support/native.rs}`.
Destination paths are this assignment's new adapter and tests. Friend uses its
already qualified `concert_invalidation::PgJournal`/`ReadBudget` and closed typed
wire conversions; Auth's journal semantics, user mutators and family scopes are
not copied. Friend repository block, unblock and account-deletion ports are
invoked directly in native qualification. Existing TLS test support issues real
CA-signed workload certificates, with no product credential fallback.

All dependencies are retained: `tonic`, `prost`, `sqlx`, `tokio`, `platform`,
`axum`/HTTP types and existing Friend crates; test-only `test-support`, `tower`,
`http-body-util`, `tokio-stream`, `chrono`, `uuid` and `serde_json`. No dependency
or lockfile update is required. Original source archives remain byte-identical.
Frozen artifact metadata pins a clean source checkpoint, all relevant source
hashes, migration SHA-384 inputs and exact copied executable SHA-256 values.

## Qualification design and current limits

Five Python harness tests pass: relative/symlink metadata refusal, exact binary
inventory, dirty source refusal and owned cleanup on forced failure. These are
nonnative checks, not transport or database acceptance. Planning provenance
check passes separately. Rust formatting and source boundaries are checked
without accessing Cargo targets.

The native runner requires five copied executables outside Cargo targets and
matching clean-source metadata. It creates a new marker/inode-owned private
PostgreSQL 16.14 cluster, disables TCP, validates socket/database/user/system
identifier identity, and removes only its owned cluster after shutdown. TLS
listeners bind only loopback ephemeral ports and are explicitly shut down.
Private PKI files use the owned temporary directory. No live/foreign database,
provider URL or production credential is inherited.

Planned native evidence includes actual mTLS plus block/unblock/deletion through
the owner repository to retained journal to both generated RPCs; no-actor replay
after deletion; missing/public/sibling credentials, workload certificate/credential
forgery, malformed protobuf bodies, epoch/future rejection, stable redelivery,
full/partial pages, forced journal failure rolling back a real block, and restart
replay of byte-identical signal messages. Deadline tests retain a table blocker,
spend part of a 250 ms budget waiting for the pool, and prove the remainder is
shared with SQL. They observe an actual blocked backend before cancelling and
require its disappearance while keeping the blocker held, including absence of
idle transaction residue. A long caller header cannot extend the local cap.
Pure body-stall checks cover the pre-decode ingress deadline. Native client
helpers bound readiness, frame decoding and response validation in one outer
operation timeout; production World consumer validation remains separate.

Existing producer evidence is documented in `friend-invalidation.md` and is not
replaced by mocks. Retained Friend units, query-shape and relationship guards,
and unchanged native relationship/revision behavior will be exercised with the
RPC qualification. The pre-existing unblock/deletion advisory-lock inversion
remains separate work; no concurrency acceptance for that path is claimed.
Docker-backed contract cases, deployed credentials, production World serving,
consumer checkpoint/fence, real multiuser IN01 and release remain `NOT_RUN`.
Commands, exit codes, native report and publication ancestry will be appended
after qualification. Direct non-force publication is authorized within scope;
there is no mandatory king/SL per-commit review or publisher queue.
