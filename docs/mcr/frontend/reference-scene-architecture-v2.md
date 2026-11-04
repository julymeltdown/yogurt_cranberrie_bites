# Architecture-v2 consumer adoption

Status: bounded consumer implementation and native qualification pass at `4cb162b71133446e73533ce3471b06fffea6327b`; serialized publication and fresh remote readback follow. This is a bounded AR02/AR04 and FE06 contribution. Whole task completion, exact reference fidelity, device acceptance and production admission remain open.

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
- Plan/archive checks pass. No production build is claimed; the actual Vite/Root/Phaser module graph compiled and ran in the passing native workflow.

New native drivers under `app/world-entry/__tests__/reference-scene-v5/architecture-v2/` use the immutable published ART scenery as their independent composition oracle. They do not call the older MAP oracle, which still intentionally describes the old architecture. New editable SVGs carry explicit architecture handoff/atlas provenance and preserve their immutable layout-v2 source lineage; the original SVGs and their qualification are not modified or inherited.

## Remaining boundaries

Actual targeted native evidence is recorded under `docs/mcr/evidence/reference-scene-v5/architecture-v2/native-04/`. Nine World/MAP cases at360/390/430 have strict zero differing RGBA channels against the independent published ART composition. All90 canonical clear-foot building-depth probes also have zero differences. Nine actual Root MAP cases, two compact-marker focus cases and three editable SVG/font/geometry renders pass. All49 MAP objects, six label rectangles, four place IDs and teardown invariants remain intact. Desktop Chrome viewports are not iOS Safari, Android Chrome or physical keyboard/notch acceptance. Synthetic map7005 is not backend allocation6. Backend allocation6 is source identity only and NOT_ADMITTED. Live Auth/server/multiuser, production admission and final target fidelity remain separate.

The Artist reports taller occupied building bodies, but the west building remains narrower and alpha edges retain source limitations. Taller source pixels alone do not establish the requested scene density, target massing, legibility or occlusion acceptance. The old layout-v2 captures show OLD architecture and are not proof of this patch.


## Retained diagnostics and resource release

Native04 ran2026-11-04T05:14:52.719Z–05:15:45.463Z under explicit `ARCHITECTURE-NATIVE-202611040508Z`, after IN01 and the earlier presentation holders released. Fresh free space at acceptance was38.442GiB with zero competing validation Chrome. Minimum observed free space was36.317GiB; sampled maximum new whole-prefix storage including owned browser profile and Vite cache was56.938MiB. Final durable evidence remains below30MiB. The one browser/server, owned profile/cache and borrowed dependency link are closed/removed; the allocation is explicitly released. No production build, install or Cargo workflow ran.

Native01 completed REVENGE360 before the following case failed at50 differing channels. Native03 retained actual/expected PNGs and exact pixel samples. Synthetic building-depth probes had changed stable paint order among overlapping equal-depth test avatars: restoring identical feet does not restore prior display-list order. Native04 isolates each era and captures before deliberately mutating the diagnostic view, then records actual before/after paint order. No production artwork, geometry, feet, camera or pixel threshold was changed to obtain a pass. Native02 was a preflight-only failure: a process predicate matched its enclosing shell command text; it now checks the Chrome executable prefix. All failed reports and sanitized logs remain retained.

The current capture packet closes82 native source pins, two supplemental source-at-candidate style pins and46 retained attempt artifacts. Original SVGs are byte-frozen. NEW SVGs change only their atlas and provenance metadata, with all remaining markup asserted equal. Actual original write timestamps and PNG dimensions are pinned for the requested screenshot handoffs.

The independently published World presentation `9356b943` changes WorldShell/CSS and adds WorldTools. It is not part of captured candidate4cb162b7. These source versions and qualifications remain distinct; no exact-pixel or native acceptance claim is made for eventual combined master. Combined publication checks cover the integrated source graph separately. Reference massing/density, all ten screens, live/backend6 admission and physical-device acceptance remain unfinished original obligations.
