# BE34/BE35 mural implementation

Status: IN_PROGRESS. Native Rust adapter qualification is NOT_RUN at this source
checkpoint. Public routes, production authority wiring, C05 mural messages and
IN04 are NOT_RUN. This document records bounded implementation, not acceptance.

## Ownership and provenance

Worker chat: `01a1a21e-ab80-7012-b9fa-cdb69eed097b`. Isolated checkout:
`/Users/lhs/Desktop/mcr-worktrees/be-mural`, branch `codex/mcr-be-BE34`, base
`51783cf5651b68ca98faede29d442c4dccc24f6f`. All mural logic is newly authored;
no legacy rendering, character artwork or upstream application code is copied.
`docs/source/` is unchanged.

Pinned inputs at the base:

- Specification: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §§8.4, BE34/BE35/BE40;
  SHA256 `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.
- C04 mural schema/operations/policy candidate:
  `b615e35bb07fb5797e6d7445a93aa6e1eeab7152`.
- BE15 transaction, receipt and outbox seam:
  `c2a2c3020121c7385680eaae7cfa6ba027fe5e01`;
  `backend/services/concert-service/src/adapters/out/repository/receipt.rs`.
- Safe domain failure pattern and migration0009:
  `9472785241c55c0a83179129a1ce3fdf80f7847b`.
- BE37 current relationship-policy pattern:
  `620f8ee38cbe277d4ead48c36fc139dd8c4fce3c`.
- Existing isolated PostgreSQL owner-marker harness reused from
  `backend/scripts/mcr/migration_smoke.py` and orchestration pattern adapted
  from `meeting_smoke.py` at the assigned base. Existing async-trait, chrono,
  serde/serde_json, SQLx, Tokio, UUID and BE15 fingerprint dependencies only;
  no dependency, lockfile, HTTP contract or transport changes.

Delivery explicitly granted a separate shared dependency carveout in this chat:
`pub mod mural;` only in domain/application lib.rs and repository.rs;
`pub mod world;` only if absent in service lib.rs; a declaration-only
world/mod.rs; SafeFailure enum/as_str/parse additions for the four exact C04
mural reasons; new migration0014 preserving every migration0009 reason.
Shared registration commit: `4f412f6`. Published migration0009 is untouched.
The feature paths remain exactly the assigned mural domain/application,
repository/mural.rs, world/mural_subscription.rs, two mural tests, two owned
Python scripts, reserved migration0013 and this document.

## Write transaction and recovery

`PgMuralRepository::place` accepts one confirmed `PlacePixel`, canonical actor,
event and idempotency key. It validates 128×128 coordinates, 16 color indices
and a JS-safe positive expected revision. Trusted `MuralAuthority` must resolve
canonical Auth/session identity, event membership, global feature eligibility,
suspension and BE37 privacy policy with current transaction-held local fences
or an equivalent external authority fence. The unavailable implementation denies
all operations; no production settings or operator roles are invented.

The BE15 producer locks the event and mural before command admission. It checks
event OPEN/LIVE, EDITABLE state, locked regions, exact expected revision and
per-actor cooldown using PostgreSQL clock_timestamp after acquiring the mural
lock. One transaction contains grid change, strictly increasing revision, original
operation, private author/cooldown, BE15 safe receipt and outbox invalidation.
A storage/outbox/commit error returns no success. Transaction rollback removes
all effects and receipt budget consumption. Retry uses the original key.

Exact key/body replay returns the original operation, even after pause/freeze;
changed canonical body conflicts. The original operation is retained separately
from delta visibility. `recover(actor,key)` only reads: freshly authorize the
receipt and original operation, require a successful receipt and matching
actor/event/revision, then return the original PixelOperation including its
next_allowed_at. Missing/pending/failed/expired status is never inferred from a
matching pixel color and never triggers automatic replay.

Paused/frozen/cooldown/locked-region outcomes persist FAILED receipts with the
exact C04 reason and no outbox or aggregate effects. A cooldown replay has no
stored retry timestamp in the BE15 safe receipt, so it returns `Cooldown(None)`;
transport composition must not invent Retry-After metadata. Fresh failures may
carry the known PostgreSQL timestamp. New commands require a new explicit action.

## Snapshot, delta and subscription

Snapshot reads hold the aggregate SHARE lock through reading revision, grid,
palette and caller-only cooldown. They checkpoint public recovery data under a
stable snapshot ID; updates to stored checkpoints are rejected. A checkpoint
contains no author, moderation or caller cooldown fields. Operator provisioning
must supply palette and cooldown; the migration has no product defaults.

Public deltas select an explicit seven-field projection and contiguous revision
order, bounded to 512 operations. Equal revision returns an empty delta. Future
revision, retained_from_revision floor, reset_revision barrier, a larger gap or
missing operation produces reset_required=true and no operations. Original
operation records remain for receipt recovery; this patch does not authorize
unbounded audit deletion or change BE15's lifetime admission budgets.

BE40 must lock the mural row, increment revision for control/restore/lock changes,
advance reset_revision to the new revision, create a new snapshot ID/time when
state changes, and never rewind revisions or cooldowns. Restore reads an immutable
public checkpoint and writes a NEW authoritative revision. BE40 operator commands,
authorization and audit are separate allocation; native tests simulate this SQL
boundary without claiming BE40/IN04 acceptance.

`MuralSubscription` is a bounded pull adapter for the existing world connection,
not another socket/server. Queue capacity is explicit (1–16 frames); each delta
has at most512 operations. Overflow, reset/gap, epoch change, read/policy failure
or authorization revocation discards queued data and requires a fresh snapshot.
Every dequeue reauthorizes and rechecks the reset boundary. A subscription keeps
a revision high-water mark across epoch changes and fails closed on DB rewind.
Transport must invalidate after send failure rather than treating dequeue as
client acknowledgment. New process connections begin with fresh snapshots.

C05 v1 currently has no mural message and C04 exposes no original-result GET.
FE33 owner `01a1a213-61d6-7a20-a8e7-476649312f8c` is using typed incumbent-connection
and verified-result ports; public wiring requires Delivery-owned contract work.
Unknown commands remain blocked until independently verified original result
is available. This implementation adds no unallocated wire operation.

## Validation evidence

- `python3 scripts/mcr/check-plan.py`: exit0, planning provenance PASS.
- `python3 -m unittest discover -s backend/scripts/mcr -p test_mural_smoke.py`:
  exit0, 2 evidence-integrity tests PASS.
- `python3 backend/scripts/mcr/mural_smoke.py --sql-only --report
  /tmp/mcr-mural-sql-03.json`: exit0; PostgreSQL16.14 isolated Unix-socket cluster,
  direct SQL validation/revision/freeze guards PASS, actual database process
  restart persistence PASS, marker/inode-owned cleanup PASS. Native adapters
  NOT_RUN; migration runtime ledger NOT_APPLIED (ordered SQL application only).
- Rust1.88 rustfmt on owned Rust sources: exit0. No whole-workspace formatting.

- First focused Rust compile: exit101, E0282/E0283 in the own-operation
  recovery projection. The closure now explicitly returns
  Result<PixelOperation,MuralError>. Lease released before source correction;
  this failed run is not passing native evidence.

Pending: retry native compile and source/binary-bound fixture runs; peer findings and
committed direct handoff. The native suite covers concurrent same-actor cooldown,
concurrent distinct-actor revision conflicts, bounds/color validation, idempotency
conflicts and original-result recovery, terminal failure replay, forced outbox
failure rollback, private projections, snapshot/delta convergence, retention,
restore barriers, bounded queues/revocation/epoch recovery and durable original
operation recovery after actual PostgreSQL and test-process restart.

Release/deployment, real two-browser multiuser flow, provider authority, C05
transport, operator provisioning, BE40 and IN04 remain NOT_RUN.
