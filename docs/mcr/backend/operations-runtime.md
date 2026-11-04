# BE36/BE38 runtime bridges

Status: IN_PROGRESS. Source base:
`4052d709fef976d14653351cd8bf78ef85bd1579`. This owner writes only new
`src/operations_live/**`, `tests/operations_live*.rs`, owned support/controller
paths and this document. Root retains lib registration, routing, configuration,
contracts, manifests, CI and new migration numbers. BE39–42 are excluded.
The persistent historical BE36/38 goal is aligned with this resumed assignment
but the goal API still reports BLOCKED; it has not been falsely resumed or closed.

## Implemented composition

`OperationsRuntime` implements Root's `ScopedProductProducer` and receives the
exact existing canonical owner scope and Product frame. It does not acquire a
second scope, seal or complete one, or attest HTTP delivery. Frame identity,
method, target, body, parent digest, operation and original deadline must match.
Closed typed requests and generated C04 response schemas bound transport values;
private inputs have no Debug representation. A dedicated configured AES-GCM key
protects opaque actor/event/operation/filter/page-size-bound pagination cursors.
Cursors expire after fifteen minutes and confer no access.

The retained `PgReports` and `PgEventOperations` producers still own authorization,
capacity, optimistic revisions, idempotency, receipts, transactional audit/outbox,
failure rollback and command gates. `CanonicalAuthority::check_in` registers the
actual caller transaction through `OwnerGuardScope::register_transaction_in`
before the producer's receipt/effects, retains actual canonical Auth/Friend
exclusion, locks current event/admission state and repeats authorization. It
clips authority and SQL statement deadlines without widening an existing
nonzero statement timeout. Role providers must retain their own current event/
actor role and revision fence on the same transaction; membership never grants
an operator role.

The built-in USER report target path checks actual canonical SafetyInteraction
and the target's current event admission under the caller transaction. It
supports zero attachments. Other target owners and READY REPORT media use the
explicit fenced evidence provider; absent evidence closes only those requests.
Notice mutations require the explicit fenced reviewed-source provider; an
operator role alone never certifies source review. Place operations use the
retained real SQL place owner with the same canonical request authority unless
an explicit fenced replacement is configured. No successful stub is supplied.

`committed_audience` uses a fresh database transaction and requires a successful,
matching receipt/outbox resource revision. Reports yield only internal reporter/
current-moderator routing; reporter identity is not a public notification DTO.
The eventual publisher must reauthorize actual recipients with the current role
owner. This seam performs no external publication or delivery attestation.

## Provenance and retained boundaries

All new rendering remains outside backend ownership. No source archive,
manifest, lockfile, migration, shared router, contract or existing producer was
changed. Retained owner references at the base are:

| Existing source | SHA-256 |
| --- | --- |
| `src/adapters/out/repository/report.rs` | `13175731ac5b92b9fd3a8767cb38cf0dfe0fed50b3c9cfd6153b134d988a5ebb` |
| `src/adapters/out/repository/event_operations.rs` | `3f84369f75e180bae61ed0a3a87d25f92ae13df0d83e2e51f7a6b874099f93d4` |
| `src/adapters/out/repository/place/operations.rs` | `bc7307202dc5f54a370bf4f25a3cca8911a66e36e079d1eba3ff8f0b27b9831e` |
| `backend/crates/concert-application/src/admin/mod.rs` | `2022359d0124ec9eb6e78c4f9f80d53fd84508970af966f2f2967b0d3b3773a0` |
| `contracts/generated/mcr-domain.openapi.json` | `79a066b88119e53f6f16876d7e3fe59a18ee956be6ed6d2af7ee1efa187e0feb` |

The first three paths are relative to `backend/services/concert-service`.
Dependencies are existing ring, sqlx, serde, serde_json, prost, sha2, chrono,
jsonschema and async-trait resolutions. OperationsError implements the standard
Error/Display/From traits directly; no service-level thiserror dependency is added. No dependency changes occur.
The SQL timeout expression follows the existing Gifts scoped budget pattern.
The Event timezone is the database's constrained `Asia/Seoul` value.

