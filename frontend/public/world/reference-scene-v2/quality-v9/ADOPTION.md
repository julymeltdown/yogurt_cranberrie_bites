# Route-edge and north-facade composition v9

Status: **CONTRACT_READY virtual-art input; overall fidelity IN_PROGRESS.**

This addresses two actual source-comparison findings: the paving union had no
defined edges, and the v8 northern canopy concealed both small northern MAP
facades. All previous source/release inputs remain frozen. The exact normalized
spatial-v4 geometry remains
`sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`.

`manifest.json` is 6,621 bytes, SHA-256
`8a2bbe62a31bce98e1bcab91bdde53c3ce0b6fd21943dae8d2da0e8003952d15`.
It closes geometry/projection, frozen v8 lineage, editable ground/vector sources,
all three era source/resource descriptions and the exact published label sidecar.

## Ground and shared world

The ground uses the original v6 materials, with two editable native shape layers
painting the actual union edge of its 47 paving polygons. Exact edge intersection
splitting and left/right union classification produces 119 boundary segments;
overlap seams and scene crop edges are excluded. These are traversable painted
edges, not new solid curbs, walls or physical route guidance. Collision, terrain,
paving polygons, 34 WORLD props/footprints, cameras, spawns, interaction cells,
place identities and original review states remain unchanged.

`ground.tsrct`, `ground-editable.json`, `ground-layout.svg`, `route-edges.json`
and actual `ground-native.png` retain editable geometry and native render evidence.
The native RGBA export has uniformly255 alpha. Runtime `ground.png` removes only
that redundant channel and compresses PNG losslessly: **every RGB pixel equals
the actual native export**, without resampling, palette quantization or alpha
resealing. `verify-route-composition.py` independently rasterizes the original
paving union and checks that all49,996 changed pixels fall within its7px boundary
neighborhood. Ground pixels away from that neighborhood remain exact v6.

Runtime ground is768×1296 RGB, **558,464 bytes**, SHA-256
`20cecda630d633f20122607d97b86fbd3f7c6c0f06483ca4d838578a9610915a`.
The three original v8 atlas files and native transparent padding remain
byte-identical; C06 scene objects are exact v8 apart from the ground descriptor.

| Era | Combined runtime PNG bytes | Pixels | PNG textures |
| --- | ---: | ---: | ---: |
| REVENGE | 1,762,305 | 2,043,904 | 2 |
| PARADE | 1,773,492 | 2,043,904 | 2 |
| DANGER | 1,759,797 | 2,043,904 | 2 |

All stay within the existing2,097,152 byte/pixel budgets. The new ground descriptor
is explicit in each v9 `scenery.json` and `resource-inputs.json`. Atlas descriptors
retain their original v8 paths and hashes; identical local copies support source
preview/editing. No third PNG texture, new package, source raster generation or
character artwork is introduced.

## MAP composition

Two new **MAP-only replacement illustrations** replace the existing unbound
`northern_hall` and `north_eastern_hall` images in the overview. They reuse the
original `north_hall` atlas frame, at(-12,6) and(542,6), size210×144, depth150.
Their `replaces_prop_id` is explicit; both interaction bindings remain null. They
do not add WORLD objects, invisible solids, places or destinations. The original
six overview representations remain exact; the v9 overview now has eight.
WORLD retains the original two physical hall placements and all34 props.

Six actual native scene exports contain two PNGs each. MAP also packages the
one exact OFL font from `../map-labels-v1/`, with the published editable icon/text
overlay; WORLD has no label overlay or font. The eight text lines and all original
icon shapes remain independently editable, not baked into the ground. All six
original label anchors/names/bindings stay exact; four labels remain decorative.
No selected pin, participant or physical meeting eligibility is fabricated.

Actual source-color probes make the facade correction observable: across all
three MAP exports,1,483 near-opaque source-color samples match at the new facade
regions, versus280 matches in the same v8 regions. The thresholds and per-era
counts are in `verification.json`. These are direct rendered-source checks,
**not target-match percentages or independent visual scores**.

## Validation, adoption and limits

`verify-route-composition.py` passes closed current C06 validation, exact source
pins and original WORLD descriptors, both budgets, unchanged source atlas bytes,
ground native/RGB parity, independent paving-edge change bounds, exact saved
native ZIP composition and two texture hashes, MAP font/text membership and
actual facade visibility. `verify-density.py` rechecks all3,888 canonical cells,
72 existing v4 additions and12 routes. Frozen v8 verification also passes.

`check-quality.mjs` with `MCR_ART_REVISION=9` passes36 actual Chrome source cases
and216 clear-foot samples at360/390/430px through two complete era cycles. MAP
font/advance checks pass; no page errors, overflow or duplicate canvas. Eighteen
target/candidate captures are saved. All six actual native exports and the PARADE
WORLD360/MAP390/WORLD430 comparisons were visually inspected. The native viewer
open returned queued; actual CLI exports supply render evidence. Plan/archive
and changed-code whitespace checks pass. No build tool or dependency was upgraded.

This remains source/native/Canvas2D evidence. No fourth scored panel,8.5 pass,
current application/Phaser, source review, backend admission, physical, device or
multiuser acceptance is claimed. Painted edges do not prove that the complete
geometric route hierarchy matches the target. Full fidelity and concrete consumer
composition findings remain in the active artist goal.

Root retains its actual spatial-v4 adoption scope; artist writes no reserved
`runtime/**` or application/backend path. V9 adoption needs an explicit new
resource/overview consumer patch and its own application checks. It does **not**
require a different canonical geometry or another server map revision beyond
the actual backend-allocated revision for0448. Existing map4/geometry899/v6 and
all v8 source/consumer pins remain immutable. No old manifest is silently repinned,
and no source qualification packet for another revision is reused as a v9 review.

```sh
python3 frontend/public/world/reference-scene-v2/build-route-edges.py
python3 frontend/public/world/reference-scene-v2/build-route-composition.py
python3 frontend/public/world/reference-scene-v2/editable-quality.py parade --revision 9
python3 frontend/public/world/reference-scene-v2/editable-quality.py revenge --revision 9
python3 frontend/public/world/reference-scene-v2/editable-quality.py danger --revision 9
python3 frontend/public/world/reference-scene-v2/verify-route-composition.py
# Serve the repository on localhost8896, select an existing Playwright package:
MCR_ART_REVISION=9 MCR_PLAYWRIGHT_PACKAGE=/absolute/path/to/installed/frontend/package.json node frontend/public/world/reference-scene-v2/check-quality.mjs
```
