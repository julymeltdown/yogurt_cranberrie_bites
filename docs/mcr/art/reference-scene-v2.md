# FE-REF-SCENE: original WORLD/MAP scenery proposal

Status: **IN_PROGRESS** final source/consumer composition. Published **spatial-v5 / quality-v10** corrects the southwest diagonal route, preserving collision and all34 WORLD props. The separate **map-landscape-v1** source refinement adds ten original MAP-only planting illustrations with no new textures, geometry or world solids. New **map-layout-v2** is a measured, metadata-only label/planting correction with matching native/mobile validation **NOT_RUN**. Earlier sources and release bindings remain frozen. These are source-art inputs, not reference acceptance, a current asset release, backend spatial authority, a physical venue map or a multiuser result. See the latest sections below; earlier sections document frozen checkpoints.

## Source and scope

Original source worker base: `1e5022885525ab56f056aa2c24100e18c4bc67ed` on isolated branch
`codex/mcr-art-reference-scene`. Exact writable scope is
`frontend/public/world/reference-scene-v2/**` except the reserved WORLD-owned `runtime/**` prefix, and this document. No application
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
That scored checkpoint is retained unchanged; subsequent required source revisions are separately versioned and unscored. Original
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

All four nested place/source provenance objects remain exactly **UNREVIEWED**, with virtual-concept semantics. This lane has no genuine newly reviewed place/operator evidence and has not acquired that review ownership. The v5 checkpoint observed a recursive backend provenance guard and an operator bundle; that historical observation does not establish a blanket external physical-review prerequisite for virtual art. Source-art qualification and actual physical-destination authorization remain separate, as clarified below. No review state was upgraded to fit admission. Product/Phaser/server/device/multiuser gates remain separate, and this handoff is not final visual acceptance.

## Required spatial-v3 / quality-v6 fidelity follow-up

The actual side-by-side target comparison showed that the upper MAP district sat too low, beyond a source-only illustration fix. This remains required reference fidelity. A new `spatial-v3/geometry-proposal.json` now supplies a concrete candidate, normalized hash `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`, with explicit `integer-projection.json` and old/new root/cell evidence. The published geometry20168 and all previous art pins are unchanged. This candidate requires a new coordinated map/resource revision; it is never mixed with the completed v4/map2 consumer.

Upper court roots move up179 virtual px and the WORLD camera changes Y192→13 to preserve the close-stage composition. A continuous lower-district redistribution retains the bottom extent1296. Prop footprints preserve shape by root-relative translation; two north halls use explicit depth96 to stay within image overhang and in-map collision bounds. Collision and terrain are recomputed from this single geometry; no new IDs or place records are created. New interaction cells are stage30,24; park38,27; gifts13,48; wall14,61. Two new spawn cells and all deltas are recorded. Six overview illustrations remain noncollidable, and the main MAP venue is smaller300x210, centerX335/root373. A second fixed upper root uses the red canopy frame, addressing the target's red tree band without random stamping.

`quality-v6/` combines the candidate with newly generated original character-free angled venue halls, three diagonal-platform concert stages and quieter pale paving/materials. Every tool output original is preserved; exact source dimensions/hashes/input roles/prompts are recorded. Existing selected characters and supplied originals are untouched. All three distinct theme atlases load only with one shared ground. Combined bytes are REVENGE1,707,562; PARADE1,727,221; DANGER1,712,974, with2,043,904 pixels each. No limit is extended. Six native masked-image projects, editable source layouts and actual renders are packaged; native asset bytes and the prior lower-map clipping regression are checked.

`verify-spatial-v3.py` checks all3,888 collision/terrain cells, explicit footprint translations/overhang, existing IDs/place records, MAP-only separation and12 named routes. `verify-quality.py` checks exact sources/manifest/PNGs/crops, current closed C06, all budgets, projected object/footprint metadata and all six actual ZIP-packaged image pairs. `check-quality.mjs` records36 actual source-reference cases/216 clear-foot samples at360/390/430px through two complete era cycles, no page errors/overflow and one canvas. WORLD and MAP target crops are directly above the candidate, not replaced by a source-only score. Native/mobile images were inspected. Quieter texture is measured separately as roughly65% lower adjacent-pixel luminance difference at controlled128px material sampling, not an acceptance score. There is no fourth scored panel or asserted8.5 pass. Perimeter density, final source/consumer composition and actual target acceptance remain required.

