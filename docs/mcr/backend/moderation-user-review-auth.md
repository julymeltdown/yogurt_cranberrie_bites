# Auth lifecycle for privileged Concert member review

Task: `BE39-USER-REVIEW-AUTH-01`. Status: `IN_PROGRESS`.

## Objective and completion criteria

Provide current Auth-owned lifecycle evidence for arbitrary moderation targets,
including deletion pending and deleted accounts, through a closed purpose-bound
read/check/settle protocol. Authenticate the actual caller and sealed parent,
retain the actual Auth/root purpose, recheck current authority, and settle only
the exact admitted read. Missing or contradictory evidence remains unavailable.
Profile/session absence cannot establish deletion. Concert retains event-scoped
suspension and privileged Profile projection; ordinary AuthSelf is unchanged.

Complete the assigned implementation and meaningful source, native and runtime
validation; fix failures; record exact source/contract provenance, commands,
results and limitations; and deliver a committed immutable handoff directly to
publisher412e. There is no king/SL review or acknowledgment stage. Runtime
qualification requires real Auth state and native purpose/root ownership rather
than positive test fixtures. Unexecuted checks remain `NOT_RUN`.

The goal API was inspected for this assignment. It still contains the incompatible
historical BE33 objective in `blocked` state. Its metadata is preserved; it has
not been reset or falsely completed. This document records the concrete BE39
objective while that unfinished goal prevents creation of a separate goal. No
token budget is set. BE42's distinct unfinished objective remains unchanged.

## Custody and resource boundaries

Use the existing exclusive Auth worktree with branch
`codex/mcr-be-BE39-user-review-auth`, based on
`2f36bc22293e8f06431ea7c02f34be94868f8440`. The frozen compiled BE42 input remains
`cd1d844ae14a5fa463399622482eedf4e5e21fe6`; its executable and original receipts
are untouched. Root's separately frozen Auth19 main is outside this assignment.

Owned changes are the new Auth `concert_moderation/**` module, dedicated gRPC
adapter, sole module exports, exact new method-policy entries, dedicated proto
and its build input, dedicated tests/support, this document and task evidence.
Migration0021 is the new durable purpose/read schema, authored after protocol
agreement. Historical SQL, main/config, ordinary AuthSelf, BE42 dispatch and
platform code are not writable. Owner205 owns the shared sealed-capture leaf,
privileged Profile purpose and Concert consumer/composition. Coordinate that
closed API before changing its protocol. Root retains literal main/native-resolver
mounts; supply a typed factory and precise mount request.

Resource owner21d continued the existing aggregate128 MiB source allowance until
2026-11-05 01:00 UTC. The initial resample charged a conservative88,475,042 bytes,
leaving45,742,686 bytes; free space was14,748,184,576 bytes, above the required
8 GiB plus remaining growth. Subsequent positive source/Git/proof growth remains
charged. This is source admission only: no compiler, target, PostgreSQL, tests,
new stack or native child is admitted. Later execution requires a distinct finite
offer. Existing unique artifacts and receipts are preserved.

## Authoritative source mapping

All source observations below are static at the Auth branch base:

- `users` owns account status and `deleted_at`. Its domain statuses are
  `PENDING_VERIFICATION`, `ACTIVE` and `DELETED`; it has no event suspension state.
- The actual deletion repository's `begin` transaction marks the user deleted,
  creates the canonical deletion saga and all eight required targets, revokes
  sessions and records the Auth audit. `DELETED` in this row alone therefore
  cannot distinguish pending cleanup from completed deletion.
- `user_deletion_sagas` and required steps distinguish incomplete/dead-lettered
  work from an actual completed saga with terminal receipts for all eight targets.
- The permanent `auth_user_tombstones` trigger records physical retirement.
  The existing invalidation journal records `USER_REMOVED`; neither a missing
  Profile nor a missing refresh session is deletion provenance.
- Existing native owner authority retains prepared source exclusion and an
  Auth-owned revision. Its `SubjectState.active` may be false for an arbitrary
  target while the moderator actor/session remains active. The new moderation
  purpose must not silently reuse ordinary active-self eligibility.

The lifecycle projection returns ACTIVE only from consistent current active
Auth state; DELETION_PENDING from an actual incomplete canonical deletion saga;
and DELETED from a verified completed saga or permanent physical tombstone.
Unknown, unsupported and contradictory combinations remain unavailable.

## Frozen contract and custody

