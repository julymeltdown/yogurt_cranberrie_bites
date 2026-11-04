# QA09 frontend recovery and lifecycle contribution

Status: **BLOCKED** on actual production bindings/producers. The source and
synthetic guard contribution is published and independently remote-verified.
This is not executed business recovery evidence. All six actual cases remain
**NOT_RUN**.

## Assignment and source

- Implementation owner: chat `01a1a202-375e-7401-bfa1-ff8721e90ed6`.
- Checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-in01-world`.
- New branch: `codex/mcr-fe-QA09`; published base
  `d62ce6979d6a3326f2da7271c3ae1eba54c6a22f`.
- Previous `codex/mcr-qa-IN01` and its completed evidence remain preserved.
  The checkout was clean before the branch change; no former writer remains.
- Root153 explicitly granted only `frontend/e2e/real/qa09-recovery/**`, this
  document, and `docs/mcr/evidence/qa09-frontend/**`. No application, shared
  contract, backend, central manifest/lock/router, or backend recovery script
  is writable under this assignment.
- Root153 is the frontend publishing developer. Developer validation and fixes
  remain with this owner; publication never waits for king/SL review or ACK.
- The prior goal query returned no unfinished goal. The new unbudgeted QA09 goal
  retains implementation, validation, actual case evidence, documentation,
  committed handoff, publication and remote postimage checks as unfinished scope.
  No reset, scope reduction or false completion is used for dependency waits.

Read the implementation plan, project assessment, execution plan, FE39
`closure-resume.md`, `closure-resume-integration.md`, the START/RESUME contract,
and the actual current consumer/router/transport source. Historical manager
review clauses in this checkout are superseded by the latest human no-review
instruction. No source archive or historical evidence is changed.

## Original criteria and operational cases

The specification's QA09 row requires database restart/backup restoration,
outbox replay, pending pickup, deletion and partial event closure. Its acceptance
requires consistent restored state, no lost committed pickup, and resumable
shutdown/cleanup. The following are labels for those clauses, not new scope.

| Case | Actual frontend observation/action | Remaining prerequisite | State |
| --- | --- | --- | --- |
| QA09-DB-RESTART | Observe the client across the producer's actual restart; read current reservation | Qualified restart producer and mounted real pickup consumer | NOT_RUN |
| QA09-BACKUP-RESTORE | Observe reauthorization/current reservation after real physical restore | Actual physical restore, same-candidate durable pickup/inventory evidence | NOT_RUN |
| QA09-OUTBOX-REPLAY | Refresh the actual inbox after the producer's relay crash/replay | Actual relay/dedup producer, canonical notification read projection | NOT_RUN |
| QA09-PENDING-PICKUP | Check the original command, retaining its private command identity | Actual pending/lost-response producer and mounted pickup recovery | NOT_RUN |
| QA09-DELETION | Check canonical deletion progress without a second deletion request | Real BE42 saga and current owner convergence | NOT_RUN |
| QA09-PARTIAL-CLOSURE | Review exact failed job/revision and explicitly resume; observe progress | Real operator authority, mounted FE39 and qualified closure effects | NOT_RUN |

Backend candidate267 agreed these names, then explicitly STOPPED/RETURNED its
QA09 assignment without authoring files or running a producer. It is **not**
the backend QA09 owner. Replacement developer
`01a1a1fd-6ec9-71e2-a2c6-b5dddca7542e` explicitly accepted the three backend
source paths in `qa-contract-coverage`, branch `codex/mcr-be-QA09-backend`,
base `2bd04679`, with its own ACTIVE unbudgeted goal. The developers agreed a
version-1 coordination envelope with a shared 32-hex run ID, case ID, full source
revisions, same-report-directory producer artifact/SHA-256, nullable ordered UTC
phases and NOT_RUN/IN_PROGRESS state. Backend observations and limitations remain
opaque; a passing physical-restore segment without an original domain boundary
does not qualify a case. Candidate267's
incompatible original BE41 goal is not this frontend goal and must not be
falsely completed/reset.

## Implemented source preparation

`observer.mjs` attaches before login to actual Playwright page events. It retains
only closed operation labels, HTTP methods/status, monotonic observation order,
fixed failure categories and world socket counts. It does not intercept traffic,
read payloads/DOM storage, retain credentials/URLs/UUIDs/query/body values, inject
Auth, or substitute a raw socket client. The single command key is converted to
a report-local HMAC; the private random salt never leaves the observer and is
erased at detach. Original status-query keys can match the command within that
observer. Event count is capped; overflow is visible and cannot certify a case.
Owned page/socket listeners are removed on idempotent detach.

`actions.mjs` uses existing accessible product controls only: current reservation,
original command status, inbox refresh, canonical deletion progress and explicit
closure resume. Resume requires an exact expected job/revision and checks its
actual review before confirmation. It cannot start closure or create a new pickup
or deletion intent. DOM snapshots expose booleans/counts only, not private text.

`journey.mjs` accepts an already-authorized actual page, observer, resource guard,
abort signal, producer-boundary rendezvous and actual-outcome waiter. It never
starts backend processes, restores databases or controls a relay. It bounds waits
by the supplied resource signal; the runtime owner must cancel/close its own
browser/context at lease end. This driver returns frontend observations as
IN_PROGRESS, never a whole-QA09 PASS based on a callback or fixture. No native
launcher is enabled before the actual runtime contract/source/slot are ready.

`producer-binding.mjs` validates that coordination envelope against expected
run/case/backend/frontend revisions and reads only the explicitly supplied owner
report plus its named, bounded same-directory artifact. The actual backend CLI
emits a six-case container: the frontend binds its outer run/backend revision,
requires every original case exactly once with consistent bindings, then selects
the expected case. Another case's execution cannot qualify that case. It checks actual bytes
against the pin, rejects symlink files, traversal, substituted candidates and
regressing/invalid timestamps, and discards opaque backend diagnostics. A valid
pin, successful segment, or complete phase metadata still cannot certify QA09.

No existing IN01 driver is modified or reused as evidence of QA09. No new package,
manifest, lockfile, production feature, contract, art or legacy rendering is added.
The original retained physical dependency directory is preserved untouched.

## Source-traced integration readiness

These findings are from base `d62ce697`, not runtime failures:

1. `main.tsx` uses `createProductionPorts`; its production `admin.authorize`
   returns false. `AdminBoundary.tsx` also renders a control-room placeholder
   when access is true, rather than mounting FE39. Root confirmed this genuine
   shared gap and routed current operator readiness to BackendRoot. FE39's
   published 118-test/native fixture proof is not actual production closure.
2. `router.tsx` mounts Gift list/detail/mine. Reservation/pickup route metadata
   has no feature element there. `CommunityFeatures.tsx` owns discovery, not the
   separate `community-entry` bridge, pickup controller and recovery journal.
   Those existing consumers do not establish their default-root activation.
3. Deletion is mounted: `/me/account` acquires the existing retained controller
   using `auth.accountDeletion`. `/account/deleted` renders retained canonical
   status recovery. Its Auth adapter uses DELETE `/api/auth/account` once and
   GET `/api/auth/account/deletion`; pending/uncertain does not mean completed.
   Recovery expiry/sign-out/session-change restrictions remain truthful.
4. Actual outbox effect/dedup proof, inventory/receipt consistency after restore,
   all deletion owners and full closure effects belong to real producer evidence.
   HTTP 200, rendered text, or a database process restart alone cannot prove them.

Root owns shared integration and coordinates main/WorldShell/router/ports leases.
This worker has reported concrete seams and does not edit or fabricate
authorization in them.

## Validation and delivery

Source-unit tests are authored for privacy export, same-command status binding,
per-report unlinkability, foreign-origin rejection, failure categories, duplicate
socket detection, bounded event overflow, listener cleanup, exact closure-review
binding, cancelled resource windows, producer run/candidate/artifact/phase guards,
and honest NOT_RUN coverage. These are synthetic unit inputs, not actual
App/service cases.

Resource21d's fresh `QA09-FE-SOURCE-202611041339Z` offer was actually accepted at
`2026-11-04T13:40:26.000Z`: 90 seconds total, 60 seconds work and 30 seconds
independent cleanup, four processes, 256 MiB RSS, 16 MiB whole inclusive carry,
fresh 16 GiB and continuous 8 GiB disk floors. The actual clean frozen source
was `a857130f925b19da01280e603d318ea5811933ed`. Goal API metadata remained
BLOCKED honestly while the aligned source validation operationally resumed;
no reset, replacement goal or false completion occurred.

The absolute retained Node 24.22.2 binary ran the following sequential checks;
the full argument arrays, timestamps and exit codes are retained in
`docs/mcr/evidence/qa09-frontend/source-round01/report.json`.

| Check | Exit | Evidence |
| --- | --- | --- |
| `node --test --test-concurrency=1` on `actions.test.mjs`, `observer.test.mjs`, `producer-binding.test.mjs` in the assigned directory | 0 | 25 passed, zero failed/skipped/cancelled; `unit.log` |
| `node --check` on each of the eight assigned `.mjs` files | 0 each | Eight syntax logs |
| Retained `oxlint --deny-warnings frontend/e2e/real/qa09-recovery` | 0 | `lint.log` |
| `git diff --check d62ce6979d6a3326f2da7271c3ae1eba54c6a22f HEAD` | 0 | `diff.log` |

The independent supervisor finished and fully released at
`2026-11-04T13:40:28.887133Z`, before work deadline `13:41:26Z` and hard cleanup
deadline `13:41:56Z`. It observed 130,285,568 bytes peak RSS, four processes and
31,136,481,280 bytes minimum free disk. Initial whole carry was 9,830,262 bytes
including all historical IN01 evidence/driver/document and all QA09 source,
document and preparation evidence. `carry-inventory.json` enumerates those
139 files. The final receipt measured 9,869,403 whole bytes including its own
evidence bytes, below the 16 MiB cap. No owned process remained alive; no TMP,
cache, dependency link, install, browser, build, database, backup, restore,
relay or native workload was created. The pre-existing physical dependency
directory remained untouched. Full release was sent directly to resource21d.

The run report SHA-256 is
`00fa188aa9ae3cb82ecb6b56e09ec765657794332ad0e0307cce71d44454fc27`.
Its source pins identify all eight tested files. The original
`source-preparation.json` retains its historical NOT_RUN checkpoint unchanged;
this actual source round supersedes that validation wait, not its live cases.
The supplemental `allocation-release.json` records acceptance, deadlines,
artifact pins and the report's pre-final-report-write byte measurement limitation.
Native allocation will be requested only when actual production inputs are ready.

The goal remains unfinished. Source completion, a committed checkpoint, or synthetic
unit success cannot close the original six actual cases. The final handoff must
record commands/exit codes, retained failures and limitations, source/contract
and actual producer pins, exact changed paths/base/commits, and Root publication
with independently checked remote postimages.

## Current external wait

Source checkpoints `1188be4f`, `07d85878` and `ab3b5b98` preserve ten authored
paths within the three exact granted prefixes. The original IN01 branch and
evidence remain unchanged. No native or validation resource is held.

A fresh fetch at remote master
`c02a3e55728bd29933d966189c28be3131a541af` confirms that the default router
still mounts only Gift list/detail/mine, production operator authorization is
false, and the admin boundary still renders the control-room placeholder.
Root owns these integration seams. Backend developer6ec9 has confirmed the
actual six-case report interface; actual domain faults/effects remain unrun.

The same external prerequisites persisted across three goal turns. The source
and API goal disposition are BLOCKED, with full original scope preserved.
The fresh source round above has now passed and released. Source publication and
independent remote proof are complete; the remaining original cases require current
production consumers and qualified backend business producers. The backend's
reported physical restore/restart support, including empty business rows, does
not close these cases or establish candidate-bound browser recovery.

## Published source contribution

The clean worker handoff `2fb112c9a96e82fc6a3ffefbdb645a3b9c55c77d`, based on
`d62ce6979d6a3326f2da7271c3ae1eba54c6a22f`, was normally published by Root as
`da9b8a98377c4ba7b0f44b7e8f62d8174901d02d`, parent
`89f26e6edf916cfd5bcdae481c24091597027448`. The worker's six source/evidence
commits are `1188be4f`, `07d85878`, `ab3b5b98`, `a857130f`, `becbdecd` and
`2fb112c9`. Publication replays their exact scoped contribution; it is not a
merge of the worker branch.

This implementation owner independently fetched master and queried `ls-remote`:
both returned `da9b8a98377c4ba7b0f44b7e8f62d8174901d02d`. The worker base is
an ancestor of both branches, and the published commit is an ancestor of remote
master. All 24 changed paths match the assignment and the publication diff;
all 24 Git blob postimages are identical in worker, published and remote trees.
All eight tested source SHA-256 values and the qualified source report hash
match remote bytes. No App/shared/backend/manifest/lock/router edit is part of
this publication. This is delivery proof, not another runtime test.

The complete path/blob/SHA/ancestry record is
`docs/mcr/evidence/qa09-frontend/publication-proof-01.json`, SHA-256
`4cd4f03f8f44fb89730981544682a12ee1fae93989e60b68db0414388d16045f`.
No extra runtime or resource was acquired for this readback. Original QA09's
six actual business recovery cases remain NOT_RUN and the full goal remains
BLOCKED; source publication does not close it.

## Resumption and managed admission, 2026-11-05

The same aligned unbudgeted goal was read as BLOCKED and retained unchanged.
Root153 literally regranted only the original three driver/document/evidence
prefixes on clean worker `0ceedf0bab49c70ae9024965f1aecb479d036dd6`.
Former FE17 and all application/backend/shared leases remain returned or excluded.
The standing publishing developer is now
`01a1a5d9-332b-7822-a8f7-412e295fa211`; no Main/SL review or ACK is required.
The worker branch/history and old evidence remain preserved without a rebase.

The historical unmounted-route findings above describe their recorded pins.
Fresh canonical source `37add6d996eacb7674c1c220150870d7090d5e09` now composes
`communityEntryPanels` and mounts `DefaultAdminPanel` through `AdminBoundary`.
That corrects the source-readiness account, but does not establish an actual
authorized pickup, inbox, deletion or privileged closure route on a serving cohort.

The genuinely missing driver seam is now authored as `managed-admission.mjs`,
with a managed entry in the existing `journey.mjs`. It consumes Root's already
parsed GateConfig and qualified RuntimeQualificationPort `inspect` method.
It projects the common existing owner proof: exact candidate/event/environment,
two existing members plus existing operator, isolated/nonlegacy/exclusive event,
six named ready service categories and contract hashes. No mural snapshot or
own-result requirement is added to QA09. It captures the original config identity
selection and page origin, rechecks before and after each inspection, rejects
resource abortion and fixes the deadline to the earliest accepted lease/proof
expiry. Subsequent proofs cannot renew it. Errors contain a fixed code only;
actor IDs, credentials, endpoint diagnostics and raw proofs are not exported.

Contract hashes are the exact canonical coherent150 C07 packet
`83f55f9639273a7994a30d6a591e642f7e4b674f`: generated domain OpenAPI
`31a167fedd6b414a9d0b48df0f18644fcbd9f7a7a9bcef855265601e1401d634`
and C05 world schema
`6ec593955a695656bcf5ec736e25fdf26593f1eff28bda657541779a1ec8cd85`.
New code is authored locally, adapting the common binding checks from
`frontend/e2e/real/mural-helpers.ts` and the actual observation shape from
`backend/scripts/mcr/in04_wall_runtime/observer.py`, read-only at `37add6d9`.
Source paths and SHA-256 are recorded in `resumption-readset.json`. No backend
implementation is imported into the frontend; the qualified owner supplies its
existing port. Runtime dependencies are only existing driver modules and Node
builtins. No new package, private wire schema, descriptor, server, harness,
runtime, fixture principal or build target is introduced.

`observeManagedRecoveryCase` applies that guard to the existing case driver for
restart, restore, outbox and pending-pickup observations only. The present Root
observer reauthorizes all three actors on every inspection, which cannot certify
intentional deletion or closure withdrawal. Those two cases explicitly require
a lifecycle-aware actual owner observer and retain the original generic driver;
they are not silently executed through an incompatible live-profile gate.
It retains the existing IN_PROGRESS-only frontend observation result. Neither a
valid callback/proof nor its source fixtures accepts a business case. The owner
still supplies actual boundary/outcome rendezvous and interruption/cleanup of
an in-flight managed inspection. No restart/control method is granted here.
The existing QA05 control only supports World/PG+Concert restart; it does not
provide QA09 physical restore, outbox, pending-pickup, deletion or closure authority.
Those interfaces must come from their real owners, rather than invented methods.

Ten focused source tests are authored for the new admission seam and its
actual journey entry, including substituted contracts/candidates/roles/services,
cross-origin navigation, config changes during inspection, expiry/renewal,
resource abort, sanitized owner failure/no retry and refusal before DOM/actions.
Validation is NOT_RUN pending a fresh finite 21d source offer; the unchanged
25-test source matrix and prior physical/browser segments are not repeated.

Root1fc explicitly reports no actual serving Concert executable/cohort/event,
private descriptor/control handle or admission window. Managed profile `bd087fde`
and adapter SHA `40fddf11e1520452b73966c8043939cbc35d65ced0e2935515018ab1051cf350`
are source-qualified only. Current canonical BE42/closure producer compilation
and actual business boundaries remain unqualified. The former backend QA09
owner6ec9 confirms its preserved v1 plan/restore-segment CLI has no current managed
control or coherent150 admission implementation; it remains physical support only.

Exact wakeup: Root1fc supplies the closed actual serving candidate/helper/binary
cohort and owned private descriptor/credential references, Root153 supplies its
matching existing frontend session/qualified inspection adapter and actual public
consumer routes, the named lifecycle producers supply each real business boundary
and consistency receipt, and 21d supplies a fresh accepted finite window. Access
bearers stay in RAM and private references are not published. No replacement
stack, duplicated Auth pair, fabricated actor or fixture-only live PASS is allowed.
Until then all six original rows remain NOT_RUN; the unfinished goal is not reset
or completed by this source preparation.

## Corrected source allocation intake

The prior 9,907,602-byte/157-file carry figure covered only selected retained
proof/source files. It was **not** complete execution WHOLE, and no offer was
accepted or source tests executed on that basis. Allocator21d clarified that
16 MiB is incremental NEW output, not WHOLE. The actual source queue remains
art231, FE38,207 and the first fitting newly ready Parade Ink priority before
this round; a queue position reserves no capacity.

Published TOOLS-FS-01 helper `f203d30c7f3f490c35860fde2371e3cdacf95987`
was located in the preserved `qa07-input-delta-332b` checkout. Actual helper bytes
match SHA-256 `1a2ed019be0af4d673786532c8b46eda81cd6137a7ac27c9f97b83a34387483c`;
the qualified runner matches
`8883f0baf88a6b883c7eb0434ddadf3ccf90fb212b655781213b2177b9bd677e`.
The archived helper owner was not restarted after the tool rejected one message.
The existing helper is imported read-only with bytecode writes disabled. No helper
tests, product tests or former source matrices are rerun for this adoption.

The read-only `managed-source/intake-inventory.json` records a complete fresh
allocated-block union: **4,001,792,000 bytes**. Every inode is counted once across
the whole QA09 checkout (including physical dependencies and all retained IN01/
QA09 evidence), shared Git, Node toolchain, Python runtime, system `/bin` and
the external helper. Per-scope quantities are 1,565,368,320;1,175,945,216;
209,088,512;1,045,037,056;6,307,840;45,056 bytes respectively. This is a
sequential complete read-only observation, not an atomic snapshot or a resource
lease. A fresh complete baseline and both live guards remain required at ACCEPT.

`managed-source/source_round.py` and `resource_guard.py` adapt the qualified
TOOLS-FS-01 primary/independent pattern to the same existing QA09 source round.
They keep separate conservative ledgers and pinned source/controller/helper bytes,
require independent readiness before a workload child, retain incomplete-scan
failures, close/reap owned groups, and preserve fixed60-work/90-FULL deadlines.
They serialize only the `ps` measurement child so both measurements themselves
fit the proposed four-ALL-process cap; ledgers are never shared. Large tool hashes
stream through bounded buffers. No new product harness or runtime is created.
Only ten new Node source cases and three changed-file syntax scopes are queued.
The dual guard/controller adoption is not claimed qualified merely because the
read-only inventory command succeeded. The exact frozen pins accompany the next
single allocator intake; 256 MiB RSS and all other actual offer limits still apply.

Root153 then clarified that logical usage must also be retained when higher.
The newer complete `managed-source/intake-inventory-max.json` therefore keeps
both raw allocated and logical inode unions and conservatively charges the
greater value for each fixed scope: **4,011,585,424 bytes**. The additional
logical metadata pass follows no symlinks, globally deduplicates inodes and fails
closed on every error or deadline; no incomplete subtotal grants capacity.
The earlier allocated-only census remains immutable historical evidence.
Both guards use this conservative measurement, not just the pre-offer inventory.

207's direct successor notice reports its source round failed preflight before
guardian/workload, with all owned resources released. That notice is not QA09
admission: it reports free space falling below its own start gate. QA09 has
accepted no offer, holds no job/target/process and has executed no new tests.
The next compact intake proposes a conservative 5 GiB WHOLE ceiling and 16 MiB
incremental output, charging the whole existing checkout as retained NEW rather
than resetting historical carry. The finite 60-work/90-FULL, four ALL processes
and 256 MiB RSS proposal remains pending actual allocator fit and offer.

## Failed first managed source round and bounded guard correction

Fresh offer `QA09-MANAGED-ADMISSION-SOURCE-20261104T2338Z` superseded only
the unaccepted start-floor proposal: 11 GiB start, 9 GiB after OTHER2 GiB and
own incremental16 MiB reservations, 8 GiB live. Fresh available was
15,476,838,400 bytes; after those reservations13,312,577,536 bytes.
Source `ef0b7990194318aef57d68349be9ef4d62dba09e` and the exact offered
controller/helper/inventory/Node/Python pins matched before acceptance.

The primary stopped before guardian launch or any workload command: its branch
check incorrectly assumed a loose Git ref file. The actual files backend stores
this unchanged branch in `packed-refs`, while the linked-worktree HEAD correctly
names `refs/heads/codex/mcr-fe-QA09`. This is an owner-authored resource-control
error, not a product failure. The immutable `managed-source/round01/FULL.json`
records commands empty, all ten new tests and three syntax scopes NOT_RUN,
process closure verified, no owned children/workflow/reservation, and actual FULL
at epoch1793835497.083765 within the original90-second deadline. Literal FULL was
sent directly to21d and successor IN03owner209 before this documentation.

21d granted a separate128 KiB TOTAL ordinary source-fix growth booking through
2026-11-05T01:00:00Z. Fresh free15,031,853,056 bytes exceeded8 GiB+128 KiB.
The corrected control reads the existing linked-worktree HEAD, then a loose ref
or the exact packed ref with a bounded read and a final loose-update check.
Missing/ambiguous/moved refs still fail closed. No ref rewrite, GitGC, extra child,
Node execution, test/helper replay or target access occurs during this correction.
The prior failed offer remains closed. Only a fresh source offer can execute the
pending ten tests and three syntax checks; retry intake waits for actual IN03
SOURCE2343 FULL/DECLINE, with all original live cases still NOT_RUN.
## Qualified managed admission source retry

The historical pending-source statements above describe earlier intake states.
Fresh offer `QA09-PACKED-REF-SOURCE-20261104T2349Z` qualified frozen source
`10f53ec0fe3143b050ec2439aaaacbf34e6695ef` without changing or extending the
closed first offer. Accepted at **2026-11-04T23:49:31.930112Z**, the round reached
literal FULL at **2026-11-04T23:49:44.946213Z**, within its 60-second work and
90-second FULL limits. FULL went directly to allocator21d and successor205
before documentation. The late reduction of peer reservations did not alter
this accepted round's conservative reservations, caps or clocks.

The new `managed-admission.test.mjs` passed **10/10** using pinned Node24.22.2
with `--test --test-isolation=none --test-concurrency=1`. Three separate
`--check` commands for `managed-admission.mjs`, `managed-admission.test.mjs`
and `journey.mjs` each exited **0**. No unchanged 25-case suite, helper tests,
install, build, browser, PostgreSQL or native target was run. Tested application
source has not changed after these commands. Exact argv, exit codes, immutable
source/tool/controller pins and log hashes are in `managed-source/qualification.json`;
raw primary and independent receipts are retained in `managed-source/round02/`.
The failed `round01` remains immutable and is not relabeled as a passing attempt.

Both guards completed without a failure; the independent guard exited zero.
Across their samples, the retained conservative maxima are **1,565,523,968 bytes
NEW**, **4,017,827,728 bytes WHOLE**, **four ALL processes** and **204,488,704 bytes
RSS**. Minimum available was **15,497,965,568 bytes**. These fit the offered
1,582,157,824-byte TOTAL NEW, 5 GiB WHOLE, four-process, 256 MiB RSS and 8 GiB
live floor. Peaks are sampled observations, not proof of an unsampled maximum.
All owned children were reaped, workflow and reservation are zero, and process
closure is verified. The separate ordinary final-handoff booking permits at most
512 KiB positive growth through 2026-11-05T01:00:00Z, retaining these higher carry
values; it grants no further workload or target execution.

Backend lifecycle source `e11e956c6` now exposes the actual typed
`operations_live/public_access/lifecycle_observation` API. Root must adapt its
existing Auth client to `AuthDeletionSagaReader::{by_saga_id,by_user_id}` and
supply the original receipt/scope/source bindings through its managed transport
and mount. Frontend introduces no private wire schema or replacement stack.
Deletion/partial closure remain refused by the unchanged live-profile guard;
403/404, a missing handle or absent response cannot imply successful withdrawal.
The backend owner reports source compilation and two pure API tests only, with
actual Auth/database/browser behavior still NOT_RUN.

This packet qualifies the new managed admission source seam only. All six
original live business cases remain **NOT_RUN**, whole QA09 is **not accepted**,
and the original persistent goal remains **BLOCKED**, unfinished and unbudgeted.
The remaining wakeup is the real Root-managed cohort/transport and lifecycle
adapter, actual producer boundaries and public consumer routes, plus an accepted
finite resource window. The committed packet goes directly to publisher412e;
no king/SL review or acknowledgment is a publication prerequisite.

Root1fc subsequently corrected the lifecycle integration gap: the existing Auth
GetUserDeletion/GetBySaga endpoints are Gateway-only, and their saga payload has
no `actor_id`. The `e11e956c6` by-saga/by-user trait therefore cannot be directly
mounted by the genuine Concert client. Its unconditional deletion-saga lookup
also blocks partial closure when there is no deletion saga. Root routed the
operation-specific reader/SQL fixes to00c and a closed `ObserveSaga` permission
over the existing Auth owner-observation transport to232. That work must bind the
original command/source/native proof and stored actor without widening Gateway
callers or fabricating by-user identity. The corrected typed source pin is pending;
`e11` compilation/two pure API tests do not qualify actual SQL or Auth integration.
The source qualification above is unchanged, frontend adds no private protocol,
and all lifecycle refusals and six NOT_RUN cases remain in force.
