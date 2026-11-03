# FE-REF-SCENE: original WORLD/MAP scenery proposal

Status: **IN_PROGRESS**. This is a reviewable PARADE illustration and geometry
proposal, not reference acceptance, a current asset release, backend spatial
authority, a physical venue map, or a multiuser acceptance result.

## Source and scope

Worker base: `1e5022885525ab56f056aa2c24100e18c4bc67ed` on isolated branch
`codex/mcr-art-reference-scene`. Exact writable scope is
`frontend/public/world/reference-scene-v2/**` and this document. No application
renderer, backend, release registry, schema, character, old scenery, root or
lockfile is changed. Supplied originals and `docs/source/` remain unchanged.

Binding visual reference: `docs/mcr/design/reference-target-2026-11-04.png`,
SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
The review page clips its unmodified third WORLD and fourth MAP panels above
actual candidate renders. The reference is never a runtime scenery texture.
The previous illustrated scenery was inspected but no pixels were reused.

## Deliverables

All following paths are relative to `frontend/public/world/reference-scene-v2/`.

| Files | Meaning |
| --- | --- |
| `painted/parade-ground.png` | Original illustrated ground, 768 × 1296; exported from editable paving/material geometry. |
| `painted/parade-source-atlas.png` | Unmodified original generated RGBA atlas, 1254 × 1254; sixteen character-free scenery frames. |
| `painted/surface-source.png` | Unmodified original generated four-material source sheet, 1254 × 1254. |
| `painted/ground-layout.svg`, `painted/scene-layout.svg`, `painted/layout.json` | Editable ground polygons/patterns and individually named scenery placements; source-frame rectangles, scale, rooted foot depths and collision footprints. |
| `painted/provenance.json`, `painted/resource-inputs.json` | Source authorship, source and derivative hashes, dimensions, bytes, canonical geometry pin and adoption limitations. |
| `geometry-proposal.json` | Version 2 shared virtual geometry, 48 × 81 cells of 16px, exact collision grid, terrain, two spawns, four existing place/interaction pairs, cameras and six runtime map labels. |
| `*-ground/scenery.tsrct`, `*-editable.json`, `*.svg`, `*-frames.json` | Supplemental native editable polygon drafts for PARADE, REVENGE and DANGER. The generated painted increment is PARADE only. |
| `resource-inputs.json`, `verification.json`, `browser-verification.json` | Native-source hashes, geometry/routes validation and actual browser asset-review evidence. |
| `review.html`, `review.mjs`, `evidence/*` | Technical comparison tool and actual captures. This is not an implemented product screen. |

Generated raster contents are not native vector layers. Layout geometry remains
editable in SVG/JSON and the native draft sources remain available. The built-in
`image_gen` tool created original scenery and material sheets. Source PNG bytes
are preserved. Browser canvas reconstructs their non-destructively sampled
frames and clipped material patterns; no reference or character pixels are
extracted, edited, or regenerated. Exact original source hashes are pinned in
`painted/provenance.json`. Generated artwork contains no humans or avatars.
No external stock, font or photo asset was embedded. New bitmap lettering in
native drafts is original; browser system type is used only for review labels.

Sixteen painted frames include stage, two white booths, CULTURE PARK sign, four
distinct tree variants, two destination pavilions, two substantial park halls,
food counter, seating, banner and a low planting bed. Eleven fixed rooted tree
placements deliberately reuse four painted variants at named parcels. Native
drafts instead retain eleven individually generated polygon canopies. No random
tree stamping or duplicate tree population is performed at runtime.

PARADE/REVENGE native drafts share a base palette; DANGER changes selected stage
and counter accents. They are structural drafts, **not** three qualified painted
skins. The default comparison shows only painted PARADE; other skin options are
disabled until native draft mode is selected.

## Geometry and rendering contract

All coordinates are virtual pixels, origin top-left. WORLD is a close crop of
the same geometry shown in the portrait MAP; no second inconsistent coordinate
system or backend IDs are created. Only `stage_info`, `park_info`, `gifts_info`
and `wall_info`, with their existing place UUIDs, are referenced. FOOD, MEETUP
ZONE, GIVEAWAY BOOTH and unbound props have null interaction identities. Their
labels convey illustration, not available backend interaction or physical facts.

Draw the ground once and draw atlas props in ascending `depth_y`; actor sorting
uses feet Y. Canopy overhang is alpha art, not a full-rectangle wall. Collision
uses rooted convex footprints and exact footprint/cell intersection including
touching; the perimeter is blocked. Grass is walkable virtual terrain. All
spawn and interaction cells are open and reachable from each spawn. No current
MapManifest, server collision release or schema is silently rebound. Integration
requires a separately coordinated server/client geometry and resource release.