The dedicated proto is pinned at
`2eaeae8d9811c1590b2df64f7b684d50c2de1c89`, SHA256
`d9da85a1db8ecd545991091ed57c638ba811b5c4ff62f4e69fe4c7e66a5d4d1b`.
Its preceding moderation-only checkpoint was
`ee626c385ef76bc278e2e5d7be8fae9647efea9f`; successor2aee2da51 adds the separate QA09 observation protocol/import, and
2eaeae8d9 adds ReadInspection.native_status=5 with a closed outcome enum. Existing
fields and methods remain unchanged. Package: `auth.concert_moderation.v1`.

The purpose includes exact actor/event/target/nonce, Auth/Friend/root views,
canonical original ProductRequest bytes, original expiry, and a private completion
secret allocated durably by Concert before dispatch. Its digest is
`SHA256("MCR-MODERATION-USER-REVIEW-V1\\0" || ReadPurpose.encode_to_vec())`.
Only actual `listConcertAdminUsers` GET directory queries and
`moderateConcertUser` POST target moderation are supported. A fictitious GET
single-user endpoint is refused. Concert owns DB-selected directory admission,
role/publication checks and immutable migration0100 custody. UUIDs alone never
admit a target. Auth contributes lifecycle, not event SUSPENDED or Profile data.

Migration0021 retains nonce, immutable purpose digest/bytes with completion secret
removed, completion-secret hash, actual native transaction identity, lifecycle
and opaque Concert custody reference. Reservation commits before callback or
PREPARE. The lifecycle transaction retains canonical saga and all eight step
SHARE locks in saga-before-step order. It does not acquire a second global/user
fence behind a mutation waiting on the root's already-held Auth fence. Current
actor/session authority is checked with the existing concrete PgOwnerAuthority.
Unknown target sets/statuses or contradictory step/saga metadata are unavailable.
Since physical preparers detach from pool permits, a shared16-slot admission
semaphore bounds in-flight native read preparers across store clones before
connection acquisition. Waiting consumes the original operation budget. The
separate durable pending/prepared capacity checks still bound retained custody.

ReadCurrent qualifies the actual native attempt with Concert, prepares its source
transaction, records HELD, calls Concert CheckRead and rechecks native/current
Auth authority. Read and Check share their original bounded operation stop.
Native identity binds the full xid, backend start/PID, actual system/database/role,
generated GID and exact purpose digest; absent prepared rows never imply abort.
`pg_xact_status` must prove a native terminal outcome.

InspectRead permits exact-purpose metadata discovery when the original Read ACK
was lost. Native None returns only this owner's stored original identity; supplied
Some must match. RESERVED/aborted remains RESERVED metadata with no read grant;
HELD requires an actual prepared transaction; SETTLED requires actual abort.
Unknown nonce is unavailable. The new native_status distinguishes owner-verified
IN_PROGRESS/PREPARED/ABORTED from the durable attempt state; no-native CANCELLED
is UNSPECIFIED, never invented ABORTED. Unknown/foreign/committed outcomes remain
unavailable. Qualify requires actual IN_PROGRESS or PREPARED; Check requires
PREPARED. Inspection does not grant new lifecycle authority.

Settle discovers immutable local custody without a row lock, then calls actual
Concert ResolveReadTerminal with the stored native identity before locking its
record. Concert requires actual `delivery_completed_at` or
`cancellation_settled_at`; ROOT_FINISHED, expiry and caller outcomes cannot suffice.
After exact tuple/custody revalidation, Auth rolls back its actual prepared
transaction and proves ABORTED before SETTLED. Native None handles lost ACKs.
Replies echo the original incoming control; callback requests use actual stored
control. A no-attempt terminal creates a CANCELLED tombstone so a late Read cannot
reserve the nonce. Discovery/reservation races fail closed for retry. Uncertain,
committed or live outcomes retain custody rather than releasing by expiry.

Owner205's Concert post-terminal drain owns readiness/shutdown/recovery routing.
These children remain outside the old child-intent settlement loop, avoiding an
early-release dependency cycle. Repeated reads cannot reuse a nonce; Inspect,
Check and idempotent Settle provide explicit recovery paths.

## Exact production composition request

Root's existing Auth process must construct `RpcLifecycleAuthority` with its
actual configured mTLS Concert Channel, Auth credential and Auth delegation signer;
construct `ModerationLifecycleStore` with clones of the SAME existing Auth pool
and configured `PgOwnerAuthority`; then mount `moderation_server` with its actual
caller registry and Concert verifier. No second Auth identity, pool, pair,
listener, synthetic resolver or positive fallback is introduced. Root retains
main/config/native resolver ownership. This branch does not claim mounting.

