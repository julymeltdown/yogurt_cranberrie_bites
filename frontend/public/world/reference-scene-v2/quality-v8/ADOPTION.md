# Natural perimeter artwork v8

Status: **CONTRACT_READY art input; final source/consumer composition IN_PROGRESS; not release-bound.**

`manifest.json` SHA-256: `0aa7ffc44d4fa69ad8d0ac9dfb8ab30907ce0e785aa461a03a1452b8461518d6`.

The v7 comparison still had sparse ornamental trees. This version replaces the four tree frames with an original natural canopy quartet and adds twelve fixed unbound grove/root groups in explicit spatial-v4. The stone planting rings disappear; connected foliage now frames the scene. All original v7 venues, stages, booths and other non-tree atlas cells stay pixel-identical. Shared neutral canopy art is intentional across the three era-specific atlases. There are no new characters, physical destinations or place identities.

The built-in imagegen original is preserved byte for byte as `sources/canopy-sheet.png`, SHA-256 `29a05b4c32508fea5af551f207b7ab28a3c451cc71baf2ecbcfe77c3567e2d95`,1,699,886 bytes. It was generated with no image inputs. Exact prompt, generation filename, reference-comparison role, alpha preservation and transforms are recorded in `sources/prompt.md`, `generation-index.json`, `source-metadata.json` and per-era provenance. No supplied/reference/character/legacy pixels were extracted. Packing uses only declared alpha-bounded crops, uniform nearest resampling and transparent native-only padding.

| Era | Combined runtime bytes | Atlas SHA-256 |
| --- | ---: | --- |
| REVENGE | 1,735,321 | `ee93dcde891054f6ef965d182af8237a09d10464ba762d017168d2bd9326bccd` |
| PARADE | 1,746,508 | `0428f976140a3ee1d59368d3016efae605a6e2b7b1470233f523e7721ec806c1` |
| DANGER | 1,732,813 | `61948cb2f642de55721257fd4104fad4a7162a49c5fcf5bef9ed4d81d969ba12` |

Each runtime scene still fetches exactly two PNGs: its1024x1024 RGBA atlas and the frozen v6 ground at `world/reference-scene-v2/quality-v6/ground.png`,768x1296 RGB,531,480 bytes, SHA-256 `e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795`. Total 2,043,904 pixels and all byte totals remain below the existing 2,097,152 limits. No extra texture, budget extension, palette quantization or source alpha reseal is used.

**New geometry is mandatory.** Normalized hash is `sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`. `../spatial-v4/ADOPTION.md` and pinned source/projection/revision evidence describe34 WORLD props, twelve new unbound solid roots and72 added blocked cells. Both views use this one geometry. Old22 placements/footprints, terrain, ground, cameras, spawn/interaction cells, six MAP illustrations, labels and all four truthful virtual-concept place records remain unchanged. This requires an explicitly allocated new server map revision greater than 4 (proposed 5), actual new canonical Tiled and matching theme-resource/consumer pins. Existing spatial-v3/map 4/v6 and all prior source bytes remain frozen. Art writes no `runtime/**`, backend or application file.

The first uncommitted eastern root obscured part of the Culture Park plaque. Actual image inspection caught it despite clear named feet. That group now sits below the plaque; a protected text-region alpha regression checks every era. `verify-quality.py --revision 8` passes current closed C06, all 34 ordered object descriptors, all source/resource hashes, exact all-channel crops, retained twelve non-tree cells, all budgets, canopy/root alignment, protected plaque text, six actual native ZIP asset pairs and exact editable image-layer membership.

Six native WORLD/MAP `.tsrct` projects, named masked-image layers, JSON/SVG sources and actual 768×1296 PNGs are included. The native-only 2048px transparent atlas padding preserves the lower-map mask fix. New lower-venue color probes compare actual near-opaque original source samples against each exported native PNG, so new tree pixels cannot hide a missing building. All2,784 selected samples pass across six exports; the original248..253-alpha source art is preserved. The native viewer open returned queued; the actual CLI 0.4 PNG exports were inspected.

`check-quality.mjs` with `MCR_ART_REVISION=8` passes 36 actual source-reference cases / 216 clear feet at 360/390/430px through two era cycles, one canvas, no overflow or page errors. All12 grid routes pass. `density-measurements.json` measures depth-composited dominant tree alpha: MAP perimeter bands have about 2.01× the prior PARADE coverage, whole MAP 1.84× and WORLD 1.28×. These are source-density changes, **not target match percentages or critic scores**. No fourth scored panel or 8.5 pass is claimed. Native and mobile reference comparisons are static art evidence, not application/Phaser/server/device/moving-avatar/multiuser acceptance.

Perimeter density now has a concrete source/geometry correction. Final consumer composition and concrete handoff findings remain open in the active art goal. Current Root publication and its reserved FE09 serial handoff continue independently. New runtime/backend ownership is assigned from the concrete published handoff; this file neither revives archived workers nor silently activates a release.
