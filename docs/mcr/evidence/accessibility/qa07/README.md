# QA07 accessibility and input qualification

Original QA07 task-local client criteria: **SATISFIED**. Qualification status:
**MOCK_READY**, native Chrome with explicitly synthetic authority. Final full run:
**36/36 PASS**, 12 scenarios at widths 360/390/430, Chrome 156.0.8078.80,
Node 24.22.2 on macOS arm64. This directory does not certify live services,
physical devices, real screen-reader announcements, IN02 or IN05.

The original QA07 row in `MCR_IMPLEMENTATION_PLAN_V2_EN.md` requires
keyboard/focus/screen-reader labels, DOM alternatives, reduced motion and pointer
cancellation. Its acceptance is: “Essential social/pickup tasks work without precise
canvas interaction; focus returns on sheet close.” The four named prerequisites
are FE18, FE19, FE32 and FE36. Pickup is FE26, not FE32; this suite deliberately
includes actual pickup UI and social-sharing UI as well as the named prerequisites.
All 112 required tasks and 61 IA entries remain required.

The only owned paths are NEW `frontend/e2e/accessibility/qa07/**` and this directory.
No application, existing fixture, backend, generated contract, manifest, lockfile,
central runner or original source artifact is changed by QA07.

## Inputs and authority

Initial isolated branch: `codex/mcr-qa-QA07-accessibility`, clean fresh-master base
`cf706882ed91a2708f3ef1e514b1fc81886cffc5`. An actual empty goal was confirmed before
creating the unbudgeted QA07 goal after usable inputs were read.

| Input | Actual code exercised | Authority limit |
| --- | --- | --- |
| FE18 | Root-mounted `WorldRooms`, `WorldNearby`, canonical Auth/privacy and immutable replication projection | Synthetic C05 snapshot and room catalog; no live room admission |
| FE19 / Root | Published `reference-scene-root/preview.html?version=spatial&era=PARADE&source=derived`, `__SCENE_ROOT__`, actual routes/WorldShell/production ports/composition/default MapScene/selected-character renderer, illustrated Places view and searchable modal | Root handoff `10784b26`; published engineering source qualification is consumed, original art provenance is preserved; source fixture is not production admission |
| FE32 / FE33 | Published `mural-entry-lifecycle/__tests__/assembly-native.html`, actual `WorldPanels` and mural provider/owner/editor through the production adapter and default MapScene | Frozen FE33 caller `ebf0473d`, receipt `eeae69be`; explicit fixture anchor, eligibility, admission and HTTP; default production path remains unavailable without those ports |
| FE36 | Published safety preview mounts actual report/block/account components and controllers; shared native SafetyDialog | Synthetic target authorization and deletion port; unavailable deletion port is separately exercised |
| FE26 | Published `gift-pickup/__tests__/preview.html`, actual GiftPickup/controller, canonical Auth, intent journal, real sessionStorage and reload | Synthetic reviewed-place authority, reservation/code/claim responses; no actual collection/principal proof |
| FE31 | Public `SharingRequest` + canonical controller, existing published headless `setup` helper and approved-social-request contract fixture | QA07-only explicit synthetic approved consent/reveal/revoke HTTP; no live reciprocal consent |
| FE05 | Public shared `Dialog`/`Button` with stable initial/return refs | QA07-only sheet caller; real native focus, inert state and CSS |

Every run records the current Git candidate plus SHA-256 pins for application source,
consumed fixtures, QA07 code, original/public world resource bytes, C04/C05/C06 inputs, generated domain OpenAPI, lockfile
and original acceptance specification. The pins bind the working candidate including
new QA07 files, rather than silently treating a precommit SHA as their source revision.
The Gift owner independently confirmed its public frontend source pin
`941d36c6438d7ece727e9cdec7e67b61c646203b`; QA07 read the actual public barrel,
controller, fixture and UI before executing it. No backend component receipt is
used as evidence of mounted pickup success.

## Reproduction

Use the retained Node 24.22.2 toolchain, retained lockfile dependencies and installed
Google Chrome. From `frontend/`:

```sh
node e2e/accessibility/qa07/native.mjs
node_modules/.bin/tsc -p e2e/accessibility/qa07/tsconfig.json --incremental --tsBuildInfoFile e2e/accessibility/qa07/.cache/qa07.tsbuildinfo
node_modules/.bin/oxlint e2e/accessibility/qa07/fixture.tsx e2e/accessibility/qa07/native.mjs
```

