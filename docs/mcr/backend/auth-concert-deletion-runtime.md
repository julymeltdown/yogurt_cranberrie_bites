# Auth and Concert deletion runtime composition

Task: `BE42-DELETION-RUNTIME-01`. Owner207 is the sole writer in
`be-shared-participant-composition`, branch `codex/mcr-be-BE42-deletion-runtime`.
Root is the publishing developer. Developer-owned validation and fixes remain
required; there is no manager review or acknowledgment gate. One canonical
publisher receipt establishes delivery; no repeated owner readback is required.

## Source and boundaries

Public base: `7eac13ce30d6dd6be4df30727f75d5df90d3489d`, newer than the assigned
`5da64426` baseline. Only these qualified read-only dependencies were cherry-picked:

- Apply `ae4078ce`, `56cdf776`, `073c7cda`, `82e9f857`, locally
  `0a0590e2`, `c4cde9c5`, `868f6efe`, `69091c53`.
- Notification receipt `c8554cb1`, `be13bc84`, `c9f8a848`, `0701a31e`, locally
  `ae45957f`, `6350c391`, `cebb416c`, `cd0f75f6`.

Dependency tip `cd0f75f6`. Root subsequently published those qualified dependency
postimages at `550fba0542d0be46521e8cefd22c1e170c7c86e4`; that canonical receipt
is sufficient. This runtime handoff contains only the new assigned runtime paths,
not a second publication of the dependency commits. No uncompiled Party source
was imported. Original `docs/source/` files remain byte exact.

The source bridge and Apply transport documents record their own provenance and
prior qualification. This unit newly authors runtime composition around those
ports. Tests reuse the existing marker/inode-owned canonical fixture environment
and read-only Auth fixture configuration. Notification construction follows its
qualified `deletion_receipt_rpc.rs` actual service constructor, not a fake success.
No dependencies, lockfiles, SQL, proto, build scripts, World leaves, Wall,
`native_resolver` or Concert main are changed.

## Runtime behavior

`auth_service::config::concert_deletion::ConcertDeletionSettings` captures
`AUTH_CONCERT_DELETION_MODE=off|managed`, explicit `AUTH_CONCERT_GRPC_ENDPOINT`,
and `AUTH_CONCERT_DELETION_DEADLINE_MS=1..5000` (default 5000). Missing mode is off.
An endpoint supplied in off mode is rejected. Managed mode requires HTTPS origins
without credentials/path/query, actual Auth credential, canonical internal mTLS,
and HTTPS for all four legacy targets. It never infers an endpoint/key, retries a
transport call, extends an original clock or installs a successful no-op.

`AppState.user_deletion_worker` is the actual existing durable `UserDeletionWorker`
with `ConcertDeletionDispatcher<GrpcUserDeletionDispatcher>`. The existing four
legacy target clients, including exact committed Notification receipt verification,
are preserved. Off mode injects no Concert owners, so all four required steps
remain retryable. The original dispatcher carries the earlier parent deadline
through the full Concert RPC; no new time is granted after an await.

Auth main captures settings before its database connection and validates explicit
`AUTH_CONCERT_DELETION_SOURCE_ENABLED=true` plus configured Concert caller
credentials for managed mode. It mounts the same canonical guarded source service
on the existing listener. Other listener roles and worker policy remain unchanged.
`build_state_with_deletion_runtime` accepts the same captured typed settings for
production and native qualification; existing capability constructors also compose
the actual worker through this implementation without changing AppConfig literals.

`concert_service::config::account_deletion_runtime` supplies:

```rust
AccountDeletionRuntime::compose(
    AccountDeletionSettings::Managed,
    same_concert_pool,
    canonical_auth_endpoint,
    Some(&canonical_concert_tls),
    canonical_concert_credential,
    AuthDeletionCallers::configured()?,
    LiveDeletionOwners {
        worlds: Some(vec![ServingWorld { bridge, sockets: &serving_socket_state }]),
        sharing_publisher: Some(actual_sharing_publisher),
    },
)?.unwrap().service()?
```