The sealed capture dependency is owner205's
`d96aa5ed98b38fff68aad8999b2117101dbd95d3` leaf
`platform::actor_delegation::concert_moderation::VerifiedModerationRead` plus its
exact export/remove/capture additions in `actor_delegation.rs`. It is installed
only after actual caller/signature/audience/actor/body/deadline verification.
The exact reserved method-policy leaf/tests come from owner205
`782413daecc0a9ada613be4467a0bef0031a6537`, with the three validator additions
composed by owner21a in `50bfb988c5d3a0f82fc337ba4a097dfeed982648`.
The entire Post commit is not an Auth dependency. These shared dependencies are
unqualified source. The exact policy slice was subsequently consumed from
owner21a packet `e7a588fa3ba27b0bc566dcfd286e510e11fbd08b` in dependency-only
commit `2adccf938ee3ee8d458179e4e8f420c90c89a0ee`; preimage/postimage hashes match.
The exact capture-only slice was then consumed from sole integrator21a packet
`a7bd449b2374e7aaecb8e90bd57e9ec169b4f2b8` in dependency-only commit
`030cf46c559a6e07e1883d6af1812b3e740f4492`. It uses owner205's byte-exact
`4609c43025dcf69575a53813cb427c27a9919075` helper/test successor and the unchanged
three capture additions. Literal preimage/postimage and patch hashes match.
No shared semantic ownership transfers to this Auth consumer.

## Developer validation state

Dedicated unit sources cover lifecycle contradictions, closed purpose bindings,
digest/secret storage, deletion command rejection and exact eight-target mapping.
Dedicated integration sources cover service caller/actor paths and wire custody.
The ignored native drill requires an explicitly allocated isolated MCR PostgreSQL
store and proves actual in-progress/prepared/aborted identity only. It does not
certify callback, lifecycle fence or delivery behavior. The accompanying test
vector records the remaining real-flow obligations without positive fixtures.

`git diff --check`: exit0. Default stable `rustfmt` failed before formatting because
`librustc_driver-e331a45a468a48ee.dylib` was absent. Installed1.97.1 rustfmt then
formatted only owned source, exit0; no compiler/build/test/target entry occurred.
Resource owner21d subsequently clarified that further formatting also needs an
applicable finite execution offer. No additional formatter is run under source
admission. The failure is preserved; no toolchain installation or repair occurred.

Compilation, protoc generation, test execution, migration application,
PostgreSQL, mTLS and native/runtime qualification: `NOT_RUN`. Shared dependency
composition, Root mounting and a finite execution allocation remain open.
Minimal shared dependency intake is complete in this private source candidate;
compilation, generated-code checks, Root mounting and runtime validation remain
open. The objective is still IN_PROGRESS; committed source is not acceptance.


### Finite validation attempt 01 (2026-11-05)

The exact source `b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc` entered the
nonignored-test allocation at epoch `1793839303.3842602`. Initial resource census
failed before either guard completed its initial allocation sample, so no format,
Cargo, protoc or test command ran. The independent guard reported an unregistered
runtime; the primary complete-accounting checkpoint also failed. The controls did
not retain the offending PID/root. A later process list cannot identify the
cause, and this record does not claim that another owner exceeded capacity.

TARGET released at `1793839304.4248052`; FULL released at
`1793839304.589601`, with no target handles, owned children, private TMP or retained
reservation. Direct receipts reached the next target owner, Root and allocator
before this documentation. The closed allocation will not be restarted. A new
finite allocation is required after correcting future census diagnostics and
reconciling actual accepted cohorts. See
[the exact attempt record](../evidence/be39-user-review-auth/validation-01.json).

The prospective whole-footprint cap was amended to 48 GiB before acceptance;
other limits remained unchanged. Root's separate 40 GiB conservative carry covers
its broader private footprint and is not an Auth or shared-target measurement.
Auth retained its higher relevant 35,427,282,944-byte prior whole-footprint floor.
No implementation or runtime acceptance follows from this failed resource round.
The API goal remains the incompatible historical BE33 objective in `blocked`
state; it was not reset or falsely completed to substitute for the current work.


### Finite validation attempt 02 (2026-11-05)

The distinct allocation accepted source `725f3c5705304220837ba46b0e9d915605b0a04f`
at `1793839997.169515` and released FULL at `1793839998.255392`. The own reader
failed to normalize Root's embedded owner record using `FULL`, although its
literal release proof existed. This is a local validation-control parser defect.
No formatter, Cargo, generation or test workload ran. All owned resources were
released and direct receipts sent before documentation. This allocation is also
permanently closed.

