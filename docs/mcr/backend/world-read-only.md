# World READ_ONLY component

Task: `C05/BE41-WORLD-READ_ONLY`. Authoring base: `a4ae8555af8d43b1cddf8b07d71f48d14ad20940`. Status: `IN_PROGRESS`; initial source/API checkpoint only. Runtime and full lifecycle acceptance are not implied by this checkpoint. The broader stored World goal remains `BLOCKED`, with explicit manual continuation authorized by Main.

## Frozen local API

```rust
pub(crate) async fn fence_event_read_only_in(
    c: &mut sqlx::PgConnection,
    event: uuid::Uuid,
    job: uuid::Uuid,
    operation: uuid::Uuid,
) -> Result<WorldReadOnlyReceipt, concert_application::commands::CommandError>;

pub(crate) struct WorldReadOnlyReceipt {
    pub(crate) event: uuid::Uuid,
    pub(crate) job: uuid::Uuid,
    pub(crate) operation: uuid::Uuid,
    pub(crate) receipt_id: uuid::Uuid,
    pub(crate) revision: i64,
}
```

The function uses the existing caller transaction and never begins or commits one. It refuses a SQLx connection outside a tracked transaction, invalid operation identity, wrong event phase, missing/mismatched closure job, and incompatible replay. The operation is exactly the existing `EffectKey { event, job, step: ReadOnly }.operation_id()`. It takes the event exclusive lock before child/World locks. START has already inserted the canonical job and changed its event to READ_ONLY in the same transaction.

The immutable `concert_world_read_only_receipts` table in migration `0023_world_read_only_barrier.sql` preserves exact event/job/operation/receipt/revision identity. Receipt revision is initially one and never changed. Existing BE41 composite migration `0024` is a separate owner's responsibility and may reference this table's unique `receipt_id` with full binding validation. No changes to the preexisting migrations or shared API registries are made here.

Before retiring authority, the component captures each original World owner as `{room, owner, epoch}` and each live membership as `{user, room, family, connection, owner, epoch}`. It deletes event-local ticket digests, marks memberships disconnected, marks rooms UNAVAILABLE, and deletes owner lease rows. Deleting leases preserves the existing `lease_until > leased_at` constraint; assigning arbitrary expired timestamps would not. All effects, captures, and receipt are in the same START transaction, so later failure rolls them back together. Exact replay returns the original receipt without repeating effects.

Closure outbox qualification uses this captured pre-close consumer owner/epoch and actual component receipt plus the canonical closure READ_ONLY effect. It does not require a live owner lease after that lease has legally been retired. A local consumer still acknowledges only its own fenced recipients; this is not a global fanout certificate.

## Phase and native handoff order

Ticket issue, actual-ticket consumption, admission, switch, owner claim/renewal and direct membership keepalive take event SHARE and require OPEN/LIVE before ticket/advisory/room/member/owner locks. Ticket consumption resolves the actual event through an unlocked read before acquiring its first lock, then checks the locked ticket identity again. Closed events cannot regain World authority even when a room row or ticket predates closure.

The existing qualified publication permit holds event SHARE and disables native handoff under the same native mutex before rolling its SQL transaction back. START's exclusive event lock therefore drains those permits. No SQL transaction may wait for an async admission/delivery gate or a native mutex while already holding conflicting event/World locks. The BE41 owner's driver prepares those local gates before entering START. Cancellation or rollback never restores removed cached grants; any later recovery requires fresh qualified authority.

## Provenance, dependencies and initial checks

This component and migration are newly authored. Existing World repository and bridge code are changed only for the phase/lock-order guards. The retained lifecycle effect identity, SQLx transaction-depth API, and immutable evidence trigger are reused. No dependencies, Cargo manifests, lockfiles, source archives, startup, HTTP/proto, community or lifecycle implementation files are modified by this owner.

`cargo +1.88.0 check --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --lib` exited **0** using the assigned BACKEND target, debug zero and incremental disabled. The initial public native-test compilation found two test-only `AssetId::new` argument type errors; they are being corrected. Native component, rollback, concurrent phase guards, actual qualified-owner/native-send drain, recovery, and integrated lifecycle validation are still `NOT_RUN` at this source checkpoint. Newly authored test files are not part of this initial API commit.

The real four-owner fixture and synthetic geometry helpers are adapted from `tests/world_owner_composition/mod.rs` and `tests/world_invalidation/geometry.rs` at the authoring base; their exact destinations and final executed evidence will be recorded with the completed checkpoint. Missing product-art metadata stays synthetic and is not product acceptance.

## Qualified World checkpoint

The initial source checkpoint is `06874327`. Subsequent exact source adds safe archived receipt replay (new effects still require initial PENDING/READ_ONLY), public phase-guard tests and real four-owner/native-socket component tests. The frozen old-Avatar baseline now passes **75** selected regressions, **three** new READ_ONLY native cases and **six** retained invalidation native cases. Every executable/controller exited zero and each owned PostgreSQL cleanup passed. Ignored database/service cases are not included in the 75 regression count.