`CONCERT_ACCOUNT_DELETION_MODE` is explicitly `off|managed`, missing means off.
Off returns no service. Root owns mounting and supervision on the actual existing
mTLS Concert listener. `AuthDeletionCallers` accepts only Auth and preserves
current/previous configured rotation; no shared credential or Gateway delegation.
The factory always creates the actual AuthDeletionSource and TicketDeletion on
the supplied canonical pool and TLS identity. Missing mTLS, credential or HTTPS
source endpoint fails construction.

Optional Sessions are constructed from the actual serving AdmissionBridge and
WorldSocketState (which checks the same bridge/runtime/transport), using the
existing SessionOwner and SessionDeletion. `Some(empty)` fails; absence remains
unavailable. Optional Sharing receives the actual publication port; the factory
constructs its sink/SharingDeletion with the same pool/source. There is no default
publisher, second World registry, positive fixture or inferred serving handle.
PrivateData is deliberately absent from the factory API and remains unavailable:
core completion cannot certify the aggregate/all required real owners.

## Developer validation

Developer qualification: **PASS** at exact code/test checkpoint
`932c867c5df00887487f08c8841a0a7d3906d0d9`. Publication is pending the named Root
publisher. Whole BE42 remains `IN_PROGRESS`; six required targets are unresolved.
QA08/load/performance are `DEFERRED_BY_USER`, never PASS. This is the qualified
runtime unit, not eight-target deletion acceptance or a separately launched
production deployment.

Frozen source/readset: **3,799** backend/contract/public/source files, all unchanged
through compilation and both native phases. All **15** supplied originals are
byte exact against public base `7eac13ce`. The compiled source commits are:

1. `f421f0896a2957a0d34e6709330b8fc2792011b4` — runtime composition and tests.
2. `61381b7cde83d93dfa2359f0eae7755ec8a516ae` — isolate negative cancellation
   scheduling from restart. Scheduling never invents a successful target.
3. `9f07e3840b9710e6ad01849401aa08972b223f5e` — actual early source opt-in checks.
4. `932c867c5df00887487f08c8841a0a7d3906d0d9` — canonical Auth HTTPS origin guard.

The following qualification-record commit changes this document only. No code,
SQL, test, runtime setting or assertion changed after the passing checkpoint.
Dependency commits above are read-only and are not republished in this handoff.

### Compilation and pure checks