Separate future controls normalize both receipt shapes and reconcile actual owner,
acceptance and root identities without erasing unresolved legacy reservations.
A read-only preflight over the actual records resolved 32 cohorts with no unknown
states. This is protocol-input evidence, not a live guard or product test pass.
The [attempt record](../evidence/be39-user-review-auth/validation-02.json) preserves
raw-proof hashes and limitations. QA09 counterpart source
`7493c1e3629842db53c4ad962cd6e516fe20185e` is recorded for future binding checks;
no Concert ownership or process-mount responsibility transfers to this Auth branch.


### Finite validation attempt 03 and source diagnostic 01 (2026-11-05)

Auth03 accepted source `cd10c8e1e710b5502afedf11b4a94bdba5d7b69b` at
`1793841000.591606`. The independent complete initial accounting sample passed,
but the primary initial checkpoint failed with `AccountingError`. The executed
controller omitted the structured error receipt and failure-time snapshot, so the
exact cause remains unknown. No formatter, Cargo, protoc or tests ran. TARGET
released at `1793841015.5690691`; FULL at `1793841020.134986`, all owned resources
released. The [attempt record](../evidence/be39-user-review-auth/validation-03.json)
and byte-preserved raw receipts retain the peaks, hashes and limitations.

The distinct no-target source diagnostic accepted at `1793841782.613973` and
released FULL at `1793841782.8133771`. It stopped before both complete initial
samples and before all diagnostic probes. Its saved pre-cleanup traceback exposed
a missing historical FE37Native03 FULL receipt at its former worktree path. The
receipt owner supplied retained bytes matching immutable commit
`bc38036e75ab82fdc9fc5c436365249b572c54fd`, SHA-256
`764aa19e5d777b5191724666f4bb3be9eebd68d97a222fdd6d0919e646a5ae00`.
Separate future controls bind that moved receipt to its exact owner, offer,
acceptance epoch, original path and SHA. Other missing receipts and required live
accounting roots remain fatal. This later failure does not explain Auth03: the
FE37 attempt accepted after Auth03 had already released FULL. See the
[source diagnostic record](../evidence/be39-user-review-auth/source-diagnostic-01.json).

Both rounds are permanently closed. Implementation remains unchanged from
`b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc`; product validation and qualified
publication remain open. A fresh diagnostic allocation and actual diagnostic
delivery precede a fresh compilation/nonignored-test allocation. Native
PostgreSQL/mTLS/process-mount flows remain `NOT_RUN`. No king or SL technical
review or acknowledgment is a delivery prerequisite. The incompatible historical
BE33 API goal remains `blocked`; no goal was reset or falsely completed.


### Source diagnostics 02 and 03 (2026-11-05)

Two separately authorized no-target rounds stopped before both complete initial
samples and before any diagnostic probes. Source02 accepted at
`1793842491.890306`, FULL at `1793842492.088338`: the own decoder required an
execution-receipt path on a closed ordinary publication without execution
acceptance. Source03 accepted at `1793842540.963774`, FULL at
`1793842541.1581938`: it required a live controller descriptor on an accepted
booking before launch. The current BE42 record explicitly retained its target
reservation while its compiler/controller had not started.

Both rounds released all owned children and reservations, with no target access,
formatter, Cargo, generation or product workload. The shared future NEW budget
of 4 MiB is conservatively fully spent (2 MiB each), without refund. All raw
failures and historical whole-footprint floors remain retained. See
[Source02](../evidence/be39-user-review-auth/source-diagnostic-02.json) and
[Source03](../evidence/be39-user-review-auth/source-diagnostic-03.json).

Separate prepared controls classify ordinary nonexecution publications first and
retain accepted execution bookings at their full declared caps until matching
literal FULL, even before a controller exists. Empty descriptors register no
PID, and unknown actual runtimes remain fatal. These are prepared corrections,
not a successful decoder or live-accounting result. Another distinct finite
diagnostic allocation is required; no existing round may restart. Original
Auth03 cause remains unknown and application/native qualification remains open.


### Source diagnostic 04 (2026-11-05)

The distinct Source04 allocation accepted at `1793842849.710218` and released
FULL at `1793842849.9480052`, all owned resources released and no target access.
It stopped before both complete initial samples and before all diagnostic
probes. The traceback shows that the own decoder still demanded a raw acceptance
file for an ordinary booking. The error did not embed the failing record; an
immediate post-release static read found one missing ordinary acceptance path
for owner231's new booking. This attribution is post-release evidence.

