# CSS-PARITY-LANDING — foreground spacing

Status: **INTEGRATED locally; publication pending**. The responsible developer
completed the residual CSS implementation and validation described below. The
first-pass publication was a milestone, not automatic screen-goal closure.
Whole-screen exact parity remains open for the explicit artwork/background and
semantic-control differences listed under Limits. No king/SL review gate applies.

## Residual assessment — completed owner validation

Renewed exclusive scope is unchanged. Root153 clarified that existing wordmark
SVG paths/source remain byte-for-byte fixed, while its CSS box position, size
and proportions may change. Selected-character gallery geometry/framing/UV/crop
and all background paint/assets remain fixed.

The first pass used crop `(5,3,254,440)`, 249px wide. The original source's
first panel declaration is XYWH `(6,4,296,524)` in the 1536×1024 PNG. Mapping it
onto the 1280×853 NEW JPEG yields LTRB
`(5,3.33203125,251.66666666666669,439.828125)`, 246.667px wide. The former crop
included about 2.333 extra gutter columns; its 0/1px copy/CTA edge errors were
not final placement precision. All first-pass raw evidence remains intact.

The full-poster SVG rule also stretched the wordmark's 286×485 viewBox into
366×629.281 CSS pixels at 390px. This pass registers its unchanged paths to the
complete source panel: inset `.89% / -.24%`, width `99.79%`, height `98.4%`.
Copy top/left/width/height are `73.11% / 18.95% / 64.55% / 13.59%`; CTA bottom
is `3.55%`, with horizontal insets `9.53% / 8.13%`. A second iteration removes
the redundant outer CSS border only from the anonymous real Link, whose printed
SVG already contains the complete symmetric frame. Disabled buttons retain
their ordinary border and truthful state; visible keyboard focus remains.

Fresh baseline, candidate and final native runs all exited 0. Final CSS SHA256:
`1f3a6e9374b4f38c0fb6098f2bb6a7384574f8550f61737b16a7dd8a20d8382c`.
The final driver exercised 360/390/430px, real sign-in return navigation, skip
and CTA focus, About disclosure, Safety focus/back/home, 420px-high viewport
reachability, disabled unavailable entry and retry. No page errors, horizontal
overflow or world/canvas host appeared. All nine protected source hashes and
selected-gallery dimensions, relative coordinates, atlas positions, background
size, transforms and pixel rendering equal their baselines at each width.

Native 360/430px poster captures were inspected from the first candidate; the
last anonymous-border refinement was exercised at all three widths and captured
at 390px with visible focus. The final comparison uses the corrected crop,
uniformly scaled against the actual 390px poster. Its coarse cream-mask overlap
supports, but does not certify, placement:

| Region | Published first pass | Final candidate |
|---|---:|---:|
| Wordmark | 0.6199 | 0.8965 |
| Copy | 0.2286 | 0.5802 |
| CTA | 0.3628 | 0.8885 |

These are binary-mask IoUs, **not** whole-screen acceptance percentages. JPEG
rasterization and incidental reference marks affect thresholds/bounds. A further
local registration search retained the current wordmark optimum; its best copy
proposal improved overlap only about 0.004, within small raster-phase effects.
Visual inspection found no further meaningful CSS-controlled correction within
this exclusive scope. Frozen art/background/control-source differences remain
explicit limits rather than reasons to alter protected inputs.

Evidence under `docs/mcr/evidence/css-parity-landing/round-02/`:

- `comparison-final-390.png`: NEW reference, published first pass, final candidate.
- `candidate-poster-360.png` / `candidate-poster-430.png`: native size checks before
  the final border-only iteration; `final/candidate-poster-390.png` and
  `final/candidate-focused-390.png`: exact final CSS captures.
- `baseline.json`, `candidate.json`, `final/candidate.json`: native behavior,
  gallery/source protection and loaded CSS hashes; all PASS.
- `residual-assessment-final.json`, `refinement-proposals.json`, `checks.json`:
  corrected registration, bounded follow-up search and validation results.
- `*-attempt.log`, `cleanup.json`: actual commands, exit codes, guard and cleanup
  outcomes. No failed validation attempts are omitted.