Map destination labels are runtime text. They are absent from the exported
ground and complete scene PNG. Stage MCR lettering and CULTURE PARK signage
belong to their scenery sprites. The technical review loads two PNG textures
per selected finish/skin and caches decoded image sets across view toggles.

The returned generated dimensions differ from requested dimensions. Actual
1254-square sources are measured and pinned; no assumed 2048/1024 grid is used.
Frame bounds are measured using source alpha, without rewriting source pixels.
RGB fringe values at alpha 1–3 are retained; ordinary alpha composition is the
review basis, not a viewer displaying nearly transparent RGB as opaque.

## Reproduction and evidence

Use the already installed tools; no dependency installation or copied
`node_modules` is required. Set `MCR_PLAYWRIGHT_PACKAGE` to an existing frontend
`package.json` whose dependencies include Playwright, and optionally
`MCR_BROWSER_EXECUTABLE` / `MCR_TSRCT` to existing executables.

```sh
python3 frontend/public/world/reference-scene-v2/build.py
python3 frontend/public/world/reference-scene-v2/render.py
python3 frontend/public/world/reference-scene-v2/build-painted.py
node frontend/public/world/reference-scene-v2/render-painted.mjs
python3 frontend/public/world/reference-scene-v2/build-painted.py
python3 frontend/public/world/reference-scene-v2/verify.py
python3 -m http.server 8896 --bind 127.0.0.1
node frontend/public/world/reference-scene-v2/check-review.mjs
```

The last command expects the scoped technical review at
`http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/review.html`.
The Python server is a local review aid, not application deployment.

Recorded successful commands, all exit 0:

- `build.py`: source generation and canonical geometry hash.
- `render.py`: all six native commits and PNG exports with existing
  Tesseract CLI `0.4.0 (d7ccb4bc20fc69f2356c53b80336e52cff31dcc5)`.
  That version matched the initial skill pin; the installed skill directory
  advanced during this run. No tool installation/update or telemetry enablement
  was performed. Browser export uses Chrome `156.0.8078.80`.
- `build-painted.py` and `render-painted.mjs`: source-preserving frame/layout
  export and actual 768 × 1296 ground decode/render.
- `verify.py`: six native PNG dimensions; six native shape compositions and
  in-bounds vertices; atlas bounds/nonoverlap; canonical hash and known IDs;
  conservative collision coverage and 12 spawn-to-point routes.
- `check-review.mjs`: **24** asset-review cases, **144** clear-feet samples,
  one canvas per review page, no horizontal overflow at 360/390/430px,
  all resource decoding and no page errors. Painted PARADE and three native
  drafts are checked separately. Evidence names include the finish.

`evidence/world-360.png`, `world-390.png`, `world-430.png` and matching MAP
captures show the actual painted comparison. These checks are Canvas2D asset
evidence, **not** current Phaser/application/server or dynamic-avatar acceptance.
Alpha samples certify clear feet at the six named points only; they do not
certify complete avatar-body occlusion while moving.

## Visual review and remaining work

First independent review of the native draft: art director WORLD 5.5/10,
MAP 4.5/10; spatial-art specialist WORLD 6.0/10, MAP 4.8/10. It did not pass.
The draft was checkpointed in `.tesseract-work/Versions/round-1/` and substantially
revised with original painted materials/assets, connected parcels and larger
architecture. Second independent painted review: art director WORLD 6.0/10, MAP 5.3/10;
spatial-art specialist WORLD 6.2/10, MAP 5.5/10. Panel score 5.3/10; it did
not pass the 8.5 threshold. No score is treated as reference acceptance.

The frontend lead inspected the improved painted comparison and identified
remaining work: Plaza must read as a broad green parcel directly below Main
Stage and above the Merch/Food crossing; planting should be denser and greener,
paving lighter/finer, and the MAP stage hall hierarchy needs refinement.
WORLD must retain the target's black concert stage and white booths. The current
checkpoint intentionally records these unresolved visual differences.

Current product integration, coordinated server/client release binding, physical
venue/operator verification, device checks and multiuser acceptance are
**NOT_RUN** here. No physical arrival, safe route, real capacity, event operation
or operator qualification follows from this virtual scene.

Committed checkpoint ancestry is recorded in Git. The persistent FE-REF-SCENE
goal remains active through concrete visual fixes, handoff and publication
verification; an art checkpoint is not task acceptance.
