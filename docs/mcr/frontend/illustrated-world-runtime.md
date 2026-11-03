# Illustrated world runtime

## Delivery and provenance

Assignment base: `5c92862`, sole branch `codex/mcr-fe-illustrated-world`. The former default-shell checkout remains stopped and unchanged. This implementation consumes the original illustrated source author `19d7085`, published as `bc60edb`, through its exact binding proposal and export receipt. The source pack was already qualified by the art owner and Main; this work adds its runtime consumer rather than another art approval gate.

The optional C06 contract checkpoint `d682bb4ea345eae9bed2132436f141822e9d8f71` was published by Main as `0fd471d`. Main's public schema regeneration and Python semantic parity commit `d434630` is imported unchanged as local `595f0dc`; it is a dependency and must not be integrated twice. Main separately delivered backend semantic parity as `028a6ac`. No generated schemas, backend validators or scripts were authored by this worker.

Original `frontend/public/world/illustrated/`, maps, themes, selected character-source pack, the supplied reference, and `docs/source/` are unchanged byte for byte. New derived detail exports live exclusively under `frontend/public/world/illustrated-runtime/`. No dependencies or lockfiles changed. Retained tools: Node 24.22.2, Chrome 156.0.8078.80, installed Playwright and existing contract-validation Python environment.

## Contract and composition boundary

`ThemeManifest.scenery` is optional. All existing v1 required fields and legacy fixtures remain valid. The new closed shape is:

```ts
scenery?: {
  version: 1;
  ground: SceneryTexture;
  atlases: SceneryTexture[];
  objects: SceneryObject[];
}
SceneryTexture = { id, path, sha256, width_px, height_px, bytes };
SceneryObject = {
  id, atlas_id, frame: { x, y, width, height },
  x, y, width, height, depth_y
};
```

IDs and PNG paths are strict, coordinates and dimensions are integers, and unknown fields are denied. The schema and semantic checks bound each skin to four textures including ground, 96 objects, 2,097,152 transferred bytes and 2,097,152 decoded pixels (8 MiB RGBA), 2048px texture sides, 1024px object sides and 128px perimeter overhang. Ground must equal the immutable map extent. Resource IDs/paths and object IDs are unique; frame bounds, atlas references, world intersection and depth bounds are checked before fetching. A depth of -1 is reserved for ground-attached details; the plaza label uses it. The granted schema evaluator change implements only `maximum` and `maxItems`, preserving strict rejection of other unsupported keywords.

`sceneryResources(theme)` is a pure hash/path/kind allowlist projection. `sceneryDescriptors(theme)` returns the ground and atlas descriptors. Neither grants provenance, identity, physical venue or privacy authority. The narrow production resolver change first validates the selected C06 metadata, then adds its original scene descriptors. Optional themes stage their complete tile/scenery resource set and detect conflicts both against admitted resources and within the candidate before one synchronous merge. Rejected optional themes cannot leave a ground, atlas or tile resource admitted. Original C03 descriptor identity, selected-source avatar checks, canonical Auth, privacy withdrawal and resource pinning remain in force.

C03 already references the C06 theme schema. No bootstrap property was added. Main regenerated the embedded core/domain schema; the actual production-resolver tests exercise that regenerated decoder. Root app/router/social composition and release configuration remain outside this assignment. The emitted `bindings.json` is explicitly a source/runtime binding proposal, not a C03 or approved avatar release.

## Loading, drawing and replacement

`loadMap` validates the original map/theme hashes and Tiled geometry before loading. For an illustrated skin it reads only the existing complete AR04 tile texture, original ground PNG, original landmark atlas and derived detail atlas. The terrain-only IDs 1–8 export is never used as the required C06 tile atlas. Each PNG retains the same-origin allowlist, exact digest, content-type, transfer/deadline, IHDR and decoded-dimension checks. Scene descriptor byte lengths are exact. Four decoded image leases share cancellation and idempotent disposal; late injected decoder results are released. The legacy no-scenery single-image path preserves its original late-decoder behavior.

The existing MapScene prepares three hidden textures and 30 image objects: one ground plus 17 individual landmark/tree instances and 12 native details. It uploads neither the complete scene PNG nor the complete foreground preview, and never fetches editing originals in the runtime. Frame/source rectangles and world placements come directly from the original proposal. Labels attach at their landmark depth; banners and lamps retain their documented base depths. The plaza label stays below players. Atlas objects use `1 + depth_y / (heightPx + 1) * .9`, matching the unchanged avatar renderer's foot-depth formula. The ground is depth 0. No new collision, spawn, interaction, arrival or mural-write authority is created; the illustrated mural is decorative source art.

Phaser 3.90 Canvas `batchSprite` expands frame output by half a source pixel when camera `roundPixels` is true. That altered the exact perimeter tree sizes in the first native comparison. Illustrated skins now set that option false on the same camera, retaining nearest-neighbor filtering and the same camera position/zoom. Legacy tile skins retain true. All 45 final era/landmark views have zero differing channels against independent Canvas composition from the original bound resources.

Existing `createWorldRuntime.replaceSkin` remains the single replacement owner. A candidate is fully loaded and validated, all new scene allocations are prepared hidden, then shown synchronously and the previous skin is removed. A failed decode, invalid binding, cancellation or preparation failure retains the previous valid skin. Partial upload/frame allocations clean themselves. Original map bytes, geometry, collision/spawn/interaction/place identity and avatar release bindings remain unchanged. Two illustrated eras must also retain identical object/frame/placement metadata. No second engine, canvas, camera, avatar renderer, socket or Auth owner is introduced.

