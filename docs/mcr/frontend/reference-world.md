# FE-REF-WORLD — scene and movement presentation

Status: **MOCK_READY** for the bounded production renderer/input fixture. Integration and remote ancestry are pending the frontend king. Actual ten-route root, canonical Auth/real socket, approved self-avatar, mobile device and multiuser acceptance are separate gates.

## Assignment and provenance

- Worker checkout: `fe-ref-world`, branch `codex/mcr-fe-ref-world`, base `f837bef676cd26869c26c172a699a91b7842d832`.
- Exact production ownership: `frontend/src/world/scenes/MapScene.ts`, `frontend/src/world/illustrated/phaser.ts`, `frontend/src/world/input/**`. Only MapScene, input/controller and input/controls CSS changed. The illustrated renderer needed no texture/depth changes.
- New tests/evidence: `frontend/src/world/__tests__/reference-world/**`; this document is the only documentation change.
- Viewed the exact supplied ten-screen PNG using `view_image`: SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. Top-row third composition supplies the visual target. Its levels, currencies, character conversations and physical routes were not implemented as invented data.
- Scenery is the existing hash-checked `public/world/illustrated-runtime/bindings.json` release, referencing source revision `19d7085` / integrated `bc60edb`. The existing immutable PNG resources, source originals, C06 geometry and bindings were not changed. The native report pins these bindings and geometry. It records actual generated fixture map/theme hashes as well as input source hashes.
- C06 schemas remain under `contracts/assets/v1/`; C05 policy remains `contracts/realtime/v1/policy.json` (`FROZEN_V1_0`). No contract or dependency change. Existing TypeScript/Phaser 3.90.0 stack retained. No old UI, character fallback, source-pack extraction, art generation or new visual dependency.

## Result

Movement controls occupy the scene rather than a separate 136px row. The closed presentation has a 64px joystick (56px on short screens), an Interact action, a 44px recenter crosshair and a 44px help disclosure. Essential labels remain accessible. The help popover contains the native nearby selector, zoom controls and complete keyboard/tap guidance. It scrolls within a bounded area. Escape closes it and returns focus to its summary. Empty nearby selection is explicit from the first refresh. Failure feedback stays visible outside help; routine movement status remains polite and visually hidden.

The native disclosure's DOM summary retains the accessible name `Controls & help`; its visible glyph is `?` / `×`. Browser automation uses `.world-controls summary` because this installed Chromium/Playwright pair does not resolve its native disclosure through `getByRole('button')`. Enter and Escape are actually exercised.

Before any server self-position or explicit view target exists, MapScene frames the existing `stage` object only when the map also has `stage_info`. The startup view shifts right/down from that object's center so the stage reads on the left and above the viewing center. It preserves native 1× pixels on the mobile fixture. World bounds still constrain the camera. All explicit camera targets override this startup framing. The production composition's C05 self-position camera fence is unchanged: local movement cannot steer it. Recenter still returns to the actual supplied self target. This never changes spawn, player positions, geometry, collisions, interactions, room or membership.

The texture upload/filtering/foot-depth/atomic replacement/cleanup paths remain unchanged. Era changes keep the same camera and geometry and one incumbent Phaser game. No YOU marker is fabricated: the fixture has no qualified self-avatar. A future supported self consumer and approved art remain their owners' work.

## Root integration seam

The input attachment adds/restores `.world-controls-host` on the existing `options.controls` element and `.world-controls-stage` on `context.host`. It retains the existing DOM port contract; it does not move controls into a different owner or alter composition code.

Root owner confirmed these shared CSS hooks, supplied independently:

- `.world-composition` positioned relative.
- Direct stage flexes with minimum height 120px.
- Direct `.world-controls-host` is an absolute overlay with `inset: 0 0 34px`; the truthful summary remains a compact 34px flow region below the scene.
- Host and root input section have `pointer-events:none`; only actual buttons, select, summary and secondary content receive pointer events.
- The input section already uses `position:absolute; inset:8px` (4px on short screens). Do not add `height:100%`, which would extend it past the bottom inset.

Root/shared CSS/composition files were read only. The native fixture uses an explicitly labeled equivalent host, with 44px test header, 44px era row and 44px qualification footer; it is **not** the production root. The frontend king owns actual root-plus-feature and all-ten-route proof after integration. Production scene height depends on that combined root, its status and actual viewport.