The [Source04 record](../evidence/be39-user-review-auth/source-diagnostic-04.json)
preserves raw proofs and limits. Its fresh 2 MiB is conservatively spent in full,
with the prior shared 4 MiB retained. Separate future controls normalize all
ordinary records before execution-proof requirements, register only factual
live descriptors, and prevent ordinary metadata from closing a nonordinary
legacy acceptance. Unknown actual runtimes and missing execution receipts still
fail closed; errors now include owner, offer, proof key and path. The prepared
correction has not passed the diagnostic. No old round may restart. A fresh
allocation is required, and compilation/product/native qualification remains
`NOT_RUN`. Original Auth03 cause remains unknown.


### Source diagnostic 05 (2026-11-05)

The fresh Source05 round accepted source
`32f6425d3d7e41a00c1b0c8ffe82c8c1d6359917` at `1793843073.2435`. Both independent
complete LIVE initial samples passed at `1793843073.6638122` and
`1793843073.663738`, before probes. The decoder resolved 37 cohorts with no
unknown states and correctly classified four actual ordinary null-cap FULL
records without inventing execution slots. The structured AccountingError
serializer passed an explicitly synthetic check; original Auth03 cause remains
unknown. No accepted null-controller records appeared in the retained final
classification snapshots, so that branch is prepared but not exercised by these
current records. Explicit fixture coverage remains required before future
product workload; this result does not silently certify that case.

FULL released at `1793843074.8936498`, exit 0, no owned children or remaining
reservation, with the direct allocator receipt sent before inspection/docs.
Both guards observed peak NEW `380928` bytes, WHOLE `43325034496` bytes, 3 owned
PIDs and maximum RSS `152420352` bytes; minimum free space was `23242031104`
bytes. All prior failures/MAX remain retained and no old budget is refunded.
The complete backend-root scanner remains unchanged.

The [diagnostic record](../evidence/be39-user-review-auth/source-diagnostic-05.json)
contains raw-proof hashes, coverage limits and the sealed executed controls.
No target stat/read, formatter, Cargo, protoc or product/native test ran. This
delivers the observed resource diagnostic, with its stated coverage limit; it
is not application acceptance. A separate finite compile/nonignored allocation
and later PostgreSQL/mTLS/process-mount qualification remain required.


### Nonignored validation attempt 04 (2026-11-05)

Auth04 accepted source `1d1facceb609392582cded138e47f779b9e2a50d` at
`1793845591.865324`. Both complete LIVE initial samples passed, followed by all
four explicit synthetic protocol fixtures, including accepted-null full
reservation and exact/different terminal matching. Those are actual control
results; they do not certify application behavior.

Before any formatter or Cargo command, both guards refused a new peer's startup
metadata. The primary snapshot caught PID18662 about70ms before its canonical
actual acceptance. The independent snapshot then found the canonical positive
booking but required a local admission log that did not yet exist. The owner
confirmed publication of its factual identity/caps before heavy census; the
local log was produced after that preparation census. Requiring that later log
was an own reader defect. A future reader must count the canonical reservation
immediately and block own product progress during bounded identity discovery,
without inventing a role or treating PID absence as release.

TARGET released at `1793845608.3711329`; FULL at `1793845608.4570901`, all owned
resources released. Direct receipts preceded inspection and this documentation.
Peak WHOLE was `48851480576` bytes, peak NEW `123453440` including retained
112MiB, and target growth0. No formatter, Cargo, protoc, executable copy or
product/native test ran. The [attempt record](../evidence/be39-user-review-auth/validation-04.json)
preserves structured causes, failure-time censuses and the executed control seal.
Original Auth03 cause remains unknown.

The round is permanently closed. The newly authorized Source/Doc8 preparation
and distinct prospective Auth05 retain all carry/MAX inside the one cumulative
2GiB family. Auth05 remains unaccepted behind its actual queue; no closed round
or failed technical result is replayed. Backend source remains unchanged from
`b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc`. PostgreSQL/mTLS/process-mount
qualification and a validated publisher412e handoff remain open.


### Auth05 controls prepared; execution not accepted (2026-11-05)

Auth04 failure evidence is committed at `fb9067f2db46e7f331219552dda8eb5f13ee5463`. Backend implementation bytes remain unchanged from `b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc`; the static backend diff is empty. Separate prospective controls are preserved in `docs/mcr/evidence/be39-user-review-auth/validation-05-preparation-controls/`, with pins, limits and explicit limitations in `validation-05-preparation.json`. The executed Auth04 directory remains unchanged and permanently closed.

