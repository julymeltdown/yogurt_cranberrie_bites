# BE33 / BE42 Notification committed deletion receipt transport

Status: `CONTRACT_READY`. Both scoped test executables compiled; all seven new
semantic checks passed (two pure, five native). The validated committed handoff
awaits normal Root publication. Original BE33 and BE42 remain unfinished. This is suppression/retention proof only, not
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

## Validation and limits

Only the two new test executables were compiled/frozen under finite resource
offers. No completed deletion-owner suite or passing pure check was rerun.

- PASS: pure Auth checks mutate all receipt fields/presence and exercise nanosecond
  binding before/after the SQL epoch. Synthetic data qualifies parsing only.
- PASS: actual Notification guarded mTLS RPC with real owned PostgreSQL checks COMMIT
  correlation, equivalent-offset replay, rebound refusal, non-Auth refusal and
  deferred-COMMIT failure without a receipt or partial durable effects.
- PASS: Auth's actual DeleteUser command creates the canonical eight-step inventory.
  Its actual PostgreSQL repository/worker and actual mTLS dispatcher refuse
  hostile successful replies and cancellation/deadline completion. Scripted
  peers are negative component fixtures, never positive owner proof.
- PASS: a paired actual Notification handler/repository lets that actual Auth worker
  acknowledge only Notification. The seven other required targets remain pending;
  bounded fixture scheduling is explicit and does not fabricate their effects.
  A real PostgreSQL and RPC restart then replays the identical durable receipt,
  refuses a one-nanosecond rebound and preserves the incomplete Auth saga.

The two databases are isolated stores in one marker/inode/UID/socket-owned PG16
cluster. The existing read-only ownership helper is reused; the historically
named `mcr_concert_notification_source` contains only Auth migrations in this
qualification. No Concert data or peer-store production access is introduced.
Private controller scripts pin source, fixtures, migrations, binaries,
commands, exits, resource limits and unconditional release. Actual Gateway EOF,
full BE33/IN03, other deletion targets, global erasure and production rollout
remain separate unqualified obligations.


## Qualified source, commands and evidence

Initial source is `be13bc84ef13e95b701108304c8eadb82dcdf1a1`, following production
implementation commit `c8554cb12867f5ccfd4a56fb14e0c66aa9fe158f`. Both test executables
compiled successfully, and both Auth pure checks passed. The first native attempt
stopped before RPC dispatch because this writer used `guard_without_delegation`
with Notification's full actor-bound method policy. Its exit 101 and resource
receipts are retained. Test-only correction
`c9f8a848765c674420c16379cb789cc7ebf44d18` selects the exact production
`guard(..., config.actor_delegation)` constructor. It changes no production bytes.

A fresh offer rebuilt only the Notification executable. Full source comparison
against the earlier frozen Auth build found exactly that one changed Notification
test file; all Auth, proto, helper and compiler inputs were byte-identical. The
Auth executable was reused by reference, and its passing pure checks were not
repeated. The final five native cases all passed against this qualified source.
Subsequent changes to this document are evidence only and require no rebuild.

Each command below exited 0. The initial compile ran for each named package/test;
the corrected compile ran for Notification only:

```sh
cargo +1.88.0 test --locked --offline -p notification-service --test deletion_receipt_rpc --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
cargo +1.88.0 test --locked --offline -p auth-service --test notification_deletion_receipt --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
```

Both builds used `CARGO_BUILD_JOBS=2`, dev/test debug information 0,
`CARGO_INCREMENTAL=0`, and unset Rust flags. Initial Cargo elapsed times were
8.25 and 19.43 seconds; the corrected Notification compile took 7.91 seconds.
Only requested test executables were copied/run. Automatically produced mains
were accounted as target output and were not copied or executed.

Copied Auth pure invocation: `notification_deletion_receipt --test-threads=1
--nocapture`: two passed, three native cases accurately ignored. Each native
function below was then selected using `--ignored --exact <name>
--test-threads=1 --nocapture`: one passed, zero failed per invocation.

| Executable | Exact native case |
| --- | --- |
| Notification | `actual_rpc_returns_committed_exact_receipt_and_rejects_rebound_and_non_auth` |
| Notification | `actual_rpc_never_returns_receipt_on_deferred_commit_failure` |
| Auth | `actual_auth_worker_refuses_hostile_receipts_and_cancellation_keeps_step_unacknowledged` |
| Auth | `actual_auth_worker_accepts_only_committed_notification_receipt` |
| Auth | `actual_restart_recovers_identical_receipt_without_completing_other_auth_steps` |

