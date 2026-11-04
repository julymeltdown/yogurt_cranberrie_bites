# Wall foreground CSS parity

CSS-PARITY-WALL qualifies an R01 / FE32 foreground CSS increment against the
supplied photo. The CSS fixture result is **MOCK_READY**; full Wall reference
parity remains open. Unsupported reference tools still require a human decision.
Root153 is the assigned integrator/publisher, not a technical approval stage.

## Source, ownership and provenance

- Checkout: `/Users/lhs/.codex/worktrees/9ada/mcr`.
- Branch: `codex/mcr-fe-css-parity-wall`.
- Original clean fetched/fast-forwarded base:
  `54320d71c4ae0bff3524d1b500cea772e0ba42e0`.
- Root's authorized two-file caption input:
  `1dbca746018bdd6d3470cacd9112fb7e7d7aca88`, cherry-picked unchanged locally as
  `87c95968605b8da3a8c35b170f7533c89787ca3e`.
  Root retains ownership of `MuralEditor.tsx` and the existing
  `__tests__/reference-target/presentation.test.tsx`. No non-CSS write ownership
  transferred to this worker.
- Sole worker-authored product path:
  `frontend/src/features/mural/editor/editor.css`.
- Worker documentation/evidence: this document and
  `docs/mcr/evidence/css-parity-wall/**`.
- Final CSS SHA-256:
  `b8bb2cb3c4e12a9719742c2636a035473f9b521531ffad70ff4626fcb98dbfea`.
  The final native report binds exactly these physical CSS bytes.
