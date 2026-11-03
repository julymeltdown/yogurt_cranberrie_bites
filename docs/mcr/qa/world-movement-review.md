# FE15 movement, camera and interaction independent review

Date: 2026-11-03. **Bounded review complete: R1 and R2 resolved in exact final
candidate `80a171d`; no remaining actionable defect identified. Parent report
review/disposition pending.** Implementation/evidence status remains MOCK_READY.
This report does not accept real networking, authoritative server physics,
venue arrival or the complete world product.

## Scope and authority

QA checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-fe-movement`, branch
`codex/mcr-qa-FE15`, initial clean base
`b1a19c425a30641410d71986714d72b7101f43e8`.
Implementation source is read from the isolated `fe-movement` checkout. QA's
only tracked writable file is this report. No implementation, fixtures,
contracts, assets, manifests, dependencies or worker browser were modified.
No Rust, PostgreSQL or shared target work was performed.

Exact worker chain: initial `0511aeab7916967e65bb4cd41508d9a213d26b4e`, then
boundary correction **`80a171db2663f407825659aa2d0b32de213e0fb8`**. The clean
worker handoff and both committed parents were verified. Authorized local QA
cherries are `a33a6993a499e54603ff6da6240bf29f430cdc47` and
`339b250e018b113ad9fb9915b2871a9972a71272`; they are worker application code,
not QA-authored integration patches. The parent takes only the separate report
commit from this branch.

The fourteen worker paths comprise this explicit scope:

- New `frontend/src/world/input/{controller.ts,model.ts,simulation.ts,controls.css}`,
  `camera/port.ts` and `interaction/selection.ts`.
- Existing `frontend/src/world/runtime.ts` and `scenes/MapScene.ts` limited seams.
- New `frontend/src/world/__tests__/movement.test.ts`,
  `movement-browser-smoke.mjs` and
  `fixtures/{movement-data.ts,movement-preview.html,movement-preview.ts}`.
- `docs/mcr/frontend/world-movement.md`.

The follow-up changes only simulation, movement unit tests and that worker
document. Cross-import and exact-path review found no central configuration,
manifest, lockfile, backend, contract, supplied artwork or `docs/source/` change.

The specification's FE15 scope is pointer-safe joystick/tap movement, keyboard
control, collision prediction, recenter, interaction selection and panel pause.
Opening keyboards/panels must not accidentally move the player. Geometry is
shared across themes; virtual movement never proves physical arrival. Root and
frontend working agreements, the implementation specification, assessment and
execution-plan references, retained FE14 source/tests and C05 policy/types were
read as authorities. Local speed/foot/radius injection is prediction policy,
not an invented BE12 rule or authorization guarantee.

## Finding R1: stale same-owner visibility continuation

**P2; parent accepted as actionable.** The initial uncommitted
`frontend/src/world/runtime.ts:139–143` set `visible`, called
`controls.setEnabled(...)`, then forwarded the original `value` to the engine
if `active === owned`. A supported reentrant attachment or neutral consumer
could call `runtime.setVisible(true)` during an outer `setVisible(false)` without
replacing the owner. The identity check still passed in the outer call.

QA's independent probe imported the actual runtime through its own Vite SSR
server, used jsdom and a minimal engine/attachment to record the public seam,
then invoked exactly that nested update. The engine received **`[true, false]`**
while the runtime remained ready and its latest visibility/control state was
true. This could leave rendering asleep with controls enabled until another
visibility update. It is a stale continuation within one generation, distinct
from the retained stop/dispose/restart generation checks.

The worker added a visibility revision and requires both owner identity and
the latest revision before the engine call. Draft and exact committed retests
produced **`[true]`**, with an explicit last-value assertion passing. The unit
regression covers both final values. Its retained pre-fix run reports 67 PASS /
2 FAIL, and final full-suite evidence includes both green cases. The actual
native fixture additionally verifies newer show leaves the Phaser loop awake
and newer hide leaves it asleep at 390px. QA independently repeated both
directions at 360px on the final candidate. **R1 resolved.**

Initial probe artifacts are preserved under
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-qa-fe15-agy8u3i5/`:

| Artifact | SHA-256 |
|---|---|
| `initial-source-probe.mjs` | `cc83a9b9de8d5e028742d9d41c0c005de1b79fa0d91b439a6d8038361c75dd70` |
| `initial-source-result.json` | `09952d3aaa6b75468efedbf4dbe2ef094608d5f097eaffe5075ff3db1832753c` |

The worker changed the draft before an original-source hash snapshot was taken;
no hash-bound original source revision is claimed. Tool-captured source and the
preserved reproducer/result establish the initial observation. Exact final
source and evidence verification is recorded separately below.