AR/FE task disposition: this lane provides original virtual-art source qualification evidence and a CONTRACT_READY candidate for required AR02-style spatial fidelity. It does not claim physical AR01 venue-source completion or FE19 product acceptance. WORLD owns canonical runtime/** static Tiled/descriptor publication; Root owns its separately assigned v4 adapter/production composition. A ready new geometry/source handoff needs exact consumer scope and server/client pin adoption directly among those developers. Current v4 source-fixture work remains independent; archived writers are not revived for status alone.

**Scope clarification from original specification:** AR01 acceptance explicitly applies source/review/photo/closure obligations to each PHYSICAL place; BE08 separates concept-only objects, and FE19 forbids presenting concepts as verified physical destinations. The four records in this scene are VIRTUAL_CONCEPT/CONCEPT_ONLY/UNREVIEWED/photoNOT_APPLICABLE. Those values alone do not prove that virtual-world delivery needs external venue/operator input. Earlier reports described current recursive/root guard behavior, not a normative blanket physical-review prerequisite. This art increment has no concrete PHYSICAL place/source request. Source-art qualification and physical authorization must remain separate, with no forged review/reseal, guard bypass or physical-flow/IN02 cut.

## Required v7 urban architecture correction

The v6 mobile comparison still read as detached cream cottages. The new sibling `quality-v7/` corrects that concrete source mismatch with twelve original broad brick venue sprites: four identities across REVENGE/PARADE/DANGER. Shallow roofs, grouped glass doors, long window bays and restrained era trims replace domestic front gables. The built-in tool edited our own frozen v6 sheet, with the supplied target used only for architectural direction. Original output bytes, exact prompt, input roles/hashes and generation filename are recorded; selected character originals and all reference pixels remain untouched.

`build-urban.py` replaces only four frames per era. Uniform nearest crops are bottom-aligned within transparent frame boxes so the source's lower, broader facade is preserved within the existing image destinations. The other twelve atlas cells remain exactly v6 pixels across every channel. Ground, placements, footprint shapes, collision/terrain, spawns/interactions, cameras, MAP representations/labels and four truthful virtual place records are unchanged. Geometry remains `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`; no new Tiled revision is needed for this source correction.

`quality-v7/manifest.json` pins sources and every theme descriptor, SHA-256 `156d416606e648dc357a686bc1fd18ec67e425c32ee0955e164277ab229d1e87`. Each theme retains exactly two runtime PNGs: its1024px atlas and the frozen768x1296 v6 ground. Combined bytes are REVENGE **1,712,617**, PARADE **1,733,123**, DANGER **1,715,601**; each is2,043,904 pixels and remains under both existing2,097,152 limits. Six fresh native editable projects, named SVG/JSON layouts and actual768x1296 PNG exports use the established native-only transparent padding workaround; exact ZIP asset hashes and lower-map visibility checks pass.

Actual exits: `build-urban.py`0; `editable-quality.py <era> --revision 7`0 for each of three eras; `verify-quality.py --revision 7`0; `MCR_ART_REVISION=7 node check-quality.mjs`0. The verifier checks the current closed C06 schema, exact original/source/resource/manifest hashes, all-channel nearest crops and transparent padding, unchanged twelve-frame cells and geometry/place metadata, all budgets, thirty actual projected venue bases (maximum3.667px above authored depth), all six packaged native image pairs and the lower-map regression. Browser evidence records36 cases/216 clear feet at360/390/430px over two era cycles, one canvas and no errors/overflow. Three-era MAP and PARADE WORLD comparison captures and native MAP renders were inspected. These source-only checks do not establish current app/server/moving-avatar/device/multiuser acceptance, and there is no fourth critic panel or new fidelity score.

This is required fidelity progress, not a new feature or scope cut. Perimeter clustering, final source/consumer composition and actual reference acceptance remain unfinished; the persistent goal remains active. The already published spatial-v3/v6 runtime work keeps its exact frozen inputs and continues independently. V7 source adoption requires an explicit matching theme-resource publication and consumer pin change by the assigned owners; this art commit does not silently alter their release bindings or write `runtime/**`.

## Required spatial-v4 / quality-v8 natural perimeter correction

The v7 comparison still showed sparse isolated ornamental trees. This candidate addresses that concrete finding with four newly generated natural canopy/root clusters and twelve fixed unbound planting groups around the outer court, side verges and lower lawn. It replaces stone-ring ornamental-tree frames with connected natural foliage while retaining the v7 urban venue/stage/booth art. The four canopy identities share a neutral environment palette across three otherwise distinct era atlases. Exact built-in generation prompt and original PNG bytes are preserved, with no image inputs or reference/character/legacy pixel extraction. Source art base is `12f6fbf880fab927be130656e05c406f0084d0c1`; subsequent peer integration does not change those frozen inputs.

This density fix intentionally requires a **new versioned geometry**, normalized hash `sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`, under `spatial-v4/`. All 22 previous integer props/float footprints remain exact; twelve new rooted unbound props bring the WORLD count to34 and add72 solid cells. `revision-evidence.json` records all 72 row-major old/new cells, each root's authored target/chosen location/purpose/terrain and all 12 grid routes. Three named existing paved verges allow a rooted planting group; the other nine require lawn. Deterministic root choice is limited to64px from the authored target, avoids prior solids/boundary, stays at least 80px from the named feet, and rejects their alpha occlusion. Terrain/paving/ground, two spawn cells, four interactions/places, six MAP-only representations, labels and cameras remain unchanged. No new place or interaction identity is introduced.

The first uncommitted eastern group hid part of the Culture Park plaque even though all six named feet were clear. Actual WORLD comparison caught it. The group was moved below the sign; the final geometry records a protected lettered-face region, and packed-source alpha is checked against it for every era. This is an additional legibility regression, not an arbitrary depth override or hidden sign mask. The new trees use ordinary foot-depth ordering and explicit solid root polygons in both WORLD and MAP.

`quality-v8/manifest.json`, SHA-256 `0aa7ffc44d4fa69ad8d0ac9dfb8ab30907ce0e785aa461a03a1452b8461518d6`, pins geometry/projection/revision evidence and exact source/theme files. Each scene still uses one 1024px atlas and the frozen 768×1296 v6 ground. Combined runtime bytes are REVENGE **1,735,321**, PARADE **1,746,508**, DANGER **1,732,813**, and each is 2,043,904 pixels; existing 2,097,152 limits remain unchanged. All twelve non-tree atlas cells are pixel-identical v7. Original source alpha is preserved, including near-opaque image interiors; no palette quantization, extra runtime texture or alpha reseal is used.

Actual validation exits: `build-density.py`0; `verify-density.py`0 for all 3,888 collision cells, exact72 monotonic additions, all old geometry invariants and12 routes; `editable-quality.py <era> --revision 8`0 for each era/six projects; `verify-quality.py --revision 8`0; unchanged-v7 regression0; `MCR_ART_REVISION=8 node check-quality.mjs`0 for 36 source/reference cases / 216 clear feet at 360/390/430px, two era cycles, one canvas and no overflow/errors; `measure-density.py`0. The art verifier checks closed current C06, all 34 ordered props, exact source/crop/resource hashes, retained cells, bounds/root alignment, the protected plaque text, six actual ZIP asset pairs and complete editable layer membership. A stronger native lower-building check compares original near-opaque locally flat colors against each actual exported PNG: all 2,784 samples pass, preventing new tree pixels from hiding the old native clipping failure. Actual native/mobile WORLD/MAP images were inspected. Native-only transparent 2048px padding remains in place, with exactly two packaged images per project.

Depth-composited source measurement records about **2.01x** the prior PARADE MAP perimeter tree-alpha coverage, **1.84x** whole-MAP coverage and **1.28x** WORLD coverage, after foreground non-tree occlusion. These are density changes, not fidelity percentages, independent critic scores or native/browser exact-pixel parity. The source/reference comparisons remain static art evidence; there is no fourth scored panel, asserted 8.5 pass or current app/server/moving-avatar/device/multiuser acceptance.

The currently published map `mcr_reference_scene_r3` revision 4/geometry 899aca…/v6 stays frozen. This geometry requires a **separate map revision greater than 4**, with proposed next revision 5 subject to allocation by the runtime/backend owners, actual newly published canonical Tiled/content hash and matching all-three-era resources/consumer pins. Source proposal4 is not server map revision 4. No `runtime/**`, backend, application, shared contract, source archive or selected-character file is changed by this art patch. Root's current spatial-v3 publication and reserved FE09 serial handoff remain primary and independent. Published source handoff supplies the concrete new scope; archived writers are not revived for status or assigned a new goal implicitly. The art goal remains active for final consumer composition and concrete handoff fixes.

## Required MAP icon-led label composition

Published v8 source commit `492160b15465818eca172f4017f3816d58c989a9` and its 99 exact owned-path blobs were verified against remote master. Its full geometry/source pin receipt was delivered directly to Root, WorldSL and spatial owner12cc. WorldSL subsequently allocated Root a new explicit scene-adoption goal and `runtime/spatial-v4/**` lease after its FE09 goal completed. World375's former spatial goal is historical; its new FE15 input goal remains active and separate. There is no artist, king or SL technical approval gate. No separate v8 C06 REVIEWED source-qualification packet exists in the artist lane; Root was told explicitly that the old899/map4 review cannot qualify new0448, and the original source/place provenance remains unchanged.

The actual ten-screen comparison still showed plain source labels instead of the target's icon-led MAP labels. New additive `map-labels-v1/` corrects that concrete finding without changing one byte of published v8 or older inputs. Its `presentation.json` is 13,268 bytes, SHA-256 `f9599a9859fe0b660fee591440dee2f7ab8e025a5f525df82091b61b091b23cc`. It pins original v8 geometry0448/manifest/projection, retains all six anchors/names/bindings, and supplies original stage/bag/cutlery/people/gift filled polygons plus editable text. PLAZA stays text-only. All rectangles are bounded/nonoverlapping; symmetric outlines preserve the no-left-accent rule. Four unbound labels remain decorative. No active location pin is fabricated.

The explicit font is a static800 instance of already-present, OFL-licensed Roboto Condensed, renamed MCR Map Labels with source/revision/license/transform hashes. Its145,012 bytes have SHA-256 `f44e493f61e2d91c2156dd5abc720eeb6274a9c02b11d9097d1ea38d50011e8b`. Native text and vector shapes remain editable. This is an explicitly adoptable vector/text sidecar, not a changed C06 schema or implicit new theme resource. Scenery still uses its frozen two PNGs and budget; there is no additional PNG texture.

Three native MAP variants render at768×1296. `map-labels-v1/verify.py` exits0: exact old scenery layers, exact two packaged PNG hashes and one font per native ZIP,24 editable text layers/15 actual rendered icons, unchanged bindings/anchors and **pixel-exact v8 outside the six label rectangles in every era**. `check.mjs` exits0:18 actual Chrome156 source-composition cases/108 labels at360/390/430 overtwo era cycles, exact font loaded, text within padding, no page errors/overflow and one canvas. The first browser check found a1.05px PLAZA font-advance discrepancy; the final4px metric allowance resolves it. Nine supplied-target/candidate captures were saved; PARADE comparisons at all three widths and all final native exports were inspected. The native viewer-open returned queued; actual CLI PNG exports provide the render evidence.

`map-labels-v1/ADOPTION.md` specifies map-coordinate scaling, baseline/icon placement, font loading, null-binding behavior and limitations. Root's immutable v8 adoption proceeds independently; this sidecar requires explicit consumer adoption and actual application validation. No v8 C06 source review, application/Phaser/server acceptance, fourth critic score or target8.5 pass is claimed. Full reference fidelity, stronger route hierarchy and concrete consumer-composition findings remain in the active art goal. No runtime/application/backend/shared/character/source-archive path was changed.

## Required v9 route-edge and northern-facade composition

The label source was published to master as `f27cc114e2482b98d75ba00ce0104008a97d2e9e`; a fresh remote snapshot `1383ae452fc81dfe5fd1e4ca4d37af8950700dda` proved ancestry and all37 changed-path blob equality. Exact published label/font/rendering instructions went directly to Root153 and WorldSL. Root owns actual application adoption; no artist, king or SL technical approval is required.

Actual continued source inspection found that unoutlined paving flattened route boundaries and the denser canopy hid both northern MAP facades. New sibling `quality-v9/` addresses those two visible findings without changing canonical geometry. Its6,621-byte manifest has SHA-256 `8a2bbe62a31bce98e1bcab91bdde53c3ce0b6fd21943dae8d2da0e8003952d15`. All34 WORLD placements/footprints, all3,888 terrain/collision cells, paving polygons, cameras, spawns/interactions, place identities/review states and original v8 atlas bytes remain unchanged. Normalized geometry is still0448. This art revision requires explicit new resource/overview consumer adoption, **not another canonical map revision beyond the actual backend allocation for0448**.

New native `ground.tsrct`, editable JSON/SVG,119 vector union-boundary segments and actual native PNG retain the source. Two editable shape layers paint walkable route edges over the original v6 materials; no internal polygon-overlap seams or crop-edge outline is drawn. Independent raster-union morphology verifies that all49,996 changed ground pixels stay within7px of the actual paving boundary. Runtime ground removes only uniformly255 native alpha, with every RGB pixel proven equal to the actual export. Its768×1296 RGB PNG is558,464 bytes, SHA-256 `20cecda630d633f20122607d97b86fbd3f7c6c0f06483ca4d838578a9610915a`. Combined runtime bytes are REVENGE1,762,305/PARADE1,773,492/DANGER1,759,797; each retains two PNGs/2,043,904pixels under existing budgets. No palette quantization or texture-budget extension occurs.

The MAP overview adds two explicit illustrations replacing existing unbound north-hall images at its smaller scale: original atlas frame, rectangles(-12,6,210,144)/(542,6,210,144), depth150, null interactions. The six earlier representations stay exact; eight total create no new WORLD solids, identities or destinations. Across all three actual MAP exports,1,483 qualifying source-color samples match at those facades versus280 in the old same regions. The new native MAPs also compose the exact published editable label/font sidecar; WORLD has no labels. This is rendered-source visibility evidence, not a fidelity score.

`verify-route-composition.py` exits0: closed current C06, all pins/resources/old WORLD objects/places, budgets, exact atlas/native padding, ground RGB parity/boundary bounds, six saved ZIP compositions/two-PNG hashes, MAP fonts/text and facade samples. `check-quality.mjs` with revision9 exits0:36 actual source-browser cases/216 clear feet at360/390/430 through two era cycles, eight MAP representations, exact label font/advance fit, no errors/overflow and one canvas. Eighteen target/candidate captures were saved; all six native exports and PARADE WORLD360/MAP390/WORLD430 were inspected. Native viewer-open returned queued. Frozenv8, density12routes/3,888cells, plan13archivehashes and code/doc whitespace checks also pass.

`quality-v9/ADOPTION.md` records exact source/consumer instructions and limitations. These are additive original virtual-art source revisions; all old source and release pins remain frozen, and artist writes no runtime/application/backend/shared/character/archive path. Painted edges do not certify complete geometric route-hierarchy fidelity. No fourth critic panel, target8.5 pass, C06 REVIEWED source qualification, application/Phaser/server/device/multiuser/physical admission is claimed. Full target fidelity and actual consumer-composition findings remain unfinished in the active goal.


## Required spatial-v5 / quality-v10 continuous diagonal correction

V9 publication was independently verified at master merge `0779101878071e89a533f7e47c2dfdef1d1c82b1`: fresh remote snapshot `14b0548a7a062380093861c338e0efdc12f49f3d` contains the payload ancestry and all88 changed-path blobs exactly. `quality-v10/previous-publication.json` records this historical receipt; it is not proof of v10 publication. The user-designated frontend/art integrator is Root153, using its isolated publishing checkout. Artist publishes only a task branch and hands off exact committed paths, without a king/SL/main-chat technical approval gate.

The target comparison still showed a major geometric mismatch: the southwest approach needed a continuous diagonal instead of the old vertical detour and duplicate cross-route. New additive `spatial-v5/` and `quality-v10/` implement that specific correction. Normalized geometry is `sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`. Exactly560 terrain cells change; all3,888 collision cells,34 WORLD props/float footprints, cameras, two spawns, four interaction cells/place records and six label anchors remain frozen. Eight MAP representations stay exact v9. The new lower west connector meets the retained lower venue link. Four approximate manual reference-road centers are7.24/0.53/0.04/13.27px from the new centerline, versus162.83/39.68/98.26/121.15px from the old route; these annotations are not independent whole-reference acceptance.

The new 7,749-byte `quality-v10/manifest.json` has SHA-256 `9048d45753f4d47828082c422061ed17418f8bfb93b51cf3e9879e821eeb0cf6`. Geometry source is184,445bytes/SHA-256`a2cdd0e60c1f9cc7c3f093643760000e91a01e1fadffb51f9a76bd2c3a7a3a0f`; projection is22,508bytes/SHA-256`ce72d8311287067d8c280f136c852a4f0024610f6550a83b12d42e4df2b82651`. `map-label-presentation.json` is13,881bytes/SHA-256`850c96b38253f4953778ac4b30d3b10372d091630b09d8a69bc425a28bcc4fd0` and explicitly rebinds the exact v1 drawing values to ef52. It pins old presentation/font lineage and new geometry/projection; old0448 sidecar bytes/review states remain unchanged. No prior source qualification is inherited.

Native `material-ground.tsrct` has121 editable original grass/cobble image layers clipped to the new paving. Exact original1254px source pixels sit within native-only2048px transparent padding. No reference, character or legacy pixels enter the art; old decorative ellipse material beds are removed. Native `ground.tsrct` adds108 original vector union-boundary segments. All47,704 paint changes stay within7px of independently rasterized new road edges. Both runtime ground PNGs preserve every actual native RGB pixel, stripping only uniformly255 alpha and compressing losslessly. Final runtime ground is549,155bytes/SHA-256`dd70d8fb02e1c6e57e9a9f9e96d69dc7c15999bc076678a7075161281c929887`. The three original v8 atlases/native padding remain exact; combined runtime bytes are REVENGE1,752,996/PARADE1,764,183/DANGER1,750,488, with two PNGs/2,043,904pixels each under existing budgets.

Actual exits: new spatial/material/edge/composition authoring0; three native era exporters0; `verify-diagonal-spatial.py`0; `verify-diagonal-composition.py`0; revision10 browser check0; frozen v8/v9 checks0; plan/archive check0. Spatial verification recomputes all3,888 terrain centers and collision SAT cells plus12 BFS routes. Composition checks current closed C06, all pins, explicit new label binding, exact saved native ZIPs/two-PNG hashes/MAP font, native RGB parity and route-edge bounds. All6,582 lawn and5,686 paving samples match original material quadrants within25/channel. Lower-building probes pass in all six renders, excluding MAP label rectangles while preserving the90percent/40-channel threshold; northern facades remain visible. Browser evidence records36 cases/216 clear feet at360/390/430 over two era cycles, exact geometry hash, font/advance fit, no errors/overflow and one canvas. All six native exports and PARADE WORLD/MAP captures at all three widths were inspected. Native viewer-open returned queued.

Actual backend allocation5 for0448 is recorded in producer commit `ed34331cac41d1997335b5f6dc93f33429005a91`, separate from database publication/admission. The new ef52 terrain requires a distinct canonical Tiled/server/client revision greater than5; proposed6 is not allocated, reserved or published by art. Current0448/map5 adoption continues independently. No old geometry/release is silently resealed. Source publication and application adoption remain separate; `quality-v10/ADOPTION.md` and `spatial-v5/ADOPTION.md` give exact boundaries. Artist changes no reserved runtime, app, backend, shared, selected-character or source-archive path.

The source goal remains active for full target fidelity and concrete consumer findings. No fourth critic panel, new8.5 score, C06 REVIEWED source qualification, current Phaser/application/server/device/multiuser/physical acceptance is claimed. The historical WORLD6.5/MAP6.0 panel stays below threshold.

Focused technical peer review found two verifier false-pass gaps while confirming the actual candidate inputs were correct. The follow-up enforces the packed ground bitmap hash and all12 per-theme geometry hashes against recomputed canonical geometry. `verify-diagonal-negative.py` exits0 and rejects both legacy-ground substitution and old-geometry metadata with freshly updated outer manifest pins, entirely in memory with zero artifact writes. These are technical regression proofs, not another critic panel or visual acceptance.
The same reviewer confirmed both findings resolved in a read-only follow-up, with no additional actionable finding in the narrow fixes/tests; `quality-v10/technical-review.json` records the scope and limitations.


## MAP planting illustration refinement

V10 source publication is independently verified at remote snapshot `63c8521cf43d7d4ab7e6fa570c819c1fe934ab26`: owner commits a24cc751/c76f5ec3 map to published c7b0cb013e5ab1d9f8cac08be1d0046c249e1cd4/06562ff1962be525ed929ef431a54bb589860373. Both published commits are ancestors; all107 owner payload blobs and105 closed Git-byte pins match exactly. The receipt is preserved under `map-landscape-v1/v10-publication.json`. V10 runtime adoption was then assigned separately to the existing Root153 consumer chat with a new isolated worktree/goal; that assignment is not startup, release or visual acceptance proof. Reserved `runtime/**` remains outside artist ownership.

The actual MAP comparison still had sparse Plaza and lower-lawn gardens. New additive `map-landscape-v1/` adds ten independently editable unbound planting illustrations using each era's original `shrub_bed` atlas frame. Its 12,497-byte `presentation.json` has SHA-256 `90402525366769febb1f8d5b779d9b293e4680858f105c88af4bf808965cc35e`. It pins exact published v10 source, ef52 geometry/projection and label composition, plus per-era source generation/provenance, original crop/packing transform, scene/layout and atlas. All original raster pixels/alpha are preserved; no generation, recoloring, reference, character or legacy sampling is introduced.

This is an explicit MAP-only presentation input, not a canonical geometry or C06 change. The first five rectangles sit wholly within the semantic green Plaza overlay; the other five are on existing canonical lawn. Every covered pixel is verified, with no label overlap and8px clearance from all six named feet. Draw depths match visible source bases within1px. All bindings are null; no WORLD object/solid, place/destination, interaction, physical eligibility or selected state follows from an illustration. WORLD ignores the sidecar. Current0448/map5 and all v9/v10 source/release inputs remain immutable; no additional map revision is needed beyond the actual ef52 binding itself. Art allocates no server revision.

All three actual native MAP variants remain768×1296. Removing the ten new image layers reproduces all93 original v10 layers exactly. Every actual output pixel outside the ten rectangles equals v10; all30 native visibility probes exceed the85percent source-alpha-support threshold (minimum 0.872). Two initially canopy-hidden beds were repositioned before these final renders. The three native ZIPs retain exact ground/native-atlas/font bytes. Runtime keeps its two PNGs/2,043,904pixels and unchanged REVENGE1,752,996/PARADE1,764,183/DANGER1,750,488 bytes. No new texture/font or budget increase occurs.

Actual exits: authoring/native exports0, source verifier0, browser check0, plan/archive check0 and scoped whitespace check0. Browser evidence records36 cases at360/390/430 over two era cycles in planting/base modes,216 label checks and216 added-decoration foot samples, exact geometry/resource dimensions, font/advance fit, one canvas, no errors/overflow. These foot samples concern the added illustrations, not complete moving-avatar occlusion. Nine target/candidate captures, all three native PNGs and PARADE comparisons at all three widths were inspected. Native viewer-open returned queued; actual CLI PNGs provide render evidence.

`map-landscape-v1/ADOPTION.md` specifies hash/era validation and stable depth ordering, labels afterward and actual runtime markers last. The designated publisher receives an exact committed handoff; adoption needs a new explicit pinned consumer patch. No routine Main/FrontendKing CC or approval/acknowledgment gate is added. The same artist goal stays active for full reference fidelity and concrete consumer findings. No fourth critic panel, new8.5 score, application/Phaser/server/source-review/release/device/multiuser/physical acceptance is claimed. Only the new source folder and this lane document are written.

## Exact-target registration follow-up: MAP layout v2

The frozen consumer packet for candidate `d5c0c9d188f68acb749d7ed2940706f34b2a8be1`
is independently verified: `capture-packet.json` SHA
`93770ac73a785f4b9da190da8d2929e130bbc7d54fa81c13b90278c9ca24eb96`, all49
artifact byte/hash pins and all65 renderer plus40 root source pins match the
represented Git input. Published checkpoint
`12a5d394eeabc89e1c7b0012fb51038f737c9a7f` is an ancestor of inspected remote
snapshot `e449890ad62e57f6708c69c9f76dc2f6f5ee5269`; all49 artifact and105 source
pins match its Git blobs too. The full stage/sign framing, compact park/wall
markers and reserved caption row resolve their earlier consumer findings for
that input. Strict zero-difference renderer checks and16 root cases are bounded
technical evidence, not reference acceptance.

Actual PARADE WORLD/MAP captures at360/390/430 were compared with the unchanged
ten-screen target. The source MAP label registration still materially differs:
the old MAIN STAGE text-paper center is86.77px left and53.57px above the measured
target center; PLAZA is42.79px right/68.20px above; MEETUP ZONE is107px below;
GIVEAWAY BOOTH is78.31px below. These are source-coordinate differences after
normalizing the original248×420 MAP crop to768×1296, not subjective fidelity
scores. All six measurements and the explicit paper-pixel selection rule are in
`map-layout-v2/target-measurements.json`.

The additive `map-layout-v2/label-presentation.json` corrects the six nominal
anchors to422,201;327,529;177,700;652,685;371,918;518,1159, respectively. It retains
all strings, existing/null bindings, rectangle sizes, original icons, typography
and exact font/license. Matching `landscape-presentation.json` changes only two
unbound PLAZA garden placements to clear the new label. No new image, native
project, font, WORLD prop, collision, identity or canonical geometry is authored.
Both old sidecars and every frozen source remain intact.

`build.py` and read-only `verify.py` exit0. Structural checks cover target-image
hash/component bounds, six text-paper center errors at most0.484 source pixels,
unchanged drawing/identity inputs, no label/planting rectangle overlaps and all
1,428 relocated destination pixel centers inside the existing PLAZA parcel.
These checks cannot prove the new visual result. Matching editable/native
composition, garden visibility/occlusion, mobile reference comparison and
explicit runtime adoption remain **NOT_RUN**; the old packet must not be reused
as their proof. `consumer-review.json`, `verification.json` and `README.md`
record the evidence and exact consumer follow-up.

The original unfinished goal is reused. Its tool metadata was still **BLOCKED**
when concrete captures and publication inputs arrived; source comparison and
this bounded correction resumed without creating or falsely completing a goal.
Full layout/proportion/architecture/texture/character composition acceptance
remains **IN_PROGRESS**. No fourth scored panel,8.5 pass, reviewed place source,
production/release, live-server, physical, device or multiuser acceptance is
claimed. Only the new source folder and this lane document change. Any consumer
follow-up needs an explicit new exact lease because the former v10 owner has
returned its implementation paths.

## Independent architecture source correction

QA06's immutable comparison pair `postfix-12a5-review/compare-map.png`, SHA
`f2bcae9c2dd87f75d19c59c20e1f2c82c68e3a4de9290bb1debc6ffde28b3338`,
confirms that the published12a5d394 baseline still has low, roof-heavy venue
forms and sparse masses relative to the target. This is a source-art finding;
the separately assigned World viewport correction and MAP layout-v2 adoption
do not resolve it by themselves. The QA pair is not evidence for either new
patch. Full reference fidelity remains open.

Named BackendSL allocation ART-ARCH-SOURCE-01 allowed exactly one cloud imagegen
sheet plus a byte-exact owned copy and small metadata, hard128MiB net peak,
without browser/native/atlas output. Own immediate pre-call capacity was
10,962,501,632B, above the8GiB+128MiB guard. Additive `architecture-v2/` now
retains an original1254×1254 RGBA sheet:2,759,982B, SHA
`ef8f0a5c05702940bae39b0e4661f6a5223f8afd1e351a96f9d28186e0d4769a`.
The old sheet and target are reference inputs only; their bytes are unchanged.
The final prompt and built-in generation provenance are recorded beside it.

Actual visual inspection finds two visible façade floors in the first three
rows and three in `west_hall`, preserving the existing four frame identities
and three era palettes. Reproducible source analysis finds12 separate major
alpha cores, none touching sheet edges. Faint raw sub32-alpha residue reaches
the left/bottom sheet edges; it is retained in the original and disclosed.
Proposed crops enclose each core with2px margins. No repaint, alpha cleanup,
packing or raster derivative was performed. Actual façades in several rows
are narrower than requested; projected uniform letterboxing is recorded,
not presented as a map-render or fidelity pass.

Only the new source folder and this ART document change. All canonical geometry,
feet, original source/character packs, resources, runtime and release bindings
remain frozen. Atlas/native composition, frame occupancy, occlusion, actual
consumer adoption and mobile target comparisons are **NOT_RUN**. These require
separate resource allocations and exact consumer ownership. The original goal
metadata remains **BLOCKED**, reused without reset or false completion while
explicitly resumed source corrections proceed.

## Architecture atlas derivatives

Named ART-ARCH-ATLAS-02 separately allocated deterministic atlas work only,
hard64MiB peak/24MiB net output. Own fresh pre-job capacity was10,452,652,032B,
above8GiB+64MiB. Additive `architecture-v2/{revenge,parade,danger}/` contains
three1024×1024 atlases, three2048×2048 padded raster counterparts and12 JSON
layout/scenery/resource/provenance files. All18 derivative outputs total
7,921,243B and are closed by `architecture-v2/atlas-handoff.json`.

The original source sheet is cropped using the recorded rectangles, uniformly
nearest-scaled and centered/bottom-aligned inside the unchanged244×180 frames.
Every saved atlas RGBA pixel matches that reconstruction. The other12 frames
and all pixels outside the four building rectangles equal the old atlas exactly;
all16 frame rectangles,34 WORLD placements/footprints/depths/interactions, MAP
representations, cameras and ef52 geometry are preserved. Original ground is
referenced rather than copied. Padded raster counterparts preserve exact atlas
pixels top-left and transparency elsewhere; they are not native scene exports.

Runtime resources retain two PNGs and2,043,904pixels per era. Total bytes are
REVENGE1,821,253/PARADE1,833,584/DANGER1,820,611, below the existing2MiB limit.
Build and independent saved-byte reconstruction verification exit0. The actual
PARADE atlas was inspected: the taller floor levels survive packing. No browser,
Tesseract project, native scene export, consumer edit or new capture is included.
Existing paired label/planting sidecars retain their original resource pins;
new atlas adoption requires an explicitly assigned matching pin closure.
Full scene massing/density, occlusion and mobile reference fidelity remain open.

## AR02/AR04 retained partial World diagnostic

The existing Root153 native01 REVENGE frame is517,101B, SHA
`a36f915dfe26822a00dac0b18dd100ee229e91115ca493e1d16d7edb33c1bb5d`,
actual360×800. ART independently checked these bytes and viewed the original
alongside the unchanged target. Published architecture source921cb3d8 is an
ancestor of capture sourcea0c88b5c, and all25 architecture-folder Git postimages
match at capture. This is an unpublished retained owner artifact, not a new
ART render. `architecture-v2/diagnostics/native-01-revenge-360.json` and `.md`
record exact inputs, findings and limits without copying the image.

The visible meeting hall's two-storey façade and reduced roof share survive
actual rendering. The MAP main `west_hall` representation is not shown, so its
known narrower massing remains open. The reported360×703 World canvas differs
from the reference354×360 PARADE World scenery crop; the extra lower streets,
lawn and architecture cannot justify camera, road, canonical-foot or fixture
changes. This frame does not establish a further raster correction.

Root reports zero differences in this completed REVENGE case and ten clear-foot
building-depth comparisons; ART did not rerun them. The next PARADE overlap case
failed50 channels, leaving native01 as a whole **FAIL**. Consumer/oracle diagnosis
remains with Root. No all-era, native, runtime, occlusion, mobile or whole-fidelity
acceptance follows. The original AR02/AR04 criteria remain open under the same
goal, whose metadata remains **BLOCKED**. No additional browser/export/resource
job or manager review gate was introduced.

## AR02/AR04 completed MAP view and venue assembly correction

Root's retained native04 PARADE MAP390×844 (409,928B,
`19d05754866c6ab54004a89dbb3e6026cf3b5b2b7b2e33c5f5a749ff1c907e03`),
NEW editable768×1296 SVG (2,657,147B,
`cb34694ac25ffa5e488aa2c98094f23a0c4950a3eb5622ed590d21f606db4cb5`)
and actual Chrome SVG raster (1,476,218B,
`9156109e82573868a1a9d24585f305b890353a96889392b34988359c590ba524`)
were independently byte/hash checked. Both actual PNGs were viewed against the
unchanged original target and the unchanged reference half of QA06's earlier
pair. All25 published architecture postimages match capture source4cb162b7;
source921cb3d8 is its ancestor. The source/artifacts were publication-pending at
comparison. No image was copied or exported by ART.

Root reports native04's full bounded technical matrix PASS; ART did not rerun
it. That capture uses prior World presentation and cannot prove later combined
master pixels. The matching MAP view now establishes an architectural difference:
the central building reads as a narrow standalone block, while the reference
venue has flanking wings. Taller floors alone do not complete its massing.

New additive `map-venue-massing-v1/presentation.json` addresses that shape with
two unbound MAP-only annexes sampled from the existing three-era `north_hall`
frame. West117,244,169×128/depth372 and east436,280,122×90/depth370 draw behind
the unchanged stage representation at185,163,300×210/depth373. The frame envelope
widens from300 to441px; this is an assembly metric, not reference segmentation
or visual acceptance. All atlas/source pixels,34 World props, six labels, ten
gardens, identities, geometry, feet, cameras and bindings remain unchanged.

Source checks validate pins, exact base placements/frames, label/garden
disjointness and12 canonical spawn/interaction clearances (minimum22px). Separate
static alpha point analysis records partial annex occlusion by the central hall
and existing trees across all three eras, without writing a raster. It is not
a native-render or real-avatar visibility proof. New composition has51 MAP
objects rather than49 and requires an explicitly recorded consumer follow-up.
Native04 evidence qualifies the earlier input only. Actual new-wing visibility,
massing, mobile captures, landscape density and full original fidelity remain
open. The same original goal remains metadata BLOCKED; no new artist browser,
export, resource job, manager review or false completion is introduced.

## Human service-label correction

The newly attached1280×853 ten-screen JPEG was actually viewed and retained
byte for byte as `map-service-labels-v1/original-target.jpg`:482,737B, SHA
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
It is source/reference evidence only, never a product texture. The earlier
reference, supplied originals, character packs and9a441859 wing inputs remain
unchanged. The human correction supersedes literal service names: FOOD becomes
FEED with a post/message glyph, MERCH becomes WALL with a mural-grid glyph,
MEETUP ZONE becomes MEET and GIVEAWAY BOOTH becomes GIFTS.

Additive `map-service-labels-v1/label-presentation.json` preserves all six panel
rectangles/anchors, palette, exact font bytes and stage/people/gift polygons.
Two new hand-authored filled vector glyphs replace utensils/shopping imagery in
this annotation layer. Four single-word feature labels use closed
`navigation_intent` values feed/wall/meet/gifts. All six annotation
`interaction_id` and `place_id` fields are null. MAIN STAGE and PLAZA are
`VIRTUAL_SCENERY_ONLY`; four features are `VIRTUAL_FEATURE_NAVIGATION_ONLY`.
The old display string survives as `source_label` lineage, not a canonical ID.
No arbitrary action URL, canonical-place remapping or physical arrival claim
is introduced. Genuine reviewed-place/catalog safety remains a separate
consumer responsibility.

The actual completed source bundle is label/glyph-only, per the final freeze
implemented in this checkout. Its top-level required_base_labels/landscape/
architecture/massing pins close the exact old lineage. It creates no new paired
landscape/wing derivative or display replacement; all old49 art objects remain
exact. Existing two wings require explicit consumer adoption. Grocery/stall
imagery remains a known separate visual concern, so the new labels alone do
not certify complete semantic-art or target fidelity.

`build.py --verify` and independent read-only `verify.py` exit0: source/image/
font/lineage hashes, six retained rectangles/anchors, four closed intents,
six null canonical bindings, glyph bounds and actual font advances fit the
available panel widths. Handoff closes six payloads. No new PNG/font, browser,
export or consumer runtime edit occurs. Actual glyph/native typography,44px
action targets, real feature routes, separate reviewed-place safety, combined
wing visibility, mobile density/framing and full attached-target fidelity remain
**NOT_RUN/OPEN**. Root owns those checks under an exact recorded consumer lease;
no King/SL review gate applies. The same original ART goal remains metadata
**BLOCKED**, reused without reset, new goal or false completion.

## Independent attached-target canopy palette correction

After sealing the service-label source, the actual attached target and published
native04 PARADE MAP raster reveal a northern-district palette difference: four
large crimson crowns dominate the candidate band between façades, whereas the
reference uses muted green/yellow foliage. Coarse matched source-coordinate
region210,0–542,160 samples contain481/53,120 red-classified points in the
attached reference versus28,935/53,120 in the actual saved raster (0.91%/54.47%).
`map-canopy-palette-v1/reference-analysis.json` records both exact input hashes,
approximate JPEG crop and RGB rule. This is a color proxy supporting actual image
inspection, not foliage segmentation, corrected render or fidelity score.

The separately versioned source overlay replaces tree_red with the existing
tree_green frame for rooted_tree_01/rooted_tree_02/grove_north_west/grove_north_gap
in PARADE MAP only. All four IDs/destination rectangles/depths,34 World props,
geometry, feet, source pixels, labels, wings, gardens, cameras and authority stay
unchanged. It adds no props, PNG, font or random filler. Original source-alpha
base ratios differ by less than0.01 source pixels at these instance heights.
The existing saved PNG is read from published Git bytes; no image copy, browser,
export or media job is created.

This source correction has its own pinned handoff and does not mutate or block
the sealed service bundle. Consumer adoption needs a separate exact recorded
scope. Actual corrected canopy visibility/palette/native/mobile comparison is
**NOT_RUN**; composition, density, framing and overall attached-target fidelity
remain open. Same original goal metadata remains **BLOCKED**.

## Superseding human direction: cozy backgrounds only

The latest human instruction requires backgrounds visibly cuter than both the
current product and the unchanged collage. The latest120DC5EC JPEG is byte
identical to the preserved target (SHA2562b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23).
Its dark palette is no longer a parity target. Historical canopy/roof diagnostics
above remain evidence, not current design acceptance. Portrait-window and
proportional-fit proposals are withdrawn. All character pixels, selected packs,
UVs, crops, framing, anchors, renderers and sizing remain unchanged. Foreground
controls, text, typography, layout, labels, intents and real functions are frozen.

GrantA022730e5/af01689f and the following sole-source allocation authorize only
the ten new files under map-cozy-v1 and this existing ART document. Work uses
the isolated art-map-cozy-v1 checkout and codex/mcr-art-map-cozy-v1 branch from
published7c25078f118da53dd9a524d0a58e707cca190f0f. The former a519 checkout and
its sealed source commits remain preserved. Root owns the shared World/Map
descriptor adapter; there is no second World pack or artist runtime ownership.

The first source changes dominant roof/façade/tree surfaces toward cream,
peach, lavender, sage and mint. It includes softer two-storey halls, rounded
foliage and restrained flower/music details. It does not rely on tiny ornaments
over unchanged dark scenery. One built-in image_gen call, with only the old
environment atlas as reference, produced the original1254x1254RGBA sheet:
2120690bytes/SHA256db77361f6366157de19e32fb73b988e5f04883269c1f0a53d9a1a086a1d0b514.
Exact prompt, original bytes and per-sprite source/packing provenance are saved.
No character reference or output is included. The model version is not exposed.

Fourteen sprites are registered into the original sixteen frame rectangles;
the stage and CULTURE PARK plaque retain exact original RGBA bytes. The
1024x1024RGBA atlas is1201118bytes/SHA2569c77e42eb377fbab1df485d6c1e8e47221ac33aebbc03e9aecba3cc7bbcded9f.
All sixteen alpha>=32 baseline rows remain exact. Rectangle crops and nearest
sampling do not repaint or threshold retained pixels. Faint interior crop-edge
residue remains; sampled exterior red specks have alpha1/255. This is disclosed
source material, not an asserted native defect or an opaque red outline.

The self-contained768x1296 ground SVG is741981bytes/SHA256e52f9acdaf4bc491664125585776ae42fe34848a59f5edc0205880095a19de40.
It embeds the exact old ground PNG and applies two code-native palette filters
using the exact43 existing paving polygons. No coordinate/topology/path/foot/
camera change is made. Root alone receives temporary ownership of the new
parade/ground.png export in its own allocated workflow. At this checkpoint the
PNG is **NOT_RUN**; the presentation correctly remains
**SOURCE_PREPARED_AWAITING_ROOT_GROUND_PNG_EXPORT**. The final combined PNG
budget is2097152bytes/2043904decoded pixels, leaving896034bytes for ground.
No closed PNG tuple or application adoption is claimed yet.

ART-COZY-SOURCE-202611040621Z was accepted after a fresh40.617GiB free-disk
guard and verified absence of old owned jobs. One generation and bounded source
packing completed. Build and deterministic verification exited0; the actual
source atlas was viewed. The output allocation was explicitly released at
2026-11-04T06:30:32Z (15:30:32KST), with6251073 counted new media-output bytes,
zero temporary bytes, no owned server/browser/native process or pending tool
job, and38.863GiB free. Remaining source code/documentation and metadata closure
do not acquire another media job. Root owns the separately allocated ground
export and actual native captures. Source images and JSON equality are not
visual acceptance.

The real candidate6ca52f8851473090479f1c13a33e879b6cd099d0 Map390x844 capture,
saved15:05:14.291358KST/SHA25687ab843422292a7701e2a3fe39a4419ef95e52721c6c2b052b617c6c2d135b01,
is preserved BEFORE evidence. Actual390px cozy World and Map AFTER, first-glance
cuteness, readability/contrast, affected360/430px views and real functional
gates remain **NOT_RUN/OPEN**. This is the first PARADE source only; REVENGE and
DANGER remain unchanged and are not cute-accepted. The original goal metadata
was freshly read as **BLOCKED/unfinished**, reused without reset or completion.

### First PARADE ground export and source closure

Root returned its immutable768x1296PNG after allocated job
COZY-GROUND-PNG-202611040637Z at2026-11-04T06:38:25.796Z, with exit0 and an
explicit stop/full release. Artist independently checked and copied exact bytes:
850849bytes/SHA25609d77ad8aa85f9d6464fa5248dae3eb91498f57b0c2183af1f90fc2e79b0dd45.
The two runtime PNGs total2051967bytes, leaving45185bytes below the2097152-byte
limit, and2043904decoded pixels. The source SVG and atlas pins above remain exact.
Root reports one Chrome156.0.8078.80/noVite/network export,1.3seconds, closed
browser, removed owned temporary/profile/link state and peak7601112output bytes.
This is attributed export evidence, not an Artist native application run.

Artist closes metadata using --close and verifies using --close --verify;
neither repacks nor exports pixels. The resulting first PARADE source status is
CONTRACT_READY_SOURCE_VALIDATED_NATIVE_NOT_RUN. Real390px World/Map AFTER and
relevant360/430px checks remain Root's separately allocated work. Original
functional, device, ten-background and human visual acceptance remain open.

The exact proposed three-file era extension has source-code preparation only:
revenge/atlas.png, danger/atlas.png and era-derivatives.json. No such output has
been created. Derivatives must use each era's own frame rectangles and original
baked-text pixels; five REVENGE and seven DANGER rectangles differ from PARADE.
Their future palette/registration mode requires an explicit binding record,
separate named output allocation and a committed first PARADE handoff. No
released media allocation, extra image generation or new character rights apply.

### Three-era source derivative delivery

The first PARADE source70f22a683ab029751548331df5cf46f2ad16baf8 was published as
97c65b8e8120eb46e72c958d1816383f31689378. Artist independently fetched master
716b749665fab1b7b05c414da4155b3046c47054, matched ls-remote, all12 source/doc
postimages, all10 payload pins and source/master ancestry. No receipt-only
commit or repeated runtime test was needed.

Root then supplied actual390x844 World/Map before/after images from
candidate4e491a4509e9cab6a3e28364db616bdab9c157f5, native-02. Artist checked all
four SHA/byte/dimension records and directly viewed the actual scenes. World
AFTER449001bytes/SHA256ff25b45a3d9f05729705fe628a82ea8ecfcda3392dde63b6951de754c2c829a8
was saved15:54:23.954KST; Map AFTER386020bytes/SHA25662114b928572cb7462f877f828c2b5ba2c7130b36181632d231eb8673a52a473
at15:54:24.864KST. Their dominant foliage/buildings/terrain visibly show the
warm cozy direction against both before and the unchanged collage. No new
registration defect was identified. The olive Map fit bands were reported to
Root as a bounded presentation-background observation, without changing CSS,
camera, layout or source PNGs. Six-character texture/frame/position/origin/size
equality and camera/semantic-target equality are attributed to Root's runtime
checks, not an Artist rerun. This is a first390px source-owner visual cue, not
human final acceptance, live/device/IN evidence or all-era qualification.

Binding7dcbc0e2 grants only revenge/atlas.png, danger/atlas.png and
era-derivatives.json in this source prefix, plus the already assigned recipe/
metadata/documentation updates. ART-ERA-PACK-202611040658Z was separately
accepted after a fresh37.571GiB free-disk guard, clean checkout and absent new
output paths. Actual packing used build.py19955bytes/SHA2566f9c122c97ceab201202a9693575edcd4f1989fb1c4b95324a21574cdf54937f
and the immutable generated sheet. One bounded Python/Pillow job exited0 in
0.657seconds. No second image generation, browser, native capture, ground export,
server, dependency or character input was used.

The exact RGB integer recipes are recorded in era-derivatives.json. All alpha
bytes are preserved before nearest sampling. Every destination uses its own era
layout; all32 frame rectangles/baseline rows, four original stage/plaque RGBA
frames and all pixels outside the fourteen replacement slots per era passed
independent checks. Source sheet, PARADE atlas, ground and registration bytes
remain exact. The following PNGs are new scenery, never character resources:

- REVENGE1176781bytes/SHA25654b5a72a1e16062f84248602f8bebaee500ca086fb58285b1689d665618dfdbd;
  combined with shared ground2027630bytes.
- DANGER1178440bytes/SHA2563184e26c5b83cf95c31f692b3087112faa93a0ef270bd5416e36fc9952d9a17a;
  combined with shared ground2029289bytes.

Each pair stays below2097152bytes/2043904decoded pixels. The32665-byte era
provenance record has SHA2564cb98c6ba28535427b4c3ed8e122e317a0f0cd1e278d2b9cb56aaf52836b5fa0.
Metadata-only closure and verification passed with13 exact payload pins. The
finite output allocation was explicitly fully released at2026-11-04T07:01:02Z
(16:01:02KST):2405246 new/changed PNG+JSON bytes, zero temporary bytes, exited
owned child/no remaining job/browser/server/process, and40353869824 free bytes.
No further packing is acquired by later prose/hash closure or publication.

The first PARADE PNG tuple remains immutable; the refreshed era-keyed source
manifest is a distinct source cohort requiring Root's explicitly assigned
consumer extension. Generated source differences do not establish visible
in-app era distinction. Actual REVENGE/DANGER390px World/Map scenes, relevant
360/430px views, all-ten background acceptance and original functional/live/
device/public-flow obligations remain open under the same unfinished goal.

### ART-BG-ERA-PUB-01 publication verification

Artist independently verified publication at 2026-11-04T07:58:07Z from
`codex/mcr-art-map-cozy-v1`, with clean HEAD
`aa7d09a498f5d6e0c5c30294155d7a4e55af6bf6`. The original unfinished goal
remains `blocked`; this receipt does not reset or complete it.

Observed source, published mapping and remote master respectively:

- Source: `aa7d09a498f5d6e0c5c30294155d7a4e55af6bf6`.
- Published mapping: `0e156ae686a9cae71704bc096b2e488b9a81ae82`.
- Master: `aea02a262f7d834be7ef2267368812520a5d9c0c`.

Commands executed in the ART checkout, with observed results:

- `git fetch origin master`, `git rev-parse origin/master` and
  `git ls-remote origin refs/heads/master`: exit 0; fetched and independently
  advertised master both equal the master SHA above.
- `git rev-parse --verify '0e156ae6^{commit}'`: exit 0; resolved the full
  published mapping above.
- `git diff-tree --no-commit-id --name-only -r aa7d09a498f5d6e0c5c30294155d7a4e55af6bf6`:
  exit 0; exactly seven paths: this document and the source prefix's README,
  presentation, handoff, era provenance and two new era atlases.
- A `python3` verifier invoked `git show <ref>:<path>` through
  `subprocess.check_output`, compared raw bytes, and used `hashlib.sha256`
  against `handoff.json` pins: exit 0. All seven changed postimages match at
  source, mapping and master. All thirteen payload byte counts and hashes pass
  independently at each ref, 39/39 checks. The identical handoff is 3174 bytes,
  SHA256 `a9f493017a0780d2a92d3376fbdf2c45cffe6570cd5948786a1c1283c7334978`.
- The same verifier compared all eight original PARADE inputs (source sheet,
  PARADE atlas and ground, registration, SVG, ground provenance, prompt and
  build recipe): 8/8 byte-identical across original source `70f22a68`, original
  publication `97c65b8e`, latest source, latest mapping and master; exit 0.
- `git merge-base --is-ancestor <older> <newer>`: exit 0 for original source
  `70f22a68` to latest own source, original published mapping `97c65b8e` to
  latest published mapping, and both published mappings to observed master.
  Each original own source commit (`70f22a68` and `aa7d09a4`) to master returns
  exit 1, as expected for these separately published mappings. Earlier
  "source/master ancestry" shorthand refers to mapped publication ancestry;
  it must not be read as original source commits being master ancestors.

This receipt changes only this document. Source files, metadata and media stay
frozen. No Root tests, consumer/runtime checks, build execution, image decoding,
new-era pixel inspection, browser, image generation, capture or resource
allocation occurred. New three-era visual inspection is `NOT_RUN`; the pending
36-PNG packet is a separate assignment. No resources are held. Publication byte
proof does not establish all-era visual acceptance or resolve the original
functional/live/device/public-flow obligations.

### ART-BG-MATRIX-01 actual three-era owner comparison

The preceding publication receipt is committed as
`bfd3bb0990a53d15a33d37da9bc1a059ea33459b`, pushed to the ART branch and
independently matched by `git ls-remote`; `git status --porcelain` was empty.
Subsequent fresh fetch and independent master readback both resolved
`70a444bf7cf6454f2ed4da7a635db2bb649738f2`, exit 0. Its ART document equals
the `bfd3bb09` blob exactly, and native-03 seal `4600542b` is its ancestor
(`git merge-base --is-ancestor`, exit 0).
Its `NOT_RUN` visual status records the state before this separate assignment.
Artist completed the following actual retained-image comparison at
2026-11-04T08:02:47Z. The same unfinished goal remains `blocked`.

Root's immutable native-03 seal is
`4600542ba8f5d16210075c93847fa60960809540`, under
`docs/mcr/evidence/reference-scene-v5/map-cozy-v1/native-03/` in the delivery
checkout. Candidate `aea02a262f7d834be7ef2267368812520a5d9c0c` is compared with
baseline `7c25078f118da53dd9a524d0a58e707cca190f0f`.
The capture packet SHA256 is
`e6b40ade602f886ad587d570432394aacb832c00c98e08dcb644af5a2fca6084`;
report SHA256 is
`8475506428171eb935c9be1b55c72768075b14d88b2978c175d50855b6724134`.

Artist's read-only `python3` verifier exited 0: all 41 packet artifact byte
counts/SHA256 values match retained files and `git show 4600542b:<path>` blobs;
all 36 screenshot report byte counts, hashes and dimensions match, with exact
coverage of baseline/candidate, three eras, World/Map and three widths. Each
screen is 844px high; capture times are pinned in the immutable report. The
packet and report also independently match Root's subsequently supplied seals.
The report's four candidate scenery asset pins (three atlases and shared ground)
match the closed ART handoff and candidate Git blobs, including byte counts,
SHA256 and revision; the independent Python/Git comparison exited 0.
No screenshot was copied, restored, regenerated or assembled into new media.

The two initially routed actual 390px Maps are included in that full matrix:

- REVENGE: 379545 bytes, SHA256
  `875ff6d6976ed2ed0ba8bef148a593e29d5154d8cf7254334d320dedc2152c11`,
  captured 2026-11-04T07:55:50.238Z (16:55:50.238 KST).
- DANGER: 381665 bytes, SHA256
  `d62c8eef74663ed6b41dcf4eb7faf272eb59538a0734803d8853df40b0acf1f2`,
  captured 2026-11-04T07:55:52.827Z (16:55:52.827 KST).

Artist directly viewed every baseline and candidate PNG, including all World
views and all widths, plus the unchanged user collage: 482737 bytes, 1280x853,
SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Each table entry represents an actually inspected, width-matched before/after
pair. PASS is the bounded scenery-owner comparison described below.

| Width | PARADE World | PARADE Map | REVENGE World | REVENGE Map | DANGER World | DANGER Map |
| --- | --- | --- | --- | --- | --- | --- |
| 360 | PASS | PASS | PASS | PASS | PASS | PASS |
| 390 | PASS | PASS | PASS | PASS | PASS | PASS |
| 430 | PASS | PASS | PASS | PASS | PASS | PASS |

In every era, dominant scenery looks visibly cuter than both its actual before
UI and the supplied dark collage. Pink blossom, soft gold, lime and muted teal
rounded canopies replace angular dark foliage; cream/peach facades, pink/lilac
roofs, flower edging and warm awnings replace dark brick and striped stalls.
The retained dark stage and plaque are deliberate original-era context. At
390px these large soft forms remain clear behind the frozen foreground labels
and characters; this judgment also holds at 360px and 430px.

No new cell bleed, detached sprite base, atlas registration defect or scenery
crop regression was seen in the eighteen comparisons. Existing edge cropping
and foreground occlusion remain visible in matched baseline views. The Map's
fit bands are largest at 360px, moderate at 390px and minimal at 430px; the
corrected sage/mint fill now harmonizes with the terrain instead of the former
olive bands. The 430px Map's different annotation/zoom positions are already
present in its baseline. Labels, connectors and controls remain visually
legible over the softer scenery. These are image observations, not a rerun of
interaction, geometry or accessibility tests.

Era intent was checked before judging similarity: binding `7dcbc0e2` explicitly
authorizes restrained channel recipes over the same fourteen environment crops,
shared ground, each era's own original stage/plaque and frozen geometry. The
immutable `era-derivatives.json` records half-up RGB recipes, with unchanged
alpha: REVENGE uses (92%, +20), (84%, +28), (87%, +24), named warm strawberry /
rose cream; DANGER uses (83%, +34), (94%, +16), (98%, +10), named apricot / mint /
lavender. Actual REVENGE roofs and foliage lean slightly rosier, while DANGER
greens/teals are slightly cooler. These are subtle palette differences over
shared shapes. World makes the preserved distinction clearer: REVENGE has
red/maroon stage trim and plaque; DANGER has cyan/yellow stage lights/trim and
green plaque accents. PARADE retains its neutral stage structure and plaque.
Map palettes alone do not reliably identify an era. That limitation is explicit;
the authorized derivative design does not promise different architecture or
terrain. Similarity is not evidence of a misbound atlas, and no stronger era
redesign requirement is invented by this comparison.

No source or consumer fix is required by this bounded owner inspection. Root
received the concrete observations directly. Root separately reports 640 Git
source/asset pins, unchanged six-character pixels/frame/geometry, camera and
label rectangles, zero-channel object-oracle differences and twelve successful
destination/back-focus actions. These are attributed Root checks, not Artist
reruns. Root also reports an inherited title-ring contrast of 2.41, below 3;
this receipt does not certify global accessibility or alter frozen styling.

Root explicitly fully released job `ERA-WORLD-MAP-NATIVE-202611040754Z` at
2026-11-04T07:56:07.729306Z: exit 0, 51.529 seconds, whole 54181888 bytes
including 7413760 carried ART bytes, temporary 29978624, durable 16891904,
minimum free 37368500224; owned temporary/dependency link absent and browser /
Vite children exited. Root remains sole packet custodian. Artist acquired no
resource, ran no browser/native/source-generation/consumer test, and changed
only this document. All source PNGs, originals, metadata, geometry, characters
and renderers remain frozen.

The bounded actual three-era World/Map owner comparison is complete. This does
not close human final acceptance, all-ten-screen acceptance, parent-goal,
functional/live/device/public-flow obligations or physical venue evidence.
Controlled synthetic C03/C05 map7005 fixtures remain `NOT_ADMITTED`; no live
authority or physical-device acceptance follows from these captures.

### Map4 first-view ground: source handoff

Root granted the four new files under
`frontend/public/world/map-firstview-cozy-v1/` and this document only. The
earlier conditional courtyard proposal is withdrawn. Artist reused the existing
checkout, preserved `codex/mcr-art-map-cozy-v1` at
`9187962f29f008956b58ce61a1c87ddc572ffb0d`, and created
`codex/mcr-art-map4-firstview` at freshly fetched and independently advertised
master `20682b5736838a4acbdc79e000350834cc553961`. The new branch and prefix
were absent before authoring. The original aligned goal remains `blocked`.

The supported App chooses its map from the validated authenticated bootstrap;
it does not choose the richer DEV scene automatically. Retained IN01 evidence
uses `mcr_reference_scene_r3` revision 4, geometry
`899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`,
768x1296, with quality-v6 ground and its original era atlases. The QA06 default
App courtyard fixture is a different 1152x896 revision-1 release. Native-03 is
the separate DEV v5 geometry with fixture revision 7005 and `NOT_ADMITTED`
status. Neither fixture substitutes for actual default Map4 qualification.
The completed read-only binding/label diagnosis was handed directly to Root,
World and implementation owner 26e; no art-label source repair was needed.

This source derives solely from the original Map4 ground, 531480 bytes, SHA256
`e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795`.
The original RGB pixels are retained inside the editable SVG, without scaling,
warping or changed alpha. An explicit integer RGB transfer warms olive lawn
to soft mint and stone to cream/peach. Sixty-one small flat flower/leaf groups
provide sparse lawn-edge detail. They introduce no props, silhouettes, labels,
navigation or obstacles, and are clipped outside the original paving and plaza.
The exact five immutable source pins and integer recipe are recorded in the
new provenance. The source SVG records editable filters and individual ellipses;
`paint.py` is the exact PNG raster authority because SVG engines may round
filter and ellipse pixels differently.

Map4 has 47 original paving polygons, not the later v5 set of 43. All 47
polygon coordinates and the six-point plaza parcel remain exact. The consumer
receives only ground pixels and plaza fill `#9fba96`; its existing plaza points,
stroke `#b4a27d`, width 6, draw order, original props and their positions remain
unchanged. No World resource, character source, atlas, signage, collision,
camera, foreground CSS or component was edited in this source handoff.

The pinned Map4 source binding contains three scene resource lists. Their first
resource descriptors are byte-for-byte equal: the same original ground path,
531480-byte hash above, 768x1296 dimensions for REVENGE, PARADE and DANGER.
The builder asserts that equality against the immutable binding hash. Root's
explicit shared-ground extension therefore permits the one PARADE-authored
derivative to be reused by 26e under each exact original theme tuple. Its granted
filename remains `parade-ground.png`; no separate era exports or era-specific
prop redraws are implied. Each adopted era still requires native qualification.

Source files delivered:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| paint.py | 13887 | 7a891a26893bd7cd0b1017857e77edd328683f3aa40d07517b76cafde23f3928 |
| parade-ground.svg | 747407 | a486652423d88870b06d3e050eeb00ae54c4e2734c1dc569663f5b245ab7638d |
| parade-ground.png | 469046 | ee2a1990788dc8e2d3f8e0897391fd0e95f1f29448baef58415001159ae9436d |
| source-provenance.json | 7319 | ad001c4a5b91cc6da2cc8d100281fb01395dd722e27f44dd9e34e90db7f0440d |

The new ground remains 768x1296 RGB. Its public path is
`world/map-firstview-cozy-v1/parade-ground.png`. Source checks and deterministic
reproduction passed. An independent checker verified embedded original bytes,
all 47 paving plus plaza SVG coordinates, the shared original ground in all
three scene lists, and no tracked modifications to old scenery, avatars or
application sources. Source image inspection confirms the intended warmer
surface and small flowers; it does not qualify an actual App view.

Resource21d offered and Artist explicitly accepted
`MAP4-FIRSTVIEW-PARADE-ART-202611040944Z` at 2026-11-04T09:45:09.085974Z,
with 18171756544 free bytes, 13126 carried recipe bytes and no other outputs.
The existing absolute Python 3.14.3 interpreter and Pillow 12.2.0 were checked;
no installation occurred. Actual sequential commands were the absolute Python
interpreter with `-B frontend/public/world/map-firstview-cozy-v1/paint.py`, then
the same command with `--verify`, each exit 0. One permitted within-round recipe
revision added the shared-era proof and removed a Pillow deprecation warning;
both commands again exited 0 and the PNG remained byte-identical. The separate
read-only geometry/input checker also exited 0. Final checks ended at elapsed
121.64 seconds, leaving cleanup time inside the 180-second round.

Full release was sent at 2026-11-04T09:47:40Z: four durable source files total
1237659 bytes, zero temporary bytes, exited owned Python children, no remaining
job/browser/server/dependency link, minimum observed free 18161160192 bytes.
No output resources remain held. Root and 26e own the separate actual default
App Map4 before/after captures and World-RGBA invariance job; Artist owns direct
inspection of their retained images and any concrete source findings. No extra
Artist native job or screenshot custody is acquired.

This ground alone does not close the original task. Actual default Map4 at
360/390/430, each adopted era, live feature links, canonical-place safety,
World/character invariance, human/reference acceptance and production mapping
remain explicit gates. The six-label Map4 component work belongs to 26e's
separate grant, reusing immutable glyph/font/style with its own Map4 anchors.
No FOOD/MERCH destination is introduced. Source status remains
`SOURCE_VALIDATED_NATIVE_NOT_RUN` until new actual consumer evidence exists.

### Renewed Map4 reference composition: active source iteration

The preliminary eligible default-App REVENGE390 image was inspected in Root's
custody, without copying or recapturing it:
`docs/mcr/evidence/map-production-binding/native-eligible-round01/after-revenge-390.png`,
SHA256 `9d95bbba2921685a49fb6428b98123a4dbf362abb55a701c567767b6042b2fdf`.
Its bytes were independently checked. This is an interrupted, unsealed packet;
it establishes neither the complete nineteen-context proof nor visual acceptance.
The new ground is visible and the six plaques are readable, but a palette-only
change leaves the architecture, isolated large props, broad paths, sparse gardens
and portrait composition short of the exact ten-screen reference. That finding
keeps the original visual task active; the earlier source freeze was temporary.

Root renewed this Artist's exclusive scope in the same checkout and branch:
`frontend/public/world/map-reference-composition-v1/` with exactly `build.py`,
`layout.json`, `source-provenance.json`, `{revenge,parade,danger}-scene.svg`,
matching three PNGs, and `README.md`, plus this document append. Earlier source
files stay unchanged. No second checkout, worker, native job, browser, character,
World, CSS, authority or release-manifest work is acquired.

The new complete Map illustration addresses the reference's fourth panel:
an overhead hall complex, compact flanking architecture, dense small trees and
planted edges, central/lower garden parcels and narrow diagonal path crossings.
The third WORLD panel's truss is a different depiction and remains unchanged.
The later human warmer/cuter instruction is retained through the existing warm
era atlases; the target's dark color equality is not a renewed requirement.
The reference is inspected directly, never sampled into runtime artwork.

The consumer contract is `map4-reference-composition-v1`, 768x1296, with one
selected-era complete raster painted once, then mutually pinned runtime labels
and actual selection markers. The incumbent plaza and prop pass is skipped in
this explicit mode. All three original Map4 tuples, original source pins,
per-era output descriptors, six semantic anchors with 112px-high exclusions,
and four separately mapped canonical display anchors are explicit in the new
layout. These display coordinates do not mutate World cells, UUIDs or authority.
Service labels carry no canonical identity. Consumer26e owns fixed semantics,
font/icons, four feature links, selection, hit areas and atomic version/failure
behavior in its newly granted v2 prefixes. The original v1 native candidate
remains independently frozen.

At authoring time only the editable builder and README exist; generated output
and source checks require a fresh accepted Resource21d slot. Actual App captures,
reference convergence, all-era/native behavior and World invariance remain open.

### Direct owner inspection: eligible Map4 native round02 retry01

Artist directly opened all nine retained AFTER images (REVENGE/PARADE/DANGER
at360/390/430) and the separate PARADE390x420 short image in Root custody.
No image was copied, cropped, rewritten or recaptured. Candidate is
`4fa9b84b1ffe20c0f4793e73ab72247e77830649`, with Root seal `4cfd4199`.
The packet remains **FAIL**, not a passed native release: its final cross-pair
camera assertion sampled differing resize phases, including heights663/791
and corresponding scrollY92.5/28.5. Root's subsequent driver-only stabilization
is a new pending proof; it does not retroactively pass these retained results.

The inspected images are under
`docs/mcr/evidence/map-production-binding/native-eligible-round02-retry01/`:

| Actual image | SHA256 |
| --- | --- |
| after-revenge-360.png | 2ce49b5dfa9dca2ff8213e07ef02d8d125564b567d380d45a2931e2c5cda2153 |
| after-revenge-390.png | 9d95bbba2921685a49fb6428b98123a4dbf362abb55a701c567767b6042b2fdf |
| after-revenge-430.png | c7d5277c51146ff6e4e233ce64162e0003ff6386a5217d397b8d8f4c8e843eaf |
| after-parade-360.png | 7d654a394f9fc263f5059b6683f93150e27db514eaaee0e98389bd21aa9abc88 |
| after-parade-390.png | 7904605a06e7d3a3665c538188d78a9fc2a7ebc2dd7c149329a8f340e015c241 |
| after-parade-430.png | ad419612cf4ee0a066c63c7fdd2198a718fff16199f74c3669525f07be57d8d7 |
| after-danger-360.png | 07fd1cd813f2bd2a3dc05207ebb7f864e7a2505f27ed48a47d523c5de1513b23 |
| after-danger-390.png | 4bacac54f2e85202280f85bdfdad421406e8282f842e751970982b0842a2ace0 |
| after-danger-430.png | 928cdca36cc1ac2fdbfb42075943f843b5751115c618fcd166db18e7711b3fd6 |
| after-parade-390-short.png | 1f7d58133352d880455a2830bf3e4ce1654cd0a2cc8481274d13f388ec3fcc1e |

Independent read-only SHA checks also pin `report.json` to
`bbce6085418f4fe9398022bc991e202591c16274d986ad3afb3758508223c721`
and Root's `canonical-native-eligible-release02.json` to
`671d7266bc77c17e25f4348eb11690610daf405016338bf582b2e380dec618f9`.
Its qualification is an explicitly eligible attendee contract fixture exercising
the actual default client; `productionAdmitted` remains false. This is not an
actual operator response, live admission, backend availability or device claim.

The visual disposition is a bounded rendering improvement, with overall
reference convergence still **IN_PROGRESS**. Warm mint lawn, cream/peach paving
and small lawn flowers are visible consistently. All six plaques are readable
and within the full-height viewport at all nine era/width combinations; no
obvious missing source texture is visible. Distinct original era building,
banner and counter colors remain visible. These images show no new source
corruption requiring a repair to the frozen first-view ground.

They also confirm the remaining source/composition findings: standalone broad
buildings and large separated canopies do not create the reference's denser
overhead architecture and planted boundaries; Plaza and lower lawns remain
sparse; broad angular paving and large open areas lack the reference's garden
rings and narrow diagonal crossings. Empty flat mint bands above/below the
scene vary with viewport width and are especially large at360px. Those bands
need an explicitly revised Map framing/fit behavior in the consumer, rather
than scenery pixels pretending to be interface or viewport fill. The new
full-composition source iteration remains necessary and is not accepted by
this historical packet.

The390x420 image visibly retains the two-line virtual-map/no-physical-arrival
caption and bottom controls. Some labels are outside that saved short view;
the image alone does not show all six simultaneously. Independently reading
the report finds ten AFTER contexts, each with four virtual actions, successful
keyboard/Back focus, minimum target height44px and width66.359375px. Those are
retained driver observations, separate from visual inspection and the failed
overall camera gate. All nineteen contexts record disposed state, zero active
sockets and zero canvases after cleanup.

Independent pairing of the nine matching full-height BEFORE/AFTER report rows
confirms non-null and equal actual World RGBA hashes, World object arrays and
original map identities in every pair. It does not resolve the camera sampling
failure. The full-release receipt records no owned process/link/temp tree left
at2026-11-04T11:40:18.062057Z. Artist held no output/capture resource for this
inspection. Parent goal, complete visual convergence and the new source's
actual App qualification remain open; no King/SL gate is introduced.

### Complete Map-only source: first rendered candidate and own findings

The fresh offer `ART-MAP-REFERENCE-COMPOSITION-202611041157Z` was explicitly
accepted at2026-11-04T11:57:54Z in this existing checkout at `fd8facea`.
The exact17772-byte builder SHA was
`5838f2ff710838e7c07fc6db636afc2b3958242319ad0f93a0465106055d0a82`;
README2405 bytes SHA
`e7f284d948a1c2eafdcd6137b52b1f8772ebf56feb02dc4832dfee8d9a548aff`.
The inspected original aligned goal remains BLOCKED, without a reset.

The actual guarded commands were the existing absolute interpreter
`/Library/Frameworks/Python.framework/Versions/3.14/bin/python3 -B`
with `frontend/public/world/map-reference-composition-v1/build.py`, then that
same script with `--verify`. Python3.14.3/Pillow12.2.0 were checked before the
two sequential children. Both exited0; combined child execution0.977seconds,
last work finished11:58:32.987032Z at38.987seconds from ACCEPT. The supervisor
enforced the150-second work cutoff, whole/durable caps and continuous free-space
floor before/during children. No browser, native capture, imagegen, dependency,
server, new checkout, temporary file or source-original mutation occurred.

Nine original input hashes, four exact original interaction cells, six112px-high
label exclusions, unique noninteractive placement IDs and three distinct pinned
era atlas inputs passed. Build and verification reproduced all generated bytes.
All three PNGs are768x1296 RGB, without characters or service lettering. Each
SVG contains the exact retained atlas plus editable surfaces and named crop
placements. These are source checks; SVG raster parity and actual App use are
NOT_RUN.

First candidate pins under the new prefix:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| revenge-scene.png | 606047 | c47093d9f03af40bed5bafca726ded9bb052b0eb7af0d765b25089a23edb1649 |
| parade-scene.png | 624680 | 757ff214e166b1cdca807e7786c8a4cf56014c8526f5fdde6b32ef25d7aa6098 |
| danger-scene.png | 612589 | abfb43f5b85c64d691d1c04dcfd5bb33f76462bf0e492c52244e2ecb282bc9ca |
| layout.json | 30448 | 450494070edbbbc1983e90aaf98844f2bafdc48fb15c12a35558e036ad23bdde |
| source-provenance.json | 21783 | 415c22e097ff83909518f406879f7fb9c8a77ff3b968bf315b17a1a4d9b77fda |

Full release was sent11:59:05Z, with all children and supervisor exited and an
independent exact-process check empty. Ten durable files total7012863bytes;
temporary0; conservative full retained-source carry8683520bytes; whole charge
15741742bytes including65536 proof reserve, below25165824. Initial free space
19560452096bytes and minimum19553435648bytes exceeded both start/floor gates.
Cleanup occupied the remaining interval before release, well within180seconds.

After release Artist directly opened all three actual saved source PNGs and
compared them with the unchanged exact reference. The multiwing upper hall,
two garden rings, diagonal crossings, smaller canopy scale and continued warm
palette are visible. The source self-assessment also finds two actionable
defects: garden borders remain sparsely planted, and the three era scenes read
nearly identically despite distinct original atlas hashes. The retained warm
facades remain more front-facing than the reference's overhead architecture.
The candidate is preserved as a source checkpoint, not accepted visual parity.
Root and26e were told to hold final asset binding/publication while Artist
prepares a same-prefix density/era-vector-color correction under a fresh slot.
Original atlas RGBA, World and character inputs stay frozen. Runtime labels,
framing, actual all-era mobile/client behavior and complete reference convergence
remain required after the source correction. No reviewer ACK gate is introduced.

### Eligible V1 round03 retry01: bounded proof passes, visual work continues

Root's separate corrected packet now records raw **PASS**, candidate
`be3fd627c248b9ed2ad34bf2a8641d18dc05f46d`, seal `48539248`, under
`docs/mcr/evidence/map-production-binding/native-eligible-round03-retry01/`.
Independent SHA checks match `report.json`
`36374fe3bb205760fda990436275458bffe70cca7d5424892e07aaeef2a86ac7`
and `artifact-index.json`
`71ac304d7b4f3d66bff367965d11adde7d162427309c070d02b0175dbed571bb`.
The first round03 probe's failed render-mount selector is retained separately;
the new passing run does not relabel earlier failed packets.

Artist directly reopened all nine actual saved AFTER era/width images and the
short PARADE390x420 image, plus `before-parade-390.png` SHA256
`f4af5febca9fa7ad9985fd1c563fa7be5731ac754bf26de4bac2ce46a5283eb2`.
All ten AFTER PNG SHA values independently equal the ten round02 retry01 pins
listed above. Visual appearance and the previously recorded source/composition
findings therefore remain unchanged. The visible before/after confirms the warm
ground/flower and semantic-label change; it does not turn the sparse original
architecture, broad paths, incomplete planting or viewport bands into a match
for the reference. All six labels remain readable in full-height captures.
The short image retains caption and controls, with some labels outside its
current view. No screenshot was copied or regenerated for this assessment.

Independent JSON pairing finds nine non-null matching World RGBA hashes,
identical29-object arrays, original map identities, settled World cameras and
settled Map-host cameras. The report contains nineteen contexts and forty
actions across ten AFTER cases. All forty record44px minimum target height,
minimum width66.359375px, successful keyboard/Back focus and exactly one active
socket, one constructed socket and one canvas with the same camera at the
destination. Every context records disposed state and zero canvases/active
sockets after teardown. These are bounded retained driver observations; Artist
did not run a second browser or independently replay those routes.

Root fully released this packet at2026-11-04T12:05:05.929783Z. The qualified
scenario remains an explicitly eligible attendee contract fixture exercising
the actual default client, with `productionAdmitted:false`. Source checks for
the exact short-caption hunk remain Consumer26e's separately queued workflow.
Bounded V1 client/resource/invariance proof is now complete; the new complete
Map composition, reference convergence, live authority, physical devices and
human final acceptance remain open. The original goal is not reset or closed.

### Published V1 readback and returned ground lease

Artist independently fetched master and separately queried the remote after
Root's bounded V1 publication `40bbf457158ef506b8b49b0b4593152e6c07bd28`.
Both reads resolved `d62ce6979d6a3326f2da7271c3ae1eba54c6a22f`; the publication
is an ancestor. All four old `map-firstview-cozy-v1` files exactly match original
source `6bb85075`, published commit, named master and Artist's checkout. The
2684-byte actual PASS append from `45b0d40e` is conserved verbatim, SHA256
`5a8d199bc347357c64c60a843395a235376216a3b127dc552007727f45126cbf`,
as is the4590-byte prior actual-image assessment from `fd8facea`. The held
`46180268` complete-scene source and its first-candidate section were absent
from that published master. Artist returned the OLD
`frontend/public/world/map-firstview-cozy-v1/**` writer lease directly to Root
and26e after this proof. The NEW composition prefix and necessary ART document
scope remain active; no new write to the returned prefix is implied.

### Corrected complete-scene source: density, era cues and overhead roofs

The first candidate remains immutable at
`4618026884c1b9a846fcd2a46567ae26340bb75f`. Its direct source findings were
addressed only in the same new composition prefix. No previous source/evidence
version was silently relabeled or deleted.

The correction adds51 named placements, for128 total. Small shrub-bed fragments
form denser inner borders around both garden rings, with additional small trees
on previously open lawns. New original vector surface/roof palettes distinguish
REVENGE rose/peach, PARADE cream/lavender and DANGER butter/sage while retaining
the warmer/cuter direction. Retained original atlas RGBA is never recolored.
For the four venue frame types, exact bottom46percent facade fragments retain
the original doors/windows/planting. Original editable roof planes replace the
omitted upper54percent only in this independent Map illustration. Full-frame
and used-facade source rectangles/RGBA hashes are explicit in provenance.
This is an authored Map-only view adjustment, not a changed World sprite,
geometry, manifest, character, place identity or physical-route claim.

Fresh offer `ART-MAP-CORRECTION-202611041238Z` was explicitly accepted at
2026-11-04T12:39:23Z on `45b0d40e57d6a2336cb93cff29fd9f7c1b4c3905`.
Frozen builder22329bytes SHA256
`aae07bd39c9c2d1bfa38321645b815f459b485d5a0cf9951ba7d6545d76447a7`
and input README3117bytes SHA256
`c539573eaea538bf66eb9abe025c04bf6de538d0ade7df569f10c454cb083a06`
were checked before execution. Post-release README wording records the completed
source checks and remaining limitations; it does not change generated pixels.

The existing absolute Python3.14.3 interpreter with `-B` ran the same `build.py`
then `build.py --verify` sequentially using Pillow12.2.0. Both exited0, combined
child execution0.988seconds, last work12:39:24.473004Z at1.473seconds from ACCEPT.
No fix retry or generation/browser/native/dependency/server/TMP job ran. The
supervisor enforced original deadlines, one live child, byte caps and free-space
floor before/during both children. Nine immutable source pins, four exact World
cells, six112px-high label exclusions and closed ten-file output scope passed.
Verification reproduced all eight generated files exactly.

Corrected source pins, all PNGs768x1296 RGB:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| revenge-scene.png | 508321 | 9a9a117a18159c422354c1967957dc0c6f1f82554055a2c88a29aa9f80951b06 |
| parade-scene.png | 522044 | c14b06dd0763fef26746cb6f7b7b5fcc24d482272251c844a1365a0f4aef5269 |
| danger-scene.png | 512256 | 58b3a9a8b71cd02d044a68fb596bf5cbbf5a383fdbee2eb2706fc8d2dc5527e0 |
| layout.json | 45370 | 418af3bf14ee2e8b79eeb2b038f64b48713de17967993aa4e7fa3ce22c8d26b9 |
| source-provenance.json | 26589 | 75a6391415528f2dd9a583aec51e74e6a35c9817c1c5985cb9d03477c080f338 |

The exact source binding, three original Map4 tuples, six service centers and
four canonical display mappings are unchanged from the agreed interface. Their
source/cell identities remain separate from virtual semantic labels. The
complete-scene consumer must pin the new raw layout/image/style/font identity
atomically, load one era, skip the old plaza/prop pass, then draw runtime labels
and actual markers. An old/mixed image or presentation identity must not acquire
new hit targets. No source UUID or FOOD/MERCH navigation is introduced.

Full release was sent12:40:06Z after every child and supervisor exited; an exact
process check was empty, with no temporary/cache/output handles left. Ten files
at release totaled6782330bytes, generated payload6756884bytes, temporary0.
Whole accounting32013690bytes includes25165824 conservative historical/source
carry and65536 proof reserve, below41943040. Initial and minimum observed free
space17364922368bytes exceeded the16GiB start and8GiB continuous floors. Cleanup
finished inside the original180-second total window; no lease remains held.

After release Artist directly opened all three corrected saved PNGs alongside
the exact supplied reference. The source defects are visibly reduced: both
rings have populated borders, era palettes are distinguishable, and the hall
has a higher-angle roof-dominant silhouette instead of the first candidate's
tall front elevations. This is ready as a corrected integration candidate.
It is not complete reference fidelity: roofs remain schematic, planted beds
repeat, edge architecture and lawn density remain less intricate/more open
than the reference. Warmer/cuter palette remains intentional. Actual runtime
labels, markers, short/mobile framing and the complete client view still need
Root's canonical captures and this owner's direct assessment/fixes. SVG/native
raster parity is not asserted; Pillow PNGs remain the raster authority.

Source status is **SOURCE_VALIDATED_NATIVE_NOT_RUN**. Consumer26e's independently
qualified115 tests/types/lint establish its unbound full-mode foundation only;
it must explicitly intake these corrected pins before actual V2 qualification.
V1's passing nineteen-context packet does not qualify these new scenes. Full
reference convergence, World/character invariance in the adopted V2 client,
live admission, physical devices and human final acceptance remain open.
The original aligned goal stays BLOCKED; no King/SL/Main technical ACK is needed.

### Reference correction02: source-authoring checkpoint, outputs unchanged

The existing ART checkout resumed on the new branch
`codex/mcr-art-map-reference-correction-02`, based on clean published own handoff
`8fcd1287496aedbde12ef0d2e426855f8f8ec4d6`. Both that handoff and first candidate
`4618026884c1b9a846fcd2a46567ae26340bb75f` remain immutable. The authoring grant
covers only the existing `build.py`, `README.md` and this ART document. The eight
generated files remain byte-exact8fcd1287; Root and26e retain that separate intake.
FE19 explicitly confirmed its frozen source04 candidate is not implicitly swapped.

The exact reference and saved correction PNGs were inspected directly again.
Its fourth panel depicts a compact overhead garden/campus with connected planted
edges. The saved source still has smooth, four-band roof planes above detailed
pixel facades, repeated identical bed rings, and extensive open lawn. The warmer
palette remains intentional; the third panel's World truss stage is not a Map
architecture target. Existing source evidence is not relabeled as a new result.

The new internally versioned recipe is `reference-correction-02`; the consumer
layout schema/version remains compatible. The facade's source rectangle remains
the exact bottom46percent of the original frame, now explicitly projected with
nearest vertical factor0.65. Its nominal destination height is29.9percent of the
fitted slot, down from46percent. The authored roof spans68percent instead of53,
with2px coordinate steps and staggered6x3px tile clusters on an8x6px grid. Four
roof-wide stripes are removed. These are defined projection dimensions, not a
claim that the resulting roof is visually accepted. Provenance will distinguish
the projected destination from unchanged full-frame/fragment source RGBA hashes.

The regularly spaced ring-bed loop and ten additional ring beds are removed.
Fifteen irregular planting footprints cover391284 gross polygon pixels before
overlap, clipping and clearance. This shoelace-area measurement describes authored
geometry only; reference vegetation coverage was not measured or invented.
Raster planting must clear the exact six label rectangles,78px path corridors
and42px garden edges. It will report actual painted ground pixels before scenery
occlusion, independently of tree rectangles. Seventeen unequal tree groups use
five differently sized/offset crops, primarily green/dark, to connect these
masses. Seven standalone beds remain as local accents. No icons, actors or new
navigation semantics are added. Exact label exclusions, all four canonical
display mappings/World cells, nine original input pins and three era tuples
remain unchanged; no client geometry dependency blocks this source work.

Read-only AST parsing passed without importing Pillow or executing the builder.
All eight saved generated files were independently compared against8fcd1287 and
remain exact. The new source is **AUTHORED_NOT_BUILT**. No new PNG/SVG/layout/
provenance output, browser/native/client process, output allocation or dependency
installation has run. A fresh named finite OFFER/ACCEPT must precede a sequential
build and deterministic verification. Its new output pins will be handed off
as a separate candidate only after those checks and direct saved-image review.
Existing validated8fcd output is not evidence that correction02 passes. Root's
actual adopted V2 captures, mobile/short framing, World/character invariance,
feature selection, live/device work and human final acceptance remain open.
The original goal remains unfinished; no reset, new budget or review ACK gate
is introduced by this checkpoint.

### Correction02 allocated output and direct source assessment

Fresh finite offer `ART-MAP-RECIPE02-SOURCE-A-202611041349Z` was explicitly
accepted at2026-11-04T13:52:35Z on clean source checkpoint
`50adff5f1baeefbc246291cbfd7543fa8e3a4f7d`. Work cutoff13:53:35Z and independent
cleanup hard cutoff13:55:35Z were fixed at ACCEPT. Frozen recipe27369bytes
SHA256 `b5349d3e415826b44b83df3348da22e6be5cccdfdb15c955a6ad02c6054df7a8`
and input README4851bytes SHA256
`e7444de70718c3b92a08e440c27e7632efb73f40f4b0c17cc4ea14623316f4fd`
were verified before execution. The post-release README records this result and
does not alter any generated postimage.

The existing absolute Python3.14.3 interpreter with Pillow12.2.0 ran `build.py`
and then `build.py --verify`, both with `-B`, in one sequential child each.
Build exited0 in0.558seconds; verification exited0 in0.471seconds, reproducing
all eight files byte-exact. Final child work ended13:52:36.972146Z. No scoped
retry, image generation, browser/native/client process, installation, PostgreSQL,
new worktree or shared cache/profile mutation occurred. A separate supervisor
polled the child's deadline, disk floor, declared scope, byte and memory caps;
its short process guard plus optional exec shell gave an ALL-owned bound of4.
Maximum sampled supervised RSS was111050752bytes, below512MiB. The supervisor
could kill the child independently at the fixed work cutoff.

Fresh task-owned carry accounting was29449863bytes, conservatively including
both preserved candidate packs13795671bytes, the entire retained warm Map-cozy
pack7388127bytes, returned ground1237659bytes, duplicated immutable input reserve
6845786bytes, ART document117084bytes and proof reserve65536bytes. The32MiB
carry allowance covers that sum. Unrelated pre-existing read-only repository
reference packages were not copied or included as new derivative-job carry.
At release the ten-file prefix totaled7973214bytes, of which generated output
was7940994bytes. Whole accounting41593182bytes included33554432bytes conservative
carry plus65536bytes proof reserve, below50331648bytes. Temporary output was0.
Initial free30629531648bytes and minimum guarded free30628261888bytes exceeded
the16GiB start and8GiB live floors. The final process scan at13:53:37.273528Z
found no live builder. Every child/supervisor had exited, with no TMP/cache,
server, output handle or lease held. FULL RELEASE was sent13:53:37Z, within the
original180seconds. Post-release text documentation and Git handoff are separate
from the completed finite output work.

Corrected PNGs are768x1296 RGB. Exact generated postimages:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| revenge-scene.png | 556834 | 18da9a4394c070769626c2f4ef66dbe062296ca308d92c1794b0a927f499d87f |
| parade-scene.png | 571425 | 938f4956ad9bb35fa4f5569607a864dda22f5588f3cf75698e8facfd3d3584da |
| danger-scene.png | 563539 | 15c2434779690d5190cacec620b83c043090410ce6fe2dc9e1e3c4b8bcdb305d |
| revenge-scene.svg | 2041648 | 560c6189eb4510dbb96deda78b1b5c3db19b401c7aa01d1bb89edeafda872902 |
| parade-scene.svg | 2074096 | 76a57b06a4b7ce5e669512898d1355b71e513b0e6c00a15eba3aa7c9f2947981 |
| danger-scene.svg | 2043860 | 09060111093989821d475c9b22b9f494ffe30aa7a5310b7101cf2da3cc8f163a |
| layout.json | 57128 | 20e12420bc234a5d65cf23c6dde1db6ef9657de467f7f287ff39dc446d0e28ae |
| source-provenance.json | 32464 | 897ff08630b8484d39e26caa73e3e4c17e8f86dd6aa6cd01e50bed7b2b89fd03 |

All nine actual source pins were independently rehashed before work and again
after output. Provenance output descriptors matched their files. Deterministic
source checks retained exact source crop RGBA, original four World cells,
six112px-high hit exclusions and the three era identities. Layout exposes
`recipeRevision: reference-correction-02`; original schema/version is unchanged.
There are155 placements. Actual planting is174672 painted ground pixels per
era before scenery occlusion, about17.55percent of the canvas; this is not a
reference similarity score, actual vegetation silhouette coverage or client proof.

Artist directly opened each saved REVENGE/PARADE/DANGER PNG and the exact supplied
reference. The repeated bed rings and four continuous roof stripes are gone.
The higher roof/facade ratio and denser tree groups are visible; warm per-era
palettes and all reserved label areas persist. However, the new8x6 ground marks
read as a conspicuous checker pattern rather than natural planting. Several
planting boundaries are hard stepped shapes. Large hip roofs still dominate
shortened facades, and open lawn remains pronounced around reserved label space.
These are concrete remaining source defects, not an accepted final match.
No claim of native/SVG parity, actual labels/markers/mobile framing or human
visual acceptance follows from reproducibility. No retry ran after the original
work cutoff. Further recipe work may proceed in the text scope, with new output
requiring another fresh finite offer.

This separately pinned candidate is **SOURCE_VALIDATED_NATIVE_NOT_RUN**.
FE19's qualified120-test/18-file8fcd intake remains separate and is not swapped
by this handoff. Root retains canonical V2 capture custody; saved adopted output
must return directly for ART assessment/fixes. World/character invariance in the
adopted client, feature selection, live admission, physical devices and human
final acceptance remain open. The existing goal is unfinished; no reset or
technical/King/SL ACK gate is introduced.

### Actual adopted V2 readback, publication proof and correction03 authoring

Root's immutable canonical packet is sealed at
`0287ef191525444fb8ffe9f2be4f733253fcb9d9`, frozen runtime source
`6b11faca8855e19ef16c4198de3c1c8fdf3a3dfa`, retaining ART8fcd1287. Raw report
SHA256 is `05e1df3a064b753c4b3a8e9b68f97d77ace91915282cc64e9087145661cd2c47`.
Artist read the packet in place under Root's
`docs/mcr/evidence/map-production-binding/native-composition-round01/`, without
copying captures, starting a browser, building output or taking capture custody.
Directly opened all nine after-era/width images, after-PARADE390-short,
before-PARADE390 and the exact supplied reference. All nineteen before/after
saved PNG hashes were independently recomputed against the report, including
the eight other before images not directly opened in this assessment.

Independent report readback verified nine exact before/after World RGBA hashes,
all29 display objects, canonical Map identity, settled World and Map cameras.
The forty actual virtual routes have minimum66.359375x44 targets, successful
keyboard/back focus and one canvas/constructed socket/active socket with the
same camera. All nineteen contexts disposed to zero canvases/active sockets.
Every context reports local Zoom/Fit/Reset and camera preservation. All after
presentations match digest
`9fcddca91030aaff0f3c3efaa1018fdeddb4708c56c9eea80024deb0bee9f522`.
Root's full application types/syntax/native commands passed and FULL RELEASE
was14:03:18.639003Z. Those execution/release results belong to Root's packet;
this owner independently inspected its saved evidence, without rerunning it.
Qualification remains the explicitly eligible attendee contract fixture with
productionAdmitted=false, not real operator/live admission or physical devices.

The actual adopted composition forms a coherent warm scene with readable six
service labels, stronger planted parcels and distinct rose/lavender/sage roofs.
It improves on the retained before view. It does not finish the reference match:
the pale diagonal paths and garden borders consume large areas, sparse lawn
persists between labels/edges, identical flower beds trace regular rings, and
flat large roof planes dominate the hall silhouette. The exact fourth Map panel
has more compact architecture and connected irregular planting. Correction02
removes the rings/roof bands but adds a conspicuous checker substrate; it remains
separate and was never represented as the adopted capture.

The360 full-height view has visibly larger top/bottom plain-green bands than430.
Source inspection of `IllustratedMap.tsx` and its `reference-scene/viewport.ts`
shows centered presentation sizing and local scrolling, separate from the World
camera. The390x420 view intentionally crops the tall illustration and preserves
the caption/footer/control area; not all six labels are visible at once. Label
paint is traced to the v2 model's78px source rectangle and retained font40/icon55
style, while its target function independently enforces44px minimums. These
explain substantial visual plaque area without licensing smaller hit targets.
Artist recommended tuning painted labels independently of hit targets to the
Map/CSS owners, retaining all112px source exclusions and actual short scrolling.
No client, CSS, runtime label, canonical marker or production World file is edited.

Publication was verified independently, not inferred from Root's message.
Fresh fetch and a separate `ls-remote` both resolved master
`181f35667a30393879c0ddcec26fc2c762281e98`. Bounded Map publication
`7a1ff7081df9f021c95ad740d259e8c52c08fbad` is its ancestor. All ten ART postimages,
total6782808bytes, compare byte-exact between own8fcd1287, that publication and
the freshly fetched named master. Comparison manifest SHA256 is
`8d877df20e799c14aa8b4755600c8bd4e6b6b27fd59df32a97529eebd14cab01`.
Correction02 commit1f2f9ff6 is absent from named master ancestry; there is no
silent swap. Both earlier candidate/source histories remain preserved.

Within the same three text paths, the next recipe is authored as
`reference-correction-03`, based on immutable candidate
`1f2f9ff6bac13467d76c430ff67a26e51b421727`. Only `build.py`, its README and this
ART append change. All eight generated outputs remain exact1f2f9ff6, preserving
their provenance even though the new recipe is deliberately not built yet.

Concrete authored corrections are46/40/28px outer/middle/core path widths instead
of62/56/40, and22px garden borders instead of34. The seven centerlines and two
garden polygons remain unchanged. Checker tiles are removed in favor of one
continuous low-contrast substrate with sparse4x2px flecks. Original upper76percent
green/dark tree fragments form123 named canopy placements, uniformly nearest
fitted without recoloring/retouching source RGBA. Future provenance records the
exact full-frame and canopy-fragment source rectangles/hashes. Circumcircle
clearance keeps canopy rectangles off60px path corridors and30px garden borders;
they also clear architecture and the exact six label rectangles. Existing crop
roles, display points and authority remain explicit/noninteractive.

The roof span drops from68percent to57percent of the fitted slot. The exact
bottom46percent facade source is now vertically projected by0.80 instead of0.65,
raising nominal facade height from29.9percent to36.8percent. Quantized roof tile
clusters remain. This reduces the roof/facade imbalance instead of adding more
surface symbols. These are authored dimensions, not measured reference coverage
or proof of the resulting appearance. Warm era identities are retained.

AST parsing and pure read-only geometry evaluation, without importing Pillow or
executing the output builder, found278 placements including123 canopies. Exact
six label exclusions pass. All nine input-pin expressions, era tuples, canonical
cells/mappings, dimensions/layout version, path centerlines and garden polygons
remain unchanged. All eight saved outputs still match1f2f9ff6. New crop bounds,
actual substrate coverage, PNG/SVG byte caps, deterministic outputs and direct
image assessment remain NOT_RUN pending a fresh finite21d output offer. No
allocation/job/lease is held. Actual integration/native checks on correction03
and full visual convergence remain open; Root alone retains capture custody.
The existing goal remains unfinished; no new goal/budget or review ACK gate.

### Correction03 finite output, saved-image findings and released resources

Fresh finite offer `ART-MAP-RECIPE03-SOURCE-A-202611041417Z` was accepted at
2026-11-04T14:18:56Z on clean source checkpoint
`c06677372aa4b4a1be589273366e217d5957b558`. Original work cutoff14:19:56Z and
independent cleanup hard cutoff14:21:56Z were fixed at ACCEPT. Frozen input pins:
recipe31720bytes SHA256
`a58fb7a47e8c9f9f7692041f998961b1e9bb1cd3e4c9abeafa5d0eb61b1f46b4`,
README7939bytes SHA256
`3f15084f105eae712d2f53cb9bf2805f39eabf6c1c3cb100e2823484acadbb6a`, and
ART document128987bytes SHA256
`b79fe72b13fe2d40445da02bd5f3179247599b9b84b2e4ab80f952878fc45a2a`.
Full HEAD, clean status and all nine actual immutable inputs were checked again
before the child commands. Post-release documentation records results without
changing the frozen recipe or generated postimages.

The existing absolute Python3.14.3 interpreter/Pillow12.2.0 ran `-B build.py`
then `-B build.py --verify`, exactly once each. Build exited0 in0.615seconds;
verification exited0 in0.504seconds and reproduced all eight outputs byte-exact.
Final work ended14:18:58.346638Z. Both commands emitted the same nonfatal
`Image.Image.getdata` deprecation warning, identifying eventual removal in
Pillow14 and recommending `get_flattened_data`. The warning is retained as a
known maintenance limitation; it did not fail the offered Pillow12.2 commands.
No retry, hidden failure, output mutation after cutoff, image generation,
browser/native/client/Node/PostgreSQL job, installation, new worktree or shared
cache/profile mutation occurred.

A separate guardian checked original deadlines, free-space floors, declared
scope, byte caps and sampled process memory before/during the single sequential
work child. It could kill that child independently. ALL-owned process bound4
included guardian, one work child, short process guard and optional exec shell.
Peak sampled supervised RSS120553472bytes plus16MiB guard/shell reserve stayed
below512MiB. Fresh accounted old carry37435756bytes plus an additional1MiB
source-Git/text/failure-proof reserve totaled38484332bytes, within40MiB. All three
historical candidate packs and original source/proof inputs were preserved.

Generated bytes totaled7738228; ten-file durable bytes7777887. Whole accounting
49786463bytes included41943040bytes conservative carry and65536bytes proof
reserve, below58720256bytes. Temporary output was0. Initial free28529123328bytes
and minimum guarded free28517769216bytes exceeded16GiB start/8GiB live floors.
Final cleanup measurement14:20:25.596829Z had28526575616bytes free and no live
builder in an exact process scan. Every child/guardian had exited; no temporary
file/cache/server/output handle or lease remained. FULL RELEASE was sent
14:20:25Z, within the original180seconds. Later text/Git handoff is separate from
the completed finite output work.

Saved PNGs are768x1296 RGB. Exact generated postimages:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| revenge-scene.png | 677852 | b503fde72ebd8c46181609cd1256809bd58eaf34b24f8811781246abbef675bc |
| parade-scene.png | 697466 | 0a25503095d2ed62960e5ff23731a35d87159301e9a3993a12ead671c057ac06 |
| danger-scene.png | 686555 | 8cc354d6666e48d183173bc73ed5c459286bdc8e28a11cce3bc2bdb0f852fbd1 |
| revenge-scene.svg | 1834992 | 6deee6f006ba023d1e9bf28a284e4a7d3a63fec0948ec1847bd21bfa252c2e66 |
| parade-scene.svg | 1867440 | dc7447be761db0719746f2aa2afedaff85ead22955c99f70a526a53d71b70117 |
| danger-scene.svg | 1837204 | 431a705f83865ddd25b33df4ee4940d055c8a29d8204edbd216c0ea09aec0b8b |
| layout.json | 101203 | 3dd7c80f6612c83da97aaae4733d236acbfc8057c18d65b6f8c9d0f40dad6792 |
| source-provenance.json | 35516 | 95605459793ab950f711791de3094daf644c4114406efa0ed9a86380d163135d |

Provenance descriptors were independently checked against all generated files;
the nine actual source inputs were rehashed again after execution. Assertions
retain full-frame/used-facade/used-canopy source RGBA pins, four exact World cells,
six112px-high source label exclusions and original era/Map4 identities. There
are278 placements including123 canopy fragments. Painted substrate is232892pixels
per era before foliage/scenery occlusion, about23.40percent of the canvas. This
is a ground-mask count, not actual vegetation coverage, reference similarity or
evidence of client behavior.

Artist opened all three newly saved PNGs and the exact supplied reference in
place. The narrower diagonal paths/garden edges are visible, the checkerboard
is eliminated, original foliage groups add useful density, and the reduced roof
depth/restored facade height improve balance. Warm rose/lavender/sage era cues
remain readable. The new continuous substrate still reveals angular parcel
boundaries, including flat edges around the reserved label rectangles. Some
upper76percent tree fragments show flat lower edges and row alignment. Large
muted roof planes and broad reserved lawn remain less compact/intricate than
the reference. These are concrete remaining ART defects, not final acceptance.

This is a separately pinned **SOURCE_VALIDATED_NATIVE_NOT_RUN** candidate.
Neither canonical8fcd nor correction02 is silently replaced. A consumer must
explicitly register the new layout/image/provenance identity and perform its
source checks before Root's next canonical capture. Existing19-context native
proof applies to8fcd only. Actual03 mobile framing/labels/markers, World/character
invariance, SVG/native parity, live/device evidence and human visual approval
remain open. Root retains exclusive native-capture custody. The existing aligned
goal remains unfinished, with no reset/new budget or King/SL/technical ACK gate.

### Correction04 targeted source checkpoint: natural planting boundaries

Continued from published immutable ART03
`94c47a8b3219efc76a4be9646014cfeaad2f1957` in the same isolated checkout/branch.
The same three text paths are the entire authoring change: builder, README and
this ART document. All eight generated files remain byte-exact94c47a8b. The
existing FE19 ART03/compact-paint source05 qualification remains untouched; no
consumer registration or saved native packet is swapped. Existing sealed actual
V2 packet0287ef19/report05e1df3a continues to describe8fcd only. The separately
source-qualified03 scene/paint requires Root's own fresh canonical captures.

Artist reopened saved03 PARADE and the exact supplied reference before editing,
retaining the earlier all-era source and actual native assessment. The source
shows the causes of the two owned defects directly: colored substrate is pasted
through angular parcel polygons and rectangular label cut-outs, while the tree
frames are cut at76percent and laid out by row/column. The targeted correction
removes that substrate paint entirely. It keeps the existing textured lawn and
garden ground beneath planting; no new colored parcel edge can appear. Parcel
polygons remain in metadata solely as invisible placement domains. The future
ground-painted pixel count is0 by design, not a vegetation-density claim.

Cluster trees retain100percent of the exact original source rectangle, including
their natural alpha boundary/root tuft. No crop edge, mask, source retouch or
recolor is introduced. The existing uniform nearest fit remains. A deterministic
area spiral with per-parcel phase replaces the row grid; it uses no random input
or generation service. Whole silhouettes/placement are the only scenery changes.
No extra symbols, actors, new navigation or retail/physical authority is added.

Read-only AST/pure geometry checks, without importing Pillow or running an output
job, find300 placements including145 cluster trees. All155 incumbent whole-frame
placements compare exactly against03 layout. Exact six112px-high exclusions,
architecture clearance and existing60px path/30px garden clearance are retained.
The approved path centerlines/46-40-28 widths,22px garden borders, garden polygons,
roof/facade parameters, palettes, four canonical display points/World cells,
nine source pins, layout schema/dimensions/version and Map4/era tuples compare
unchanged against94c47a8b. Selected characters and production World are untouched.

Recipe revision is `reference-correction-04`. Its output remains NOT_RUN and no
finite lease/job is held. A fresh request to21d includes the exact existing ABS
Python/Pillow build and deterministic verification, followed by one sequential
preview child producing nine private temporary source-raster PNGs: each era at
360x608,390x658,430x726, with nearest sampling and half-up aspect dimensions.
The frozen inline preview script is933bytes SHA256
`4b2898bff04c7c458929e6ec2c5743b19cff1033265637d77de24aa0313af221`.
Preview temp is separately capped and must be independently removed by the
original hard deadline, including if the reviewer does not return. No preview
is a browser/App capture or proof of labels, controls, hit targets or runtime
framing. Root retains sole canonical native custody. Actual saved04 images will
be inspected before a separately pinned candidate is handed off; no automatic
adoption or visual/runtime/reference acceptance is implied by this source draft.
The existing goal remains unfinished and no review ACK gate is introduced.

### Correction04 finite source validation and nine-scale image assessment

Fresh offer `ART-MAP04-SOURCE-B-202611041443Z` was explicitly accepted at
2026-11-04T14:44:00Z on clean published source checkpoint
`dbfc7c30e1fda1e7dde71c224ad8be3171ca8e7c`. Original work cutoff14:45:00Z and
automatic cleanup hard cutoff14:47:00Z were fixed before children started.
Frozen recipe29706bytes SHA256
`51abee3bfcb78d34eb9d3e3934d225c23b9963a45905355d719814145d0e0d19`,
README10719bytes SHA256
`ef9801eb29988cdb0532b80d8c5a8818438b4c7657d6fc366b9b0f2c61ebbc64`, and
ART document138237bytes SHA256
`03769a2038f9d4e4f01ea1c799cfa9d54f9751c175a4d715a27b5f0b2833a923`
were checked with full HEAD/clean status and all nine actual input hashes.
Post-release text records results without changing the frozen recipe or images.

Existing ABS Python3.14.3/Pillow12.2.0 ran three work children sequentially:
`-B build.py` exited0 in0.600seconds, `-B build.py --verify` exited0 in0.440seconds,
and the frozen933-byte inline source-preview script exited0 in0.329seconds.
Final work ended14:44:02.767496Z. The verifier reproduced all eight generated
files byte-exact. All three commands had empty stderr; there was no warning,
failure or retry. No browser/native/Node/PostgreSQL/build installation, image
generation, new worktree or shared cache/profile mutation occurred.

The independent guardian enforced the original60-second work deadline,180-second
total clock, disk/byte/process/memory limits before and during each child. It
remained alive during bounded read-only image inspection and independently owned
automatic cleanup of the private preview directory, even if the reviewer never
returned. After all nine previews and three full PNGs were opened, Artist sent
explicit RELEASE. Guardian cleanup finished14:45:27.907546Z, removing all nine
preview files and their directory, before its hard cutoff. A fresh independent
check at14:46:26.133474Z confirmed directory absence and no live builder.
FULL RELEASE was sent14:46:26Z. No child/guardian/server/TMP/cache/output handle
or allocation remained. Later source documentation/Git handoff is separate from
that completed finite job.

Fresh owned historical/source carry46272418bytes was covered by50331648bytes
conservative carry. New generated output7825406bytes and ten-file durable source
7865831bytes stayed below10MiB. Peak temporary2001234bytes stayed below4MiB;
final temporary0. Peak whole accounting60264249bytes included all old carry,
new durable/temporary output and65536bytes proof reserve, below67108864bytes.
Initial free26357325824bytes/minimum26044157952bytes exceeded16GiB start/8GiB live
floors; final independent free26041339904bytes. Peak sampled supervised RSS was
124059648bytes plus16777216bytes guard/shell reserve, below512MiB. ALL-owned
process bound4 included guardian, one work child, short process guard and optional
exec shell; work children never overlapped.

Exact generated04 postimages, all PNGs768x1296 RGB:

| File | Bytes | SHA256 |
| --- | ---: | --- |
| revenge-scene.png | 713028 | ad378c9b889d1a7f33c49ed3cceabef140cda3cbe5d64e46d5a2139516d0a148 |
| parade-scene.png | 731192 | dbcc63274381f1ffe6128cc6b9caaedb2e406618f8464f218f554ec6294edabc |
| danger-scene.png | 720445 | a2dcf7f7346f01702b7dc033e4c85f2441165678a794b9563087f04bf07908e3 |
| revenge-scene.svg | 1827514 | 747c73424bf28099fd4f08a1b5aeae90b11df926464478be3e053ed02a0b3211 |
| parade-scene.svg | 1859962 | 0c6bc6256506b000dc6669940c482d5d8b057b95957e21732f8451209566b0eb |
| danger-scene.svg | 1829726 | e743bef95981fc5a434c80bf36fc839c67b0bfcf6b2a2b54d657339c9de7c6f6 |
| layout.json | 108021 | 2c14cc0145e19054ce4c65c8edef574e6156cf3eab7a14dff224b6c008c37f8d |
| source-provenance.json | 35518 | 741f5eb2ee4dd91070fd43f0b1f839d69b3b95406b59df6e730052257d63638c |

All generated descriptor hashes/lengths and nine actual source input hashes were
independently checked again after execution. Actual new layout schema/version,
dimensions, originalMap4, six label anchors/exclusions, four canonical display
anchors/World cells, path centerlines, garden polygons and authority exactly
match03. The source authoring comparison also fixed approved path widths,
roof/facade/palette geometry and all155 incumbent whole-object placements.
There are300 placements including145 whole-silhouette cluster trees. New colored
substrate pixel count is0 in each era; placement-domain area is not misrepresented
as painted vegetation coverage.

The nine source-only nearest scale previews were directly opened in place before
their automatic cleanup. Their retained manifest is below; these temporary
images are intentionally not durable capture evidence. Width360 has height608,
390 has658 and430 has726. The933-byte inline script/hash above defines their
sampling; no labels, controls, client viewport or World render was involved.

| Era / width | Bytes | SHA256 |
| --- | ---: | --- |
| REVENGE360 | 188479 | 2a44ba255d2b76c3270a96ec395f5b954eef3bd68d14c95619380e46850b7845 |
| REVENGE390 | 214097 | 888ec5cb1092af46200612834b214c8a09232825cba34ae2d580b41ad98df2ef |
| REVENGE430 | 257977 | 73a4430b47a2898d26311e13736ff7f5a35bce6e13ab35f66803a3a113fc8d92 |
| PARADE360 | 192158 | a2b6ad53f2d41859a08e68d75d4bcb7b8ef7e96d72baa3ee2f326fb9678739db |
| PARADE390 | 218493 | 24ae48e8d86ccc53ed641586f3bf67153a0c0e238306a1088517c914055d299b |
| PARADE430 | 263662 | 8a8a2c9faf3af9b3643eb2b4755f4210f447dc263d044406ee99fae4848c8af4 |
| DANGER360 | 190092 | 4f58ca138791a85c069aadb4e0d524399f7bd74c3b22f07dfcd973280b720b4b |
| DANGER390 | 216050 | bd884497746c95486dc8de6d04ce284a2a9367a7531316dae0523bfb74281750 |
| DANGER430 | 260226 | c91a676b7d49d0d8ecc4958bee0cafc9f23d69c914133fd37034b828c9c7c421 |

Direct source inspection of those nine previews plus all three full-size PNGs
shows both targeted defects addressed: colored angular parcel boundaries are
absent; complete original lower alpha edges replace the flat76percent cuts;
cluster trees form irregular groups instead of rows. The approved scenery,
warm era palettes and label-reserved ground remain coherent. This is a source
finding at the declared raster scales, not an App/runtime acceptance result.
Broad open lawns around label reserves and the large uniform roof slopes remain
less compact/intricate than the exact fourth Map reference. Roofs, paths and
anchors were deliberately preserved within this targeted two-defect correction.

Status is **SOURCE_VALIDATED_NATIVE_NOT_RUN**, separate from frozen ART03/paint/
CSS native cohort868c50b6. Root/26e must explicitly register matching04 layout,
images and their paint identity before any new canonical client qualification.
No frozen FE19 candidate, selected character or production World is changed.
Actual04 labels/mobile/short framing, selection/World invariance, SVG parity,
live/device evidence and full visual/reference approval remain open. Root alone
retains native custody. Earlier packets/candidates and failures remain preserved;
the existing aligned goal is unfinished, with no new goal/budget or review gate.

### ART03 actual native readback and scoped remote publication proof — 2026-11-05 KST

This is a read-only assessment of the frozen ART03/compact-paint/corrected-CSS
runtime, followed by this documentation append. No generator, browser, preview,
native test, registration or asset replacement ran in this assessment.

The inspected packet is
`docs/mcr/evidence/map-production-binding/native-composition-art03-retry01/`
in Root's `fe-delivery-publish` checkout. Its actual `report.json` SHA256 is
`9c653908d8afebcecdff920f880731e92709d0175543fb59c774b319e68b8683`;
the frozen runtime candidate is `fe0f7ab545aea9317382130483898e9aa3bcb2aa`.
It binds ART03 `94c47a8b3219efc76a4be9646014cfeaad2f1957`, consumer qualification
`66eacf888541e526da88f68dae7dbebffbcae3d5`, presentation digest
`4922af8c4761014a2dd672ec52610ab9abd394be0bcc0b29e7780421167600ca`,
and CSS SHA256
`b0cc09eaaa88ffc90b165d24f573f01d9c59021322455658d411bab8fabaaae6`.

Directly opened in place: all nine AFTER captures across REVENGE/PARADE/DANGER
and widths360/390/430 at height844, PARADE390x420, both PARADE390 forced-focus
captures, and the exact `docs/mcr/design/reference-target-2026-11-04.png`.
No new image or duplicate evidence packet was created. Independent checks of
the retained report and files passed:

- All19 case screenshot hashes match actual bytes. The two additional focus
  PNGs are separately hashed below; the report records their filenames without
  screenshot hashes. The packet contains21 PNGs in total.
- All nine full-height before/after pairs preserve the entire World record:
  identical RGBA hash, all29 object records, World camera, presentation camera
  and map identity. All19 cases retain local Zoom/Fit/Reset camera invariance,
  report no runtime errors, and dispose their canvas and active socket.
- All40 feature actions retain Enter/Back focus, one World canvas, one created
  and active socket, and the same camera. Minimum measured target size is
  66.359375x44 CSS pixels. The native source additionally checks full target
  visibility after local scroll, center obstruction and full-rectangle control
  overlap, rather than relying on a center-point check alone.
- All60 measured toolbar/zoom controls meet44x44. All ten AFTER cases retain a
  solid focus outline at least3px wide. All60 independent original40px-font
  ink measurements fit inside their paint rectangles; minimum rendered label
  scale is13.826497395833334 CSS pixels.

| Additional directly viewed capture | Bytes | Independent SHA256 |
| --- | ---: | --- |
| forced-focus-parade-390.png | 201782 | e8f41c8805b44ff9d595459f023bd8cad2af163e366f94b85890defc49c559e9 |
| forced-focus-parade-390-short.png | 75167 | 501ff60b54a32c0fd3a41304751705fb176c5eaf2f702515c108bfda99d5c03f |

The native implementation was inspected at
`frontend/src/app/world-entry/__tests__/map-presentation-v2/native.mjs`.
Production rasterization deliberately disposes its temporary label FontFace;
an empty retained font set is valid. The toolbar uses explicit weight800,
14px `"Arial Narrow", sans-serif` fallback. The separate metric probe verifies
original font bytes SHA256
`f44e493f61e2d91c2156dd5abc720eeb6274a9c02b11d9097d1ea38d50011e8b`,
loads them temporarily for measurement, then removes that face in `finally`.
These metrics do not establish a persistent production font owner. The78px
transparent interactive rectangles remain distinct from112px source scenery
exclusions. The earlier incorrect font-assumption FAIL and its ten PNGs remain
preserved under `native-composition-art03-round01/`.

Visually, the compact painted labels are legible across the nine full-height
captures and reveal more scenery. Caption and controls remain readable, with
a visible Fit outline in forced colors. Width360 has visible contain bands;
390 has narrower bands and430 fills most of the viewport. The390x420 capture
shows a central scene crop with fixed caption/controls; reaching outer labels
requires local scrolling. It does not prove simultaneous visibility of all six
labels, and provides no reason to shrink interaction targets.

Actual ART03 pixels confirm the two source defects already corrected in ART04:
angular green substrate boundaries around reserved ground and flat/row-aligned
tree fragments. ART04 `97d9d3f40e0d17ab536ac696bdb2140093225f6a` removes the
colored substrate and restores complete original tree alpha with irregular
placements. Its source inspection remains separate from this ART03 runtime.
Open lawns around label reserves and broad uniform roof slopes remain less
compact and intricate than the reference. No additional ART03-driven geometry
change was made during the pending ART04 intake. Actual ART04 client framing,
labels, World invariance and full visual/reference acceptance remain open.
This packet is an eligible-attendee contract fixture with production admission
false; it establishes no live/operator admission or physical-device result.

The retained independent release record reports FULL at
`2026-11-04T14:54:26.141975Z`, before the original WORK14:55:35.617315Z and
HARD14:56:35.617315Z deadlines, with no living owned process/listener/temp
prefix. That historical record was read; no new resource lease was requested.

Fresh remote verification at `2026-11-04T15:01:13.627389Z` independently fetched
`origin/master` and separately queried `git ls-remote`: both resolved to
`f02fe29c69627007aa15eb1e17699b9e3806fba8`. A bounded23-path comparison proved
exact Git blobs and modes against scoped publication
`3ea8703fb62cba6760e12a534551c198d99a5c54` and that fresh master:

- Ten owned ART03 pack files, 7,778,836bytes, compared to source `94c47a8b`.
- Thirteen supporting native-pinned source05/consumer/CSS files compared to
  frozen `fe0f7ab5`: the ten `map4-service-v2/` files named in `sourcePins`,
  `map-presentation-v2/loader.ts`, `places.css` and `IllustratedMap.tsx`.
  Every supporting file also matches its retained report SHA256.

All23 paths total7,944,219bytes and have mode100644. The reproducible manifest
uses these ordered paths (Git's sorted ten-pack listing, then the report's
sourcePins order filtered to the thirteen paths), with keys `path`, `mode`,
`blob`, `bytes`, `sha256`, encoded as UTF-8 JSON with sorted keys and separators
`,`/`:`. Its SHA256 is
`25df148c3905d1d6b6e8687ea6547f75aebffbe59f2e21c02aa0c5ca7b43cd29`.
This is the owned pack plus supporting consumer/CSS proof, not a repeat of
Root's entire85-file publication audit. All ten working ART04 pack files were
also byte-compared to `97d9d3f4` and remain unchanged. Remote ART03 publication
does not adopt ART04. Root retains publication/native custody; the full visual
continuation remains active with no new review or receipt gate.
