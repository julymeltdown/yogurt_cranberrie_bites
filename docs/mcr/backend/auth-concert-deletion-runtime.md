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

Status: `IN_PROGRESS`. Source, scoped tests and operational qualification are being
prepared. Cargo, non-test Auth main compilation and native execution are `NOT_RUN`
until a fresh allocator offer is accepted. No prior lease or performance result
is inherited. QA08/load/performance remain `DEFERRED_BY_USER`, never PASS.

Required native proof uses real Auth signup/email verification/login/DeleteUser,
actual production AppState worker, guarded source/Apply mTLS, factory Ticket commit,
and actual Notification owner/receipt. The six remaining required targets must
stay unresolved; unreachable legacy Profile/Friend/Post are negative unavailability,
not positive business qualification. Physical PG and all RPC restart, exact receipt
replay, hostile/missing response, earlier/expired clock, observed real transaction
cancellation/drain and recovery are separate assertions. Empty Ticket/Notification
population only qualifies commit/receipt composition, not populated business
cleanup. Single test-process TCP/mTLS servers are not separately launched mains.
Root's real Concert-main mount remains a separate owner delivery obligation.
