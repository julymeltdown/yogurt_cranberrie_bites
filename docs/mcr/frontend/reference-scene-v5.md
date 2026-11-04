# V10 diagonal scene runtime adoption

Status: IN_PROGRESS. This is the bounded FE06/FE14/FE19, AR02/AR04 and QA06 contribution, based on published `93ef0d895dcc4b247b727e2cccba0bb84576b262` in `codex/mcr-fe-reference-scene-v10`. World `55a93289fcd4a2fbc6b94298c3b5b21dd769dc21` records the original leases. The later exact grants cover v10 Map fit/reset and only MapScene's refreshCamera presentation branch/import. The camera port policy and all earlier source/runtime versions remain unchanged.

## Source identity and authority

Artist publications `c7b0cb013e5ab1d9f8cac08be1d0046c249e1cd4` and `06562ff1962be525ed929ef431a54bb589860373` provide spatial-v5/quality-v10 geometry `sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`. The publication verified 107 paths and 105 closed pins; the runtime derivation rereads those pins. It verifies exactly 560 terrain changes, zero collision changes and unchanged 34 WORLD props, spawn cells, places, interactions, camera rectangles and plaza parcel. All three era resources are independently pinned.

The finite Tiled map is 24,058 bytes, `sha256:e5837dfe08a8d2f1f7cdb4428b87965b4a7c6ecdb1458b8fe910e3f4c2112078`. Three row-major 48×81 layers preserve terrain, collision and overdraw. Source checkpoint `473534296490bbcd4c63a4a0eb85ce7fdd57215a` originally carried a null/pending server revision and binding hash `86a5cf9891b77ed31054e4696ade8ae54092d5cbe1f7e8f442643be8b4192244`. That historical tuple has not been relabeled as an allocation.

Backend reservation `390bb9a57c2ede32b814e8c36f8d1c85a10a99a2` actually allocates `mcr_reference_scene_r3` revision **6**, source identity only. Its reservation bytes hash to `771fabbcc0243cff645a2f7588fcaa4d68f056663b2b3ede5d86e2024d213317`. The separate frontend metadata checkpoint `5d3e81f125dd6bf7f507dcf96914f6878c8fe6f4`, published as `135ab0fe`, pins that committed evidence. Its source-binding hash is `8cb8d18a9c6bc07eead30e7a2474ca5fb9adb3f23ddade911840aad6bf134041`; Tiled/art/geometry/source-input bytes stay unchanged. Neither reservation nor source binding selects a database/event-current release. Status remains NOT_RELEASE_BOUND and NOT_ADMITTED.

The MAP-only landscape source `f19027fa96abd90eb11ffda825deda95c3091647` adds ten unbound original shrub beds. Its separate consumer derivation verifies all 30 closed source pins and the existing era atlas/frame equality. The overview contains 49 objects: the previous 39 MAP representations plus ten decorations. WORLD stays at 34 props. No place, interaction or resource DTO is invented for planting.

## Implemented behavior and original criteria

| Original work | Delivered source behavior | Remaining acceptance boundary |
|---|---|---|
| FE06 World entry | Versioned root overview selection uses the incumbent owner and strict exact map/theme/geometry bindings. | Actual production source review, admission and live Auth/server journey remain separate. |
| FE14 scene rendering | Finite Tiled input, closed resources, scenery validation, shared default Phaser scene and packed-frame depth rendering. Old versions keep their own resolvers and semantics. | Final fractional-camera renderer qualification is pending. |
| FE19 Map | One current runtime overview, six painted labels, four existing place links, local fit/reset/zoom and list/legend/detail recovery. No World camera writes from Map controls. | Final compact-marker/caption capture rerun is pending. |
| AR02 source adoption | Exact artist ground/props/diagonal terrain and ten MAP-only planting decorations. Strict crops and stable foot-depth composition. | No original-art rewrite, physical-place verification or whole-product fidelity claim. |
| AR04 era adoption | Distinct pinned REVENGE/PARADE/DANGER resources, same geometry/places and guarded optional selection. | Original UNREVIEWED provenance remains unchanged; unqualified optional production eras stay denied. |
| QA06 bounded repair | V10 initial Map and Reset fit; two unpainted targets use 32px icon ink in 44px links with full accessible names; the caption occupies its own row. | Independent source comparison, actual devices and full QA06 remain open. |

`spatial-v5/model.ts` claims the new geometry/runtime/source namespace rather than swallowing old synthetic revisions. It rejects mixed old scene/geometry/Tiled/theme, foreign IDs, revisions below the new minimum, changed places/spawns/frames/depth and corrupt resources before publication. Loaded-but-superseded resources are disposed. Overview label ownership releases the old font owner before switching versions and cleans on private withdrawal.

The narrowly granted camera helper applies a presentation scale derived from the authored 584×584 World rectangle. Logical zoom 1–8 remains unchanged. Bounded look-ahead preserves the stage/self framing near spawn, then follows movement beyond that range; server feet, collision and input policy stay authoritative. Projection tests preserve the existing half-open canvas hit boundary. Source footprint extents use a numeric epsilon; interior pixel points and self must round-trip. Resize must leave authoritative self feet unchanged.

## Qualification and reproducibility

Use the retained Node 24.22.2 frontend dependencies and `MCR_FE14_PYTHON` pointing to the existing contracts-validation Python. No dependency install or lockfile change is required. Both derivation scripts live under `frontend/src/world/reference-scene/spatial-v5/` and reproduce the finite input/44 source pins and separate 30-pin MAP-only sidecar.

The broad regression run passes **366 tests in 33 files** after merging published peer input54e1325f, including all reference-scene versions, root v4/v5 contracts, map/theme/illustrated/scenes/avatar/Places. Full TypeScript and scoped warning-free lint pass. Planning/exporter checks preserve the 13 original archives. Native runners live in the new `spatial-v5` and root `reference-scene-v5` test prefixes; each enforces the 8 GiB disk floor and records the actual tested parent and source pins.

`docs/mcr/evidence/reference-scene-v5/root-qualified/native.json` records the first passing 14-case actual Root/default Phaser matrix: three widths × three eras, real Places navigation, initial fit/reset display measurements, 390×554 World and 390×754 Map captures, font corruption recovery, catalog mismatch/reload, production default denials and singleton/private cleanup. It predates the compact-icon/caption repairs and must not be presented as their final proof. Prior failed diagnostics and precise source pins remain identifiable separately. An independent QA06 inspection found the shortened Map caption covering GIVEAWAY BOOTH; the current source fixes the overlap by moving only the v10 caption into its own flow row.

The renderer's earlier camera-aware oracle passed era replacement/occlusion before failing at a 390px landmark on one sampled row (77 pixels, 208 channels). The threshold remains zero. Installed Phaser 3.90 applies the same GeometryMask in both its child loop and batchSprite; the independent oracle now includes both clips and completes asynchronous character acquisition before observing one frame. The final guarded rerun is pending restored disk capacity. No failing result is relabeled as a pass.

All fixture map revisions are synthetic **7005**, separate from actual backend allocation6. These are desktop Chrome source fixtures with actual production Root/default Phaser and synthetic HTTP/C05 peers. Live Auth/Gateway, server admission, two real accounts, physical iOS/Android, software keyboard/notch behavior, artist/product fidelity acceptance and release readiness are not claimed.
