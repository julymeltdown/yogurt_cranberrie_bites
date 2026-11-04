# MAP layout-v2 frozen consumer qualification

The bounded AR02/AR04/FE06 adoption consumes both published artist sidecars from196cd0a2/d1a5deaa. Previous source/captures remain unchanged. Full work-item or whole-art fidelity acceptance is not inferred.

- `capture-packet.json` pins the21 final native/editable artifacts and identifies the49 represented source pins.
- `qualified-v2/native.json` is the final passing runtime report at765a4053: nine actual Root MAP cases, two marker cases and three actual Chrome-rendered editable SVGs.
- The14 MAP/marker PNGs cover360/390/430 ×threeeras,390×754 and closed/unavailable semantics. Every overview has zero differing channels against independent source composition.
- Three `editable-*.svg` files embed pinned art/font and editable crops,49 objects, six labels and the paired source metadata. The matching PNGs were rendered and inspected; geometry/font/data equivalence is verified. SVG-versus-canvas pixel equality is not claimed.
- `qualification.json` and `checks/` record258 regressions,67 final focused cases, full application TypeScript, scoped lint, immutable tuple checks and plan preservation.
- `diagnostic-camera-resize/` preserves the first failed harness assertion and screenshot. Opening a full panel resizes the hidden World canvas; the corrected driver isolates MAP controls at a fixed viewport and preserves authoritative feet across navigation/resize. This diagnostic does not qualify the final input.

After native capture only two generator scripts and the reproduction test changed. Their default CLI now emits only the granted MAP files; the mutable ART lane-document pin is read from immutable source publication d1a5deaa. The46 runtime/data/driver pins, generated sidecar bytes, editable files and tested browser input remain exact. Fresh focused tests and full types cover the generator/test refinement.

With retained Node24.22.2 dependencies and Python, run from repository root:

```sh
python3 -B frontend/src/world/reference-scene/spatial-v5/build-source-inputs.py --map-layout-v2
python3 -B frontend/src/world/reference-scene/spatial-v5/build-map-landscape-inputs.py --map-layout-v2
MCR_MAP_LAYOUT_V2_OUTPUT=<new-absolute-directory> node frontend/src/app/world-entry/__tests__/reference-scene-v5/map-layout-v2.mjs
```

The browser command requires a new named coordinator allocation and fresh8GiB/process guards. The completed allocation is released; it does not authorize a new run. One browser/server workflow, estimated0.18GiB peak/hard0.2GiB and20MiB durable budget applied; all owned browser/server/cache resources are closed.

Both generation modes (no arguments or the explicit flag) reproduce identical consumer bytes without rewriting canonical source-inputs/Tiled/runtime/camera/allocation. The original UNREVIEWED provenance and production denials remain truthful; fixtures use7005, distinct from actual backend source reservation6. Live Auth/server, multiuser, devices, keyboard/notch behavior, complete source fidelity and production admission remain unproven. Architecture-v2 source and the separate World presentation correction remain independent follow-ups.
