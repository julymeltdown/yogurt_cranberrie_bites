# World Profile producer

Task increment: BE04/BE13 World self Profile producer. Status: `CONTRACT_READY`.
Delivery: `julymeltdown/mcr`, `master`, through the designated integrator.

This increment supplies current self content to a sealed World projection context.
It does not install the runtime recipient/publication bridge or enable
`production_owners`. It does not certify BE13/14, distributed invalidation,
multiuser visibility, or release acceptance.

## Source and ownership

The isolated branch is `codex/mcr-be-world-profile-producer`, based on
`1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`. Context prerequisite
`353e7f8ebe96bac3c90134e6ca60e627589decb5`, `9aabea8`, `65419f6`, and
`4fc7d33` were imported as separate prerequisite commits. The integrator must
integrate the original prerequisite series once and exclude these duplicate
cherry-picks from this handoff.

All new transport/render-independent adapter code is authored for MCR. The
existing `profile_application::concert_identity::{ConcertProfileRepository,
ReadConcertSelfProfile}` and Profile `PgConcertProfileRepository` are reused
in place, at the pinned base revision; their implementation is unchanged.
The native fixture imports the retained
`backend/services/profile-service/tests/support/concert_identity_fixture.rs`
ownership, reader-role and seed helpers read-only. The native controller uses
`backend/scripts/mcr/migration_smoke.py::Cluster` for private marker/inode-owned
PostgreSQL lifecycle and exact-root cleanup. These sources retain their existing
provenance and histories. No `docs/source/` content is modified.

Approved shared support is limited to the appended Profile proto, Profile module,
startup and optional Concert caller, Concert Profile generation/module export,
the exact platform method exception and Profile audience admission to the
dedicated read key domain, plus the Profile test-only dependency. The separately
approved `method_policy.rs` change is test-only: it scopes retained completeness
to `ProfileService`, checks the new service independently, and rejects unaccounted
services. Its production legacy table is unchanged. The lockfile
adds only that dependency edge and retains all versions and checksums. A
source-level traversal verified Profile's normal/build dependencies cannot
cycle to Concert.

## Contract and behavior

`profile.v1.ConcertWorldProfileService/ReadCurrent` is a new service, separate
from both legacy `GetProfile` and Gateway-only `ProfileService.ReadConcertSelfProfile`.
The request binds canonical, nonnil actor/event/subject/session-family/request
UUIDs, self purpose and microsecond expiry in one signed protobuf body.
`WORLD_PROFILE_PURPOSE_SELF_PROFILE = 1` maps the sealed context's
`WORLD_SELF_PROFILE` intent. Subject must equal actor; unspecified or any other
purpose is denied. The canonical actor field is protobuf field 1.

Only a registered Concert caller with an independent dedicated Concert read
proof addressed to Profile and this exact method may dispatch. The existing
`CONCERT_RELATIONSHIP_SIGNING_SEED` / `CONCERT_RELATIONSHIP_VERIFYING_KEYS` domain
now admits the exact Profile audience. Gateway trusted keys, rotation keys and
the parity key remain disallowed in that domain. No wildcard service, caller,
method or actor-path exception was added. Existing Gateway routes retain their
policy and verifier. Missing dedicated verification configuration omits the new
service; empty, malformed or overlapping configured keys fail startup.

Response fields are exactly `subject_id`, `display_name`, and
`observed_at_unix_micros`, with `cache-control: no-store`. There is no bio,
handle/count, avatar URL, account claim or session claim. Avatar revision and
configuration remain Concert-owned. The reused self use case still validates
stored bio internally; malformed stored bio therefore makes the whole content
unavailable even though no bio is transported.

The handler preserves the verified signed proof stop, original transport stop
and two-second resource ceiling, then clips by the signed request expiry. It
scopes the same monotonic stop into the retained SQL repository. Pool acquisition,
transaction setup, local SQL statement timeout, content SELECT and response all
consume that budget. The repository reads current content and deletion tombstones
in one uncached statement snapshot. Fixed errors exclude internal diagnostics.
Both clocks are rechecked after owner I/O, including noncooperative completion.

