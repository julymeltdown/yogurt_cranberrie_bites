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
