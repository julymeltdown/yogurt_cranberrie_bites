# Continuous diagonal source composition v10

Status: **CONTRACT_READY virtual-art candidate; overall fidelity IN_PROGRESS.**

This corrects the actual southwest-road hierarchy, rather than adding another
paint treatment to the old vertical detour. It is paired exclusively with
`../spatial-v5/`, normalized geometry
`sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`.
The immutable source-art execution base is
`4af00fdb5048bb080e49dcfbddcc2cee61b45478`.

## Ground and geometry

The new diagonal and lower west connector change560 terrain cells. All collision
cells,34 WORLD props/footprints, original identities, spawns, interaction cells,
cameras and eight published v9 MAP representations remain unchanged. The original
12 routes remain reachable. Old source and release inputs are not modified.

`material-ground.tsrct` contains121 independently editable original-material image
layers clipped to the new paving union. It reuses exact grass/cobble quadrants of
`../quality-v6/sources/surface-sheet.png` at the original128px tile scale/origin.
The native-only2048px source padding preserves all original1254px source pixels
and alpha. No reference, character or legacy image pixels enter the ground. The
old decorative elliptical material beds are removed.

`ground.tsrct` adds two editable vector edge layers over that material render.
There are108 union-boundary segments, excluding crop edges and overlap seams.
The boundary verifier independently rasterizes the new paving union and confines
all47,704 curb-paint changes to its7px edge neighborhood. This comparison is
against the new material ground; it does not imply unchanged old terrain.

Both native ground exports have uniformly255 alpha. Runtime PNGs remove only
that redundant alpha and compress losslessly; every RGB pixel equals the actual
native output. Runtime `ground.png` is768×1296 RGB,549,155 bytes, SHA-256
`dd70d8fb02e1c6e57e9a9f9e96d69dc7c15999bc076678a7075161281c929887`.
Original v8 era atlases and native transparent padding are byte-identical.

| Era | Combined runtime PNG bytes | Pixels | PNG textures |
| --- | ---: | ---: | ---: |
| REVENGE | 1,752,996 | 2,043,904 | 2 |
| PARADE | 1,764,183 | 2,043,904 | 2 |
| DANGER | 1,750,488 | 2,043,904 | 2 |

All remain below the existing2,097,152 byte/pixel limits. C06 objects and atlas
descriptors remain exact v8; only the explicit new ground descriptor changes.

## MAP presentation

`map-label-presentation.json` is an **explicit new geometry binding**, not an
implicit reuse of the0448-bound v1 sidecar. It pins the new geometry/projection,
exact v1 drawing lineage and font. All dimensions, six label names/anchors/bindings,
icon polygons, font/style values and drawing instructions remain equal to v1.
Four unbound labels stay decorative. No active pin or participant is fabricated.
The new binding needs fresh consumer validation; old review/release status does
not transfer. The inherited v8 manifest is atlas lineage, not the new geometry.

Each era has native768×1296 WORLD and MAP projects/exports. Each native project
packages exactly two PNGs; MAP also packages the one exact OFL font and eight
editable text lines. All vector labels stay editable. WORLD has no labels/font.

## Actual verification

`verify-diagonal-spatial.py` passes all3,888 terrain-center and SAT collision
checks,560 exact changes, immutable34-prop/identity/cell/camera fields and12 BFS
routes. Four approximate manual reference-road anchors are within16px of the
new centerline; that is a narrow annotation check, not a whole-map score.

`verify-diagonal-composition.py` passes the current closed C06 schema, all manifest
and new label-source pins, both budgets, exact atlas/padding bytes, native RGB
parity, saved ZIP compositions/assets/fonts, union-edge paint bounds and actual
rendered source-color probes. All6,582 lawn and5,686 paving samples match their
original material quadrant within25 per channel. Lower-venue probes pass in all
six exports; MAP label rectangles are excluded from those visible-source samples,
with the original90percent/40-per-channel threshold retained. Northern facade
probes remain visible. No source pixels or alpha are modified to pass a probe.

`check-quality.mjs` with revision10 passes36 actual Chrome156 source-browser cases,
216 clear feet at360/390/430px over two era cycles, exact expected geometry hash,
one canvas, loaded font, label text fit and no errors/overflow. Eighteen comparison
captures are saved. All six native exports and PARADE WORLD/MAP comparisons at
all three widths were visually inspected. Native viewer-open returned queued;
actual CLI exports provide render evidence. Frozen v8/v9 and plan/archive checks
pass. No package or tool was installed/upgraded.

Focused technical peer review found two verifier false-pass gaps; the current
assets themselves were correct. The verifier now hashes the ground project's
embedded PNG against the new material ground and enforces every theme's geometry
binding against a recomputed canonical digest. `verify-diagonal-negative.py`
rejects a substituted legacy ground bitmap and all12 theme documents changed
to the old hash, even with refreshed outer manifest pins. Both mutations stay
in memory and perform zero artifact writes; results are in
`negative-verification.json`.

## Adoption and remaining work

The designated frontend/art integrator publishes the committed source handoff;
artist does not push master. This source publication is separate from adoption.
Current0448/map5 work continues independently. New ef52 terrain requires explicit
canonical Tiled/server/client binding at a revision greater than allocated5;
proposed6 is not allocated or reserved by art. Never reseal old geometry/releases.

Use the matching ground, geometry/projection, resource descriptors, eight MAP
representations and new label presentation together. Source-art qualification,
resource admission, actual Phaser/application composition, device/multiuser flows
and final target acceptance remain separate obligations. No fourth critic panel
or new8.5 score is claimed; earlier WORLD6.5/MAP6.0 scores remain below the target.
The artist goal stays active for full fidelity and concrete consumer findings.
This patch adds no physical venue qualification and changes no runtime/backend/
application/shared/character/archive path.

```sh
python3 frontend/public/world/reference-scene-v2/build-diagonal-spatial.py
python3 frontend/public/world/reference-scene-v2/build-diagonal-ground.py
python3 frontend/public/world/reference-scene-v2/build-route-edges.py --geometry-revision 5 --quality-revision 10 --base-ground quality-v10/material-ground.png
python3 frontend/public/world/reference-scene-v2/build-route-composition.py --revision 10
python3 frontend/public/world/reference-scene-v2/editable-quality.py parade --revision 10
python3 frontend/public/world/reference-scene-v2/editable-quality.py revenge --revision 10
python3 frontend/public/world/reference-scene-v2/editable-quality.py danger --revision 10
python3 frontend/public/world/reference-scene-v2/verify-diagonal-spatial.py
python3 frontend/public/world/reference-scene-v2/verify-diagonal-composition.py
python3 frontend/public/world/reference-scene-v2/verify-diagonal-negative.py
# Serve the repository on localhost8896 and use an existing installed package:
MCR_ART_REVISION=10 MCR_PLAYWRIGHT_PACKAGE=/absolute/path/to/installed/frontend/package.json node frontend/public/world/reference-scene-v2/check-quality.mjs
```
