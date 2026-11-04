# MAP northern canopy palette v1

Status: **CONTRACT_READY_METADATA_RENDER_VALIDATION_NOT_RUN**.

Actual inspection of the human's attached ten-screen target and the published
native04 PARADE editable raster shows a specific palette mismatch: the target's
northern MAP district has muted green/yellow crowns between façades, while the
candidate has a broad crimson band. The fixed central-north region excludes the
two lateral red-roof façades. A coarse red-color proxy is0.91% in the attached
reference versus54.47% in the actual saved raster. `reference-analysis.json`
records the exact image hashes, approximate crop, samples, RGB rule and limits.
This is supporting color evidence, not vegetation segmentation or a fidelity score.

The source correction substitutes existing `tree_green` frames for four existing
`tree_red` instances: rooted_tree_01, rooted_tree_02, grove_north_west and
grove_north_gap. IDs, x/y, dimensions and depth remain exact. It adds no props,
textures or fonts. Existing yellow/green crowns, northern façades, main venue,
two wings, ten gardens, service labels and actual route intents remain unchanged.
Only PARADE/MAP is covered by the supplied visual evidence; other eras and every
World view remain unchanged. No physical plan or arrival claim follows.

The source-alpha base difference is below0.01 source pixels for each replaced
instance. This is a frame-sampling calculation, not native occlusion proof.
Existing source/character/PNG/geometry/foot/authority data remain immutable.
The actual saved comparison raster is read from its published Git blob without
creating an image copy or starting a browser/render/export job.

Reproduce source analysis and metadata checks with:

```sh
python3 -B frontend/public/world/reference-scene-v2/map-canopy-palette-v1/build.py --verify
```

The handoff closes four source payloads. Root needs a separately recorded exact
consumer scope to adopt this optional source overlay. It must not alter or block
the already sealed service-label/action/wings bundle. Actual corrected native
palette, canopy visibility,360/390/430 output and attached-reference comparison
remain NOT_RUN. Spatial density, framing and overall target fidelity remain open
under the same unfinished ART goal; metadata remains BLOCKED.