The explicitly selected `serve_actual_rpc_for_auth_worker` Notification case
ran twice solely to provide the actual guarded RPC for the paired before/after
restart phases. These are fixture lifecycle invocations, not extra semantic
checks or separate product acceptance. Real PG postmaster changed from 45779
to 47356 after a clean stop/start; this is not a crash-recovery claim. Native
controllers completed in 13.676 seconds for the failed attempt and 23.112
seconds for the final qualification, total 36.788 seconds. All phase exits,
including both actual server shutdowns and migration-ledger checks, were 0.

Notification's seven and Auth's eighteen actual SQLx migration SHA384 receipts
matched their frozen source manifests. Prepared transactions and fixture roles
were both zero before teardown. The real Auth Notification step succeeded;
all seven other required steps and the whole saga remained incomplete.

Private evidence is retained under `/private/tmp/mcr-be33-transport-c8554cb1`
(first attempt) and `/private/tmp/mcr-be33-transport-c9f8a848` (qualified rebuild).
No private keys, database files or ambient data are committed.

| Retained artifact | SHA256 |
| --- | --- |
| Qualified Notification executable, 24,417,168 bytes | `86193552ee3f1d38c7143bc9c6343259fa733cca62e81b9c70ef39ac066d168a` |
| Reused Auth executable, 25,009,712 bytes | `8e20472dc947bd3628d0246d48bb2dfe360b380b8cd4a551afabe54521aaf223` |
| Qualified `artifact-receipt.json` | `5b3975ec8c67ebd28c0072100b4ac730738c9f4a1db2e7471a941058413fa4c0` |
| Qualified `qualify-result-1793807362.json` | `2b348963ee180794b7acf8a3a86e4640e6bac1a2761addec7885a685264e9e7d` |
| Qualified full source archive | `ee1ad15ca00716a1aacb026edc90502551270986b9f1d69d597a412274e6b48c` |
| Original Auth artifact/source receipt | `8d6b1b6731270d32673ca5f2d3c57680cce4dfb0645edeb71ffb3a792ab32a27` |
| Prior passing Auth pure output | `7a37502ce3e6462c12f269128e7aaf89b4bc5d908a8e600a0ad3e874e2e15b71` |
| Failed-attempt full release | `9f42ccae3a87143a0f842a17137d102e7254083e2a37c4e8b6397c9424130abf` |
| Final `full-release.json` | `b31977c4625bac8cf9f289d8fcd469503bc88a7e14b8e1ef8c168eee75e1f93b` |

## Resource release and delivery policy

Offers were `BE33-RECEIPT-TRANSPORT-MEDIA-202611041538Z` and
`BE33-NOTIFY-REBUILD-MEDIA-202611041549Z`. Exact owner build scripts received
explicitly authorized byte-identical mtime refreshes for the known shared Cargo
source hazard; complete before/after hash, inode, device, mode, size and timestamp
receipts are retained. No cache was deleted or new target created.

The first target release was 15:39:21.427636 UTC and full release 15:40:30.237081
UTC on 2026-11-04. The fresh rebuild target was unconditionally released with
zero compiler/handles at 15:49:21.578634 UTC, before every copied/native action.
It was never accessed again. Final full release was 15:50:33.810658 UTC. Both
marker/inode-owned PG roots, all TMP/PKI, test/controller process groups and
artifact handles were absent; retained source/failed artifacts were preserved.

Conservative whole usage peaked at 3,456,806,912 bytes under the 5-GiB cap;
minimum observed free space was 18,815,324,160 bytes. Peak counts were eight
build processes, two rustc processes and sixteen native processes, including
actual PostgreSQL children. Prior failed artifacts and 50,053,120 bytes of target
growth remain charged in the nonshrinking carry. Corrected target allocation
growth was zero; no APFS shrink is used to reduce historical accounting.

The named Root publisher receives the meaningful committed functional handoff
and performs normal publication. One canonical publisher receipt is sufficient
for remote delivery facts; no repeated owner readback/proof-only commit, new
goal or ACK wait is required. Unrelated docs-only master changes do not require
rebase/rebuild. QA08/load/performance is `DEFERRED_BY_USER`, not PASS. Original
BE33/BE42 required behavior and their remaining functional gates remain open.