The typed Concert client accepts only `&WorldProfileProjection`, obtained from
`WorldOwnerRequest::projection_parent(&checked_auth)?.profile_projection()`.
It reserves the shared single Profile attempt and wraps channel readiness,
signing and RPC I/O in `attempt.run`. Parent clones/derivations cannot mint more
attempts. Immutable source/credential expiry, shared cancellation, inherited
ambient deadline and the original wall/monotonic stop can only shorten the read.
There is no retry or cache. Returned subject, safe bounded display name, current
finite observation time and no-store metadata are checked before consumption.
The local observation retains event/family/request correlation and must be
checked against its live context. It is content, not admission or permission.

## Verification and reproducibility

Completed source checks:

- `python3 scripts/mcr/check-plan.py`: exit 0, planning/provenance PASS.
- Python AST parse of the native controller: PASS.
- Explicit lockfile comparison: only Concert's `profile-service` dependency
  edge differs; versions/checksums unchanged.
- Rustfmt 1.88 parser/format for newly authored Rust files: exit 0.
- `git diff --check`: exit 0.

Initial compilation under the explicitly allocated existing target passed for
Profile RPC, Concert library, Platform library, and Profile library. Copied
executables passed all 187 Platform tests and four retained Profile method-policy
tests. The target was released before executable qualification. No third target,
Cargo or rustc was used without a lease.

The first copied execution found an overly strict test assertion: tampering with
the actor can correctly produce `PermissionDenied`, while digest failures produce
`Unauthenticated`. The test now accepts either denial and still requires zero
owner dispatches. Six other signed/deadline/cancellation cases passed in that
attempt. Typed-client cases exposed a stale shared-target Platform feature
variant that still rejected the exact Profile audience; the fresh Platform and
Profile RPC variants accepted it. The first pin/failure logs are preserved at
`/private/tmp/mcr-world-profile-pinned-kgyagrxc/first-attempt.json`, including
the original binary/source pins. The subsequent explicitly allocated backend
target lease rebuilt all 33 workspace-local packages after package-scoped cache
cleanup, preserving third-party dependency caches. All exact compile and runtime
checks below then passed. An earlier Auth `protoc` shadow-path cache
error was fixed by package-scoped `contracts-build` cleanup; no product source
or proto workaround was applied. A test helper's intercepted response-body
bound was also corrected before the successful compile.

The source tests exercise real generated RPCs with production caller/signature
middleware on controlled plaintext loopback, including each signed scope field,
wrong callers, wrong key domains/audience/method/actor, replay, malformed scope,
expired/missing original deadline, current-content failures, cancellation and
noncooperative owners. Typed-client tests derive the context through a signed
controlled Auth owner observation; they cannot construct a raw projection.
The native ignored cases use an explicitly owned PostgreSQL 16 store for current
edits, transaction visibility, privacy canaries, deletion/tombstone fences, owner
lock deadlines, single-connection recovery, permission failure, outage and recovery.

`backend/services/profile-service/tests/support/world_identity/native.py`
has two phases. Under the allocated target lease, `--pin CARGO_JSON` selects the
exact Profile RPC, Concert, Platform and Profile library test executables by manifest/target/test
profile, copies them into a private unique directory, and records executable and
source SHA-256. After releasing the target, `--pinned PIN_JSON` verifies those
pins, runs the tests and creates a fresh marker/inode-owned native PG cluster.
Only the cluster created by that controller may be stopped or removed. Logs,
commands, exits, source pins, binary pins and cleanup receipt remain in its
private output directory. No database credentials or canary content belongs in
the engineering evidence. SQL files are applied directly in retained order;
this is not SQLx migration-ledger or full-service-startup qualification.

### Final qualified execution (2026-11-04)

