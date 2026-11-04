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
