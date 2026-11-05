# Auth-owned deletion saga observation

Task `QA09-AUTH-OWNER-OBSERVATION-01`; status `IN_PROGRESS`.

Implement the separate owner observation on the existing Auth pool/transport,
validate canonical source provenance and stored actor from real Auth state,
complete relevant developer-owned tests/runtime fixes, document exact evidence,
and hand a committed immutable result directly to publisher412e. No king/SL
review or acknowledgment is required. The current goal API retains an incompatible
historical blocked BE33 goal; its truthful metadata is unchanged. This durable
assignment adds no token budget and does not complete BE39 or BE42.

## Wire and authorization

The source proto is `backend/proto/auth/v1/concert_moderation.proto` at
`2eaeae8d9811c1590b2df64f7b684d50c2de1c89`, SHA256
`d9da85a1db8ecd545991091ed57c638ba811b5c4ff62f4e69fe4c7e66a5d4d1b`.
`AuthConcertDeletionObservationService/ObserveSaga` is a separate background
purpose. Its exact policy is `AUTH_CONCERT_DELETION_OBSERVATION_POLICY`, Auth
audience and registered Concert caller only, no claimed user actor path.
The handler additionally requires actual peer certificates and original bounded
transport/request expiry. Moderator proofs, Gateway claims and arbitrary UUIDs
cannot substitute for this purpose's canonical source command/binding.

Input is the exact original Auth-verified DeletionCommand, expected nonzero
32-byte source binding, nonzero16-byte request nonce and original <=5s expiry.
Output echoes those four fields and returns stored saga/user/actor, closed saga
status and all eight canonical closed target/step statuses. Unknown/missing/foreign
source or inconsistent state is an error; it never means withdrawn or complete.
There is no lookup-by-user fallback or existing public Auth caller widening.

The canonical source predicate and hash are adapted from the existing
`concert_deletion_source.rs` at `cd1d844ae14a5fa463399622482eedf4e5e21fe6`.
No existing SourceServer semantics are modified. One bounded repeatable-read
transaction checks the exact saga/user/actor/retention, actual deleted user,
revoked sessions and identities, positive security generation and matching Auth
`USER_DELETION_REQUESTED` audit identity/time/saga/retention metadata. It recomputes
`SHA256("MCR-AUTH-DELETION-SOURCE-1\\0" || canonical command || audit UUID ||
created seconds || created subsecond nanos)` and compares the expected binding.
Original audit JSON precision is parsed in Rust and compared at PostgreSQL
microsecond precision, matching the existing source implementation.

The transaction locks the actual saga and all eight steps and returns only their
closed stored states. Exact canonical `CONCERT_TICKETS`, `CONCERT_SESSIONS`,
`CONCERT_SHARING`, `CONCERT_PRIVATE_DATA` names are required alongside PROFILE,
FRIEND, POST and NOTIFICATION. Terminal step timestamps and completed saga status
must agree; incomplete/unknown/contradictory projections remain unavailable.

## Composition and ownership

`deletion_observation_server(existing_pool.clone(), actual_caller_registry)` is
the typed production mount factory in the dedicated Auth adapter. Root owns
literal registration on its existing mTLS Auth listener and actual CA/registry
configuration. This adds no Auth identity, pair, listener, pool or configuration.
Build output lives in the dedicated `concert_moderation` directory and reuses the
existing `.authority.lease.v1` and `.authority.deletion.v1` generated types.
No existing main, AuthSelf, SQL migration, SourceServer or BE42 test was changed.

Owner00c owns the operation-specific QA09 reader. Deletion operations require the
actual original command/source provenance; PARTIAL_CLOSURE remains saga-free,
using its actual closure/operator proof. There is no invented Auth saga for that
operation. Sharing/adapter owners persist and deliver original source provenance.

## Evidence and remaining work

Source tests cover malformed command/source/nonce/expiry rejection, canonical
eight-target mapping, unknown alias/status refusal, policy callers and exact wire
echo. They are not positive owner/runtime proofs. Source formatting and whitespace
results and the shared128MiB/01:00UTC source admission are recorded with BE39;
this task does not receive a second allowance.

Protoc generation, compilation, test execution, actual source binding/stored actor
runtime checks, mTLS refusals, timeout/cancellation and mounted QA09 consumer flow
are `NOT_RUN`. A distinct finite execution offer and actual composition are
required. This source checkpoint is not accepted or runtime-qualified.


The shared BE39/QA09 finite validation attempt 01 failed its initial resource
census before format, Cargo, generation or tests. All owned resources were
released, and the allocation is closed. The [attempt record](../evidence/be39-user-review-auth/validation-01.json)
preserves the exact source, failure, raw-proof hashes and remaining obligations.
The checks above remain `NOT_RUN`; a new finite allocation is needed.

The distinct second validation allocation also stopped before any product command:
the owned resource reader missed an embedded `FULL` state. Its exact record and
future normalization correction are in
[attempt 02](../evidence/be39-user-review-auth/validation-02.json). The reader's
read-only protocol preflight does not qualify ObserveSaga or any QA09 runtime case.


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
