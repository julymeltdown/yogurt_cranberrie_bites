# CSS-PARITY-LANDING — foreground spacing

Status: **MOCK_READY**, bounded CSS correction validated. Exact whole-panel visual
acceptance remains open. The persistent goal stays ACTIVE through publication
readback and resolution of the non-CSS differences below.

## Scope and provenance

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

## Result and comparison

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

## Owner validation

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
The handoff message records the actual commit. Remote ancestry is recorded only
after publication, rather than treating the local commit as delivered master.