## Finding R2: valid open-bottom route refused

**P2; parent/worker finding, independently confirmed by QA.** Final-disposition
review of initial commit `0511aeab7916967e65bb4cd41508d9a213d26b4e` was held when
the parent identified an open-bottom-border mismatch. `walkable` permits a foot
ending exactly at world height. `route` derives a cell with
`floor(point.y / tile_height_px)`, which is then one row beyond the grid.
The resulting center is outside the map and the early segment check returns
null even though both endpoints are valid and connected.

QA probed the exact committed simulation bytes in its authorized QA cherry,
not the worker's already-corrected working copy. With a 4×4 open grid, 32px
tiles and an 8×6px foot, these all returned null promptly while both endpoints
were walkable: `(48,48) → (48,128)`, the reverse, and
`(48,128) → (80,128)`. A player predicted onto the open bottom edge can lose
tap routing until another input moves it off the boundary.

**No reconstruction hang was reproduced.** The early invalid-anchor segment
check returns before parent reconstruction in these cases. The parent corrected
its earlier risk wording to the same precise false-refusal finding. A bounded
parent walk remains a requested defensive correction, not evidence of a hang.
The exact initial simulation SHA-256 is
`1e80d5b1553c77ae6255ca9f190ff09da22636ee35713094354bdbf17fe88842`.
Its bytes and results are preserved as `simulation-0511aea.ts` and
`bottom-edge-initial.json` in the owned QA evidence directory.

At exact `80a171d`, start and target indices are clipped to valid occupied
cells after whole-foot walkability checks. The requested world target is
preserved, not moved inward. BFS still checks full-foot segments and uses
four-neighbor paths. `reconstructCells` requires valid start/target and a
self-rooted start, validates every parent and limits traversal to the bounded
grid size; corrupt, missing, self-cyclic and multi-node-cyclic chains return null.

The worker added start/destination/same-point and both bottom-corner routes,
plus bounded valid-chain and ten invalid-chain regressions. QA independently
ran ten legal routes on 2×2 and 4×4 open grids, checking exact final endpoints,
every point and segment walkable and path length bounded by cells+1. Five
corrupt chains returned null; a valid chain reconstructed correctly; an actual
outside point remained rejected. These checks import the exact corrected QA
source without reimplementing its routing. **R2 resolved.** This is local
prediction evidence, not an assertion of server collision agreement.

## Source and test review

The movement model emits only C05 `axisX`, `axisY`, `buttons`; it owns no socket,
stream/epoch/sequence, token, membership or authoritative position. It consumes
the frozen policy's 20Hz simulation target and normalizes diagonal axes. The
DOM controller owns one bounded RAF accumulator and clears elapsed time on pause
instead of replaying background time. Interaction requests are one-shot local
intents, not server acceptance.

The collision module validates finite policy and C06-compatible grid bounds,
checks the complete injected bottom-center foot rectangle, bounds elapsed time,
and subdivides motion before axis-separated sliding. Tap routing is bounded
four-neighbor BFS, with full-foot segment checks; blocked/nonfinite destinations
fail locally. Nearby interactions require an open place, explicit local range
and a clear foot sweep; selection remains stable while eligible.

Keyboard input requires focused canvas/joystick and excludes editable/dialog
ancestors, composition and modifiers. Pointer state owns one captured pointer;
wrong-pointer changes do not steer it. Cancel/lost capture, blur, hidden state,
focus/panel changes, abort and disposal clear movement, routes and queued
interaction. Disposal removes owned listeners, capture, RAF and DOM, while
preserving later attribute replacements. Consumer callbacks are fenced before
late step intent emission. Exact final unit and native evidence covers these
boundaries as recorded below. Immediate safety-neutral intents may occur outside
the 50ms cadence; FE17 must coalesce/rate-limit actual wire traffic against C05's
30/s limit. There is no transport send in this increment.

The Phaser seam uses actual camera scroll/zoom and `getWorldPoint` with canvas
CSS scaling. It refreshes the matrix before projection, retains camera state on
resize and supports local look-away/recenter. It does not add a second game or
enable Phaser input plugins. Retained inherited drain and readiness fences are
still present. The retained native report exercises inherited close exclusion,
late image release and stop/dispose/restart at readiness. This supplements the
unchanged source fences; it does not certify future transport ownership.

## Independent native draft probe

QA launched its own headless installed Chrome **156.0.8078.80** and an owned
Vite server at `127.0.0.1:45837`, with temporary cache outside both checkouts.
It used the worker's synthetic resource fixture read-only. This was a separate
browser/process from the worker's. Both closed normally; a subsequent listener
check found no server on that port.

At 360×800 and 360×420, the actual draft Phaser/DOM probe passed:

