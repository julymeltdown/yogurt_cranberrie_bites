# FE-REF-SCENE: original WORLD/MAP scenery proposal

Status: **IN_PROGRESS** visual fidelity. Original painted REVENGE, PARADE and DANGER v5 inputs are now **CONTRACT_READY** with shared geometry and six native editable projects. This is not reference acceptance, a current asset release, backend spatial authority, a physical venue map or a multiuser acceptance result. See the latest v5 section below; earlier sections document the frozen v3/v4 checkpoints.

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

## Frozen v3 deliverables

All following paths are relative to `frontend/public/world/reference-scene-v2/`.

| Files | Meaning |
| --- | --- |
| `painted/parade-ground.png` | Original illustrated ground, 768 × 1296; exported from editable paving/material geometry. |
| `painted/parade-source-atlas-v3.png` | Current original generated RGBA atlas, 1254 × 1254; sixteen character-free frames with stronger pixel clusters and outlines. |
| `painted/surface-source-v3.png` | Current original four-material sheet, 1254 × 1254. Earlier source sheets remain unchanged siblings. |
| `painted/ground-layout.svg`, `painted/scene-layout.svg`, `painted/map-layout.svg`, `painted/layout.json` | Editable ground and named WORLD props / MAP representations; source sampling, scale, foot depths and collision footprints. |
| `painted/parade-world.tsrct`, `painted/parade-map.tsrct`, matching `*-editable.json` / `*-native.png` | Two actual editable image-layer projects and 768 × 1296 PNG renders; individually movable masked layers. |
| `painted/provenance.json`, `painted/resource-inputs.json` | Source authorship, source and derivative hashes, dimensions, bytes, canonical geometry pin and adoption limitations. |
| `geometry-proposal.json` | Version 2 shared virtual geometry, 48 × 81 cells of 16px, exact collision grid, terrain, two spawns, four existing place/interaction pairs, cameras and six runtime map labels. |
| `{skin}-{ground,scenery}.tsrct`, matching JSON / SVG / frame metadata | Supplemental native polygon drafts for PARADE, REVENGE and DANGER. Painted delivery is PARADE only. |
| `painted/prompts.md`, `painted/native-evidence.json`, `painted/verification.json` | Exact final built-in generation prompts, native command exits, source / mask / placement checks. |
| `resource-inputs.json`, `verification.json`, `browser-verification.json` | Native-source hashes, geometry/routes validation and actual browser asset-review evidence. |
| `review.html`, `review.mjs`, `evidence/*` | Technical comparison tool and actual captures. This is not an implemented product screen. |

Generated raster contents are not native vector layers. Layout geometry remains
editable in SVG/JSON and the native draft sources remain available. The built-in
`image_gen` tool created original scenery and material sheets. Source PNG bytes
are preserved. Browser canvas reconstructs their non-destructively sampled
frames and clipped material patterns; no reference or character pixels are
extracted, edited, or regenerated. Exact original source hashes are pinned in
`painted/provenance.json`. Generated artwork contains no humans or avatars.
No external stock, font or photo asset was embedded. The final style edits use
our own source sheets and the supplied reference for scenery style only; no
supplied character is sampled or changed. Exact final prompts and input roles
are in `painted/prompts.md`. New bitmap lettering in
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

All coordinates are virtual pixels, origin top-left. WORLD and portrait MAP use
one canonical geometry, with explicit overview representations. No second
coordinate system or backend IDs are created. Only `stage_info`, `park_info`, `gifts_info`
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

The stable geometry hash is
`sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5`.
There are 22 physical props and six overview illustrations. MAP replaces the
black stage sprite with a representative venue hall and overlays a green Plaza
gathering zone. WORLD retains the black stage and the walkable paved court.
Overview hall, seating and cropped edge architecture have no collision footprint
and do not create invisible WORLD solids. This distinction follows the supplied
reference's different WORLD and MAP depictions. The green overlay denotes a
virtual gathering zone; it does not alter canonical terrain, imply actual lawn,
or establish physical routing. MAP labels remain functional text metadata.

