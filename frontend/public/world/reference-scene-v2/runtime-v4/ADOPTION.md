# Compact PARADE runtime derivative v4

Status: CONTRACT_READY art inputs; PROPOSAL_NOT_RELEASE_BOUND. Frozen source checkpoint: `8f89d1905a86143d34ff77d2951fc60651ab2497`. Source v3 and the original source PNGs are unchanged. This sibling exists because source v3 exceeds the existing combined C06 scenery limits and contains fractional visual depths.

Use `parade-scenery.json` as the exact closed C06 `Scenery` object. It has one ground, one atlas and 22 WORLD objects. Use the incumbent loader/LoadedScenery pipeline; no extra resource, parallel attachment or budget extension is needed. It does not qualify a release or modify its registry. Server/client owners must coordinate actual C06 pins and authoritative geometry before displaying this scene.

| Resource | Dimensions | Bytes | SHA-256 |
| --- | --- | --- | --- |
| parade-ground.png | 768x1296 RGB | 693939 | 5e27d2e223ba4fd36d1464577c3e9d9ff8d3f2db5db984e0306587a5d2143af5 |
| parade-atlas.png | 1024x1024 RGBA | 1253878 | ff68a358fad4e03ceaaf5fd610a9e6cdad9a167963e3ec00545c909ba6bb0dfe |

Combined: **2,043,904 pixels**, **1,947,817 bytes**; each is below 2,097,152. Ground RGB re-encoding is lossless because every source alpha is 255. Atlas frames use an explicitly declared 0.75 nearest-neighbor derivative, centered in fixed 256px cells with transparent padding. Source sheets are preserved; there is no color quantization. `cross_banner` uses the corrected source-only crop `{x:695,y:934,width:170,height:320}` to exclude neighboring furniture spill. Every source/packed rectangle and transformation is in `parade-resource-inputs.json`.

`parade-integer-projection.json` records all 28 authored/runtime rectangles, depth values, local foot anchors and deltas. Round to the nearest integer with exact ties away from zero. Runtime depth is rounded authored depth; runtime local foot is rounded `(authored depth - authored y)`; runtime Y is the difference of those two integers. X/width/height round independently. Actual maximum depth and Y error is 0.49 virtual px; actual maximum local foot-anchor error is 0.30px. Stable ascending depth order is unchanged; source array order resolves ties. Consumers must use these pinned integers, not apply a separate rounding rule.

Canonical geometry stays `sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5`: original footprint polygons, collision/terrain/overdraw, 48x81 cells, 16px tiles, existing IDs, spawns and interaction cells are unchanged. Raster overhang never creates collision. Provenance remains virtual and unreviewed for physical venue claims.

MAP additionally uses `parade-map-metadata.json`: six semantic illustrations, replacement relationship for the stage, Plaza parcel, labels and existing place metadata. These are **not** C06 WORLD objects and have no collision footprints. Default zone rendering is the already authored native solid `#7a8742` (native RGBA `[0.48,0.53,0.26,1]`), outline `#b4a27d`, width 6 virtual px. An optional patterned variant may sample `[0,0,48,48]` from the loaded ground. Do not fetch a third surface-source PNG. Render labels as UI text; null bindings remain unbound.

`parade-layout.json` retains names/cameras/footprints for developer inspection and MAP composition; `parade-scenery.json` is the closed C06 projection without extra art fields. The original full source and editable projects remain in `../painted/`.

Validation: `python3 ../verify-runtime.py` verifies closed schema, exact pins, pixel-identical ground, source-preserving nearest crops, nonoverlap, integer projection, depth order and untouched footprints. `node ../check-runtime.mjs` checks source/compact Canvas2D comparisons at 360/390/430px: six cases, 72 clear-foot samples, no page errors/overflow. Both MAP comparison panels intentionally use the same approved solid zone to isolate packing differences. Evidence is local asset validation, not app/Phaser/server/multiuser or moving-avatar acceptance.

Painted REVENGE and DANGER remain unavailable. Do not alias them to PARADE or use native polygon drafts as product substitutes. All-three-era delivery and target fidelity remain required and in progress. The third-round v3 scores do not qualify this derivative; no fourth scored panel is claimed.