The final application-wide type check uses `tsc -p tsconfig.json --incremental
--tsBuildInfoFile e2e/accessibility/qa07/.cache/application.tsbuildinfo`. The selected
component regression command is:

```sh
node_modules/.bin/vitest run \
  src/features/world-rooms/__tests__/ui.test.tsx \
  src/features/places/__tests__/places-ui.test.tsx \
  src/features/mural/editor/__tests__/reference-target/presentation.test.tsx \
  src/features/safety/__tests__/controller.test.ts \
  src/features/gift-pickup/__tests__/pickup.test.tsx \
  src/features/social-sharing/__tests__/ui.test.tsx \
  --cache=false --reporter=dot
```

The native runner allocates a private loopback port, checks an 8 GiB free-space
floor, writes only its owned evidence prefix/cache, and closes browser/server in
`finally`. `QA07_RUN` selects an owned evidence subdirectory; `QA07_WIDTH=390` and
`QA07_ONLY=profile` narrow a diagnostic run. A narrowed run is not the full matrix.
Each case uses a fresh context with reduced motion and records CSS width/browser
version, observations, failures, screenshots, DOM/ARIA snapshot and native Chrome
AX role/name nodes. Unnamed interactive AX nodes fail the case. Actual keyboard
Tab/Shift+Tab reaches controls; Enter/Space and text events operate them. No scripted
focus, click or fill substitutes for these journeys. Bridge navigation is used only
for the separately labeled focused-textarea unmount trigger. Trusted CDP touch
events produce a real `pointercancel`, with `isTrusted` and pointer type asserted.

## Initial findings and owner fixes

| Finding | Evidence | Disposition |
| --- | --- | --- |
| Report dismissal restores opener focus but disables Block user in the same retained safety owner | `initial/390-safety.aria.txt`, `initial/native.json`; repeated in `independent/native.json` | Genuine FE36 lifecycle failure. `controller.close()` invokes privacy-style cleanup to `closed`; UI locks `closed`. Assigned FE36 owner `01a1a204-c583-7670-965a-4891b8b1289e` diagnoses/fixes under its own lease. QA07 owns no source hunk. |
| Full mobile panel pauses replication and clears accessible Nearby roster | `root-pre-fix/390-root.aria.txt`, `root-pre-fix/native.json` | Known genuine FE15 `composition.setVisible` failure, independently reproduced through Map/Places/Back/Nearby keyboard navigation. FE15 owner `01a1a202-375e-7401-bfa1-ff8721e90ed6` owns source repair. |
| Focused profile textarea unmount leaves World input disabled after returning live | `profile-pre-fix/native.json`, repeated `independent/native.json` | Known genuine FE15 WorldShell typing lifecycle failure. Same FE15 owner repairs; QA07 must recheck the actual publication. |
| Initial root selector used nonexistent “Fit map view” | `initial/native.json` | Harness error corrected to actual “Fit map”; no product failure inferred. |
| Initial mural traversal outlasted the fixture socket's 60-second lifetime | `initial/native.json` versus `diagnostic/native.json` | Harness traversal corrected to choose forward/reverse native Tab; same frozen source passes without extending its lifetime or faking connection authority. |

Unaffected native client cells already execute sheet focus/inert/reduced motion and
trusted cancellation, explicit social reveal/hide/revoke, recipient private pickup
instructions/code, owner collection input/receipt, uncertain reservation recovery
across reload, mural arrow/Escape and DOM coordinates/explicit commit. Account
deletion unavailable/confirmation/focus checks pass at 360/390/430. The selected
existing six component suites pass 90 tests (`component-tests.log`). These passes
do not waive the affected cells or establish a successful full matrix.

The complete pre-repair run is `pre-owner-fixes/native.json`: Chrome 156.0.8078.80,
33 executed cells, 24 PASS and 9 FAIL. Each genuine failure repeats at all three
widths. Fresh block-confirmation focus and account checks pass independently.

FE15 owner publication `50d950d79939a539ebe953744287f7a280ce488b` repairs the two
World lifecycle defects. QA07 independently rebased onto current master
`8a5e9ea430efc1af153894901218f95542dcc803` and rechecked the actual unchanged Root
fixture with the repaired application source. `fe15-recheck/native.json` passes
Map/Places/Back focus, available Nearby DOM roster, Rooms navigation and exactly one
socket at all three widths. `fe15-profile-recheck/native.json` passes actual native
profile typing, focused textarea unmount, live World return, enabled keyboard-reachable
Recenter and exactly one socket at all three widths. These six affected rechecks
do not substitute the FE15 owner's results for QA07's own execution.

