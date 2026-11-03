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