## Validation and remaining delivery

Rust 1.88 rustfmt and changed-path diff checks pass (exit 0). Added regression
source covers report spoofing/duplicate keys, closed routes and filters,
report DTO privacy, and cursor tampering/expiry/all request binding changes.
These are transport/component checks and grant no positive owner authority.
Compilation and all new Rust/native tests are NOT_RUN pending Root's exact lib
registration and a fresh named shared target lease. Source handoff is a
checkpoint, not task acceptance.

Remaining required work: meaningful current canonical-owner/native transaction
and authorization qualification, source-bound artifact freeze, unconditional
shared-target release before isolated private PG, regression evidence and committed
handoff updates/fixes. Root must configure actual role/review/evidence providers
and compose HTTP routes before any affected operator/media/notice flow is
available. Root confirmed no qualified operator-role owner/schema exists at
this base; no role table, self-grant command or JWT role claim is invented here.
Full HTTP, multiuser, publisher delivery and product/release acceptance are
NOT_RUN. Historical seventeen-test BE36/38 receipts certify their pinned old
source only and are not reused as runtime proof for this module.

Native harness source now adds exact three-database Auth/Friend/Concert fixtures
using real signed Gateway parent capture, production mTLS owner stores, Root
scope ledger and authenticated settlement services. The fixture copies identify
upstream path, base commit and SHA-256 in each owned support file; upstream files
remain unchanged. Cases exercise committed member reporting and private outbox
routing, actual dual-owner transaction registration, idempotent replay and own
report privacy, absent operator/evidence rejection without receipts/effects,
parent-frame rebinding refusal before tracking, and actual account revocation
exclusion until terminal settlement followed by fresh replay authorization.
No fixture grants a production operator role or fabricates a positive scope.

`backend/scripts/mcr/operations_live_smoke.py` freezes only the exact debug-zero
`operations_live_contract` and `operations_live_native` Cargo JSON artifacts and
pins the backend/contracts source inventory. Its execution phase uses only
verified copies, creates an owned Unix-socket-only PG16 cluster, authenticates
root inode/token/system/database identity, checks real migration ledgers and
tears down only its known owner prepared transactions. Any forced private
rollback is recorded as cleanup, never runtime success. Unknown prepared work
retains quarantine. The controller never invokes Cargo. Required sequencing is
fresh named lease, compile, freeze, unconditional shared target release/no
handles, then private native execution. At this source checkpoint native
compilation/execution remains NOT_RUN; controller AST parsing and rustfmt pass.

Additional controller regression check: `python3 -B
backend/scripts/mcr/operations_live_controller_test.py` passes 3/3 (exit 0),
covering duplicate/missing/wrong-manifest Cargo artifacts, changed pinned source
or copied binary, and symlink artifact refusal. These fixtures never execute a
fake artifact and do not certify Rust compilation or owner authority. Planning
check rerun passes (exit 0), including all original archive checksums; application
build/tests/deployment remain NOT_RUN. The canonical native source contains four
actual-owner cases; execution remains pending, not an accepted task status.

## Request-scoped provider interface correction

The new module now exports `ScopedOperationsRoles` and `ScopedReportEvidence`.
Both `check_in` methods receive `&Arc<OwnerGuardScope>` before the actual
`&mut PgConnection` and their retained typed query/input/time parameters.
`OperationsOwners.roles` and `.evidence` use these scoped provider traits. The
retained repository's `FencedOperationsAuthority` and `FencedReportEvidence`
interfaces are unchanged; the new wrappers bridge them explicitly.

This closes a composition gap in the first source checkpoint: a target owner
otherwise lacked the exact canonical scope needed to enroll actual target
subjects, and a role owner lacked the scope needed to retain actual disclosure
fences through Root delivery. Implementations must reuse this scope, keep
current role/target/media revision fences through the protected transaction,
and keep a privileged-read role owner's actual fence through delivery terminal
with supported durable settlement. Creating a second scope, trusting only a
role snapshot, or acknowledging an unregistered child is not qualified. The
interfaces do not themselves implement those absent owners or certify their
behavior. Default missing providers still reject only their affected requests;
the actual member USER/no-attachment path and retained SQL place composition
are unchanged.