Plaza now sits directly below Main Stage and above the Merch/Food crossing.
`gifts_info` is associated with the lower Merch venue; the close-view white
booth is unbound scenery. WALL's proposal interaction was moved to a clear lane
after a browser alpha check caught occlusion by the relocated lower tree. The
final collision, routes and all six clear-foot samples pass. Native WORLD/MAP
projects package the same two source images; their layers remain independently
editable with explicit crop masks. Native MAP uses solid green for the semantic
zone; browser/SVG MAP uses a lawn pattern. Their physical geometry is identical.

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
python3 frontend/public/world/reference-scene-v2/editable-painted.py
python3 frontend/public/world/reference-scene-v2/verify-painted.py
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
- `editable-painted.py`: two packaged image-layer compositions committed and
  rendered with Tesseract, exit 0. Initial sourceRect export showed neighboring
  atlas pixels; explicit rectangular masks and full-source transforms fix it.
  The broken trial PNGs were replaced by fresh verified renders. Content-hash
  asset IDs prevent stale source imports.
- `verify-painted.py`: four source sheets preserved by hash, current resource
  hashes/dimensions, sixteen source frames, unchanged rooted geometry, unique
  native layer/mask IDs, two packaged image sources and both native renders.
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

Third/final panel: art director WORLD **6.5/10**, MAP **6.0/10**; spatial-art
specialist WORLD **6.6/10**, MAP **6.4/10**. Per-design panel scores are WORLD
6.5 and MAP 6.0. Set panel scores progressed **4.5 → 5.3 → 6.0**. This version
has the highest score and is retained after the skill's three-round limit;
it **did not reach 8.5/10** and does not establish reference acceptance.

Revisions added broader pixel clusters, block plaque lettering, a taller
diagonal stage, finer pale cobbles, rooted canopy proportions, meaningful
Merch/Food ordering, a broad semantic Plaza and a distinct MAP stage hall.
Remaining critic findings are denser perimeter clustering, broader venue-style
architecture rather than detached cottages, softer environmental texture,
stronger diagonal route hierarchy and icon-led runtime destination labels.
No further rendered change is made after the final scored version. Original
characters were intentionally absent and not penalized. Critics did not evaluate
application behavior, server bindings, moving-avatar occlusion, other skins,
source integrity or editability; developer checks cover only the stated bounds.

Current product integration, coordinated server/client release binding, physical
venue/operator verification, device checks and multiuser acceptance are
**NOT_RUN** here. No physical arrival, safe route, real capacity, event operation
or operator qualification follows from this virtual scene.

First published checkpoint: `f4619879513c7a9e403bae25722b2390f527a69b`,
rebased onto `1b3ad56e` and published non-force to master with remote HEAD
verified. The next source checkpoint preserves it in ancestry. Only the exact
assigned paths are authored here; peer history is reconciled before publication.

The persistent FE-REF-SCENE
goal remains active through concrete visual fixes, handoff and publication
verification; an art checkpoint is not task acceptance.

## Runtime derivative v4: bounded C06 adoption

A new versioned sibling `runtime-v4/` resolves two concrete implementation findings without changing the frozen source checkpoint `8f89d1905a86143d34ff77d2951fc60651ab2497`: combined source texture size exceeded the existing C06 budgets, and fractional authored depths did not satisfy closed C06 integer fields. All prior source, geometry and editable project bytes remain unchanged.

The exact `runtime-v4/parade-scenery.json` uses the existing Scenery v1 schema and ordinary two-texture loader. Combined resources are **2,043,904 pixels / 1,947,817 bytes**, below both existing 2,097,152 limits. Ground remains 768x1296 with pixel-identical RGB; the 1024x1024 atlas is an explicit 0.75 nearest-neighbor source derivative with no palette quantization. One banner crop excludes neighboring source-cell furniture. Hashes, transformations and consumer guidance are in `runtime-v4/ADOPTION.md` and `parade-resource-inputs.json`.

Artist-owned integer projection is explicit for 22 WORLD props and six MAP-only representations: nearest integer, ties away from zero; rounded absolute depth minus rounded local foot anchor determines Y. Actual maximum absolute depth/Y error is 0.49 virtual px, local foot-anchor error 0.30px. Stable depth order and all original footprint polygons, collision, terrain, IDs, spawns, interaction cells and geometry hash are unchanged. MAP's semantic zone uses the authorized native solid #7a8742 with #b4a27d/6px outline; an optional pattern can sample the already loaded ground. No third source texture is needed.