Rust **1.88.0**, locked/offline, `-j 2`, dev/test debug=0, incremental=0,
`profile.dev.package.contracts-build.opt-level=2` (inherited by test profile), both
`RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` **unset**, no compiler override/wrapper.
The three exact commands, sequentially, all exited **0 on their first attempt**:

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test concert_deletion_runtime --no-run --message-format=json
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --bin auth-service --message-format=json
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test account_deletion_runtime --no-run --message-format=json
```

Cargo reported **21.29s / 10.37s / 45.16s**. The guarded complete compile/freeze
phase took **83.895s**. The explicit Auth command compiled its **non-test main**;
that binary was not copied or launched. Only these two test executables were
frozen outside the target (**55,237,888 bytes aggregate**, below 96 MiB):

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `concert_deletion_runtime-tests` | 13,290,720 | `fed01c1004eb21b5d49b2d91fbdc8ed43d9d7086bb83fd20960a6ce6c4a404b5` |
| `account_deletion_runtime-tests` | 41,947,168 | `1fdfe3cefcca640350f5ac36f5612f94b20f0da9ed1ee667373d15dee6e6ded8` |

Both copied executables ran with `--test-threads=1 --nocapture`: **four pure checks
passed**, and the one native case was accurately ignored in the default run.
It was then explicitly executed twice; it was not omitted from native evidence.
The pure checks cover explicit modes, origin/clock shape, off-mode non-ACK for all
four required Concert steps, missing mTLS, Auth-only rotation credentials, factory
absence in off mode, and canonical source origin rejection.

### Actual native runtime proof

Copied Concert executable, exact test:
`actual_production_worker_factory_commit_restart_and_non_ack`, arguments
`--ignored --exact NAME --test-threads=1 --nocapture`, with
`MCR_BE42_RUNTIME_PHASE=SOURCE`, then `RESTARTED` after physical PostgreSQL restart.
Both phases exited **0 on the first attempt**:

| Phase | Test body | Controller wall time | Result |
|---|---:|---:|---|
| SOURCE | 8.49s | 8.787s | PASS |
| RESTARTED, fresh test PID and all RPC servers | 4.09s | 4.469s | PASS |

The complete native controller took **33.935s**, including owned cluster, PKI,
creation of the five isolated inventory databases, four pure checks, both native
phases, ledger checks and teardown. Every native case stayed below 45s; cumulative
native work stayed below 180s. Real PostgreSQL **16.14** and actual TCP/mTLS were
used. The servers run in the native test process, not separately launched mains.

SOURCE uses the actual production `build_state_with_deletion_runtime` AppState
and existing worker. Its actual signup, generated email challenge/verification,
login/refresh session and DeleteUserUseCase create all identities and the durable
saga. No positive identity, session, deletion claim, Auth source or business
receipt is inserted as fixture SQL. The eight due steps produce exactly **two
successes and six retriable failures**, with no completed saga:

- Ticket: actual guarded Auth source, production Concert factory Apply service,
  real common TicketDeletion transaction/fence/progress commit and exact verified
  response reach Auth before its step is marked succeeded.
- Notification: actual owner repository, production service state and guarded
  mTLS RPC commit the immutable receipt; Auth's unchanged typed verifier accepts
  that exact tuple/retention/fence/effect-XID proof before its step succeeds.
- Sessions, Sharing and PrivateData have no live owners supplied. Legacy Profile,
  Friend and Post endpoints are deliberately unreachable negative dependencies.
  All six remain unresolved. Those three legacy refusals are **not positive
  business qualification**.

The factory source startup path refuses absent, false or invalid explicit opt-in
before serving; accepted true uses the canonical configured Concert caller
registry. Invalid Auth credential, wrong server workload certificate, nanosecond
retention rebound, expired original clock and missing required owners are refused.
`Some(empty)` World ownership fails construction. No empty parallel World or fake
positive Sharing publisher is created.

A separate authenticated hostile Concert peer supplies only missing/malformed
proof or a delayed error. The actual worker retains its failed step; no boolean,
RPC success or absent receipt can complete it. An earlier 35ms parent beats the
configured 5s clock, and reusing the expired clock dispatches nothing. Cancellation
waits until that authenticated peer has observed dispatch before aborting; the
durable step remains unresolved. Only negative fixture scheduling is parked for
isolation; no target receives fabricated success.

The actual-factory cancellation case holds the real common user advisory lock,
observes the real Ticket transaction waiting in PostgreSQL, and then aborts the
actual worker. After its original deadline, the test releases the lock, waits for
**all other Concert transactions to drain**, and verifies zero fence/progress
commits for that command and no Auth ACK. An expired dispatch is still refused.
The actual worker subsequently recovers its expired lease and commits one Ticket
success with the other seven steps unresolved.

After a **physical PostgreSQL restart**, a different native test PID reconstructs
all Auth source, factory Apply and Notification RPC servers and the same actual
worker composition. It retries the six original unresolved steps, yielding
**zero new successes**, and replays both already committed owner receipts. Ticket
progress/source binding and Notification receipt remain byte-equivalent JSON;
all eight required steps remain present, exactly two succeeded, saga incomplete.

Migration ledgers pass in both phases: **Auth18 / Concert44 / Notification7**.
Friend and Profile migrations are **NOT_RUN**, not PASS. All five inventory
databases have zero prepared transactions before restart and teardown.

Positive Ticket and Notification population is empty: this unit qualifies actual
commit/receipt/worker composition and recovery, **not populated business cleanup**.
The real Sessions adapter is compiled; its native serving World drain and actual
Sharing publisher join are not positively qualified by this missing-owner cohort.
Root's separate live composition remains responsible for those supplied handles,
actual Concert-main mounting and supervision. PrivateData aggregate/all-owner
qualification remains a separate owner obligation; core completion never enables
its ACK. No Gateway/device/public journey or whole BE42 acceptance is inferred.

### Allocation, failures and release

Evidence root: `/private/tmp/mcr-be42-runtime-tn_kk128`.
Authoritative standing offer `BE42-RUNTIME-MEDIA-DIRECT-NEXT-202611041701Z` was
clarified by the allocator after the peer's narrower copied template. The initial
stricter peer intersection is preserved as historical admission evidence; the
current standing offer governs, **without resetting the actual ACCEPT epoch**.
The observed peak also fits the earlier stricter limits.

- Actual ACCEPT: **2026-11-04 17:07:34.952352 UTC**, source `932c867c`.
- Fresh full checkout **1,308,602,368 B**, full common Git **1,102,843,904 B**,
  all retained/historical charges included: **4,252,688,384 B carry**.
- Planned whole reservation: carry +2 GiB, rounded to **6 GiB** (below 12 GiB
  allowed); NEW total cap2 GiB. Both flags unset and resolved Cargo/Rust/PG/OpenSSL
  hashes were pinned. A conservative **4 GiB** unknown/other NEW reserve was kept,
  never assumed zero; fresh free **31,555,420,160 B**, margin remained above9 GiB.
- Independent guardian ran before each phase's children. The source bytes of
  Auth/Concert build scripts and three owned composition roots were hashed before
  and after permitted metadata-only timestamp refresh for known shared-cache
  freshness hazards. Their bytes, Git source and frozen readset did not change.
- All three compilations, all four pure checks and both native phases passed on
  their first attempt; **no Cargo/native failure, assertion relaxation or repair
  rerun**. The earlier default stable formatter could not load its local dylib;
  the pinned 1.88 formatter succeeded. Planning check and scoped diff check exit0.
- Target RELEASE: **17:10:13.967341 UTC**; compiler0, `lsof` handles0, build guardian
  reaped, two frozen executables hashed. Target passed directly to owner00c's
  current `625c3993` scope before any copied native or PG start.
- **No target access after RELEASE**. Native guardian uses the frozen target
  charge only. Original absolute clock plus post-release work180s/FULL240s wins;
  no implicit extension or resource-manager correctness permission.
- FULL RELEASE: **17:11:24.227539 UTC**, **70.260198s** after target RELEASE.
  Owned PG/PKI/TMP removed, both guardians and all eight recorded owned PIDs absent,
  no owned evidence handles, all five prepared counts0. Free **31,250,173,952 B**.
- Peak whole **4,470,382,592 B**; target-positive **53,526,528 B**, outside growth
  **162,623,488 B**, common Git growth **1,544,192 B**. Target-positive growth is
  retained as historical charge for future rounds even after release.

Frozen checkpoint SHA-256:
`81cb1cc2dd4b5bf5e85f7418233db7cd085b07b4c4dd77d7cff5cfec2e913a6a`.
Native report SHA-256:
`c9441e03d0ff0885bf5c1281d3c893157f196819e42c80285ed2580955f1400f`.
The evidence root retains Cargo JSON/stderr, source/readset/tool pins, full
commands, both phase reports, guardians/high-water data, owned cleanup and release
receipts, and the two frozen executables. No old Apply/Notification cohort was
rerun, no new replay/readback-only goal or proof commit is required.
