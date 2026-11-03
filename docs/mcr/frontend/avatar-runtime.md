# FE12 shared avatar compositor and existing-scene renderer

Status: **MOCK_READY** for current C06 runtime behavior. Production artwork is
**unavailable**. This delivery does not accept AR03, the full FE12/art milestone,
FE14/FE16/FE17, real backend physics, device rendering, or IN01.

Base: `535667975bb441a2c87e878e197965f8c773f600`; branch
`codex/mcr-fe-FE12`; isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/fe-avatar-runtime`. The goal covers implementation,
checks, documentation, committed handoff and concrete fixes. Current coordination
routes commits/API requests to Delivery; main/World SL track progress and are not
mandatory code-review gates. No acknowledgement wait or push is required here.

## Requirements and source boundaries

FE12 in `MCR_IMPLEMENTATION_PLAN_V2_EN.md` requires deterministic composition,
frame/anchor mapping, texture caching, and shared profile export. Existing C06
v1.0.0 remains authoritative: `contracts/assets/v1/{avatar-manifest,avatar-config,
common}.schema.json`, `docs/mcr/assets.md`, and the source-pack revision 3 rules in
`docs/mcr/character-art.md`. Contract fixtures are metadata tests, not art.

Worker writes are `frontend/src/entities/avatar/runtime/**`,
`frontend/src/world/avatar/**`, and this document. Explicit coordinated exceptions
are the minimal capability/status changes in `world/runtime.ts`,
`world/scenes/MapScene.ts`, and `world/composition/index.ts`. Directly relevant
existing test extensions are exactly `world/__tests__/runtime.test.ts` and
`world/composition/__tests__/composition.test.ts`, under `frontend/src/`.

No root/router/main/ports/config, dependencies/lockfile, contracts/generated,
backend, character source-pack/catalog, supplied originals or art derivatives
are edited. No legacy renderer/art is imported. New render code is authored
here. Headless schema validation in `entities/avatar/runtime/schema.ts` adapts
the existing `world/map/schema.ts` evaluator at the base revision, registering
the checked-in avatar schemas instead of inventing another wire shape.
Existing canonical hashing, strict JSON, bounded PNG resources/decoder,
FE14 engine, FE17 projection and generated AvatarConfig types are imported.
Test HTTP/WS framing adapts composition's `__tests__/native.mjs` at the base;
synthetic terrain helpers and contract fixtures remain test imports only.

## Stable consumer API and pinning

`frontend/src/entities/avatar/runtime/index.ts` is the shared product entry:

```ts
const compositor = await createReviewedAvatarCompositor({
  document: bootstrap.document.avatar_manifest,
  pin: { manifest_id, manifest_version, content_hash },
  binding: reviewedAvatarBinding,
  resources: reviewedAllowlistedResources,
  signal: currentPrivateOwnerSignal,
})
```

Pin the exact current authorized C03 document and captured user/event privacy
lease. A fresh lease cannot authorize an old document. Each consumer owns its
instance; no global outfit cache or browser persistence exists. The caller must
abort/dispose on its privacy scope's invalidation. The shared builder binds its
compositor disposal to that signal after loading; aborted/stale loads release
their decoded image. Product world rendering uses this same builder.

The returned `AvatarCompositor` provides:

- `manifest`: validated immutable C06 catalog.
- `acquire(config)`: a ref-counted `{ key, canvas, fallback, release() }` lease.
  Its 320×256 canvas contains all twenty composed cells in the C06 layout.
- `thumbnail(config, pose, signal?)`: `{ canvas, fallback, dispose() }`, with the
  exact corresponding 64×64 pixels copied from the same composed texture.
  Consumers can export that owned canvas through standard browser PNG methods.
  Thumbnail disposal/abort clears its pixels; compositor disposal clears all
  its outstanding thumbnails as well.
- `read()`: bounded cache counts; `dispose()`: idempotent cache/image/thumbnail
  withdrawal. An acquired texture must be released when no longer in use.

`AvatarConfig`, `AvatarManifest`, `Pose`, `Direction`, `Layer`,
`ReviewedAvatarBinding`, `validateConfig`, `frameIndex` and `sourceRect` are
exported. Config remains the existing five-slot C06 shape; FE13 can implement
draft/save/cancel/conflict behavior in its own feature paths without modifying
entities/world. `Pose` is `{ direction: down|left|right|up,
animation: idle|walk, frame: 0 for idle / 0..3 for walk }`.

The layer order is exactly body, eyes, hair, outfit, accessory. Cells are 64×64,
foot anchor 32,60, direction rows down/left/right/up, idle column 0 and walk
columns 1..4. Source rectangles use the item's untrimmed 320×256 region.
Nearest-neighbor copies preserve transparent and intentional white pixels;
there is no white-pixel removal, recoloring, trimming or alternate layer order.

Validation checks schema/version/content hash, exact manifest pin, item/layer
and tone membership, unique IDs, valid presets/fallback, cell alignment,
non-overlapping regions and bounded texture dimensions. Invalid frames or
configs fail; no unknown fields/URLs/new item shapes are accepted.

## Reviewed art binding and concrete production gap

`ReviewedAvatarBinding` is a local integration receipt, not a C06 wire field:
`sourcePackId`, `sourcePackRevision`, `manifestHash`, `texturePath`,
`textureHash`, `sources[{sourceId, sha256}]`, `reviewedBy`, `reviewedAt`,
`bakedSkinToneId`, and optional `fallbackPresetId`. Original source SHA-256
values use the exact source catalog's unprefixed hex format; texture/manifest
hashes use C06's `sha256:` prefix.

Production requires selected `mcr_user_characters_20261103` revision 3 sources,
matching active original hashes (never the archived duplicate), explicit
review identity/time, C06 `ORIGINAL_ART`/`REVIEWED` provenance for that pack, and
an exact manifest/texture resource binding. The resource loader independently
checks PNG bytes/hash/dimensions and the caller's allowlist. No binding,
missing image, wrong version/item, or unsupported tone is unavailable. The
renderer removes that entity instead of retaining an old sprite or substituting
arbitrary artwork. Other valid entities may remain visible.

C06 maps tone IDs to colors but defines no tint mask/channel or baked-body
variant mapping. The coordinated decision is an explicit release-local baked
tone binding. Unsupported tones are rejected; blanket tint is prohibited.
Additional tone semantics or layered categories require a reviewed contract
decision. An optional fallback requires explicit approval of this manifest's
complete preset with the same supported baked tone and selected source pack.
Unsupported tones and manifest versions are rejected even when a missing-item
fallback is explicitly approved. It never mixes versions/layers, silently persists a replacement config, or
fetches a legacy fallback. No default fallback approval ships in this patch.

AR03 owns actual derived PNGs/source-to-derived hashes and reviewed geometry.
Delivery owns release/contract decisions. Its front-idle whole-character cutout
proposal can be delivered as an explicitly labeled review/preview artifact.
Delivery explicitly retained current C06: there is no implicit static-to-five-
layer/four-direction conversion, duplicate walk-frame claim, invented backend
item ID or silent product adapter. The existing JPEG contact sheets do not
satisfy this atlas. A compatible approved atlas/binding is still required for
production rendering; this missing art does not prevent the bounded runtime
implementation and test-art proof.

## One-engine rendering, cache and privacy

`createReviewedAvatarRenderer({ binding?, resources, walkFrameMs? })` implements
the existing `WorldRendererPort`. It receives an optional `AvatarScenePort`
forwarded from the same FE14 engine through ControlContext/composition. That
port creates textures/sprites in the existing Phaser scene and subscribes to
its existing UPDATE event. No extra Phaser game, socket or RAF is created.
World constructor/resolver/replication APIs remain unchanged.

Only FE17 authoritative/interpolated `sample/view` positions, facing, moving,
and authorized avatar config drive sprites. FE15 predicted positions do not
enter this renderer. The foot origin is `.5, .9375`; depth remains between map
terrain and overdraw, ordered by virtual y. Local walk cadence is configurable
50–1000ms/frame, default 120ms; it is presentation timing, not server physics.

Identical outfits share one scene texture. Composed cache keys include catalog
hash and canonical config. At most 50 composed 320×256 RGBA textures are cached,
with ref counts and inactive LRU eviction; active leases are never evicted to
make room. A smaller explicit cache capacity fails closed when fully pinned.
This bounds composed pixels to 16,384,000 bytes, excluding object overhead;
active scene copies use at most the same amount, and a decoded atlas is bounded
to 2048² pixels. PNG input is bounded to 4MiB. Phaser owns a separate pixel copy,
so destroying a texture cannot erase cached/profile pixels.

Roster removal, resync/hide/invalid membership and unknown config clear old
sprites synchronously. Privacy/abort disposal also removes texture leases,
frame/status listeners, thumbnails, cache canvases and decoded atlas. Engine
shutdown independently clears scene-owned resources. Late asynchronous atlas
completion disposes its result and cannot restore sprites/status.

Renderer leases optionally expose `readStatus()/subscribeStatus()` with
loading/ready/unavailable. Composition reports this truthfully and disposes
the subscription synchronously, even if an injected unsubscriber throws.
Existing synchronous renderers without these methods retain their behavior.
No decorative left accents, new product navigation or fake product artwork
are introduced.

## Verification

Pinned Node 24.22.2 and pnpm 11.7.0 performed an offline frozen install, exit 0;
no dependency resolution changed. Commands below ran from the worker frontend:

```sh
export PATH="/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH"
MCR_NODE=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node
MCR_PNPM=/Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs
MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python "$MCR_NODE" "$MCR_PNPM" exec vitest run \
  src/world/avatar/__tests__/contract.test.ts src/world/avatar/__tests__/renderer.test.ts \
  src/world/composition/__tests__/composition.test.ts src/world/__tests__/runtime.test.ts
"$MCR_NODE" "$MCR_PNPM" build
"$MCR_NODE" "$MCR_PNPM" exec oxlint src/entities/avatar/runtime src/world/avatar \
  src/world/runtime.ts src/world/scenes/MapScene.ts src/world/composition/index.ts \
  src/world/composition/__tests__/composition.test.ts src/world/__tests__/runtime.test.ts
MCR_AVATAR_RUNTIME_EVIDENCE=/Users/lhs/Desktop/mcr-worktrees/fe-avatar-runtime/frontend/src/world/avatar/__tests__/evidence/native-final-04 \
  "$MCR_NODE" src/world/avatar/__tests__/native.mjs
```

| Check | Result |
| --- | --- |
| Focused affected tests, four files | Exit 0, 187/187 (23 avatar contract/cache, 2 late renderer lifecycle, 27 composition, 135 runtime). |
| TypeScript / production build | Exit 0. Vite reports the existing large-chunk advisory; no config/dependency change was made to hide it. |
| Scoped Oxlint | Exit 0, no findings. |
| Native controlled browser | Exit 0, Chrome 156.0.8078.80 at 390×844 and 1280×900, 309 source/contract SHA-256 records. |
| Planning checker, repository root | `python3 scripts/mcr/check-plan.py`, exit 0; planning provenance only. |
| Boundary checker, repository root | `python3 scripts/mcr/check-boundaries.py --base 535667975bb441a2c87e878e197965f8c773f600 --lane frontend --allow-doc docs/mcr/frontend/avatar-runtime.md`, exit 0, 24 paths. Exact assignment and cross-imports also checked manually. |
| Whitespace check | `git diff --cached --check`, exit 0. |

Selected native evidence is `world/avatar/__tests__/evidence/native-final-04/`
under `frontend/src/`: `report.json`, `entities-390.png`, `entities-1280.png`.
The fixture uses actual Phaser 3.90.0, canonical Auth HTTP, native WebSocket,
FE16/FE17 and clearly labeled synthetic geometric PNGs generated only under
test paths. It verifies actual scene sprites/textures, shared texture reuse,
all twenty frame/profile RGBA equality and golden layer/alpha/marker pixels,
anchor/authoritative position, all four walk columns in every direction,
avatar revision replacement, remote delta removal, unknown-item unavailability,
intact cached restoration, corrupt PNG hash rejection and synchronous privacy
withdrawal with balanced cache/atlas/sprite/texture/listener/game disposal.

The report binds source/contract SHA-256 values to the prerequisite HEAD plus
tested working-tree bytes; hashes are checked against the committed delivery
source. Both inspected screenshots explicitly label synthetic test art. At both
widths, two actual sprites initially share one texture and one scene frame
listener; privacy withdrawal leaves zero of each, cache created/removed counts
2/2, atlas disposal 1, and peak game/authorized socket counts 1. It does not
pretend a future commit SHA existed during execution.
Preliminary artifacts are superseded by the selected run. An initial affected
runtime run used global Python without C06 URI/date-time plugins and failed two
existing format checks; final validation uses the qualified existing FE14
interpreter, whose required format plugins were verified. No global Python or
central requirements were changed.

Production-approved artwork, real backend/multiuser IN01, physical devices,
load and final mobile layout acceptance remain NOT_RUN. The static review
cutout is not accepted as full C06 art. Product registration and real art
binding remain Delivery/AR03 work; no task/IA requirement was cut.