- Focused keyboard movement from `(80,112)` to `(92,112)`, followed by a neutral
  intent and a stable foot after release.
- Native CDP touch movement and `touchCancel`, followed by a stable predicted
  foot and no page error.
- Actual world-point round-trips through zoom 1/2/3 and resize, within `1e-6`.
- Six visible button/select controls each at least 44×44 CSS pixels.
- Stop/drain removed the canvas and movement controls.

QA inspected `native-360.png`: synthetic terrain and a geometric foot marker,
legible controls, ordinary symmetric borders and focus styling; no character
replacement or decorative left accent. This is draft fixture evidence at one
width, not four-width final evidence, a real device or real network acceptance.

| Artifact | SHA-256 |
|---|---|
| `browser-probe.mjs` | `3e0e7aa1fab72480775c3f05ac2a4b0291895a0fbf63b901559bfc0524f07063` |
| `browser-result.json` | `62d0083f94ca434ef46a36c83b64a72dcce2b7cfe896b4fccb7f9e42ab16f6d2` |
| `native-360.png` | `def2086e67e34d3f04821f75a5fc55bdf74a2a423d44a8d752185d8337cb2f0b` |

The first QA browser-launch attempt failed because a CommonJS package's dynamic
import did not expose the named `chromium` binding. The QA harness switched to
its actual CommonJS export and the subsequent native run passed. No product
change was made to resolve that harness error. The source/SSR probes used Node
26.7.0; required worker toolchain/gates remain a separate final-evidence review.

## Exact final evidence review

Final worker receipt: `/tmp/mcr-fe15-evidence/boundary-final/receipt.json`,
SHA-256 `0a693454a6f1dfc53fb224f3f6c29601ede7edbc3df849d7a32efe2d6f865874`.
QA independently verified all **69** referenced hashes and byte counts:
14 delivered files, eight read-only provenance files, 14 check logs, 23 native
artifacts and ten historical logs. Delivered bytes match both exact worker
commit `80a171d` and the authorized QA cherry. Provenance bytes also match the
assignment base. `boundary-receipt-verification.json` in the owned QA directory
records the comparison. A receipt binds reviewed local evidence; it is not an
independent rerun of each worker command.

Key final source hashes:

| Path under `frontend/src/world/` | SHA-256 |
|---|---|
| `input/simulation.ts` | `f07ce399ae7dab373c80a97b97b6a5d960d05be38a3d84f31d0c1f60f91f37d3` |
| `__tests__/movement.test.ts` | `86f3f6fd645728f037c7f11a58f34e7939d08bae391f1e00184a7c5e8a741d94` |
| `runtime.ts` | `062d3e4272ebefc85e8c5505b2305ab5bd2d8fb21db1024f2d5843a899d8ffee` |
| `scenes/MapScene.ts` | `04061aa690705faca2c9a28c829329096256292c8e3496f25895b631cb11b88f` |
| `__tests__/movement-browser-smoke.mjs` | `3396fda707046c1b52fe11d161e86614460930336da47ac8bcdce27a819143b9` |

QA read the following worker logs and the scripts/tests producing the relevant
evidence. Commands and exact local paths are also recorded in the final worker
document, SHA-256
`551561e628476b6b8d3ddf6219b8ba6e09e19e4b0122cbcbf99009ca4e2e5741`.
Worker environment: Node 24.22.2, pnpm 11.7.0 and installed Chrome 156.0.8078.80.

| Worker command/check | Observed result and evidence |
|---|---|
| `MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python corepack pnpm@11.7.0 --dir frontend test` | Exit 0, 958 tests / 22 files, including 87 FE15 cases; `/tmp/mcr-fe15-full-tests-boundary.txt`. Summary log contains no failure or skip. |
| `corepack pnpm@11.7.0 --dir frontend build` | Exit 0, TypeScript and production build; `/tmp/mcr-fe15-build-boundary.txt`. Retained 544.82kB main-chunk warning remains. |
| `corepack pnpm@11.7.0 --dir frontend lint` | Exit 0; `/tmp/mcr-fe15-lint-boundary.txt`. |
| `MCR_FE15_EVIDENCE=/tmp/mcr-fe15-evidence/boundary-final node src/world/__tests__/movement-browser-smoke.mjs` from frontend | Exit 0; `/tmp/mcr-fe15-browser-boundary-final.txt` and final native report. Four widths and all short viewports PASS. |
| `MCR_FE14_EVIDENCE=/tmp/mcr-fe15-evidence/retained-fe14 node src/world/__tests__/runtime-browser-smoke.mjs` from frontend | Exit 0; `/tmp/mcr-fe15-retained-runtime.txt` and retained report. Four widths, held-close/readiness races and resource failure/hash/late cleanup covered. The injected `FE14_SYNC_START_THROW` unhandled rejection is explicitly controlled; page-errors array is empty. |
| `python backend/scripts/docs/generate-mcr-core.py --check`; `python scripts/mcr/validate-core.py` | Both exit 0; C03 generated artifacts current and 33 fixtures CONTRACT_READY. Producer runtime remains NOT_RUN. |
| `python scripts/mcr/validate-assets.py`; `python -B -m unittest discover -s scripts/mcr/tests -p test_assets.py` | Both exit 0; C06 metadata CONTRACT_READY and 27 regressions PASS. Physical artwork/license/resource truth is not accepted. |
| `node scripts/mcr/check-ui-origin.mjs` | Exit 0; C08 static origin/import/byte checks PASS. Not complete product UI acceptance. |
| Plan and exact frontend boundary checks | Exit 0, fourteen assigned paths; receipt includes `/tmp/mcr-fe15-plan-boundary.json` and `/tmp/mcr-fe15-boundaries-boundary.txt`. |