`pack-runtime.py` and `verify-runtime.py` exited 0. Closed current C06 schema validation, source/PNG hashes, exact nearest-neighbor crops, ground pixel equality, frame bounds/nonoverlap, unchanged footprint metadata, stable object order and explicit projection all pass. `check-runtime.mjs` exited 0: six WORLD/MAP cases at 360/390/430px, 72 clear-foot samples across source and derivative, no page errors/overflow. Actual 390px WORLD and MAP comparisons were visually inspected. These are asset checks; application/Phaser/server, moving-avatar and multiuser acceptance remain separate.

This derivative is CONTRACT_READY art input, still PROPOSAL_NOT_RELEASE_BOUND. The third/final scored panel applies to frozen v3; the packing comparison is not a fourth fidelity score. Painted REVENGE and DANGER and further reference-fidelity work remain required in the active goal. PARADE-first publication neither cuts those eras nor authorizes their aliasing or polygon fallback.

## Three-era original painted inputs v5

Current art availability is now **CONTRACT_READY for all three eras**, under the new sibling `eras-v5/`. Earlier PARADE-only availability statements describe their frozen checkpoints. Overall reference fidelity remains **IN_PROGRESS**, product admission remains **NOT_ADMITTED**, and the persistent goal remains active for actual fidelity and concrete review fixes. No old source/pin was overwritten.

Original REVENGE and DANGER sheets were generated as character-free edits of our own frozen v3 source. A new neutral PARADE v5 sheet derives from the new REVENGE sheet, preserving its broader, lower urban venue architecture. All three source PNGs remain byte-identical to the built-in tool outputs; exact prompts and generation references are preserved. REVENGE uses claret/ivory with petal-diamond banner, PARADE bone/silver/charcoal with marching chevrons, and DANGER cyan/yellow with lightning. Broad quiet terrain is intentionally shared, while all three atlases and source hashes are distinct. No character, supplied original or old scenery was sampled or changed.

Each theme supplies a 1024x1024 atlas plus the frozen 768x1296 RGB ground. Combined bytes are REVENGE **1,820,481**, PARADE **1,839,826**, DANGER **1,819,166**; combined pixels are **2,043,904** per theme. Existing byte/pixel limits remain unchanged. Every closed C06 object uses exactly the v4 integer destinations and original footprints; all six MAP-only representations, labels, place values and shared geometry hash remain identical. Source alpha varies by art skin and does not change collision.

`eras-v5/manifest.json` pins exact scenery/layout/MAP metadata/resource-inputs/provenance/source/prompt files and the shared integer projection. `eras-v5/ADOPTION.md` gives the consumer contract and limitations. Six new native WORLD/MAP projects, editable JSON, named SVG layouts and native renders are included. The native CLI 0.4.0 initially clipped image masks below the packed atlas's 1024px source height, omitting lower-map buildings despite exit 0. Actual image inspection caught it. Native-only transparent 2048px padding now fixes those masks without rewriting any runtime or original source pixel. Projects were recreated fresh; ZIP contents contain exactly two image files whose actual packaged hashes match the padded native atlas and shared ground. A lower-giveaway pixel regression prevents repeating the missing-building result.

Actual validation exits: `build-eras.py` 0; `editable-eras.py` for each of three eras 0, with six native exports; `verify-eras.py` 0 against the current closed C06 schema and exact source/resource/metadata/packaged-native hashes, nearest crops, transparent padding, unchanged prop/MAP geometry and distinct original sources. `check-eras.mjs` 0: **36 cases / 216 clear-foot samples**, at 360/390/430px with two full era cycles, one canvas, no overflow/page errors. Mobile WORLD/MAP and native renders were visually inspected, including the repaired lower-map objects. No fourth critic panel, new fidelity score, dynamic-avatar proof or current app/server acceptance is claimed.

A concrete static-map dependency was handed directly to WORLD. It exclusively owns NEW `frontend/public/world/reference-scene-v2/runtime/**`; art retains all other source/fidelity paths and does not write that prefix. WORLD published canonical static Tiled in its own checkpoint; the art lane removed its unpublished provisional duplicate serializer after confirmed reproduction. No competing canonical map remains. Root application activation has not been transferred to this art lane.

All four nested place/source provenance objects remain exactly **UNREVIEWED**, with virtual-concept semantics. This lane has no genuine newly reviewed place/operator evidence and has not acquired that review ownership. Art creation, technical schema validation and source availability cannot satisfy the backend's recursive provenance policy or operator bundle. A real separately assigned provenance review is still required; no review state was upgraded to fit admission. Product/Phaser/server/device/multiuser gates remain separate, and this handoff is not final visual acceptance.