Commands used Rust 1.88, locked/offline dependencies, at most two build jobs, dev/test debug zero and no incremental output, under the exclusive BACKEND reservation. After Cargo JSON selected the exact five executables, copies and all 1631 backend/contract source hashes were frozen outside the target. `lsof +D` returned no target handles before PostgreSQL startup. The controllers use distinct newly initialized private socket-only clusters, strict marker/inode/database/PID checks and an 8 GiB disk guard. No live database or source archive was changed.

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test world_read_only --test world_invalidation --test world_runtime --test world_ws --lib --no-run --message-format=json
python3 backend/services/concert-service/tests/world_read_only_support/native_smoke.py --binary "$MCR_WORLD_READ_ONLY_BINARY" --lib-binary "$MCR_WORLD_LIBRARY_BINARY" --freeze "$MCR_WORLD_FREEZE_RECEIPT" --report "$MCR_WORLD_READ_ONLY_REPORT"
python3 backend/services/concert-service/tests/world_invalidation/native_smoke.py --binary "$MCR_WORLD_INVALIDATION_BINARY" --freeze "$MCR_WORLD_FREEZE_RECEIPT" --report "$MCR_WORLD_INVALIDATION_REPORT"
```

The five copied binaries also ran with `--test-threads=1`: library 30 passed/9 ignored; runtime 28 passed; WebSocket 16 passed/4 ignored; invalidation 1 passed/6 ignored; READ_ONLY 0 passed/1 ignored. The native controllers explicitly selected and ran their ignored tests.

| Evidence | SHA-256 |
| --- | --- |
| Source/executable freeze | `38695dece4e41e49e778c58afd88483574bdb8194f1c9bdf806def9b28045cf9` |
| World library executable | `47deab00b7e255b191ed60c3cb2b18ddcd3df48c9eefb952796175c5f81a998f` |
| READ_ONLY executable | `5eec7b0c7ef99596b9d97dea750b895000e7126284bfc84b44e357988375a127` |
| Three-case READ_ONLY native report | `d02f670bce9f84164bf647f594f35480094601082fffd12f195609fef41bea8f` |
| Six-case invalidation native report | `a98eeb69e5c974b17036fb57f39bf387ecb27b695f84a3dcb4e8c12e5ed12da8` |

The public repository test holds a real event exclusive lock and observes the issue path waiting at the event SHARE before any room lock; a NOWAIT room probe remains available. After READ_ONLY commits, issue/consume/admit/switch/claim/renew deny authority while preexisting rooms/tickets remain, isolating phase enforcement from the barrier's other effects.

The component test runs the actual START producer with the real World SQL owner, injects failure after World effects and proves complete SQL rollback. It proves old cache grants remain removed, a pending removal blocks late publication, and explicit recovery still needs a fresh real Auth/Friend/Profile/Concert admission. Successful START captures exact old owner identities, legally retires leases, returns an identical durable replay receipt, refuses autocommit, and permits only the captured owner's local closure consumer application after its live lease is gone. The global outbox remains LEASED because this component test does not certify community/fanout completion.

The native drain test uses real `ProductionOwners` publication authority, its actual event SHARE transaction, `WorldRuntime`, real `SocketTransport`, loopback WebSocket input/snapshot, and an independently coordinated in-flight native write. Actual START is observed waiting for that permit; pause cannot disable native handoff or finish SQL rollback before the native mutex releases. START then commits only after the permit drains. No later private frame is handed off, and owner/direct keepalive renewal is denied after closure. This is not a substituted SHARE lock or a successful boolean owner.

Operator-role authority is an explicitly synthetic fixture; the four owning read services and World SQL/native effects are real. The World-only lifecycle wrapper certifies only this component. BE41 owns and must independently qualify its actual community/composite/driver, later lifecycle steps and root integration. No full BE41 acceptance, remote invalidation readiness, product art or serving-ledger/IN01 claim is made.

### Copied fixture provenance

- `tests/world_owner_composition/mod.rs` at authoring base → `tests/world_read_only_support/{owned.rs,qualified.rs}`: ownership checks, real Auth/Friend/Profile gRPC harness and actual Concert/production-owner setup, adapted to separate READ_ONLY databases and tests.
- `tests/world_invalidation/geometry.rs` at authoring base → `tests/world_read_only_support/geometry.rs`: synthetic metadata geometry; internal crate imports only, no product artwork.
- `tests/world_invalidation/native.rs` and its controller at authoring base → `qualified.rs` Trace and `native_smoke.py`: real native transport tracing and independent OS coordination, source-bound copied-executable/private-cluster checks.
- New public `tests/world_read_only.rs`: direct repository phase and lock-order proof.

All exact reports, Cargo JSON, source pins and copied executables are retained outside the target in task-local evidence. BACKEND was explicitly transferred to BE41 after freezing; copied native runs do not retain a target lease or touch another owner's cluster. Canonical selected-static Avatar compatibility is separately coordinated and is not credited by this old-Avatar baseline.