The receipt's fourteen checks contain **thirteen exits 0 and one intentional
historical exit 1**: the original R1 red regression. It must not be described as
fourteen green checks. Initial `0511aea` evidence had 940 tests / 69 FE15 cases;
the corrected 958 / 87 result supersedes that count. Initial receipt SHA-256
`760fe5d60c238dbd1720ff23c79c8927910ff558e5e8369672c093a77bfa5755`
and its 67 hash-bound references were checked before correction. Receipt plus
referenced check/evidence/history files are preserved in the owned QA directory's
`initial-worker-evidence/`, including `receipt-0511aea.json`, without overwriting
the prior observation.

The final native report covers 360×800, 390×844, 430×932 and 1280×800, plus each
width at 420px height. QA visually inspected all eight movement/short-control
images. Their final hashes equal the already inspected initial-candidate images.
Controls remain legible and at least 44px; terrain walls are map content, with
no decorative left rails. Actual pointer/CDP touch, cancellation, panel blocking,
tap/collision, one-shot interaction, camera zoom/resize and teardown are exercised.
Production `/`, signup, verify-email and legal/privacy checks report no fixture,
canvas, Phaser or API request; distribution scans exclude the FE15 fixture marker.

Historical worker harness failures remain visible: wrong runner path, duplicate
variable parsing, offscreen touch target and an invalid desktop-scroll assumption
corrected by zooming. Exploratory fifth/sixth/eighth passes precede final pinning;
the interrupted seventh run has an empty log and is not success evidence.
Intermediate parse/floating assertion and tuple-spread errors were corrected
before final validation. These are not silently counted as final passes.

## Independent final probes and QA validation

QA replayed the old committed bottom-edge source, then imported the corrected
QA-cherry source for the ten-route/five-corrupt-chain checks above. QA also
repeated its runtime source probe against the unchanged final runtime bytes:
the stale call is suppressed and the engine receives `[true]`.

The final owned native probe uses exact corrected application bytes, its own
browser and port 45837. Keyboard/release, CDP touch/cancel, zoom 1/2/3 across
360×800 and 360×420, six 44px controls and empty page errors all pass again.
For same-owner nested visibility, actual browser RAF accounting settles at one
pending frame after newer show and zero after newer hide; controls agree with
the final visibility. Stop/drain removes both canvas and controls.
The first boundary probe attempted a Phaser debug global not exposed by the
fixture and failed in the harness; `browser-global-debug-failure.txt` preserves
that attempt. The successful probe observes browser RAF without product edits.

Final owned artifacts include `bottom-edge-fixed.{mjs,json}`,
`browser-probe-boundary.mjs`, `browser-result-boundary.json`,
`native-360-boundary.png` and `source-{probe,result}-fixed` files. Their exact
hashes, byte counts and QA command/exit records are in
`boundary-independent-evidence.json` in the owned QA directory. All QA browser
and server resources were closed; the final port-listener check is empty.

QA's own final checks: `python3 -B scripts/mcr/check-plan.py`, frontend boundary
checker from the original base with both exact worker/QA documentation paths
allowed, `git diff --check` for candidate and staged QA report, all exit 0.
The separate report commit changes this file alone; parent integration must
take that commit, not the local application cherries.

## Disposition and limitations

R1 and R2 are resolved against exact final `80a171d`; no additional actionable
defect was identified within this bounded source, regression and native-fixture
review. The unbudgeted persistent goal remains ACTIVE until the parent reviews
the committed QA handoff and resolves any report fixes. This does not certify
BE12 authority, FE17 networking/reconciliation, real multiuser behavior, selected
character artwork, physical device input or physical venue arrival; those gates
remain NOT_RUN here. Implementation status is MOCK_READY.
