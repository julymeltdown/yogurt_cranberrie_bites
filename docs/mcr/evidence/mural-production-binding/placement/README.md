# Product virtual mural placement — source handoff

Task `WALL-PRODUCT-VIRTUAL-PLACEMENT01` supplies one concrete product placement for Root's default Wall producer: **top-left `(312,624)`, size `128x128`, depth `0.5`, exclusive texture key `mcr-product-virtual-wall-map4-r4-v1`**. The complete machine-readable input is `placement01.json`, SHA256 `cbfb6fd02b9ca1b376f48afdecb8ff9801c0664bb20dcfc402b94b0dc64ac0f0`. This is a new **PRODUCT_VIRTUAL flat paving raster**, not an existing upright wall or physical place. Source proposal is complete; adoption and actual runtime rendering are NOT_RUN. This input does not confer mural write permission.

## Exact map and source

Bind all four identity fields and canonical dimensions together:

| Field | Required value |
| --- | --- |
| Map ID / revision | `mcr_reference_scene_r3` / `4` |
| Map content hash | `sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8` |
| Geometry hash | `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f` |
| World dimensions | `768x1296` canonical pixels; `48x81` cells of `16x16` |
| Source delivery | `20531df752a12a84143eedcce6151b6d89530190` |

`source-pins.json` records13 exact source/evidence Git blobs, modes, byte counts and SHA256 hashes from that fixed delivery. The retained public bootstrap is read only for original manifest/geometry/scenery metadata; its disabled capabilities are not reused as eligibility. The actual canonical Tiled map, geometry proposal, all three22-object World scenery lists, original ground, renderer origin/depth code and mural sink are read directly. This is the retained quality-v6 World scene, not the ART04 Map-only composition or a display-label projection. No map/hash/release identity is changed.

## Placement reasoning and artist coordination

The rectangle occupies `[312,440) x [624,752)` in top-left-origin World pixels. Artist231 independently read the original ground/PARADE atlas and renderer sources and recommended this exact rectangle, including camera/occlusion limits. The developer directly opened the original ground and the retained `quality-v6/parade-world-360.png` comparison in place. The comparison is historical source-only art evidence and does not contain this new mural; no new preview/render was made.

Source metadata arithmetic finds no intersection with any of the22 World prop bounding rectangles in REVENGE, PARADE or DANGER. Tree11 endsY617, leaving7px above; meeting pavilion beginsY779, leaving27px below; western merch hall endsX220, leaving92px on the left. The72 cells touched by the half-cell X origin (columns19–27, rows39–46) are all walkable. Both spawn centers and all four actual canonical interaction centers lie outside. Community spawn center `(504,760)` is64px right of the raster edge; its nominal64px avatar's left edge is32px clear. The rectangle is visual only: it adds no obstruction, spawn, interaction or movement rule. Transient walking avatars can cross it and must remain visible over it.

The incumbent mural sink uses `setOrigin(0,0)` and directly consumes the supplied depth. The World ground is depth0; ordinary scenery is `1 + footY/(height+1)*0.9`, and normal avatars use depth1–2. Therefore0.5 deliberately places a **flat** virtual raster above ground and below source props/selected characters. It is not a wall foot-depth computation. A128x128 mural uses nearest-neighbor128-to128 display without aspect distortion. Exclusive key has no existing reference under application/World assets at the frozen source; actual runtime key collision must fail closed rather than overwrite any texture.

Earlier candidate coordinates were discarded during source authoring after checking the actual cameras. `placement01.json` contains exactly one proposal. No alternate/fallback anchor is authorized.

## Retained camera evidence and limits

The existing Map native subpacket at `docs/mcr/evidence/frontend-combined-adoption/default-round01/map/report.json` is1,055,439 bytes, SHA256 `478b2c6bc41f0ea21321d87a39eecaf66e685f4c3f992292dba566e3bb44c133`, frozen candidate `002c123a306c7853f52161f33b423e875c21ad2c`. Its normal World camera at360 isX298..646/Y92.5..755.5: the complete proposed raster fits, with14px left and3.5px bottom margin. Normal390/430 recorded World views also contain it, as do their larger initial Map-route camera extents. This is arithmetic on existing public camera metadata, not an actual mural screenshot or a promise for every self position.

The recorded390x420 World view isY304.5..543.5 and does **not** show this raster. Moving/short/panel camera behavior and mural readability require Root's actual intended-route native validation. No camera recenter, teleport, auto-pan or viewport rewrite is part of this proposal. The margin near the normal view's bottom must be assessed truthfully in that validation rather than hidden.

## Authority and required production behavior

Placement authority is the explicitly assigned product-virtual source proposal. No authority is derived from stale `virtualwall_info` cell28,22, a proposal cell14,68, retained Map4 `wall_info` cell14,61, any Map-only WALL label/marker/leader line or a fixture's raster. Physical-place provenance is not applicable. The actual frozen Map4 interaction metadata is documented only to exclude it as an authority source; it remains unchanged.

Root owns the production factory, actual current WallTools/server eligibility, admitted World/privacy lifecycle, runtime validation and publication. The factory must match the exact map tuple/dimensions before allocating anything, borrow the one incumbent scene, and obtain current mural/authorization eligibility independently. Binding mismatch, stale/withdrawn release, privacy change or texture ownership collision must withdraw/fail closed. The fixed texture key is a resource identifier, never a permission signal; lifecycle teardown must release its image/texture/listeners before reuse. No routing through a synthetic place or label is needed.

## Ownership, validation and committed handoff

Only this new `docs/mcr/evidence/mural-production-binding/placement/**` prefix is authored in the existing exclusive `fe-runtime-preferences` checkout. Map consumer/source06, ART pack, World/provider/runtime, source originals, manifests/contracts, dependencies and peer worktrees remain unchanged. Root receives the exact committed three-document handoff directly; no King/SL inspection, review or acknowledgment is required.

Completed validation is bounded **read-only source/provenance and rectangle/camera arithmetic**, not an executable application suite: fixed-revision `git show`/`git ls-tree`, original JSON parsing and manual metadata readback; all three scenery lists have22 objects, zero rectangle intersections, all72 touched cells walkable, no spawn/interaction center inside, three full-height saved World views contain the proposal and the short view does not. Fixed-source `git grep -F` finds no preexisting exclusive key (expected exit1). No browser, server, renderer, build, tests, source generator, install, ART allocation or runtime resource lease was run/held. Actual producer/eligibility/native/live/device checks remain NOT_RUN by this developer.

Actual persistent-goal readback remains the historical blocked Map objective. This new documentation-only placement assignment is incompatible with that metadata and was reported to Root; it was not reset or falsely completed. The source handoff proceeds without treating the goal metadata as an approval stage.
