# BE33-DELETION-RECEIPT-01 — Notification deletion proof

Status: `IN_PROGRESS`; compilation and native qualification are `NOT_RUN`.
This is an aligned prerequisite of original BE33, not whole BE42, private-data
port, physical erasure, public IN03 or release acceptance.

## Ownership and provenance

Source base: `10bde5f3f33438740f2fa8696becf4ae1aa1a09c`, the normal published
`89f26e6e` successor. Existing exclusive `be-product-feature-admission` checkout,
branch `codex/mcr-be-BE33-deletion-receipt`; prior clean `2ced9803` and its startup
branch are preserved. The original aligned BE33 goal is unfinished. Its historical
`BLOCKED` API metadata is retained, not reset or falsely completed.

Root grants exactly seven paths: new `src/concert/deletion_owner.rs`, only its
export in `src/concert/mod.rs`, only the existing deletion command/receipt path
in `src/adapters/out/persistence/notification_repository.rs`, new
`tests/deletion_owner.rs`, this document, new
`migrations/0007_deletion_receipt_fences.sql`, and only new receipt/fence guards in
`src/config/schema-probe.sql`, all below Notification service except this doc.
No Auth, wire, central manifest/lock, main, transport, other startup/Concert SQL,
frontend or frozen `docs/source/` change is authorized or made.

The inherited repository account-deletion transaction, original bounded purge,
Auth-only RPC/method policy and request/reply are reused from the base. They
originate in the pinned port-rust foundation, imported by `909e2bf4`; this leaf
records its exact MCR source revision and preserves migrations 0001–0006 byte
for byte. Existing SQLx, chrono, UUID and JSON dependencies suffice. No dependency
or rendering code is added. The existing BE33 private PostgreSQL ownership guard
is read-only reused by the new tests.

The latest no-review direction overrides historical review/acknowledgment text:
implementation owners choose/run validation and fixes; Root publishes normally
without a king/SL/QA approval stage. Root owns returned startup files and all
excluded paths. Its RW10 migration-input window was reported FULL released
2026-11-04 14:02:18.727647 UTC; authoring 0007 here never mutates Root's inputs.

## Exact behavior and policy boundary

The existing Auth-only `NotificationService/ApplyUserDeletion` remains unchanged:
`(saga_id,user_id,actor_id,retention_until RFC3339) -> {applied: bool}`. Its
repository path now uses `apply_user_deletion_receipt(DeletionCommand)` internally.
The existing caller authentication and canonical Auth saga remain mandatory.
Constructing a command in server code is not authentication, public permission,
a client-supplied proof or a new transport endpoint.

`DeletionCommand` binds all four inputs, including exact retention nanoseconds.
PostgreSQL timestamps retain their original microsecond storage behavior, while
separate seconds/nanoseconds prevent a sub-microsecond rebound from becoming an
exact retry. Nil identities and leap-second nanoseconds fail closed. Debug output
redacts identifiers and dates. There is no invented retention-duration policy.

The repository starts one actual transaction and uses its unchanged
`lock_subjects` absent-row creation and ordered UUID locking. That same connection
applies original actor/recipient suppression, legacy suppression synchronization,
soft deletion and original audit behavior. It then establishes a positive,
identity-generated permanent subject fence and an immutable v1 operation receipt
linked to that original audit and full command. A native XID witness and deferred
constraint require the current transaction's legacy receipt/audit plus actual deny
and no remaining live subject notification at commit. No second connection,
queued work or independently committed receipt can acknowledge those effects.

Only after a successful COMMIT reply does the repository expose
`CommittedDeletionReceipt`: version, full original command, positive fence
revision, owner-native effect XID and recorded write time. Recorded time is not
called a PostgreSQL commit timestamp. A failed/uncertain commit yields no typed
success; exact retry recovers the same immutable receipt. A changed subject,
actor or even one retention nanosecond under the same saga fails closed.

A positive permanent fence is anchored to the existing state used by ordinary
creation and Concert ingress/replay. Deny flags cannot be cleared or deleted;
clearing `deleted_at` cannot resurrect denied notifications. Immutable v1 fences
and receipts reject update/delete/truncate. Their linked original audit and legacy
receipt reject mutation too. Unlinked historical rows keep their compatibility
behavior, but the migration never backfills v1 evidence. An existing mutable
receipt/audit without provable v1 evidence cannot be retrospectively acknowledged.

Soft-deleted payload fields and original retention scheduling remain unchanged.
The existing bounded retention purge may remove eligible notification rows, and
retains command/fence/audit proof. This leaf does not invent a per-field physical
erasure inventory, treat a date as blanket purge permission, or claim the existing
boolean is a complete BE42 acknowledgement. Root retains authenticated bridge,
canonical orchestration, transport and full private-data policy/composition.

## Runtime grants and schema dependency

Root retains `runtime-grants.sql`. The exact required addition is SELECT/INSERT
on `notification_deletion_fences_v1` and `notification_deletion_receipts_v1`, plus
USAGE on `notification_deletion_fences_v1_fence_revision_seq`. No update/delete
permission is needed. Root confirmed it will install these with qualified 0007
intake after tables exist; this writer does not edit that file or publish early.
The new native test uses exactly those extra fixture grants with the retained
runtime allowlist and a disposable non-owner role. This is distinct from proving
Root's eventual published provisioning command.

The selected schema probe checks new tables, types, identity, keys, foreign keys,
all new evidence/deny triggers and the deferred commit constraint. Migration
checksums remain the inherited runtime's responsibility; this is not a claim of
exhaustive schema/privilege drift detection.

## Required qualification

New target: `cargo +1.88.0 test --locked --offline -p notification-service
--test deletion_owner --no-run --message-format=json`, only under a fresh finite
21d offer and explicit ACCEPT, with its named existing target, source/runtime/
fixture pins and whole retained accounting. No target or runtime is currently
reserved. Freeze one emitted test executable outside the target and release the
target unconditionally before any native test/PG process; preserve failures.

Eight regular checks and two externally staged restart checks are authored:

1. Typed command rejection, exact nanoseconds and redacted diagnostics.
2. Real committed receipt, exact retry, all rebound fields and retained bool path.
3. Actual deferred-commit failure rolls back effects, receipt, fence and audit.
4. Immutable evidence/deny, direct resurrection refusal and bounded purge.
5. Ordered creation/deletion contention, concurrent retry and Concert replay.
6. Actual SQLx 0006→0007 migration retains unproved historical rows without ACK.
7. Schema probe rejects disabled evidence/commit guards; false native XID denied.
8. Minimal non-owner runtime grants commit but cannot rewrite proof/audit.
9. Explicit seed commits and discards a response in a separate process.
10. After an actual owned PostgreSQL restart, explicit recovery proves identical
    receipt/fence/native identity and continued deny. Without a real restart this
    phase fails; no default skip counts as success.

No case has run yet. The restart seed/recovery are ignored in ordinary harness
execution solely so the owned external runner can perform the real restart between
those exact phases. Source-only compilation/formatting cannot certify any of
these native behaviors. Existing unrelated provider/startup tests are not rerun.
Actual commands, exit codes, fixture/runtime pins, cleanup and publication proof
will be recorded after the authorized finite qualification round.