- New reference: original supplied `1-Photo-1.jpg`, 1280 × 853, SHA-256
  `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
  Approximate Wall region: x773 y450 width237 height397; canvas x779 y510
  width222 height253. These are manual photo estimates, not a raster-match score.

All CSS is authored for MCR. No old rendering, asset, dependency or headless
module was imported. The owned driver adapts the existing qualified
`editor/__tests__/reference-target/browser.mjs` at the original base; its
production fixture, HTTP adapter, controller, input and deterministic snapshot
remain unchanged. The original supplied reference and `docs/source/` are
untouched. Root's caption is an explicit external input, not CSS-generated text.

## Implemented foreground and iteration

The canvas viewport uses the photo's approximate 222:253 ratio. Existing renderer
resize handling preserves uniform square pixels and pointer mapping. Local
Pencil/Pan/Fit tools occupy the upper rail; Zoom controls occupy the lower rail.
Tool icons remain their actual actions. The palette has tighter symmetric
framing and one horizontally scrollable row of all 16 server colors. Targets
remain at least 44 × 44 CSS pixels.

The caption uses a narrower face and truthful status uses condensed readable
text. Actual 390px candidate-2 capture showed the 24px caption below the photo's
relative scale. The final candidate-3 increases it to 30px and reruns the entire
360/390/430 interaction matrix. The legacy driver's 75px canvas-top guard was
replaced with a photo-derived 90px allowance: the supplied photo scales to
approximately 99px at 390px width, and the final actual offset is 79px. This
records a changed reference-derived criterion rather than hiding a failed result.

Inset two-tone canvas/palette focus rings stay visible over different pixel
colors, with Canvas/CanvasText treatment in forced colors. Focus styling does
not change layout or consume pointer input. Background declarations, shared
window/titlebar/chrome, mural pixel data, write authority, cooldown and modes are
unchanged by this CSS patch. No FOOD/MERCH, left accent rail, generated label,
fake control, unsupported tool or replacement image was added.

## Native evidence and measured result

Actual Chrome **156.0.8078.80**, Node **24.22.2**, mobile/touch emulation, 844px
height and reduced motion. Final source-bound report:
[final native report](../evidence/css-parity-wall/candidate-3/report.json).

| Width | Panel width × height | Canvas width × height | Canvas top | Canvas share | Palette height |
| --- | --- | --- | ---: | ---: | ---: |
| 360 | 348 × 650.48 | 342 × 389.75 | 79px | 59.92% | 56px |
| 390 | 378 × 684.67 | 372 × 423.94 | 79px | 61.92% | 56px |
| 430 | 418 × 730.27 | 412 × 469.53 | 79px | 64.30% | 56px |

The old-CSS 390px baseline is 378 × 668.72 with a 372 × 409.19 canvas and 62px
palette. Both baseline and candidates consume the same authorized Root caption.
Baseline CSS comes from the original Git blob; candidate CSS comes from the
physical checkout. This is an active feature fixture, not production-root proof.

The worker opened and compared the baseline390, candidate-2 loaded360/390/430,
normal/forced focus, and final candidate-3 loaded360/390/430, normal/forced canvas
focus, palette focus and explicit-selection preview captures. Final capture
links: [390px](../evidence/css-parity-wall/candidate-3/390-loaded.png),
[360px](../evidence/css-parity-wall/candidate-3/360-loaded.png),
[430px](../evidence/css-parity-wall/candidate-3/430-loaded.png),
[focus](../evidence/css-parity-wall/candidate-3/390-focus.png),
[forced focus](../evidence/css-parity-wall/candidate-3/390-forced-focus.png),
[selection](../evidence/css-parity-wall/candidate-3/390-preview.png).

The native driver passes the visible `THE BLACK PARADE` caption and true
128 × 128 dimensions in expanded help; every one of 16,384 raster cell centers;
Fit-corner and zoom-center pointer mapping; all 16 palette colors and scrolling;
44px visible targets; no page overflow or page errors; keyboard and pointer pan,
selection and Escape; one explicit confirmation POST; server-derived cooldown;
unknown/pending command recovery without replay; frozen/paused/read-only viewing;
privacy clearing; and reachable recovery at 420px height.

## Commands, failures and resource release

All commands ran from this checkout using the absolute pinned Node, bin-first
child PATH and the retained read-only dependencies. Exact argv, timings and
exit codes are recorded in [the resource receipt](../evidence/css-parity-wall/resource-release.json).

| Workflow | Result |
| --- | --- |
| Existing presentation test via `vitest run --config <owned tests.config.mjs> --configLoader runner` | Exit 0; 4/4 tests, including Root caption and true dimensions. |
| `node <owned native.mjs>`, baseline390 | Final exit 0. |
| Same driver, candidate-2 and candidate-3 at360/390/430 | Both exit 0; final CSS is candidate-3. |

The first baseline and candidate-1 failed on the harness's CommonJS Playwright
import. The next baseline failed on a dangling transient Chrome-profile symlink
in byte accounting. Another baseline failed because programmatic focus after
pointer input did not establish keyboard modality. The driver now uses the
CommonJS entry, ignores symlinks/transient missing files for accounting, and
presses Tab before asserting keyboard focus. All failed logs/reports and the
intermediate capture are retained; no production behavior was changed to make
those probes pass. The private test configuration emitted a dynamic-import
analysis warning; all four tests passed. Neither private config writes to a
borrowed `.vite-temp` path.

Offer `CSS-WALL-FIRST-202611041128Z` was accepted at11:28:39Z, with work cutoff
11:32:39Z and release deadline11:33:39Z on2026-11-04. A process/time/bytes/disk
supervisor was installed before children. FULLRELEASE at11:31:52.335Z took
193.335s: peak whole30,427,314B, temp29,280,891B, durable1,155,988B, minimum
free20,412,588,032B. Port5299 was bind-free and the owned dependency link,
Chrome/Node/Vite, TMP, profile and caches were removed. A last-inventory
short-lived `ps` PID was confirmed missing by fresh `ps -p` exit1; the corrected
release audit records no live owned PID. No install, build, Rust or DB job ran.
The latest offer permitted only this driver and four tests; the broader planning
checker, full build/typecheck, actual-root and live-service gates are NOT_RUN
for this increment.

## Remaining limits and handoff

- The human-pending Erase/Fill/Undo tool decision is unresolved. Current Pencil,
  Pan, Fit, Zoom, Cancel and Confirm semantics remain truthful; CSS does not
  claim to reproduce the photo's literal unsupported icons/actions.
- The deterministic fixture pattern is not the photo's marching figure or
  approved product mural art. No source pixels, raster grid painter or art was
  changed. Photo counts and cooldowns are not fabricated.
- All 16 real server colors plus 44px targets cannot have the photo's tiny-cell
  density at360px. The palette remains intentionally scrollable and accessible.
- Shared shell/titlebar/close chrome belongs to Root153. The standalone qualified
  feature fixture does not certify the actual root's geometry. The default app's
  guarded unavailable path and authority were not changed or bypassed; a new
  actual-root runtime capture was not part of this allocation.
- Real multiuser IN04, devices, screen readers, deployment and release remain
  NOT_RUN. Fixture success does not imply server acceptance or full visual parity.

The aligned unbudgeted persistent goal remains unfinished; the goal API still
reports BLOCKED and cannot be reactivated by this worker's status tool. The
resource blocker is resolved; remaining goal requirements are publication/remote
proof and the literal reference-tool/content limits. No goal reset, smaller
replacement goal or false completion was used. Root153 receives the committed
CSS/evidence handoff directly. There is no king/SL review or acknowledgment gate.

The post-capture source consistency check matched every final report source hash
against the physical checkout and both borrowed caption files against Root's
exact input blobs. `git diff --check` exited0. `git fetch origin master` completed
without changing the worker checkout. The worker commit stages only the sole
CSS path and the two explicitly owned documentation/evidence prefixes.

The initial staged whitespace check caught trailing spaces and a terminal blank
line in the passed presentation test's raw stdout. Exact raw UTF-8 bytes and
their hash are preserved in `presentation-raw.json`; the readable
`presentation.log` is whitespace-normalized and its manifest entry updated.
This changes no test result or product source. The final staged whitespace
check passes; the failed initial check is recorded rather than omitted.
