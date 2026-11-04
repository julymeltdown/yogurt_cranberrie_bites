# Spatial-v3 / quality-v6 original scenery candidate

**CONTRACT_READY candidate; NOT_RELEASE_BOUND; target fidelity IN_PROGRESS.** Required follow-up to the upper-district mismatch, using one separately versioned geometry and three distinct original skins. All published v3/v4/v5 source files and pins are preserved.

Start with `manifest.json`. It pins `../spatial-v3/geometry-proposal.json`, the explicit integer projection, all source PNGs/prompts, and each theme's closed C06 scenery/layout/MAP metadata/resources/provenance. Geometry is `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`; it must never mix with geometry `20168…` collision or release metadata. The spatial handoff explains each root, cell and camera change.

The new original source sheets address concrete comparison findings: twelve varied oblique urban halls (four identities × three eras), three matching diagonal-front concert stages, and quieter pale paving/olive material samples. Other frames use audited own original v3/v5 sources, with explicit crop/resample provenance. PARADE restores the original bright red canopy sample and all three reuse it at a second fixed upper root. No characters, supplied original artwork, old external scenery or source PNG pixels are rewritten. `sources/prompts.md`, `sources/stage-prompt.md`, `generation-index.json` and `source-metadata.json` record actual inputs/output hashes and measured dimensions; generated bitmap contents are not native vector art.

| Theme | Combined runtime bytes | Combined pixels |
| --- | --- | --- |
| REVENGE | 1707562 | 2043904 |
| PARADE | 1727221 | 2043904 |
| DANGER | 1712974 | 2043904 |

All fit existing 2,097,152 byte and pixel limits. Each loads only the shared768x1296 RGB `ground.png` and its own1024x1024 RGBA `{era}/atlas.png`; native2048 transparent padding is not a runtime fetch. Standard C06 Scenery v1 projection remains closed and includes22 WORLD objects. `{era}/layout.json` and `map-metadata.json` explicitly carry the new geometry, six noncollidable MAP illustrations, shared green semantic Plaza, stage display label override and unchanged existing place records. Semantic MAP art is never a source of physics or new IDs.

`{era}/{era}-{world,map}.tsrct`, editable JSON and native PNGs provide six actual editable projects. Named SVG wrappers and editable ground patterns are also included. Every native project packages exactly two verified image files; native-only transparent padding avoids the observed CLI0.4 image-mask clipping. Original generated source sheets remain separate, preserved rasters. Source sampling, masks, geometry and placements are independently editable.

Verification: exact source/manifest/resource hashes and per-channel nearest-neighbor crops; closed current C06 schema; all three resource budgets; exact proposed physical placements/footprints; MAP separation; six native packaged-asset hashes and lower-parcel render regression. Spatial verification checks all3,888 canonical cells and12 routes. Browser comparison checks36 cases (two era cycles × three widths × WORLD/MAP),216 clear-foot samples, one canvas, no errors/overflow at360/390/430px. The actual supplied target crops appear directly above the candidate in captures. Characters are intentionally absent from candidate art; their selected source-pack rendering belongs to the app lane. Static foot checks do not establish moving-avatar occlusion.

Paving's adjacent-pixel luminance difference drops about65% at the same128x128 material sampling, a narrow measurement of quieter texture. The semantic MAP stage shrinks406x292→300x210, and its canonical upper-district location changes with the new geometry. These measurements are not a fidelity score. Native/mobile comparisons were inspected; no fourth critic panel, 8.5 pass, final reference acceptance, actual app/Phaser/device/multiuser release proof or physical venue qualification is claimed. Perimeter density and final in-app composition remain required fidelity work.

Original AR01/BE08/FE19 distinguish concept objects from PHYSICAL destinations. All existing virtual place records stay exactly CONCEPT_ONLY/UNREVIEWED; no actual PHYSICAL place/source request arises from this virtual-art increment. A top-level map/theme art-source guard is separate from physical authorization and must be honestly qualified by its owner. No forged REVIEWED metadata, silent reseal, contract bypass or removal of physical flows is authorized.

The runtime/** canonical static resource prefix stays reserved to WORLD. Ready adoption needs explicit server/client map-version and resource pins plus a concrete consumer assignment, without making the independent v4 source-fixture delivery wait for this candidate. No runtime artifact or root application path is modified here.
