# FE06 theme consumer — 2026-11-03

Status: **MOCK_READY**, pending integrator review and production composition.
Worktree `/Users/lhs/.codex/worktrees/3c8d/mcr`, branch `codex/mcr-fe-FE06`,
base `600582d6b5deda6037c190c303f80752ba4a6bb3`. No push from this lane.

## Behavior and boundaries

Newly authored Revenge, Parade and Danger semantic palettes in
`frontend/src/shared/styles/themes/themes.css` supply FE05 variable names plus
focus/success/warning roles. Normal text pairs pass 4.5:1 contrast. The selector
uses native radios, a fieldset/legend, 44px label targets, visible focus and live
loading/error feedback. Pending radio choice is separate from the committed
shell palette; keyboard navigation can supersede a pending load. No decorative
left rails or new character artwork are included.

`createThemeController` owns one immutable user/event scope. Its injected
`ThemePreferences` port is deliberately synchronous and local. The optional
`createLocalThemePreferences(storage)` adapter stores only a theme ID under
`mcr:theme:v1:<encoded userId>:<encoded eventId>`. Selection is written after
successful skin commit, never on asset failure or an obsolete request. Save
failure retains the applied skin, announces that it was not saved, and lets the
user choose the same era to retry. Corrupt stored values are ignored. Dispose
before logout/account/event transfer; a new controller restores only its scope.
This is device persistence, not a BE07 server preference implementation.

`createThemeLoader` consumes composition-owned C06 release pins and allowlisted
resources through retained `loadMap`. Missing production resources stay
unavailable; no synthetic fallback is shipped. `runtime.replaceSkin` keeps the
current resource until a decoded, bounded and validated replacement commits on
the existing `EngineLease`. Superseded, aborted, timed-out and late ignored-abort
loads release decoded resources without replacing the world. Stop/dispose also
cancels replacements. Injected synchronous engine stop/dispose is fenced. Reentrant replacement requests
wait until the synchronous visual/resource commit finishes; cancellation during a
successful commit cannot release its displayed image. Independent QA found and
reproduced that race in the initial handoff; this follow-up corrects it. If the
newer request fails, controller feedback reconciles to the actual committed skin
without claiming the obsolete choice was saved. Reusing the active resource lease
is a no-op and cannot dispose its displayed image.

The compatibility check preserves the entire immutable map manifest, including
map ID/revision/content hash and tilemap path/hash, geometry, collision, spawns,
places/provenance and interactions, plus world size and avatar manifest
ID/version/hash. Candidate manifests and their C06 references/hashes are
revalidated. Approved release pins and byte checking remain the resource
loader's responsibility.

The parent authorized the additional `world/map/tiled.ts` seam: its exported
`THEME_IMAGE_MARKER` is exactly `mcr-theme.png`, inert logical export metadata.
The validator accepts that marker or the existing exact theme texture path;
other image paths/URLs/data/traversal remain rejected. It never resolves or
fetches the marker. Only the independently pinned, allowlisted and hash-checked
`ThemeManifest.texture` PNG is loaded. All finite-grid, firstgid, dimensions,
columns/tilecount and external-reference restrictions remain. This is a consumer
encoding convention; frozen C06 schema/fixture hashes and revisions are unchanged.

An initial candidate incorrectly allowed per-theme map bindings under one
ID/revision. Parent review rejected it. Full map equality and one immutable Tiled
resource now replace that candidate; final evidence below supersedes its earlier
renderer-only results.

Phaser prepares a fresh nearest-neighbor texture and hidden terrain/overdraw
layers first. Preparation failure removes partial layers/texture and leaves the
old skin intact. Commit changes only map render layers/background, then releases
the old texture/image. It does not restart Phaser, reattach movement, recenter
the camera, mutate avatar configuration or own a socket/room. Replacement is
instant; shell color transitions are 120ms, disabled by OS reduced motion or the
explicit `ThemeScope reducedMotion` setting. The separate settings owner supplies
that setting; this feature does not add animation to the canvas.

## Integrator composition seam

Import `shared/styles/themes/themes.css` once at the browser entry. Create the
controller for the current shell identity/event using its existing ready runtime,
initial theme, injected preference port and `createThemeLoader` release catalog.
Call `restore()` after runtime readiness. Mount `ThemeSelector` in the theme panel
or shell header; wrap the existing shell in `ThemeScope` (or bind its existing
root's `data-mcr-theme` to `controller.read().selected`). Keep that root stable so
the world host never remounts. Dispose the controller before shell ownership
transfers. The parent accepted this seam; entry/router/base stylesheet are not
edited here. No manifest, lockfile, backend, auth or source-art changes.

## Provenance and focused verification

All feature UI/CSS/controller code is authored for MCR. Retained FE14 map/resource
validation and Phaser runtime at the base above are extended only through the
assigned runtime/MapScene seam and the explicit parent-authorized Tiled validator extension. C06 asset schema/fixtures originate at `12acc2d`, contract
version `1.0.0`, unchanged. Test-only swatches and release catalogs live under
owned `__tests__` paths and are visibly labeled SYNTHETIC FE06. No Bigyeting
rendering, copied visual assets, legacy avatar or generated character art is used.

Toolchain: Node `24.22.2` at `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`,
retained pnpm `11.7.0` at `/Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs`.
Run from `frontend/` with that Node first on PATH:

- `MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python pnpm test src/features/theme src/world/theme src/world/__tests__/runtime.test.ts src/world/__tests__/movement.test.ts`: exit 0, **262/262**. The existing Python contract-format environment is required for retained differential tests; plain host Python lacks URI/date-time checkers.
- `pnpm build`: exit 0 (retained entry chunk-size warning); `pnpm lint`: exit 0, no warnings.
- `node src/features/theme/__tests__/browser-smoke.mjs`: exit 0, installed Chrome `156.0.8078.80`, widths **360/390/430**. Actual Phaser pixels, radio keyboard navigation, changed textures, camera/movement/canvas preservation, request failure, partial engine preparation failure, rehashed incompatible geometry/avatar and changed-map-binding rejection, supersession, scoped restore, texture-callback reentrant switching and late disposal pass. One active texture/tilemap/image remains after each swap; zero owned textures/tilemaps/object URLs/RAFs after disposal. No page errors.
- Root `python3 scripts/mcr/check-plan.py`: exit 0; planning-only PASS. `git diff --check`: exit 0.

Native report/screenshots: `/tmp/mcr-fe06-evidence/report.json`,
`theme-360.png`, `theme-390.png`, `theme-430.png`. The 390px screenshot was visually
inspected. Re-run the committed harness to regenerate evidence.

AR04 production textures and parent entry composition remain separate gates.
BE07 server preference synchronization is not implemented. The native fixture
uses room/avatar identity sentinels, not a real connection or character renderer.
No real two-account flow, production art approval, real device or IN01 acceptance
is claimed. FE06 is not marked ACCEPTED by this worker.