The only runtime type extension is optional `EngineLease.readScenery()`: bounded texture/object counts and current/peak raw RGBA size, measured from the incumbent texture manager's registered image dimensions. It exposes no Phaser scene or texture capability and creates no listener. These logical pixel bytes exclude renderer overhead and browser/process memory; this Canvas renderer does not supply a GPU memory measurement.

## Deterministic detail export

`world/__tests__/illustrated/export.mjs` reads the unchanged era landmark SVG documents and exports only their original `label-*`, `banner-*` and `lamp-*` groups with Chrome's generic sans-serif font. Each group's integer bounding box plus 2px padding becomes one frame in a 512×256 transparent atlas. World coordinates and original base depths are retained in the derived descriptors. No character or image generation tool was used for these derivatives. The five large original raster inputs remain untouched and are not runtime resources.

The export receipt records Chrome version, original SVG/binding/receipt hashes and exact derived PNG hashes, byte lengths and dimensions. Repeating the export with the retained Chrome reproduced all seven derived artifacts byte for byte. Browser/font changes can alter exported bytes; committed descriptors pin this particular export. Scenery transfer sizes, excluding the existing tile PNG and Tiled JSON, are 551,505 bytes REVENGE, 529,746 PARADE and 547,109 DANGER.

## Validation and evidence

All final checks exited 0:

- `node frontend/src/world/__tests__/illustrated/native.mjs`: actual `loadMap`, same-origin bounded resource reader, `createWorldRuntime`, incumbent Phaser and camera. Widths 360/390/430 × three eras × five landmarks give 45 zero-difference pixel cases; six more captures show front/behind occlusion. Total 51 captures. Canvas and 44px era control are fully within the viewport.
- Native rectangular TEST ONLY marker: 81 magenta pixels visible behind a tree versus 392 in front at each width, using the original avatar scene port and foot-depth formula. It is a depth probe, not character art or an avatar fallback.
- Native atomic replacement: the held frame/skin remain unchanged while the next ground is delayed. Three registered scenery textures total 6,225,920 raw RGBA bytes (5.9375 MiB). Preparing old+new peaks at six / 12,451,840 bytes and returns to three. Digest, dimensions, aggregate budget, geometry and missing binding rejection keep the old signature. Cancellation does likewise. Four decoded-image Blob URLs become zero at disposal; all scene textures/canvases are released, one original game is destroyed once.
- From `frontend/`, `MCR_FE14_PYTHON=/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python node node_modules/vitest/vitest.mjs run --configLoader runner --no-cache --maxWorkers 4 src/world src/app/world-entry/__tests__ src/app/__tests__/default-shell`: 649 tests in 23 files passed, zero skipped. This includes 32 new contract/loader/resolver/preparation cases plus 25 independently authored shared metadata parity cases and existing world/theme/avatar/network/root tests.
- Full types: `node node_modules/typescript/bin/tsc --noEmit --incremental false --project tsconfig.json`.
- Lint: `node node_modules/oxlint/bin/oxlint src vite.config.ts`.
- Production build: `node node_modules/vite/bin/vite.js build --configLoader runner --outDir /tmp/mcr-illustrated-production --emptyOutDir`; existing chunk-size warning retained.
- `python3 scripts/mcr/check-plan.py`: planning provenance only, not application acceptance.
- Derived export reproduction, immutable source hash audit and final `git diff --check` passed. `delivery-pins.json` binds current code, schema, dependencies, original inputs, derivatives, tests, documentation and evidence to the tested checkout.

The exact ten-screen reference, SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`, and current 390px era/landmark/depth captures were inspected directly. The runtime now displays the supplied stage, architecture, foliage, paving and labels rather than the former colored tile scenery. It retains AR02 orthogonal geography, with different scene density and layout from the reference. This fixture does not certify the reference's full product chrome, diagonal geography, crowd, characters or all ten screens. No decorative left accent pattern was introduced.

Failed development attempts are retained as logs (the separately named development-failure image is historical): export in an SVG document initially created a canvas in the SVG namespace; a Phaser debug-global assumption was replaced by actual bounded allocation observations; exact pixel checks exposed the half-pixel renderer issue; an initial unit command used the wrong working directory and a cross-realm typed-array assertion; the first broader run found the legacy late-decoder behavior and was corrected without editing old tests. Final fixture paths are independent of working directory, resolver arrays compare bytes, and final pixel checks require exact equality. No failed attempt is presented as acceptance.

## Remaining product boundary

Contract support is published; the runtime implementation and deterministic actual-engine evidence are delivered for integration. Native fixtures assemble original scenery/map bytes with an explicitly unavailable TEST ONLY avatar identity. Actual production-resolver unit fixtures reuse existing simulated reviewed descriptors to test authority fences; they are not new approved-art pins. Production C03 binding still needs the canonical independently qualified selected avatar and original release configuration. Live authenticated multiplayer, real sockets, physical devices, complete visual/product acceptance and IN01/IN05 are NOT_RUN here. Main owns serial integration and publishing; those external gates do not invalidate or block this delivered consumer implementation.
