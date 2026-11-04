# BE42 PrivateData core deletion

## Current resource-procedure correction (2026-11-05 KST)

The bounded core source is published at `52ab02173eb7c0d240d4765a267155eb97c6e3c0`; whole PrivateData/BE42 and canonical Auth-to-Concert runtime composition remain INCOMPLETE. The previously reported 44-ledger runtime results must be separated from resource admission: **RESOURCE_PROCEDURE_FAIL**, not a scheduler-qualified allocation PASS. The responsible developer's read-only `compile-provenance-audit.txt` records the actual Cargo artifact root `/Users/lhs/Desktop/mcr-build/backend`, source `7acf9b85a344429aa3e587a5469af74251b2e7bc`, compile finish 2026-11-04 15:44:32 UTC, freeze 15:44:47 UTC, and **target removal** 15:44:54 UTC. Removal of the shared target was not ordinary lease release and has no retrospective authorization. The reported environment used nonempty `RUSTFLAGS=-C debuginfo=0`; the exact raw invocation argv was not separately captured. Cargo JSON proves emitted paths, but does not supply the missing argv/environment receipt. Cleanup was reported at 15:45:38 UTC. The raw source archive, readset, executable hashes, failed fixture attempts and native observations remain preserved under `/Users/lhs/Desktop/mcr-evidence/be42-private-data-44ledger-202611041544Z/`.

The earlier 3 source checks, 2 writer-fence checks and native erase/restart results are retained as **owner-reported bounded-core results with these provenance/resource limitations**. They do not certify whole PrivateData, repair the resource procedure, or establish a new coherent Root/runtime cohort. Root's independent current-cohort qualification is NOT_RUN while its fresh resource allocation is unavailable. No target was inspected, recreated or deleted for this correction.

Status: **IN_PROGRESS**. The bounded core leaf is implemented here. Whole
PrivateData deletion remains **INCOMPLETE** until every owner category in the
[field inventory](account-private-data-field-inventory.md) has a real owner
step and the Auth-to-Concert transport is qualified.

The implementing developer owns the ten leaf paths named in the original
assignment, the native qualification file
`tests/account_private_data_native.rs`, and the single
`pub mod account_private_data;` export hunk in `src/lib.rs`. Root confirmed
both latter grants explicitly; no other `src/lib.rs` hunk belongs to this
delivery. Shared Auth-to-Concert composition, common deletion sources and
publication remain Root responsibilities. A successful leaf receipt is not a
whole PrivateData acknowledgement.

## Receipt and authority

The public entry is `PrivateDataCoreDeletion::apply` using the shared
`WorldDeletionCommand` with `WorldDeletionTarget::PrivateData`. The command must
pass through the mandatory `DeletionAuthority`, which supplies a nonzero source
binding and exact original deadline. The deadline must be future and no more
than five seconds. Re-verification occurs before commit; a changed source or
bound command rolls back the transaction.

The core receipt binds the user, saga, actor, nanosecond retention timestamp,
target command digest and verified source digest. Its fence revision is one,
and the receipt cannot be rewritten to a different command. The durable cursor
is a UUID lower bound over `(user_id,event_id)`, while scan, neutralized and
removed counters advance monotonically. Each transaction handles at most 128
event ids, and repeated application resumes from the committed cursor.

## Lock and transaction order

One shared PostgreSQL advisory lock, keyed by the canonical Auth user, is taken
first. It precedes event, admission, participant, avatar and authority locks.
The participant preference writer, avatar writer and prepared Profile
coordinator use the same lock before their existing row locks. This prevents a
normal writer from passing a pre-fence check and committing after the erase.

The core receipt, permanent fence, participant neutralization, avatar removal
and transaction-local controlled bypass commit together. A transaction-local
setting is accepted only after authority verification and the user-first lock;
the resulting trigger prevents every other direct personal write and delete.

## Dispositions

Existing participant personal settings become `PARADE` with low power, reduced
motion and sound all false. The participant key stays because it is the
canonical aggregate reference. Existing avatar selections are removed. Other
personal values and their owning services are listed in the inventory and must
not be inferred from core success.

## Validation boundary

The source tests cover command binding, deadline/source failure, redaction and
lock precedence. The ignored native test is intended for a marker-owned fresh
PostgreSQL database and covers multiple events, non-target user preservation,
idempotent replay, the permanent write fence and receipt completion. Rust 1.88
locked/offline compilation and the native run require a fresh named allocation.
The two additional grants authorize the native test and exact module-export
hunk only; historical pinned-source receipts do not qualify a later tree.
Fresh bounded allocation `BE42-PRIVATE-DATA-RESTART-BACKEND-202611041534Z`
passed on source `9575be685a4dd2f0fde55e0e7b25668f960840a6` (parent
`f167a770c308060df6d87db175e13dd0219454c7`; native postimage
`f72f85ce9ee17cc52e349c5308bb745d239e5823002b13df0dcf95e7f138db05`). The build
used Rust 1.88.0, locked/offline dependencies and two jobs. After the target
was released with zero handles, the copied source suite passed three cases,
the writer-fence suite passed two cases, and one fresh PostgreSQL 16.14
database applied all 43 migrations. The native case completed the receipt at
`COMPLETE|131|131|131`, neutralized 131 participant rows, removed 131 avatars
and left one permanent fence. A physical stop/start of the same owned database
kept the receipt unchanged; replay without reseeding passed, with zero
prepared transactions. The owned database was then stopped and removed. A
previous run at source `ebbd27ce` passed its pre-restart case but its fixture
attempted to seed fenced rows after restart; that failed attempt is retained.
Evidence is under `/Users/lhs/Desktop/mcr-evidence/be42-private-data-restart-202611041534Z`.
This qualifies only the bounded core leaf.

The final merged tree `7acf9b85a344429aa3e587a5469af74251b2e7bc` added one
peer migration after that pass. Fresh allocation
`BE42-PRIVATE-DATA-44LEDGER-BACKEND-202611041544Z` recompiled this exact tree
with Rust 1.88.0, two jobs, locked/offline dependencies, and the same three
tests. After zero-handle target release, copied checks passed three plus two
cases. A fresh PostgreSQL 16.14 database applied 44 migrations and recorded
`COMPLETE|131|131|131`, 131 neutralized participants, 131 removed avatars,
one fence and zero prepared transactions. Physical restart and replay without
reseeding retained the same receipt and state. The owned database was then
stopped and removed, with zero target handles. Evidence is under
`/Users/lhs/Desktop/mcr-evidence/be42-private-data-44ledger-202611041544Z`.

This leaf does not provide canonical Auth transport, production route wiring,
foreign service database writes, or full BE42/IN02 acceptance.
