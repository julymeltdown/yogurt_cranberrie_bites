# Architecture-v2 consumer adoption

Status: source implementation and non-native checks complete; native qualification pending a newly allocated capture slot. This is a bounded AR02/AR04 and FE06 contribution. Whole task completion, exact reference fidelity, device acceptance and production admission remain open.

## Source and authority

The consumer base is published `9530caa3fa6a31fd86a5d060d6b8adf7eda77adf`. Artist source `1bcf52081f8c6ed7dfdc0dceba90e7d5155579fa` was published as `34c8300c`; deterministic atlas source `585baf9fae121ed68b26ec886221c0171e28b799` was published as `921cb3d850925a489302ebcc5483fba71ac835a5`. The architecture handoff is 8,302 bytes, SHA-256 `98484ffd7c3301f7ba3b8cc5221be29d48386ec1dcc2eb67015c2dffc76a5fc0`. Its 18 outputs total 7,921,243 bytes. These source assets are immutable inputs to this patch.

The optional reference consumer now requires those exact architecture atlases for its existing ef52 scene. Its stale old-atlas and mixed-resource candidates fail before resource fetch/decode, even if the caller recomputes manifest hashes. A reference claim using an architecture path also fails closed when its geometry is foreign. The ordinary resource loader retains image hash/size checks and disposal ownership.

`build-architecture-inputs.py` closes every output pin, compares the published frames/placements and normalized scenery to the frozen base, checks per-era byte/pixel budgets, then writes exactly `architecture-source.json`, `architecture-pin.json` and the supplemental `runtime/architecture-v2/source-binding.json`. It never runs either older builder. The binding remains `NOT_RELEASE_BOUND` and `NOT_ADMITTED`; the geometry source revision remains historical base provenance. Existing UNREVIEWED/CONCEPT_ONLY source-fixture provenance is preserved.

| Era | Runtime atlas bytes | Atlas SHA-256 | Ground + atlas bytes |
| --- | ---: | --- | ---: |
| REVENGE | 1,272,098 | `108d5483711e6bd755b328d08d3535141954ef81adc6eabfd066a88d8fe278aa` | 1,821,253 |
| PARADE | 1,284,429 | `aab0383b8963f1b681acf81d22d2be88e130744554311e74f0793efbe388e529` | 1,833,584 |
| DANGER | 1,271,456 | `20f218966eb17f50992191f1128b45ca02aac9b00a1c1f39ba71637b68afa29d` | 1,820,611 |

Each era remains below 2 MiB and uses 2,043,904 source pixels. Ground is the existing 549,155-byte, 768×1296 image with SHA-256 `dd70d8fb02e1c6e57e9a9f9e96d69dc7c15999bc076678a7075161281c929887`.

## Frozen behavior

The source geometry hash stays `sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395`. Original source-inputs, geometry revision, tile pin, backend allocation, canonical Tiled bytes, original source binding, cameras, collision/feet, all 34 World placements and 49 MAP representations remain unchanged. All 16 packed frames remain unchanged; only the four building-frame pixels change. The other 12 frames and every pixel outside those four rectangles are independently checked byte for byte in all three eras.

The label/landscape generators and all four generated MAP sidecars remain unchanged. The MAP landscape check still compares its immutable source atlas against the BASE descriptor before the separately pinned architecture replacement. New World and MAP scenery use the same effective atlas ID; overview frame crops, display anchors, exact label rectangles, font, icons and all four place IDs are retained. No new destination, REVIEWED provenance, network/authority behavior or backend reservation is inferred.

WorldShell, shell CSS, composition, Motion, Root/router, loader, Phaser and MapScene are outside this patch. The separately staffed World presentation and portrait changes are not included in this base.

## Verification

- 286 affected regressions across 26 suites passed, including all 48 initial focused architecture/model cases. Added negative tests cover stale/mixed/cross-era atlases and foreign-geometry claims before resources.
- All 18 published outputs and nine frozen input pins match SHA-256 and size. All three generated outputs reproduce byte identically.
- An independent 8-bit RGBA PNG decoder confirms zero different channels outside the four fixed building rectangles in every era; pixels inside the building rectangles do change.
- Full TypeScript `--noEmit --incremental false` passes with zero diagnostics; scoped lint passes with zero warnings. The first type invocation used a nonexistent config path; the correct `tsconfig.json` invocation then exposed four readonly assignments in negative tests, which were fixed before the passing run.
- Plan/archive checks pass. No production build is claimed; native Vite compilation is a separate pending check.

New native drivers under `app/world-entry/__tests__/reference-scene-v5/architecture-v2/` use the immutable published ART scenery as their independent composition oracle. They do not call the older MAP oracle, which still intentionally describes the old architecture. New editable SVGs carry explicit architecture handoff/atlas provenance and preserve their immutable layout-v2 source lineage; the original SVGs and their qualification are not modified or inherited.

## Remaining boundaries

Actual targeted native World/MAP comparisons, Root navigation/cleanup, 360/390/430 viewport screenshots, compact-marker focus and new editable renders will be recorded under the new evidence prefix after an actual fresh resource grant. Desktop Chrome viewports are not iOS Safari, Android Chrome or physical keyboard/notch acceptance. Synthetic map7005 is not backend allocation6. Backend allocation6 is source identity only and NOT_ADMITTED. Live Auth/server/multiuser, production admission and final target fidelity remain separate.

The Artist reports taller occupied building bodies, but the west building remains narrower and alpha edges retain source limitations. Taller source pixels alone do not establish the requested scene density, target massing, legibility or occlusion acceptance. The old layout-v2 captures show OLD architecture and are not proof of this patch.
