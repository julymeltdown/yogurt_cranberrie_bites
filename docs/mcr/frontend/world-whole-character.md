# Whole-character avatars in the existing World scene

Task: `FE-WORLD-WHOLE-AVATAR`. Status: `INTEGRATED` for the bounded consumer qualification below. Root selection, authenticated live admission and multiuser acceptance remain `NOT_RUN` by this task.

## Ownership and ancestry

The isolated task branch is `codex/mcr-fe-world-whole-avatar`, initially based on master `391bc21525453f05d809a45eb4a9b4966a70c2ae`, reconciled through `8430cd57ebb628fc093cb43f5ab547f042440830` for validation. The later reconciliation through `6ccb4e8f` adds peer Auth/profile implementation, qualification and coordination documentation; all native source/artifact pins still match, and full frontend types were rechecked. The implementation commit containing this document is the source handoff; its publication receipt is reported with the verified remote ancestry.

Exclusive changes are `frontend/src/world/avatar/whole-character.ts`, `frontend/src/world/avatar/scene.ts`, `frontend/src/world/scenes/MapScene.ts`, new tests/evidence under `frontend/src/world/avatar/__tests__/whole-character/`, and this document. No root composition, transport/decoder, contract, layered renderer, producer, art, dependency or source archive was modified. All new rendering logic is authored here; no old UI code was copied.

The consumer uses published producer `9fbc89426c6d0781c6f7a1f1a78ef722656407e5` and the real C05 union decoder compatibility already in its ancestry. Whole-character contracts are frozen at `3398d833`; reviewed art source revision is `7e07f7328f079991a9aedb7ad83a4b682decb543`. Manifest `mcr_selected_static_20261103` version 1 has canonical content hash `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`. The untouched compact atlas is 384×64, 36030 bytes, SHA-256 `e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe`. Native evidence pins the exact producer, contract, adapter, scene and image bytes.

## Consumer behavior and root hook

Import `createWholeCharacterWorldRenderer` from `world/avatar/whole-character`. Root chooses the renderer using the admitted manifest and supplies `resolveRuntime(context)` returning its already-admitted, fixed-scope borrowed runtime. The exported `BorrowedWholeCharacterRuntime` exposes only scope, currency, validation and acquisition. The adapter never constructs or disposes that runtime. A missing, incompatible or stale owner fails closed. Root integration remains separately owned.

The adapter uses the incumbent `AvatarScenePort` and scene frame callback, canonical replication entity coordinates, foot anchor (32,60), and existing scenery depth. It creates no Phaser game, connection, prediction, timer or animation loop. All six presets use their genuine front-idle cell and frame 0, including while the canonical player is moving or facing another direction. No unprovided walking or directional frames are synthesized.

The new optional `staticTexture64` scene method accepts exactly one 64×64 cell and frame 0. The existing `texture` method retains its exact 320×256 sheet and 20 frames. Scene-owned canvas copies prevent texture disposal from erasing shared decoded image/cache pixels. No padding converts a static cell into a legacy sheet.

Rows share one consumer lease and GPU texture per validated config. Removing the last row releases that config's resources; avatar revision/config and room/epoch changes evict prior resources. Invisible, closed, resyncing, suspended, missing or invalid rosters clear GPU allocations. Capacity, duplicate identities, finite map bounds and scope generation are checked before admission. Abort-resistant late acquisitions are released without painting; synchronous privacy lease revocation clears textures, sprites and scene subscriptions immediately. Failed assets do not start a new request on every frame.

## Validation

All listed commands exited 0 after consuming the published producer. Node 24.22.2 used existing delivery dependencies via a temporary symlink; no installation, copy or lockfile change occurred. The resolved existing Vite was 8.1.3. Browser/build work observed the 8 GiB disk floor. Temporary build/cache directories and the dependency symlink were removed after validation.

| Check | Command / execution | Result |
| --- | --- | --- |
| Unit and compatibility regressions | From `frontend`: `node node_modules/vitest/vitest.mjs run src/world/avatar/__tests__/whole-character/renderer.test.ts src/world/avatar/__tests__/renderer.test.ts src/world/avatar/__tests__/contract.test.ts src/world/__tests__/runtime.test.ts src/world/theme/__tests__/replacement.test.ts src/entities/avatar/whole-character/__tests__/runtime.test.ts src/world/network/__tests__/transport.test.ts` | 381 tests / 7 files; 20 new adversarial renderer tests |
| Full frontend types | `node node_modules/typescript/bin/tsc -b --pretty false` | PASS |
| Scoped lint | `node node_modules/oxlint/bin/oxlint src/world/avatar/whole-character.ts src/world/avatar/scene.ts src/world/scenes/MapScene.ts src/world/avatar/__tests__/whole-character` | PASS |
| Native incumbent engine | `node src/world/avatar/__tests__/whole-character/native.mjs` | PASS, 360×800 / 390×844 / 430×932 |
| Production bundle | Vite `build({configLoader:'runner',build:{outDir:<mkdtemp>,emptyOutDir:true}})`; output removed in `finally` | PASS; existing large-chunk warning |
| UI origin | From repository: `node scripts/mcr/check-ui-origin.mjs` | PASS, no violations |
| Planning provenance | `python3 scripts/mcr/check-plan.py` | PASS; original checksums/exporter probes preserved |
| Patch hygiene | `git diff --check` and exact scoped staged-path review | PASS |

The existing runtime tests use qualified `MCR_FE14_PYTHON` with the installed contracts-validation environment. The new unit tests cover config refcounts, six identities, invalid phases, cancellation, late completion, changed revisions, privacy invalidation, coordinates, capacity/duplicates, invisible/missing samples, generation and epoch replacement, malformed crops, reentrant texture teardown and callback-registration abort.

Native `evidence/native.json` records browser version, source hashes, RGBA cell hashes and image captures. It uses actual reviewed PNG bytes, the published shared runtime, canonical Auth/privacy controller with an isolated identity fixture, real C05 Snapshot validation and the production Phaser scene. Every width verifies six distinct cells sharing one decoded compact image, six config textures/sprites, exactly two asset requests (descriptor and compact PNG), no detail request, canonical movement, all three eras with unchanged geometry, visibility clear/remount without refetch, config deduplication to five textures, synchronous privacy clear, and zero remaining textures/sprites/listeners/canvases after disposal. One game mounts and destroys once. The static/legacy scene contracts reject seven invalid size/frame combinations while retaining legacy frame 19.

The three captures were visually inspected: all six selected identities are visible without clipping, substitution or fabricated animation. The fixture deliberately shows the qualified existing scenery; this task does not claim adoption of later scene proposals or completion of the full ten-screen product target.

## Remaining acceptance boundaries

The deterministic roster is admitted through the real decoder but has no live socket or Auth server. This evidence does not certify root renderer selection, save/reload identity persistence, real multiuser presence, authorization/capacity enforcement on the server, physical devices or release readiness. Those remain separate integration and acceptance work; a passing isolated native scene must not be used to close them.
