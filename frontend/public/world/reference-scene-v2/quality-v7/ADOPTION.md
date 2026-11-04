# Urban venue correction v7

Status: **CONTRACT_READY art input; visual fidelity IN_PROGRESS; not release-bound.**

This required source correction replaces the four cream cottage-like building frames with original broader brick cultural venues. It does not modify any v6 input or the spatial-v3 geometry. The supplied ten-screen target was used for architectural direction only. No reference, character or old application scenery pixels were extracted. The original built-in imagegen output is preserved at `sources/venue-sheet.png`; `sources/prompt.md` and `generation-index.json` record the exact prompt and input roles/hashes.

The manifest SHA-256 is `156d416606e648dc357a686bc1fd18ec67e425c32ee0955e164277ab229d1e87`. Paths and per-file hashes are in `manifest.json`. Each `{revenge,parade,danger}/` supplies `scenery.json`, `layout.json`, `map-metadata.json`, `resource-inputs.json`, `provenance.json` and a 1024x1024 RGBA `atlas.png`.

| Era | Combined runtime bytes | Atlas SHA-256 |
| --- | ---: | --- |
| REVENGE | 1,712,617 | `7cb3dc89ef4afd11914141ac14cf748b84c28b0acc42fc3de615477e42492dcf` |
| PARADE | 1,733,123 | `99fd6891a0e8b6fba627e6c92e3aba24726cffe84500169f5f4b8e766d8ccbdf` |
| DANGER | 1,715,601 | `589def1802e6c7ab240b3766465de18ce214c9dc14811ed27e5c460c83819942` |

Every era deliberately reuses the frozen v6 ground at `world/reference-scene-v2/quality-v6/ground.png`, SHA-256 `e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795`, 768x1296 RGB, 531,480 bytes. Each scene fetches exactly this ground and its own atlas: 2,043,904 pixels, under both existing 2,097,152 budgets. There is no extra runtime texture, budget increase, palette quantization or v6 alias substituted for a missing era.

Only `meeting_pavilion`, `giveaway_pavilion`, `north_hall` and `west_hall` frames change. Their original source crops are resized uniformly with nearest sampling into bottom-aligned transparent frame boxes. This preserves the lower urban facade silhouette within the existing destination rectangles. Other twelve atlas cells are byte/pixel identical to v6. Exact transforms and padding are recorded; `verify-quality.py --revision 7` reconstructs entire changed atlas cells and compares every RGBA channel, checks the retained cells, source pins, budgets and current closed C06 schema.

The geometry hash remains `sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`: 48x81 cells of16px, all22 WORLD placements/footprints, six MAP representations, two spawns, four interactions, collision, terrain, cameras and labels remain frozen v6/spatial-v3 values. All four truthful virtual-concept place records remain unchanged. `venue-shape-evidence.json` measures all30 affected projected physical/overview instances across three eras: actual opaque bases remain within3.667px above authored depth. This is source-root alignment evidence, not new collision authority or moving-avatar proof.

Six native `.tsrct` files, independently editable named masked-image layers, JSON, SVG and actual768x1296 PNG renders are included. Native-only2048px transparent atlas padding preserves the prior lower-map mask fix; each actual ZIP contains exactly two correct image assets. The viewer open returned queued, so delivery uses the actual CLI0.4 exports and source files. `environment.json` and per-era `native-evidence.json` record tool versions and command results.

`MCR_ART_REVISION=7 node check-quality.mjs` passes36 comparison cases and216 clear-foot samples at360/390/430px, two complete era cycles, one canvas and no overflow/page errors. Actual three-era MAP and PARADE WORLD screenshots and native MAP renders were inspected against the unmodified target crops. This is static asset comparison, not current application/Phaser/server/device/multiuser acceptance. No fourth scored panel or8.5 pass is claimed. Perimeter clustering and final reference composition remain required in the active art goal.

Adoption is explicit source work for the assigned runtime/backend/root owners: retain the existing spatial-v3 map geometry, publish the selected new theme resource pins and use the matching v7 frame/MAP metadata. This package does not update the published v6 producer or any runtime descriptor/release binding. It does not require a new Tiled/collision revision or a blanket physical venue/operator prerequisite for virtual art. Do not silently switch existing consumers, forge place review states, or present virtual concepts as verified physical destinations. Art does not write the reserved `runtime/**` prefix.
