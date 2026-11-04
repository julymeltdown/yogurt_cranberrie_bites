# BE33 / BE42 Notification committed deletion receipt transport

Status: `IN_PROGRESS`. Source implementation is ready for bounded compilation
and new transport validation; those checks are currently `NOT_RUN`. Original
BE33 and BE42 remain unfinished. This is suppression/retention proof only, not
global private-data erasure or whole-saga acceptance.

## Assignment and provenance

Task: `BE33-DELETION-RECEIPT-TRANSPORT-01`. Base:
`f02fe29c69627007aa15eb1e17699b9e3806fba8` on delivery repository
`julymeltdown/mcr`, branch `master`. The existing exclusive Notification worktree
is reused on `codex/mcr-be-BE33-deletion-receipt-transport`; earlier branches,
receipt evidence and stopped twelve-path assignment are preserved.

Root explicitly transferred only these ten paths and deletion-specific hunks:

- `backend/proto/notification/v1/notification.proto`: additive receipt only.
- `backend/crates/notification-application/src/dto/notification_dto.rs`: new receipt DTO.
- `backend/crates/notification-application/src/ports.rs`: fail-closed receipt method and Arc forwarding.
- `backend/crates/notification-application/src/use_cases/apply_user_deletion.rs`: receipt-preserving entry.
- `backend/services/notification-service/src/adapters/out/persistence/notification_repository.rs`: committed-owner adapter forwarding.
- `backend/services/notification-service/src/adapters/in/grpc/server.rs`: ApplyUserDeletion handler only.
- `backend/services/auth-service/src/adapters/out/grpc/user_deletion_dispatcher.rs`: Notification client and verifier only.
- `backend/services/notification-service/tests/deletion_receipt_rpc.rs`.
- `backend/services/auth-service/tests/notification_deletion_receipt.rs`.
- This document.

All logic is newly authored against the retained dependencies. The existing
0007 owner implementation supplies its actual committed receipt; no SQL,
manifest, lockfile, main, build file, mount, other target client or original
`docs/source/` content is changed. Both services already generate Notification
protos. The latest developer-owned validation/no-review policy applies; no
king/SL/Root approval or acknowledgment gate is introduced. Root remains the
named publishing developer.

## Behavior

Previously the Notification RPC returned `applied` alone, and Auth discarded its
entire successful response. Auth could therefore acknowledge an unproved deletion.
The additive `committed_receipt` field preserves `applied = 1`. A missing receipt
from an old peer now fails closed in the new Auth client.

The owning adapter calls the existing `apply_user_deletion_receipt` transaction
and maps the returned typed receipt only after its real COMMIT acknowledgment.
The application port defaults to `DependencyUnavailable`; legacy unit success
cannot synthesize evidence. Its Arc implementation forwards the receipt method.
The existing legacy entry remains compatible; the Auth-only RPC selects the new
receipt entry. Authorization, mTLS workload policy and mounts remain unchanged.

Auth requires true `applied`, version 1, canonical exact nonnil saga/user/actor,
exact retention seconds and nanoseconds, positive fence revision, canonical
positive PostgreSQL xid8 and a valid write-observation timestamp. Both retention
parts have explicit protobuf presence, including legitimate zero values: omitted
precision must not match an epoch/whole-second command by accident. RFC3339
retention offsets represent the same instant; no SQL microsecond rounding is
used for this binding. Recorded time is not presented as a commit timestamp.
The receipt grants no caller authority; authenticated owning transport and
transactional source evidence are still required.

The unchanged Auth-Notification edge deadline and original propagated clock
bound the call. An exhausted clock also refuses completion after response.
Missing, false, partial, rebound or malformed proof returns the closed Auth
`DependencyUnavailable` error without private values. Cancellation drops the
worker dispatch before its existing success transition; retries retain the same
command and recover the owner receipt.

## Planned validation and limits

Only the two new test executables will be compiled/frozen and run under a fresh
finite resource offer. No completed deletion-owner suite is rerun.

- Pure Auth checks mutate all receipt fields/presence and exercise nanosecond
  binding before/after the SQL epoch. Synthetic data qualifies parsing only.
- Actual Notification guarded mTLS RPC with real owned PostgreSQL checks COMMIT
  correlation, equivalent-offset replay, rebound refusal, non-Auth refusal and
  deferred-COMMIT failure without a receipt or partial durable effects.
- Auth's actual DeleteUser command creates the canonical eight-step inventory.
  Its actual PostgreSQL repository/worker and actual mTLS dispatcher refuse
  hostile successful replies and cancellation/deadline completion. Scripted
  peers are negative component fixtures, never positive owner proof.
- A paired actual Notification handler/repository lets that actual Auth worker
  acknowledge only Notification. The seven other required targets remain pending;
  bounded fixture scheduling is explicit and does not fabricate their effects.
  A real PostgreSQL and RPC restart then replays the identical durable receipt,
  refuses a one-nanosecond rebound and preserves the incomplete Auth saga.

The two databases are isolated stores in one marker/inode/UID/socket-owned PG16
cluster. The existing read-only ownership helper is reused; the historically
named `mcr_concert_notification_source` contains only Auth migrations in this
qualification. No Concert data or peer-store production access is introduced.
Private controller scripts will pin source, fixtures, migrations, binaries,
commands, exits, resource limits and unconditional release. Actual Gateway EOF,
full BE33/IN03, other deletion targets, global erasure and production rollout
remain separate unqualified obligations.
