# FE-REF-WALL reference layout handoff

The production wall presentation now follows the supplied bottom-row fourth
WALL composition: compact black headings, a dominant portrait canvas viewport,
right-side tool stack, framed single-row palette, and small truthful status.
This is a presentation implementation using the existing real raster and
controller, not a screenshot substitute or an assertion of matching mural art.

## Scope and provenance

- Assigned isolated checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-wall-reference-target`.
- Branch: `codex/mcr-fe-ref-wall`; base
  `f837bef676cd26869c26c172a699a91b7842d832`.
- Product source commit: `832c7de9c57f6f8931b5564f61f68d82fa99d89d`, parent
  `f837bef`. Exactly three product paths changed:
  `frontend/src/features/mural/editor/MuralEditor.tsx`,
  `frontend/src/features/mural/editor/editor.css`, and
  `frontend/src/features/mural/sync/MuralSyncPanel.tsx`.
- Additional owned paths: new editor/sync `__tests__/reference-target/**` and
  this document. Existing tests, controller/input/HTTP/contracts, root/shared
  components/styles, manifests, locks, backend and art are unchanged.
- The exact viewed reference has SHA-256
  `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
  The coordinator's durable copy is
  `docs/mcr/design/reference-target-2026-11-04.png` at `cdcd834f`.
  This worker verified the supplied clipboard PNG against that hash, without
  duplicating it into product assets or importing it as a texture.
- Rendering remains newly authored React/CSS/Canvas. Existing frozen-contract
  test fixtures, canonical HttpClient, editor/sync controllers and texture-sink
  fixture are reused from the assigned base. There is no new dependency or old
  rendering/artwork import. `docs/source/` remains byte-identical; planning
  provenance checks pass.

At assignment, `get_goal` returned no unfinished goal. A new unbudgeted
FE-REF-WALL goal was created and returned `active`, covering production layout,
meaningful validation, inspected native evidence, documentation, committed
handoff and concrete fixes. The selected model/reasoning was preserved. Former
checkouts remain read-only. Frontendking integrates and pushes; this writer does
neither. The final source pin was sent directly to Frontendking, Operations SL
and the separately assigned QA owner before documentation completion.

## Layout and behavior

The existing `ConcertWindow` remains the semantic owner. Its standalone title is
compact, followed by a black dimension heading derived from the loaded
snapshot. An event/album title is not invented when the DTO supplies none.
Canvas comes before instructions/status. Pencil and Pan occupy the upper right;
Zoom in/out and Fit occupy the lower right, with the actual zoom factor. Every
button is at least 44 × 44 CSS pixels. Pencil previews; pan/touch drag never
writes. No bucket, erase, undo or unsupported role control is introduced.

The viewport aspect is 10:11, close to the reference's approximately 274 × 304
canvas. The authoritative model stays 128 × 128. The canvas backing height
tracks the viewport while both axes use the same width-based scale. A square
grid is centered in the portrait viewport with small vertical margins. Raster
drawing, selection outline, pointer conversion, keyboard centering, zoom and
vertical pan bounds use the same offset/viewport height. Pixels are not stretched
and tapping a margin cannot select an out-of-bounds pixel. The existing input
module and command controller are unchanged.

All 16 actual snapshot palette colors remain available. Their 44px targets form
one deliberately horizontally scrollable row rather than compressed tiny
swatches or multiple rows. A visible `16 colors · scroll →` cue accompanies the
ready footer; the palette also has a named focusable group and screen-reader
instructions. Left/Right/Home/End scroll its focused container; Tab reaches each
color button, and Enter selects without a POST. Selected colors retain a check
and `aria-pressed`. Overflow is confined to this labeled control, not the page.

The preview/Confirm/Cancel row appears only for a real selection or unresolved
command. Confirmation is explicit and sends one pixel; Enter on the canvas
never confirms. The row shows the selected coordinates/color and confirmed
color. Cancel/Escape discard only the local preview. Coordinates and instructions
are in a native expandable `Coordinates & help` control, retaining labeled
number inputs, bounded validation, keyboard preview and a separate confirmation.

Bottom status reflects actual confirmed revision, snapshot state, palette,
availability, server-derived remaining cooldown and original command state.
Synchronization errors, gap and original-result warnings stay outside collapsed
help. Pending/unknown recovery remains visible, retains the same key across a
snapshot GET, and checks only the original receipt/result without another POST.
Paused, frozen and read-only walls retain pan/zoom/fit while placement and color
changes stay disabled. Privacy cleanup removes the raster, palette and preview.
No balances, progress counts, cooldowns, names or live activity are fabricated.

## Root seam

The root owner supplied the planned inherited `--mcr-wall-viewport-height` and
`--mcr-panel-available-height` seam, plus automatic `ConcertWindowEmbedding`.
This patch consumes only the height variable in scoped CSS; no uncommitted root
source or new shared export is imported. A first embedded window can have its
duplicate frame/title suppressed by that separately owned context.

The canvas height is constrained by available root height but never below its
square-grid width, preserving Fit semantics. At short heights, normal vertical
panel scrolling keeps recovery and expanded keyboard controls reachable. Native
checks exercise 420px height with the root height variable. Full production-root
embedding needs its own combined evidence after the root pin; standalone checks
here do not claim it.

## Measured native result

Actual Chrome `156.0.8078.80`, 844px height, touch/mobile emulation, reduced motion:

| Width | Panel height | Canvas | Canvas top within panel | Canvas / panel height | Palette height |
| --- | ---: | --- | ---: | ---: | ---: |
| 360 | 635.53px | 340 × 374px | 72px | 58.85% | 62px |
| 390 | 668.53px | 370 × 407px | 72px | 60.88% | 62px |
| 430 | 712.53px | 410 × 451px | 72px | 63.30% | 62px |

The reference's approximate canvas/frame height share is 64%; measurements are
visual crop estimates, not a claim of exact pixel equivalence. The candidate
retains accessible target sizes and explicit write confirmation. QA independently
measured the old 390px baseline as a 1101px panel, canvas offset 226.47px,
346px square canvas and 140px palette. That baseline was reported by the QA
owner; the new measurements above were executed by this writer.

Fifteen PNGs and `report.json` live under
`frontend/src/features/mural/editor/__tests__/reference-target/evidence/`.
Loaded images at all three widths, plus the 390px preview and unknown states,
were visually inspected against the exact source composition. The test mural's
original deterministic pattern is clearly fixture content, not the reference
illustration or production data. The report binds the final three source files,
unchanged input/controller/HTTP dependencies, fixture/harness, shared window and
public mural schema by SHA-256 to the base. Final source hashes were rechecked
against commit `832c7de9` after capture.

## Validation

`NODE` is `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`.
The existing dependency installation was linked temporarily through
`frontend/node_modules`; no install/copy/lockfile change occurred. The browser
runner uses and removes its own assigned-path Vite cache. No Cargo, database or
service access was used.

| Command | Exit | Evidence |
| --- | --- | --- |
| `NODE frontend/node_modules/vitest/vitest.mjs run frontend/src/features/mural/editor frontend/src/features/mural/sync` | 0 | 131 tests / 9 files, including all 126 existing tests plus 5 new presentation tests. |
| `NODE frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false` | 0 | Whole frontend strict types. |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src/features/mural/editor/MuralEditor.tsx frontend/src/features/mural/sync/MuralSyncPanel.tsx frontend/src/features/mural/editor/__tests__/reference-target frontend/src/features/mural/sync/__tests__/reference-target` | 0 | Scoped lint. |
| `NODE frontend/src/features/mural/editor/__tests__/reference-target/browser.mjs` | 0 | All three widths; raster cell equality, portrait and zoom pointer mapping, palette reachability, keyboard/drag behavior, single explicit POST, real cooldown, pending/unknown GET recovery, frozen/paused/read-only/privacy, short-height recovery, no page errors or page overflow, visible targets ≥44px. |
| `NODE node_modules/vite/bin/vite.js build --configLoader runner` (cwd `frontend`) | 0 | Production bundle; retained large-chunk warning. This does not certify root-flow or deployment acceptance. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance PASS, including original archive checksums. |
| `git diff --check` | 0 | No whitespace errors. |

The native runner verifies every one of the 16,384 visible cell centers against
the current snapshot palette/pixels, not just canvas presence. It checks fit
corner `(127,127)` and zoomed center `(64,64)`, actual local preview/cancel,
coordinate preview, Enter-only confirmation, one POST, cooldown derived from a
real fixture response, preserved original key after GET and original-result
recovery. Fixtures go through the production canonical HTTP/sync path.

Material probe corrections: the first native scenario attempted FROZEN → PAUSED
within one fixture lifetime and correctly failed closed. The probe now opens a
fresh context for the independent paused case; no unfreeze capability was added.
JSDOM did not hide closed-details descendants in its role query, so the focused
test asserts the actual disclosure state while Chrome verifies real visibility.
New test-only typing errors (`exact` in Testing Library options and a narrowed
authority-port type) were fixed without changing production contracts.

Disk-floor coordination paused new captures/builds when free space fell below
8 GiB. The authorized sparse checkout skips backend/historical PNG worktree
material while retaining supplied public originals and adding explicit includes
for new owned evidence. Free space recovered above 8 GiB before captures/build;
the browser checks the floor before capture. Final evidence is approximately
736 KiB. Temporary local build output, browser cache and dependency link are
removed at final handoff; no other writer's data is removed.

## Acceptance boundary

Task-local implementation and native fixture evidence are **MOCK_READY** and
committed for Frontendking integration. Independent FE-REF-WALL-QA consumes the
exact final three-path source pin through read-only Git blobs and owns its own
report; it is not a mandatory approval gate. Full root composition, real server
multiuser convergence, IN04, physical devices/assistive hardware, load and
deployment remain **NOT_RUN** by this presentation increment. No required scope
or release acceptance criteria are cut or silently replaced by fixture results.