The suite also exercises the actual public recipient consent form: native keyboard
platform selection alone cannot enable approval; the separate explicit reciprocal
consent checkbox is required. Approval renders metadata without silently revealing
the synthetic account name. `consent-first/native.json` passes all three widths.
This expands the final matrix to 12 scenarios at three widths (36 cells).

## Original criteria mapping

| Original requirement | Implemented native observation |
| --- | --- |
| Keyboard navigation | Tab/Shift+Tab traversal, Enter/Space activation and actual text events across shared sheet, safety, social consent/reveal/revoke, pickup, map, Rooms and Nearby |
| Focus | Modal initial focus, forward/reverse trapping, inert background, report/block/account Escape return, map-sheet Escape return, Root Back return, focused-textarea unmount recovery |
| Screen-reader labels | Native Chrome accessibility-tree role/name output and no unnamed interactive nodes; explicit labels for report reason/text, social checkboxes, pickup code input/output, map image/search, mural surface/palette/coordinates/status |
| DOM alternatives | Authorized Nearby self/others, Rooms list, searchable Places sheet, labeled mural coordinates; essential actions do not depend on precise canvas input |
| Reduced motion | Actual shared Button transition changes from 120 ms to zero under media preference, sheet animation/transition disabled; journeys run with reduced motion |
| Pointer cancellation | Trusted native touchStart/touchCancel -> pointercancel (`isTrusted`, touch type); sheet stays open and mural produces no preview or write |
| Essential social/pickup without precise canvas | Explicit reciprocal consent approval, reveal/hide/revoke; reserve/unknown/reload/original-command recovery, private instructions/code, owner one-time collection form/receipt |
| Sheet-close focus return | Shared sheet Escape and Done restore opener; actual Places sheet Escape restores Places; actual safety modal closure restores its opener |

FE36 owner source repair `705da35a5f77e2207dc805d38efd518968085e26`, evidence
`abae824d8069624efe5384344e05c9a700813b7f`, publication
`69d4a0acd381692c4864f6cc549dbbd882aed240` separates benign current-owner dismissal
from real privacy/identity cleanup. The block guard remains unchanged. QA07 read
the actual repair and independently re-executed its original failing report-draft /
Escape / Block-confirmation sequence in the full final matrix. It passes at all
three widths. No manager result or source-owner test is substituted for this run.

Final tested candidate: `5eb418391012cb2cc435ef3dcb08884b95055720` (three QA07
harness commits on published FE36 repair and reconciled FE15/FE07/art peer history).
`current/native.json` is the authoritative complete **36/36 PASS** report;
`current/input-pins.json` binds the exact implementation/fixture/contracts/art bytes.
Every native interactive AX node has a name. Root observations independently retain
one original game (peak one, same instance) and one socket through panel journeys
at all widths. `checks.json` records commands, exit codes and the pin comparison.
Application and owned-fixture type checks, lint, 90 tests/six component suites,
planning preservation and the static UI-origin gate pass. The latter is a static
source check, not whole QA02 visual acceptance. No production code was authored by
QA07; its harness is outside application entrypoints and its fixture requires DEV.

Historical red reports are retained as regressions and explicitly superseded by the
full final run. The original task-local criteria are satisfied in this client-fixture
scope; real service/device/assistive-technology gates below remain independently open.

## Live and device limits

Product/Root owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` and Gift owner
`01a1a267-37a0-7723-a4e9-a3a6f5b27f52` confirm no current positive live pickup entry.
Missing exact ports include Root Gift public-router/ScopedProductProducer composition,
actual canonical Auth/Friend/local Gift privacy guard/recovery binding, private
pickup/code SDK3, qualified Claim trusted-network binding, and isolated principals/
event seed. GiftRuntime default-unavailable pickup/code and closed Claim are not
replaced by a fake live success. Source-only unpublished backend candidates are not
used as runtime proof. These ports limit live acceptance only; frozen native client
checks continue independently.

Physical iOS/Android, hardware touch/keyboard, assistive technology speech/output,
live canonical two-principal social/pickup and production admission are NOT_RUN.
Native AX names/DOM alternatives are label evidence, not proof of real screen-reader
announcements. This suite does not assert WCAG conformance or release readiness.
