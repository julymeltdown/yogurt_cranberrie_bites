# BE33-DELETION-RECEIPT-01 — Notification deletion proof

Status: `CONTRACT_READY`; compilation and all ten scoped checks passed.
Normal publication and Root-owned grant provisioning remain pending.
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
fixture pins and whole retained accounting. The completed finite reservation and release are recorded below. Freeze one emitted test executable outside the target and release the
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

The restart seed/recovery are ignored in ordinary harness
execution solely so the owned external runner can perform the real restart between
those exact phases. Source-only compilation/formatting cannot certify any of
these native behaviors. Existing unrelated provider/startup tests are not rerun.
Actual commands, exit codes, fixture/runtime pins, cleanup and publication proof
will be recorded after the authorized finite qualification round.


## Completed qualification and retained evidence

Qualified clean source: `373f65c19b66f030a4aa5a5f691924c9beeff964`.
Implementation stack: `fe743e77`, source-only fixture refinement `b74fcd34`,
SQLx epoch precision fix `373f65c1`; no Rust/source fix followed compilation.
The pre-accept precision fix preserves the retained SQLx 0.8.6 codec's
microsecond truncation relative to 2000-01-01, including past deadlines. Exact
nanosecond command binding is separately retained and exercised natively.

Offer `BE33-DELETION-RECEIPT-MEDIA-202611041421Z` was explicitly accepted
2026-11-04 14:26:20.065841 UTC after its source pin was amended. Literal target:
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`. Original absolute
compile/freeze deadline 14:36:20.065841 UTC; work 14:39:20.065841; full cleanup
14:42:20.065841. Target release tightened native work to 14:30:18.978969 and
FULL to 14:33:18.978969, preserving independent 180-second cleanup reserve.
No new worktree, target, package installation, peer build or cache deletion.

Exact build, exit 0:

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target \
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline \
  -p notification-service --test deletion_owner --no-run --message-format=json \
  --config profile.dev.package.contracts-build.opt-level=2
```

Both Rust flags were unset. Compiler `rustc 1.88.0 (6b00bc388 2025-06-23)`.
Cargo: 7.50 seconds; supervised archive/build/freeze: 10.90 seconds; copy/freeze:
0.303 seconds. The sole copied test is 14,153,248 bytes, SHA-256
`32e190792ac3f1eb6acd6e57ffd3813c09613818cae98b8299accb64b74e6f58`.
Automatic main artifact metadata is retained; main was not copied or run.

Evidence root: `/private/tmp/mcr-be33-deletion-fe743e77/`. Full tracked build,
contract, SQL, runtime and fixture inputs plus this document were archived and
hash-pinned externally (29,966,353 source bytes). Archive SHA-256:
`61a0aba87f7925fc941d07af86337b5216fbd8ad52747d1390e2e867e679ef6b`.
Cargo JSON/stderr, runtime executable hashes and linked-library inventory are
retained. Artifact receipt SHA-256:
`62fbf0a100b24243e097f3fb0a81377b2fe4902d080aa6f1bf09d38b7153f2e2`.

The target was released **unconditionally** at 14:27:18.978969 UTC after fresh
`lsof +D` exit 1/empty output proved zero handles. No target access followed.
Positive target allocation growth was 14,938,112 bytes and remains charged.

The first private-fixture attempt stopped during `initdb`, before any case ran:
the supervisor incorrectly treated an initialization-only bootstrap PID file
as a ready postmaster. Its failure report, stdout/stderr and original controller/
watchdog files are preserved. The owned fixture was removed. A private helper-only
fix tracks the actual PostgreSQL process tree during initialization and reserves
ready-server identity checks for the helper's validated started state. The same
source, runtime pins and copied binary then ran within the original lease. No
Rust rebuild, source change, target access, clock reset or additional permission.

Successful copied commands (each exit 0), using only the guarded private PG16
socket and allowlisted `mcr_concert_notification` / `mcr_concert_notification_source`:

```sh
/private/tmp/mcr-be33-deletion-fe743e77/deletion_owner --test-threads=1 --nocapture
# 8 passed, 0 failed, 2 intentionally staged restart cases ignored
/private/tmp/mcr-be33-deletion-fe743e77/deletion_owner --ignored --exact \
  restart_seed_committed_receipt_with_lost_reply --test-threads=1 --nocapture
# 1 passed, 0 failed
# Owned pg_ctl stop/start: actual postmaster PID 25708 -> 26088.
/private/tmp/mcr-be33-deletion-fe743e77/deletion_owner --ignored --exact \
  restart_recovers_exact_receipt_and_permanent_deny --test-threads=1 --nocapture
# 1 passed, 0 failed; postmaster start timestamp must differ too.
```

All ten scoped cases ran successfully. Regular suite: 0.57 seconds; successful
private setup/qualification/restart/cleanup: 15.09 seconds, cumulative native
attempts 16.25 seconds. The seed deliberately discards a committed response; this
is durable retry/reply-uncertainty evidence, not a claim of a transport packet
fault or real Auth coordinator journey. Both databases have seven exact SQLx
migration SHA-384 receipts. The 0006→0007 case uses actual SQLx migration execution;
mutable historical rows are preserved and cannot retrospectively ACK. The runtime
role case uses a real disposable non-owner role and minimal required fixture
grants. There were zero prepared transactions and zero leftover fixture roles.
Qualification receipt SHA-256:
`e7a14a32978a06c2e785234bb03ebda0df558a13582470ea5d1652c494b414de`.
0007 SHA-384:
`30bc40628a1bbf2690645b7da7e63ca940929b48afec7ea81541ea022857232ae56263ac975976c03d084ff69b780bb8`.

Independent watchdog and in-process guards enforced time, whole bytes, physical
source/Git growth, outside/target/copy bounds, free space and ALL owned processes
before child admission and during work. Accepted conservative carry:
838,860,800 historical bytes + 1,236,221,952 full physical checkout bytes +
1,068,015,616 full common Git bytes = 3,143,098,368 bytes. Nothing was omitted or
reset when allocation fluctuated. At release, positive source/Git growth 1,568,768,
retained target growth 14,938,112 and all private output 21,725,184 gave a
3,181,330,432-byte snapshot. Whole peak: 3,256,381,440 < 4 GiB; minimum free:
27,160,539,136 bytes > continuous 8 GiB. Peaks: all compile processes 6, rustc 1,
all native processes 13, within 12/2/28 limits. Source/proof growth after this
snapshot remains charged; evidence and prior history are retained.

FULL release: 14:30:13.038435 UTC, 174.06 seconds after target release, before the
360-second bound and the independent cleanup deadline. Owned PG/TMP was removed;
fresh private `lsof` and tracked process inventory proved zero copied handles or
owned processes. No PKI, application or TCP service ran. Full-release SHA-256:
`e4af55e327ce161a04149bc595c83541c549e4e31369de52c4238be01c0f5359`.

## Publication boundary

Only the seven owned paths and the qualified implementation/evidence commits go
to Root. Root owns runtime-grant provisioning and normal master publication.
Its active RW11 seven-directory migration-input window remains immutable; this
owner's qualified 0007 must not be intaken there until actual RW11 FULL release.
That input hold is resource/publication routing, not technical approval. This
writer stops after committed handoff and retains concrete developer-owned fixes.
Own fresh remote ancestry/postimage proof follows normal publication. Full BE33,
BE42, private-data erasure and public IN03 remain unaccepted by this evidence.