## Native evidence

`frontend/src/world/__tests__/reference-world/evidence/native.json` contains final source SHA-256 pins, exact image hashes, Chrome version, measured geometry, actual camera transforms, map/theme identities, allocations, assertions and cleanup. Eight JPEGs total roughly 0.5MiB; no large duplicate capture set. Native Chrome: `156.0.8078.80`, CSS pixel density 1.

| Width × height | Actual visible scene | Height at short 430px viewport |
|---|---:|---:|
| 360 × 800 | 668px | 298px |
| 390 × 844 | 712px | 298px |
| 430 × 932 | 800px | 298px |

Assertions cover full startup stage extents, native zoom, no horizontal overflow, scene clipping against ancestors, all closed controls ≥44px, help Enter/open/close/Escape/focus return, bounded short-screen secondary-control focus, local keyboard movement, neutral intent after release and pointercancel, blur cancellation, blocking-panel pause/resume, real camera recenter and projection roundtrip, all three era replacements with unchanged geometry/camera and one game, input-consumer error presentation and complete disposal. The companion `?input=1` fixture uses existing synthetic terrain, production input/Phaser/runtime, actual selectable nearby target and exactly one Interact intent. It supplies no substitute character.

Visual inspection viewed the supplied reference, historical landmark baseline and final normal/expanded/short/error evidence. Relative to the baseline, the stage is fully readable and controls stop taking flow height. This does **not** establish near-identical art fidelity: current art is an orthogonal 1152×896 map with 29 bound scenery objects, sparse lawn and a 224×151 stage. The target has a diagonal, much denser stage/foliage composition and approved characters. Renderer-only framing cannot supply those pixels or safely change projection/interaction geometry. The frontend king has a separate source-art proposal owner. No unqualified replacement is consumed here. At an actual distant server self-position the camera follows that position, even if the stage leaves the view.

## Validation

All commands below completed with exit code 0 on the final production source:

- Pinned Node `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`.
- `node frontend/src/world/__tests__/reference-world/native.mjs`: three scenery widths, three short viewports, positive synthetic selection/Interact, eight images, no page errors.
- From `frontend`, `node node_modules/vitest/vitest.mjs run src/world/__tests__/movement.test.ts src/world/__tests__/controls-layout.test.ts src/world/__tests__/reference-world/controls.test.ts src/world/__tests__/illustrated/preparation.test.ts`: **101/101 tests**, four files. Qualified existing Python supplied through `MCR_FE14_PYTHON=/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python`.
- `node node_modules/typescript/bin/tsc --noEmit --incremental false`.
- `node node_modules/oxlint/bin/oxlint src/world/input/controller.ts src/world/scenes/MapScene.ts src/world/__tests__/reference-world`.
- Vite production `build({configLoader:'runner',build:{outDir:<mkdtemp>,emptyOutDir:true}})`, temporary output removed. Existing large-chunk warning remains; no build failure.
- `node scripts/mcr/check-ui-origin.mjs`: no violations. Static origin checks do not replace visual review.
- `python3 scripts/mcr/check-plan.py`: original checksums, nine exporter probes per source, task/IA structure pass; not application acceptance.
- `git diff --check` and exact assignment/path review.

No install or dependency copy. The only temporary own `frontend/node_modules` symlink points to the existing delivery dependencies and is removed at handoff. The own checkout was sparsified using the king's exact instruction to skip backend and historical PNG evidence; original tracked bytes are unchanged and no deletions staged. Disk was checked above the 8GiB floor before bounded validation. No global cache/worktree/data deletion.

Initial harness corrections were missing schema filename, unsupported WebP screenshot output (changed to JPEG), and native-summary role lookup (changed to its DOM selector and explicit label). One test-authoring command used a root-relative path from the frontend directory and wrote nothing; corrected immediately. Early harness failures are not passing proof. Final report/images and command outcomes correspond to the final source.

## Delivery

Implementation, scoped validation, evidence and this handoff are committed together. The frontend king integrates and publishes; this writer does not push. Record authored and integration SHA mapping in the king's handoff. The active unbudgeted goal remains unfinished until published ancestry is verified and concrete review fixes are resolved. Live/device/real multiuser acceptance remains unclaimed.
