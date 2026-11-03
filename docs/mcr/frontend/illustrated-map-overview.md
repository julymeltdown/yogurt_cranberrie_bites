# UIV05-MAP illustrated overview

Status: **MOCK_READY**. The existing default `/world/map` Places screen consumes
an optional static overview from its incumbent world owner. This is a frontend
implementation and deterministic browser qualification, not a live release or
complete visual acceptance. Main owns integration and publication.

Base: `1163b827f9e62390163788bb674d24855816e0ec`.
Branch: `codex/mcr-fe-illustrated-map`. Delivery ancestry is this document's
containing commit over that base; no remote publication is asserted here.

## Source and scope

Contracts remain C03 HTTP OpenAPI **0.1.0**, C05 realtime and C06 assets
**1.0.0**, with the existing scenery extension version **1**. No contracts or migrations are added.

The implementation follows specification §§5, 6.2/W02–W03, FE19 and the project
assessment/execution plan. The exact ten-screen reference was viewed and its
SHA256 verified:
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
The reference's Map screen informs the illustrated overview and labeled places;
its diagonal layout does not authorize changing the existing orthogonal geometry.

Original ground and individual atlas objects come from the existing scenery
delivery `19d7085` / integration `bc60edb`. The already delivered loader/Phaser
consumer is author `2afb527a03b13411f92fa8d0b0db636a868d45ea`, integration
`3eefd4ef`. No source artwork, character pack, contract, geometry, manifest,
lockfile, backend, root router, bridge or production-owner file was changed.
The narrow forwarding extension in `world/composition/index.ts` and
`app/world-panels/index.tsx` was explicitly granted by Main before editing.

New rendering and lifecycle logic is in `world/illustrated/overview.ts`.
`world/runtime.ts` supplies the selected decoded lease after successful engine
startup/atomic skin commit and withdraws it at stop. Existing composition and
Places entry ownership forward only a read/subscribe port. Places controller
changes add optional overview binding without changing its HTTP authorization,
map validation, target eligibility or camera calculations. UI/CSS changes are
limited to the illustrated figure and its numbered DOM markers.

The isolated native fixture derives its Auth/default-shell setup from existing
`app/__tests__/default-shell/qualified.tsx` and `qualified-data.ts`, and original
scenery assembly from `world/__tests__/illustrated/fixtures.ts`. These are MCR
test sources, not reused Bigyeting rendering. Its simulated reviewed admission
metadata and inherited geometric avatar resource are explicitly **TEST ONLY**;
they do not approve characters, venue provenance or a production C03 release.

## Ownership and lifetime

The runtime projects its **already loaded, hash-checked selected scenery**.
There is no new fetch, asset allowlist, network authority, game, connection,
render loop or Phaser capability. A temporary 2D canvas draws only the original
ground and individual objects in the incumbent renderer's stable depth order.
It never reads the game canvas, avatars, replicated players or private entities.
No flattened source scene PNG is consumed.

The result exposes only a blob URL, dimensions and exact map ID/revision/hash,
geometry hash and theme ID/revision/hash. Session generation, canonical user,
event, privacy policy, mount identity and abort lifetime remain pinned by the
existing presentation/Places owners. Every read checks that owner; the controller
also compares the current map release and dimensions. No identity is restamped
onto an older overview. Static presentation does not authorize a place or imply
physical presence.

Replacement withdraws the snapshot synchronously, notifies consumers before
revoking its old URL, and fences asynchronous PNG results with an owner generation.
One browser encoding and one latest pending raster are permitted; superseded
pending canvases are zeroed. Every retained canvas is zeroed on completion or
withdrawal. Disposal clears subscribers, pending work and the last URL; late
completion cannot allocate a new URL or restore presentation. Existing scenery
bounds apply: at most 2,097,152 raster pixels and a 2 MiB encoded PNG. Browser
encoder working memory and whole-process memory are not measured by this check.

During encoding, missing/invalid resources, failed PNG decoding or mismatched
pins, the existing schema-derived schematic remains available where the map is
valid. A map/catalog mismatch keeps the existing recovery UI and suppresses both
diagrams. The optional state field preserves older injected/meeting consumers.

Markers use immutable interaction-cell centers and the current authorized
catalog's order/labels/closures. Removed catalog entries disappear; unavailable
or closed targets stay disabled under existing `canShow` checks. Their 44×44 DOM
buttons call the existing virtual camera operation. Search, readable list,
physical/concept distinction, details, Back focus, zoom and recenter remain in
the same Places flow. No decorative left accent was added.