Resource offer `CSS-LANDING-FINAL-202611041246Z` was explicitly accepted at
12:46:57.9 UTC on 2026-11-04, on source HEAD
`46143c6f2ecdf0ecdeddf24e64f13026cb688518`. Native jobs used absolute Node
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`, read-only
retained dependencies, one private serial Chrome/Vite workflow and the owned
`/tmp/mcr-css-parity-landing-round-02` root. Accepted free space was
17,603,637,248 bytes; the supervisor continuously enforced the 8GiB floor,
214,748,364-byte total and 128,849,018-byte temporary caps. Entire durable
evidence was about 8.75MB, under 15MiB, with 15 new PNGs (cap 16).

All validation finished before the 12:53:57.9 UTC active stop. Final plan check
started 12:53:31 UTC and exited 0; `git diff --check` also exited 0. The plan
checker covers planning provenance only. Native commands used
`MCR_LANDING_ACCEPT_EPOCH=1793796417.9 MCR_LANDING_ROUND=round-02` with
`round-02/supervise.py`, phase baseline/candidate, and iteration final for the
last native run; the comparison script used `--final` for the last assessment.
Final temporary cleanup completed at 12:56:03.607 UTC, **late** against the
12:54:57.9 UTC hard cleanup deadline because context compaction delayed removal.
Browsers/server had already closed in the driver finally block. The owned link
and temporary root are removed, port 5291 is free, and FULLRELEASE including the
late-cleanup exception was sent to resource21d. Last browser/server closure is
bounded above by the successful candidate log mtime 12:51:45.775270 UTC; last
child exit by 12:53:31.859333 UTC. Exact close/exit instants were not recorded.
The owned link/TMP removal completed at 12:56:03.607082 UTC. The timestamped
FULLRELEASE confirmation is in `cleanup.json`; the original message's exact
delivery instant was not locally recorded. This is functional/native PASS and
**HARD CLEANUP DEADLINE MISS**, never lease-compliance PASS.

The owned supervisor now authors a detached per-job deadline custodian that
survives completed tool calls/context compaction, stops only the recorded child
group with matching process birth, and removes only the recorded owned link/TMP
inodes before the hard deadline. The supervising parent reaps its child and
records exact future stop timestamps. This source follow-up is **NOT_RUN** after
release; it does not retroactively fix the missed lease. No future runtime job
may start without a fresh allocation and activation of this custody mechanism.
No further output job was run.

Residual branch: `codex/mcr-css-parity-landing-residual`, synchronized base
`a5c6cad52e630d61aa4ce581e7230c13cc5feb25`. Historical readback receipt is
`cfa2c9641229d5f982350bf7027cb4e63e45bd0c` (equivalent to `ee437727`, published
by Root as `f33c9f14`). Candidate source commits are `e9f7aa1379e11220888375c1b135e062b1707466`
and `46143c6f2ecdf0ecdeddf24e64f13026cb688518`; final handoff adds the validated
border iteration, harness, evidence and this document. The cumulative residual
postimages must be delivered, not the final border-only diff in isolation.
Root153 remains sole publisher. Fresh publication ancestry/postimage readback
is pending; no king/SL technical inspection or acknowledgment is required.
The persistent residual goal remains unfinished until that delivery duty is met;
its prior resource-blocked metadata has no manual resume operation.

## Historical first-pass scope and provenance

- Owner chat: `01a1a5d8-ebac-7cb0-b770-e6dc5ea07cb7`.
- Exclusive worktree: `/Users/lhs/.codex/worktrees/fb96/mcr`.
- Branch: `codex/mcr-css-parity-landing`.
- Fetched, clean fast-forward base: `29a42ca1f5d86262ee18141d2bab75996af7e6fd`.
- Root153 explicitly granted `frontend/src/features/public/ui/public.css`, this
  document, `docs/mcr/evidence/css-parity-landing/**`, and the optional new
  `frontend/src/features/public/ui/__tests__/css-parity-landing/**` harness.
- Product change: **only** `frontend/src/features/public/ui/public.css`.
- New reference: supplied `1-Photo-1.jpg`, SHA256
  `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
  The exact original is preserved as `reference-original.jpg` in the evidence
  directory. Analysis crop: original coordinates `(5,3)` through `(254,440)`.
- Tested CSS SHA256:
  `941242817f37b1426e71c68696254ce630cd81da94b6196779961db2a346af96`.

No source logic, UI, or asset was copied from another project. Existing React,
Router, Vite and Playwright are retained. No dependency, contract, source archive,
background paint, shared stylesheet, selected art, TSX, backend or World changed.

## Historical first-pass result and comparison

The initial actual 390px capture showed the lower printed copy slightly too wide
and tall, and the CTA too wide to the right and too low. The patch narrows and
shortens the copy box, moves it slightly upward and left, and moves/narrows the
real CTA to the reference bounds. Its accessible name, sign-in destination,
disabled state and minimum target size are unchanged.

Visual inspection covered the baseline/candidate equal-size comparisons, candidate
390px focused capture, and native 360/430px poster captures. The wordmark and
selected gallery were retained. At 390px, all four gallery widths, heights,
positions relative to the poster, atlas positions, background size, transforms
and pixel rendering match the baseline; the computed background paint and hidden
legacy scene payload also match.

Coarse cream-pixel bounds in the fixed copy/CTA regions provide supporting
measurement: summed edge-coordinate error decreases from 10 to 0 pixels for the
copy and from 13 to 1 for the CTA. This threshold measurement uses a resized JPEG
reference; it is **not** exact pixel equality or an art-acceptance score. See
`foreground-bounds.json`, `measure.py`, and the two comparison PNGs.

## Historical first-pass owner validation

Resource round: `CSS-PARITY-LANDING-202611040922Z`, accepted 09:22:51 UTC,
fully released 09:27:27 UTC on 2026-11-04. One private Chrome/Vite workflow;
13 PNGs and approximately 4 MiB durable evidence. Private server/browser, port
5291, temporary files and the absent-only dependency link were released.

| Command / check | Exit / result |
|---|---|
| Absolute Node 24.22.2, `MCR_LANDING_PHASE=baseline …/css-parity-landing/native.mjs` under `supervise.py` | 0; baseline 390px PASS |
| Same command, `MCR_LANDING_PHASE=candidate` | 0; 360/390/430px PASS |
| Skip link, visible CTA focus, sign-in return target, About disclosure, Safety navigation/focus, home return, 420px height, disabled entry and retry | PASS at all three widths |
| Selected atlas 384×64, four characters, unchanged 390px gallery and background, no canvas/world host, no horizontal overflow | PASS |
| `python3 scripts/mcr/check-plan.py` | 0; planning scope only |
| `git diff --check` | 0 |
| `python3 docs/mcr/evidence/css-parity-landing/measure.py` | 0; coarse bounds only |

The harness uses actual production Landing/PublicFrame/routes and the existing
explicit MemorySession/readiness reference fixture. Baseline CSS is read from
the pinned Git blob; candidate uses the actual edited file. It never substitutes
the app, background or character assets. Raw native attempt logs and JSON records
are retained. There were no failed attempts.

## Limits and direct Root153 requests

1. The supplied photo has different character appearances, outlines and crowd
   spacing from the mandated selected-static gallery. The exclusive lease
   explicitly preserves that gallery's source, UV, frame, crop and rendering;
   changing it is outside this CSS task. Preserve the selected artwork and keep
   exact character parity open unless its owning implementation changes.
2. The photo's cemetery, web and rose differ from the current cute background.
   Background paint and assets belong to their existing owners and are excluded
   here. This change preserves them, as directed.
3. The header's real sign-in arrow does not depict the photo's decorative
   minimize/close glyphs. Changing semantic controls or SVG contour source requires
   its implementation owner; CSS must not create counterfeit actions. No extra
   control or asset replacement is requested by this patch.
4. Real provider/backend journeys, actual iOS/Android, whole-app build and release
   acceptance are **NOT_RUN**. The CSS was parsed/rendered by Vite in the native
   fixture; no TypeScript or dependency changed. Runtime provider claims remain
   outside this result.

Direct committed handoff and the exact remaining differences go to frontend
publisher Root153 (`01a1a153-1227-7e72-801f-a2a6b91981e1`). Root publishes to
`julymeltdown/mcr/master`; there is no king/SL review or acknowledgment gate.
Worker commit: `2c8d8476b9acd14f1b8d1d06ab8ac0c25a0fe254`.
Published commit: `baad4944f4cf3e6ee3156436d8044c543a780d44`.
Fresh owner fetch/readback on 2026-11-04 at 09:31 UTC verified that the published
commit is in `origin/master`, its exact 28-path set equals the worker handoff,
and all 28 postimages equal both the worker and current remote master byte for
byte. The published CSS hash equals the tested CSS hash above. See
`publication-readback.json`. No peer file or unvalidated product edit was added.