Rustfmt, changed-path diff check and the three controller boundary regressions
pass (exit 0). Rust/native checks for the corrected interface remain NOT_RUN
until exact Root registration and the queued named target allocation.


## First two-artifact build and signed-parent regression (2026-11-04)

Root explicitly delegated only the one-line `pub mod operations_live;` insertion
in the service library; separate commit `a314e85fdc73a686cc71e1e7efa0d92cdc7371af`
preserves every other export. No root router, action variant or manifest rights
were acquired.

A fresh named BACKEND allocation was accepted at that clean pin for exactly
`operations_live_contract` and `operations_live_native`, one Cargo invocation:

```text
cargo test --locked --offline -p concert-service --test operations_live_contract --test operations_live_native --no-run --message-format=json -j 2
```

Rust 1.88, dev/test debug zero and incremental zero were used. Fresh process/
target handle scans were empty and free space was 10.440 GiB. The first attempt
exited 101 in 2.011 seconds: a cached contracts-build library embedded another
worktree's protobuf include path. Binary inspection confirmed it; only that
package's eight cached entries (835,883 bytes) were removed. No broad cache,
source or manifest was changed for the repair.

Subsequent bounded own compile fixes were:

- `e43c1042e7855f83515f3c6f53a9dcf5c6ec5a3f`: replace an undeclared service
  thiserror derive with standard trait implementations. Correct the real parent
  digest to hash compression flag zero, four-byte big-endian message length and
  the full prost payload, matching platform VerifiedParentAuthority, and require
  the actual ProductService Dispatch parent method. The BE39 owner independently
  identified this real parent-frame issue. Add a production signature/workload-
  guarded captured-parent regression, with authority-field and target rebinding
  rejection, rather than a synthetic GuardBinding.
- `fa83a9da924218898ec653b487ce6e4fbaaf7420`: keep the request body distinct
  from the collected RPC response body in the owned capture fixture.
- `0f94b15eed8164230c9db0946ef17f98f15be99e`: explicitly type the native
  fixture's delivery secret as 32 bytes.

The identical two-target command finally passed at clean `0f94b15e`, exit 0 in
4.043 seconds for the final invocation, with 10.180 GiB remaining. Earlier
compile attempts remain recorded outside the checkout; no native success is
inferred from compilation. The existing parent-capture regression increases the
contract inventory to five; native inventory remains four.

Both exact Cargo JSON executables and 1,925 backend/contract source pins were
frozen outside BACKEND. BACKEND was unconditionally released with no handles or
own compiler processes before any copied test execution. Frozen pin:
`<owned-evidence>/mcr-operations-live-frozen-rxa0b2yl/pin.json`, SHA-256
`699a64aae434ee3f8b090238d7fdde5d3a0d3b7cf2150a416c70c93b5d2e8b6b`.
Copied contract binary SHA-256:
`72f3565efbe288048e410e4e3d85a30b8834aba79170bc368cc1ac595deb9b43`.
Copied native binary SHA-256:
`6552ab537f93665f7424b4e7bbc3153ac4f84a4908cadc3cb6c18b9a662f5707`.

The copied controller run **FAILED**, exit 1, before PostgreSQL startup. Contract
execution exited 101 in 0.487 seconds: four cases passed, while the actual parent
case correctly rejected an invalid owner binding. The owned capture helper had
shadowed its HTTP method parameter with the RPC method constant. Correction
`07b7f61e7f77a0e3a4e36004ff636e87e8ffca17` separates `http_method` and
`grpc_method`; no production guard check is bypassed. The failed native report
is preserved at
`<owned-evidence>/mcr-operations-live-frozen-rxa0b2yl/native-report.json`, SHA-256
`50e7debec471a10d5c9e47567f662fcc30aead57d7db9b23f6b78d622083db5d`.
No cluster started, no native cases ran, and the source correction invalidates
that older frozen source inventory for reuse. A fresh named allocation is
required for the same two targets before corrected copied qualification.

