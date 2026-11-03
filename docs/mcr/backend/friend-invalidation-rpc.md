# C05 Friend invalidation guarded RPC

## Assignment and current state

Task `C05-FRIEND-INVALIDATION-RPC` adds the canonical generated
`relationship.v1.FriendConcertInvalidationService` to the published Friend
producer. Base: `d8f9f9ac0b2aa60c7c861a2701bb62c37f9a0b8c`; isolated branch:
`codex/mcr-be-friend-invalidation-rpc`. Contract: revision 1,
`docs/mcr/world-invalidation-contract.md`, published at `9204e1e5`, and
`backend/proto/relationship/v1/relationship.proto`.

Status: `CONTRACT_READY` for the guarded owner RPC, with native component
qualification complete. Publication integrates the bounded owner scope; production
World acceptance remains separate. The broader BE13/BE14 persistent goal remains
unfinished and historically `blocked`; it is reused manually without reset, token
budget, or false completion.

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

Passing native evidence includes actual mTLS plus block/unblock/deletion through
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
and unchanged native relationship/revision behavior were exercised with the
RPC qualification. The pre-existing unblock/deletion advisory-lock inversion
remains separate work; no concurrency acceptance for that path is claimed.
Docker-backed contract cases, deployed credentials, production World serving,
consumer checkpoint/fence, real multiuser IN01 and release remain `NOT_RUN`.
Commands, exit codes and frozen native report follow below.
Direct non-force publication is authorized within scope;
there is no mandatory king/SL per-commit review or publisher queue.

## Validation receipt — 2026-11-04

Implementation ancestry: `1f83d4d5` → `c6ba6fc8` → `fd1a8c74`, directly on
assignment base `d8f9f9ac0b2aa60c7c861a2701bb62c37f9a0b8c`. The second checkpoint
pins large typed native futures on the heap; no alternate test stack is used.
The third addresses the specific cancellation-evidence review finding below.

- MEDIA was explicitly transferred after Auth's unconditional release. Fresh
  free disk was 13.0 GiB before the first build and 12.60 GiB before the final
  build; `lsof` returned exit 1/no target handles before both. No third target,
  new dependency, configuration change, or foreign process cleanup was used.
- Rust `+1.88.0`, `--locked --offline`, jobs 2, dev/test debug info 0 and
  `CARGO_INCREMENTAL=0`. Both focused compilation runs exited 0. Final build
  completed in 3.27 seconds with no warnings.
- Exact Cargo-JSON-selected service/RPC/relationship/journal/unit binaries and
  source SHA-256 manifests were copied outside the target. Zero target handles
  were verified before either private PostgreSQL run. No build overlapped PG.
- Native runner: **98 Rust tests passed**: 4 new pure RPC checks, 87 retained
  Friend units, 2 retained relationship guards, 1 journal shape regression,
  3 new native RPC scenarios and 1 unchanged native relationship/revision case.
  All exact native selections require `1 passed; 0 failed` and exit 0; ignored
  discovery is not counted. PostgreSQL 16.14 restart and owned cleanup PASS.
- `python3 -m unittest discover -s backend/scripts/mcr -p test_friend_invalidation_rpc_smoke.py -v`:
  5 passed, exit 0. `python3 scripts/mcr/check-plan.py`: exit 0, planning
  provenance only. Rust formatting, path/import/preimage boundaries and
  `git diff --check` passed.

Focused build (with authorized retained target and the environment above):

```sh
cargo +1.88.0 test --locked --offline -p friend-service \
  --test concert_invalidation_rpc --test concert_invalidation \
  --test concert_relationship --test unit --no-run --message-format=json
```

Final native command from repository root:

```sh
python3 backend/scripts/mcr/friend_invalidation_rpc_smoke.py \
  --binaries-json /private/tmp/mcr-friend-invalidation-rpc-20261104/frozen-2/binaries.json \
  --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-friend-invalidation-rpc-20261104/native-2.json
```

A rerun requires a new report path. Final report SHA-256:
`40a781be5cb77bf51db484a7cd92e684a63f49095d4598340240fe9818dc22f4`. Source checkpoint: `fd1a8c74eb97a6bfc1ba0b9f158f1f721a1de4c6`.
Frozen RPC test executable SHA-256:
`9c34c5da81ba51a1de006a8ead2bd1758473df109303f75f585d974cf3f59b6f`. The report and frozen metadata include
all five binary identities, exact source hashes and unchanged migration receipts.
All runtime source/contract/manifest/lockfile hashes match the final checkpoint.
The first passing native report is superseded by the second's stronger transport
cancellation observation. Both actual owned cluster roots were stopped and removed.

### Cancellation evidence precision

A targeted review noted that disappearance within 1,100 ms after abort could be
explained by the request's ordinary 900 ms deadline. That does not require an
immediate PostgreSQL cancel acknowledgement, but it weakens an independent
transport-cancellation claim. A test-only wrapper now records when the returned
guarded service future is dropped while pending, explicitly drops that inner
future before notifying, and never modifies the production adapter or journal.
After observing the actual SQL lock wait, the native test aborts the HTTP/2 client
call, requires the pending-drop notification within 200 ms and at least 100 ms
before the original deadline, then separately checks disappearance of that exact
backend while retaining the table blocker.

This proves transport-driven RPC future cancellation independently of deadline
expiry. SQL cleanup remains bounded by the original statement timeout plus the
test's observation allowance; it does not prove instantaneous PostgreSQL cancel
acknowledgement. The final run passed both observations with no test stack override.

Native TLS qualification uses the real generated adapter, credential/method
guards, retained mutual-TLS material and actual repository/database mutations.
It does not claim a full operational Friend process boot with every dependency,
production deployment credentials, World consumer acceptance or IN01. The service
binary's migration/verification CLI is exercised; the narrow main registration
is compiled and statically checked to omit this service without TLS.