Compile environment: Rust 1.88.0, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`,
explicit existing target lease, at least 8 GiB free disk. Each command below
exited 0 with `build-finished.success=true`; exact Cargo JSON is retained in
`/tmp/mcr-world-profile-build.4KiK4Q/backend-requalification/`.

```sh
cargo test --locked --offline -p profile-service --test world_identity_rpc --no-run --message-format=json
cargo test --locked --offline -p concert-service --lib --no-run --message-format=json
cargo test --locked --offline -p platform --lib --no-run --message-format=json
cargo test --locked --offline -p profile-service --lib --no-run --message-format=json
```

The controller pinned the four corresponding executables, the exact manifest,
target, debug/test profile, source hashes and Cargo JSON before releasing the
target. The final release receipt records zero owned Cargo/rustc processes and
zero open files in that target. Native execution used only private copied binaries.

```sh
python3 backend/services/profile-service/tests/support/world_identity/native.py --pin /tmp/mcr-world-profile-build.4KiK4Q/backend-requalification/pin-input.json
python3 backend/services/profile-service/tests/support/world_identity/native.py --pinned /private/tmp/mcr-world-profile-pinned-8cim_s4_/pin.json
```

| Executed boundary | Passed | Exit |
| --- | ---: | ---: |
| Signed Profile RPC security/deadline/cancellation | 7 | 0 |
| Sealed typed-client scope, original stop, cancellation and single attempt | 4 | 0 |
| Complete Platform regression | 187 | 0 |
| Retained and new Profile policy completeness | 4 | 0 |
| Native current rows, transaction visibility, fences, lock timeout/recovery, permission failure | 3 | 0 |
| Native typed client through real Profile owner/database | 1 | 0 |
| Stopped owned database | 1 | 0 |
| Restarted owner/current rows recovery | 1 | 0 |

Native PostgreSQL was 16.14 (Homebrew). All seven retained Profile SQL files
were applied directly in order. The private server was stopped and its exact
marker/inode-owned root removed. A final independent receipt read reverified
every pinned backend source hash, every copied executable hash and root absence.

Evidence: `/private/tmp/mcr-world-profile-pinned-8cim_s4_/native-report.json`,
SHA-256 `35d2dcc01348c648e60c8784ceacbbed1a176b9cf9dcee0aa326c410c9133c4e`.
Its `pin.json` SHA-256 is
`4e8938af030c95525b936bf2a77d05b6f2ab275252780d311d01cadc2c6c3ba8`.
Target release receipt SHA-256 is
`2ad69f3d99d044145cf91f70fbab0abbbb8996937e993509848350ecad306dc4`.

| Copied executable | SHA-256 |
| --- | --- |
| `world_identity_rpc` | `6d77b7054b715fc3fa83db7c194d04942e4b85d4d43f44e2fd941d98d4fbeccb` |
| `concert_service` | `fa868b8244b738b343d30304e3c4ec213750a22bec56a9817a8c958884884200` |
| `profile_service` | `88f22f38e33e81ba19c52374c277788097bfe27057ea89b397391bdb2e34dad5` |
| `platform` | `0413c7081fe8c874d7b0f54c880f913cd9b6f04a3c32e3e60f6c26c20d604dab` |

The targeted peer review found the retained single-service proto completeness
assumption; the approved test-only correction is covered by the four passing
policy tests. It found no concrete authorization, content leak, deadline
extension or cancellation defect. Executable proof above supplies the separate
runtime evidence.

TLS, deployment, runtime recipient/publication composition and real multiuser
flows remain `NOT_RUN`. The typed-client native case uses a signed controlled
Auth observation to derive its sealed context; it does not certify a live Auth
session or actual runtime ticket consumption. Those boundaries are qualified by
their respective owners. No World `production_owners` configuration is enabled.

## Exact delivered paths

- `backend/Cargo.lock`
- `backend/crates/platform/src/actor_delegation/relationship_read.rs`
- `backend/crates/platform/src/grpc_method_policy.rs`
- `backend/crates/platform/src/grpc_method_policy/world_profile_tests.rs`
- `backend/proto/profile/v1/profile.proto`
- `backend/services/concert-service/Cargo.toml`
- `backend/services/concert-service/build.rs`
- `backend/services/concert-service/src/adapters/out/mod.rs`
- `backend/services/concert-service/src/adapters/out/world_profile.rs`
- `backend/services/concert-service/src/adapters/out/world_profile/tests.rs`
- `backend/services/profile-service/src/adapters/in/grpc/mod.rs`
- `backend/services/profile-service/src/adapters/in/grpc/method_policy.rs` (test-only)
- `backend/services/profile-service/src/adapters/in/grpc/world_identity.rs`
- `backend/services/profile-service/src/config/mod.rs` (optional Concert caller only)
- `backend/services/profile-service/src/main.rs` (new guarded service only)
- `backend/services/profile-service/tests/world_identity_rpc.rs`
- `backend/services/profile-service/tests/support/world_identity/mod.rs`
- `backend/services/profile-service/tests/support/world_identity/native.py`
- `docs/mcr/backend/world-profile-producer.md`

The preexisting broader BE04 persistent goal remains truthfully `BLOCKED` in
the goal service; Main authorized aligned manual continuation for this bounded
increment. Sending an assignment or producing source does not change that status.