Status remains IN_PROGRESS, corrected Rust/native validation NOT_RUN. Absent
actual operator-role, reviewed-notice and additional target/READY-media providers
do not waive any required criteria. The separate qualified Public Profile
0824d8c7 source/receipts remain untouched.

## Corrected capture build and native failure — 2026-11-04

Fresh allocation `OPS-RETRY-BACKEND-20261104T0408Z` was explicitly accepted for
the same two targets at clean `33271b2d20716421e9ad764089f2cb9b0a847784`.
Rust 1.88, locked/offline, two jobs, debug zero and incremental zero were retained.
Fresh process and target-handle scans were empty; free space was 10.132 GiB.
The first invocation exited 101 after 2.85 seconds because the shared
contracts-build cache embedded the BE39 checkout path. Only its eight proven
cache entries, 835,882 bytes, were removed. The second exact invocation passed,
exit 0 in 20.995 seconds, with 10.111 GiB free.

Both executables, Cargo JSON and 1,925 source pins were frozen before the shared
target was unconditionally released, with no compiler or executable handles.
Pin: `<owned-evidence>/mcr-operations-live-frozen-9tlj5lhh/pin.json`, SHA-256
`9da1d045d43bc235ce5211f5b03135ccc353d17076c32f0f97172ac5679725e4`.
Contract binary SHA-256:
`8acd741dd29bcf82f982f938afbbe340cda09c043eeb4b365c0c6e6a90440a91`.
Native binary SHA-256:
`7775a751e12eb492af194ddcc619ba98a651cc5452a8f03282cda376b3671f1b`.

All five copied contract cases passed, exit 0 in 0.621 seconds, including the
actual signed-parent capture. Contract log SHA-256:
`4fc24b451ea3fd5dbc0c913a7790466cdff9fc90c4daddd56acd8e98971f0c1c`.
The native batch failed its first case and timed out after 60.014 seconds during
the second. No native case is qualified. Native report SHA-256:
`1c0782cadd72d2a5b780bb2ccbaa074de765f73324780375cd18077d47a15b04`.
Its private PostgreSQL 16.14 cluster used max_prepared_transactions 256. Cleanup
required two forced private owner rollbacks; zero prepared transactions remained,
the server stopped, and its exact owned root was removed. Cleanup is not a pass.
Measured combined new compile/frozen/runtime output peak was 554,258,432 bytes
(0.5162 GiB), below the explicit 1.0 GiB cap; free space was 9.774 GiB.

A separate copied diagnostic selected only
`absent_operator_or_evidence_provider_has_no_receipt_or_effects` with `--exact`
and `--nocapture`, on another fresh owned three-database cluster. It failed,
exit 101 in 0.881 seconds, with `Owner settlement incomplete`. The original
qualification report was preserved. Diagnostic report:
`<owned-evidence>/mcr-ops-retry-0408-97fpqr4b/diagnostic-first-report.json`.
Diagnostic log SHA-256:
`ea65d5dd1a786bda84a178a2a90c055408fb4510b475a50ee4a2be5f637ed3ba`.
This cluster also required two forced private rollbacks; zero prepared work,
stopped server and exact root removal were verified. All build, failed batch and
diagnostic evidence under that retry directory remains preserved.

Static inspection identifies a settlement gap: the fixture configures
`PgNativeAbortResolver`, which intentionally leaves commits UNKNOWN, while
Operations registers retained read and command transactions that can commit.
No Operations same-transaction commit witness exists yet. The exact failing
operation within the multi-operation negative case is not established by the
old log. The owned fixture now adds operation/request context to terminal errors,
and the controller uses `--nocapture` so an early failure survives a later timeout.
These harness edits do not change authority, resolver outcomes or business logic.
They invalidate the older pin for rerunning against the changed source inventory;
their Rust build/native execution is NOT_RUN pending a fresh named allocation.

The shared build target and the entire workflow reservation are unconditionally
released. Work is source-only with no implicit target, executable or PostgreSQL
hold. An exact shared-schema transfer has been requested from the delivery
integrator before introducing a producer-specific immutable commit witness.
Overall BE36/BE38 remains IN_PROGRESS; real operator, reviewed-notice and additional
target/media qualification remains open.