The reader now retains the full canonical positive acceptance before a controller's local append log exists. Exact terminal history closes only its matching owner/offer/actual acceptance epoch; ordinary observations cannot erase an execution reservation. Unknown runtime processes remain counted and block Auth progress during a bounded newborn identity reconciliation interval. Both samplers share a gate that can pause only birth-verified own child groups, with no peer signals; absence and PID reuse do not resolve an identity. The complete root scanner is unchanged. These controls and ten synthetic protocol cases are authored, **not executed or qualified**. No formatter, Cargo command, product test or target access ran in this preparation.

`AUTH232-FAILED04-PREPDOC8-0233` funds only this preparation through 03:15 UTC, with its entire additional 8 MiB charged once inside the cumulative Auth 2 GiB family. Prospective `AUTH232-NONIGNORED05-0233` remains unaccepted, behind actual queue dispositions and fresh capacity. Its limits retain at least old128 MiB plus fresh8 MiB, WHOLE64 GiB, target growth1.5 GiB/outside0.5 GiB,32 PIDs/8 GiB RSS, fixed900-second WORK/1080-second FULL. Existing required developer-owned validation, fixes, documentation and direct publishing handoff remain unfinished; PostgreSQL/native/mTLS gates remain `NOT_RUN`. No king/SL review or acknowledgment is required.


### Auth05 closed before initial samples; separate decoder repair prepared (2026-11-05)

The actual `AUTH232-NONIGNORED05-0233` round accepted at `1793848796.0535738` and closed failed before either complete initial sample, target root accounting, protocol fixtures or product commands. TARGET released at `1793848797.28386`; FULL released at `1793848797.383717` with target access `NONE`, zero workflows/children/reservation and no commands or copied executables. Literal receipts and both structured failure traces are preserved under `docs/mcr/evidence/be39-user-review-auth/validation-05-raw/`, with exact executed controls under `validation-05-controls/` and disposition in `validation-05.json`. Releases went directly to the queued owners before diagnosis or documentation. The closed round will not restart. No fresh WHOLE/NEW peak was established; incoming historical maximum `58,895,065,088` bytes and all higher actuals remain binding.

The saved failure is an owner-authored regression at executed `budget.py:209`: the historical FULL reader passed multi-record JSONL to a single-document JSON decoder. A real shared `native10-controller.jsonl` input has a 368-byte first line matching the exception offset. Its pathname was not captured in the failure receipt, so the exact runtime input remains an inference rather than a saved path witness. PI12's separate observation of the exited guardian as `<defunct>` remains historical evidence; no retrospective PASS or identity exemption is claimed. Backend bytes remain unchanged from `b0f2b56217bb48ab2c0c1f7c0e42720de6cbf8bc`.

Fresh ordinary `AUTH232-CLOSED05-REPAIRPREP4-0326` funds only evidence/docs and separate successor control authoring through 04:00 UTC. Its entire 4 MiB is charged once inside the same cumulative Auth 2 GiB family, retaining at least old137 MiB plus fresh4 MiB, with no refund/reset. Helpers publish their own kernel-measured birth, executable image, cwd and argv before other work. No fixture, guard probe, compiler, app, PostgreSQL or target work is authorized under this preparation book.

The separate successor controls restore suffix-aware JSON/JSONL validation for every FULL-receipt consumption path, preserve byte-hash rejection, reject malformed later records, and add exact input identity to future decoding failures. Three authored regression cases cover canonical and embedded multi-record receipts, malformed later records, and changed literal bytes; all thirteen prospective cases are `NOT_RUN`. The complete accounting scanner is unchanged. Preparation files are preserved under `validation-06-preparation-controls/` and `validation-06-preparation.json`. No nonignored06 execution grant exists: source/control pins, fair queue, fresh capacity, dual LIVE complete initials and developer-owned product validation remain required. No king/SL review or acknowledgment is required.

PI11 SOURCE02 subsequently reported an unknown-process stop involving ordinary helper PID99493. Kernel birth/image/argv, role/cwd, observedAt and ordinary-scope endedAt are preserved in `repair-prep4-0326-observation.json`, with the exact helper source. Atomic replacement completion and actual OS-exit times were not captured; the peer interruption time alone cannot establish that its census followed publication. The historical failure remains a failure, with no retrospective compliance/PASS or compiler/runtime scope inferred.