## Validation

All final commands exited **0**. Run frontend commands from `frontend/`, using
the retained dependencies and pinned Node 24.22.2. `NODE` below means that Node
executable; `MCR_FE14_PYTHON` points to the existing contracts-validation venv.

| Check | Command / result |
|---|---|
| Affected tests | `MCR_FE14_PYTHON=<validation-python> $NODE node_modules/vitest/vitest.mjs run --configLoader runner --no-cache --maxWorkers 4 src/world src/features/places src/features/theme src/app/world-entry src/app/places-entry src/app/world-panels src/app/__tests__/default-shell` — **31 files, 764 tests passed** |
| Strict types | `$NODE node_modules/typescript/bin/tsc --noEmit --incremental false --project tsconfig.json` |
| Lint | `$NODE node_modules/oxlint/bin/oxlint src vite.config.ts` |
| Production build | `$NODE node_modules/vite/bin/vite.js build --configLoader runner --outDir <temporary-output> --emptyOutDir` — existing large-chunk advisory remains |
| Native default flow | `$NODE src/app/__tests__/default-shell/illustrated-overview/native.mjs` — Chrome **156.0.8078.80**, widths **360 / 390 / 430** |
| Source origin | From repository root: `$NODE scripts/mcr/check-ui-origin.mjs` — no violations |
| Planning only | From repository root: `python3 scripts/mcr/check-plan.py` — PASS; its application NOT_RUN fields describe that planning command only |
| Whitespace | `git diff --check` |

The first broad test attempt used system Python without required date-time/URI
format checkers. Rerunning with the existing validation venv passed all 764 tests;
no dependency installation or lock change was needed. Native development also
corrected test-only map identity/token fixtures and reset the expanded panel's
capture scroll position. Final evidence records the passing candidate.

The 17 new focused producer/consumer probes cover depth ordering, exact pins,
bounded rapid replacement, missing/invalid/oversize output, incomplete decoded
leases, synchronous withdrawal/revocation, late callbacks, theme revisions,
catalog closure/removal/mismatch, account changes and subscription cleanup.
Existing affected tests cover runtime/theme rollback and release ownership.

The actual default router, canonical Auth HTTP path, production resolver/owner,
incumbent Phaser and actual Places component run in the native fixture. Its HTTP
and C05 peer are deterministic in-process fixtures. At all three widths it checks
illustrated arrival, search/list, keyboard marker activation, zoom/recenter,
detail/Back focus, theme changes, absent/invalid/broken image fallback, delayed
and superseded encoding, closed/removed/mismatched places, account withdrawal
with late completion, disposal and legacy no-scenery default fallback.

All **nine** complete RGBA pixel comparisons match independent draws from the
original ground and individual atlas rectangles (three themes × three widths).
This proves static scenery parity and exclusion of private entities; it does not
prove similarity to the reference's art direction or approved character rendering.
Each illustrated session has one Phaser mount/peak and one disposal, at most one
active socket, and **41 created / 41 revoked URLs** including loader leases.
Expanding the panel hides the world and follows the existing close/reconnect
policy: two sequential socket connections are observed, never two concurrent
connections. Camera identity, room and geometry stay unchanged.

## Evidence and remaining gates

Evidence lives under
[`frontend/src/app/__tests__/default-shell/illustrated-overview/evidence/`](../../../frontend/src/app/__tests__/default-shell/illustrated-overview/evidence/).
`native.json` records cases and exact pixel hashes; 24 screenshots cover the
three widths. `tests.log`, `types.log`, `lint.log`, `build.log`, `ui-origin.log`
and `plan.log` retain command evidence. `source-pins.json` records exact base,
existing-file preimages and unchanged input hashes; `delivery-pins.json` binds
the delivered code and evidence. Local machine paths in logs are normalized.

Reviewed captures:
[390px default illustrated overview](../../../frontend/src/app/__tests__/default-shell/illustrated-overview/evidence/illustrated-revenge-390.png),
[390px expanded overview](../../../frontend/src/app/__tests__/default-shell/illustrated-overview/evidence/illustrated-danger-expanded-390.png),
[360px image fallback](../../../frontend/src/app/__tests__/default-shell/illustrated-overview/evidence/fallback-broken-360.png).

Live C03 release binding, approved selected-pack character completion, real
multiuser services, venue review, iOS Safari/Android device checks, load/FPS and
release acceptance remain **NOT_RUN** here. This delivery adds no new barrier to
those owners and claims no physical arrival, safe route, character or release
acceptance. The original geometry/reference difference remains explicit.