## Operations native transaction witness source

The delivery integrator explicitly delegated only new migration
`backend/services/concert-service/migrations/0047_operations_native_witness.sql`.
Reserved 0044/0045/0046 migrations and other shared paths remain outside this
assignment. The separately delegated Social receipt hook was copied unchanged in
dependency commit `54c5ab87`: source commit
`fe86cafc34cb8cc11e5b9573d4a64e3c868a3732`, exact `receipt.rs` blob
`5c52fe67de67fbe9d3a3208efb460d5cb9006dd6`, source patch
`<owned-evidence>/mcr-social-central-fe86-gcej2zlq/social-core.patch` lines 159–200.
Its method is `CommandProducer::completed`, called immediately before commit for
verified replay and fresh receipt projection. Its default returns no proof. This
copy does not claim that the dependency has reached master; Root integrates the
same postimage serially. No other Social source is copied or altered.

New `operations_live/settlement.rs` captures the actual Concert transaction and
registers it with the existing canonical scope before protected work. The exact
full verified Product request digest remains its effect binding. After current
admission/role checks succeed, CanonicalAuthority writes an immutable native
witness on that same caller connection. Existing retained producers remain the
business-rule owners; neither `report.rs` nor `event_operations.rs` is edited.
Because those private producers do not forward a completion port to Operations,
the native witness uses a deferred SQL receipt guard rather than assuming the
shared hook's default is evidence.

The INSERT trigger checks actual system/database/full XID/backend identity,
the exact confirmed Root registration and parent binding, a published ACTIVE
scope, and the full-request digest/reference. It retains a SHARE lock against
Root seal through transaction completion. UPDATE, DELETE and insertion from a
different transaction are rejected. A deferred check requires the actual matching
command receipt when a key is present, permits its real PENDING/SUCCEEDED/FAILED
state, and checks the original request expiry. Authorized reads have no command
receipt requirement. Rollback removes the witness and any local effects.

`OperationsNativeResolver` retains the existing native abort/in-progress proof.
COMMITTED additionally requires the exact immutable witness and confirmed
ROOT_FINISHED ledger, matching parent, instance, scope, request digest and full
native identity, plus actual `pg_xact_status='committed'`. Safe evidence is labeled
`OPERATIONS_NATIVE_TRANSACTION_V1`; it contains no business success state or
private report text. Missing/mismatched witnesses remain UNKNOWN. Root production
activation is separate from this module and the owned native fixture.

The native fixture now uses this real resolver. Two new cases check immutable
witnesses, active-scope refusal, actor/request-digest mismatch, an actual committed
transaction with no witness, and a real FAILED duplicate-report receipt settling
without an additional outbox effect. Existing successful report, verified replay,
read, negative provider, real Auth revocation and parent rebinding cases remain.
The controller requires all five contract and all six native cases to pass.
New migration/Rust/native validation is NOT_RUN until a fresh named build and
copied runtime allocation. Source formatting and provenance checks are separate
from that required proof. Operator/review/extra-target/media criteria remain open.

## Qualified native witness and restart — 2026-11-04

The preceding NOT_RUN state is superseded for exact source
`4353a0e5aa5acd13692e3baad14b9c2d96c35b23`, child of witness implementation
`9808edaa3dfda9a98da988d3e476a3fc89c7ae72`, exact Social dependency `54c5ab87`
and preserved diagnostic commit `eaea282f`. The isolated branch remains rooted at
`4052d709fef976d14653351cd8bf78ef85bd1579`. Only the assigned Operations paths,
explicitly allocated migration 0047 and exact delegated Social receipt postimage
were changed. Existing original failure evidence was preserved.

Named offer `OPS-WITNESS-MEDIA-202611040512Z` was explicitly accepted before
accessing `/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`. Fresh clean
9808 source, 38,305,308 KiB free, zero target handles and zero Rust/Operations
processes were verified. The owned fix window added only the requested restart
controller check in 4353; no Rust semantic change followed 9808. Python provenance
boundaries passed 3/3, exit 0; source formatting/diff checks passed. The planning
checker had also passed with all 13 original archive checksums intact; it remains
planning-only evidence.

The exact two-target Rust 1.88 locked/offline command, two jobs, dev/test debug zero
and incremental zero, passed at clean 4353, exit 0 in 25.940 seconds. An initial
invocation exited 101 after 1.522 seconds because the cached contracts-build
library embedded the canonical-cancellation checkout's protobuf path. Binary
inspection confirmed that cause. Only its three library/dependency files and four
corresponding library fingerprint files, 27,531 bytes total, were removed. No
broad clean, new target, manifest or shared source change was used. Both attempts'
Cargo JSON and stderr remain outside the shared target under
`<owned-evidence>/mcr-ops-witness-media-shjmbvp7/`.

Both exact executables and 1,927 backend/contract source pins were frozen at
`<owned-evidence>/mcr-operations-live-frozen-k9fon2i_/pin.json`, SHA-256
`d008b192897b0ba7df44c54505837114d300aa3990f454f12d9e140313b676c6`.
Contract binary: 18,410,496 bytes, SHA-256
`98fa580020e3292ecb5ea0cdd54c55a490d7b06ef30740075ea4ee5a19889586`.
Native binary: 43,754,544 bytes, SHA-256
`943666ffd99219d9d80fecae514c3a8d682f7abdac18b464c3881413aca74082`.
The shared MEDIA target was unconditionally released with zero handles before
any copied test/PostgreSQL execution. Other workers' processes were untouched.

The copied controller exited 0. All **five contract cases passed**, exit 0 in
0.490 seconds, and all **six native cases passed**, exit 0 in 3.321 seconds.
Contract log SHA-256:
`4fc24b451ea3fd5dbc0c913a7790466cdff9fc90c4daddd56acd8e98971f0c1c`.
Native log SHA-256:
`5243d671d32c0dd27fad58ade629bb4727b82c7d0eef56801aaad1b5f5f7c90b`.
The native cases exercised actual Auth/Friend owner enrollment and settlement,
current Auth revocation blocked through terminal delivery, receipt replay and
private report invalidation, denied operator/evidence providers with no effects,
parent rebinding before registration, immutable native witness checks, active
scope/mismatched binding/missing witness rejection, and a real FAILED duplicate
receipt settling without a second successful invalidation. PENDING receipt support
is a deferred SQL rule, not a separately exercised business path in these cases.

The private PostgreSQL 16.14 cluster used max_prepared_transactions 256. Actual
migration ledgers matched Auth 17, Friend 17 and Concert 28, including new 0047.
After all native cases, zero prepared transactions were required before restart.
Restart changed the postmaster; immutable witness, actual receipt-state and
outbox digests remained identical. Migration ledgers remained identical, and
every stored witness still had native `pg_xact_status='committed'`. This is restart
persistence evidence; it does not claim a restarted product RPC or Gateway flow.

Native report:
`<owned-evidence>/mcr-operations-live-frozen-k9fon2i_/native-report.json`, SHA-256
`9a891a713cec370b4897119ae4ad908d05d27e69642e862d80c86b4f1f4f7efb`.
Cleanup required **zero forced private rollbacks**. Zero prepared work, stopped
server and exact owned root removal (`<owned-evidence>/mcr-pg-c3m_6h_q`) were verified.
The copied runtime reservation was unconditionally released to the scheduler.
Measured combined new compile/frozen/runtime peak was 555,581,440 bytes
(0.5174 GiB), below the 1.0 GiB cap; remaining free space was 36.323 GiB. There are
no remaining target, executable, workflow or PostgreSQL holds for this allocation.

This closes the bounded Operations native commit-witness regression on the named
source cohort. It does not identify which operation caused the older diagnostic
failure, alter the historical failure result, activate Root's production runtime,
or establish current operator/review/additional-target/READY-media providers.
Whole BE36/BE38 remains IN_PROGRESS with those required criteria open. Root owns
integration/publication; these commits have not been pushed directly by this lane.
