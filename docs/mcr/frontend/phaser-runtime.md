# FE14 Phaser runtime and map resource foundation

Status: **MOCK_READY**, bounded synthetic runtime qualification only. Parent integration and review are pending. FE14 as a whole, C06 byte-complete production assets, protected-shell wiring, AR02/AR03, real world admission and IN05 are not accepted by this evidence.

Assignment base: `244d35a85dd884655e270f7c6af6a90657fabfbd`; isolated branch `codex/mcr-fe-FE14`. This implementation adds only the assigned runtime, map consumer, scene and feature tests/fixtures, plus this document. No rendering was ported from Bigyeting. No production asset or selected character sheet was modified or used. No dependencies, retained lockfile resolutions, routers, session/transport/replication/entity/avatar code, backend, shared contracts, or `docs/source/` were changed.

## Implemented behavior

`frontend/src/world/runtime.ts` owns a single engine and detached/attached mount under one host. `start()` reserves synchronously, returns its existing in-flight promise, and loads resources before dynamically importing Phaser. Stop/dispose invalidate the generation, abort pending work, remove mount/resize observation/fallback listeners/resize RAF, and destroy the engine. Later asynchronous results cannot publish readiness or allocate a replacement game. Reentrant subscribers and a factory that calls stop while allocating are covered. The host claim remains until asynchronous engine teardown completes; a fresh generation cannot be accidentally released by an older drain. A different owner receives `host-busy` until release. `settled()` exposes completion for a future host adapter. Explicit start retries after failure; disposed owners never restart.

`frontend/src/world/map/` validates the actual checked-in C06 common/map/theme v1 schema shapes using a bounded local evaluator. It supports only those current schema keywords, fails closed on unknown keywords/references, and performs no network schema resolution. Its types are internal projections; runtime schema/semantic validation is authoritative. The trusted caller must provide explicit map/theme/avatar release identity pins. An internally consistent but unapproved revision is rejected.

Raw JSON parsing rejects duplicate keys (including escaped aliases), fractional/exponent spellings, unsafe integers and invalid JSON before information can be lost by ordinary parsing. C06 canonical bytes are ASCII-escaped, integer-only, compact and sorted by Unicode code point, including astral versus BMP keys. Content hashes omit only top-level `content_hash`; the geometry hash covers the full geometry. Python parity covers the current map and all three theme golden fixtures, Korean/emoji/control/surrogate data, floats and duplicate negatives.

Grid dimensions, cell/tile/texture bounds, tile alignment and GIDs, unique IDs, default spawn, walkability, place references, fixture-only place provenance, theme identities and exact sets are checked. Every interaction must be reachable from every spawn. Component labelling preserves the owning Python reachability rule without repeated full-grid BFS. Theme data cannot replace geometry. Returned metadata is recursively frozen. `NOT_PROVIDED` yields `unavailable` before any resource read; there is no generated image/map fallback.

Resources require an injected immutable allowlist of exact relative `world/...json|png` paths, digest and kind, under one explicit same origin. Traversal, percent encoding, remote/data URLs, queries, fragments, redirects, unexpected response URL/type/size and actual-byte hash mismatches fail closed. Fetch uses `credentials: omit`, same-origin mode, no-referrer and immutable cache semantics. Content bytes are bounded while streaming. PNG signature/IHDR bounds are checked before real browser decoding, then actual decoded dimensions must match. Blob URLs are owned and revoked. Deadlines and abort races remove their timers/listeners; noncooperative injected promises are fenced rather than assumed physically cancelled.

Bounds: manifest/Tiled JSON 1 MiB each, PNG 4 MiB, JSON depth32/nodes100000/string65536 code units, grid128×128/16384 cells, tile128px per side, texture2048×2048/4194304 pixels, 256 places/spawns/interactions each, viewport2048px per side and default asynchronous stage deadline12s. These are explicit consumer limits, not amendments to the schema.

## Finite Tiled subset and renderer

`map/tiled.ts` accepts only normalized finite orthogonal, right-down, non-infinite Tiled JSON with string `version: "1.10"`; exact manifest dimensions and tile sizes; exactly three plain tile layers named `terrain`, `collision`, `overdraw`; and one inline tileset named `theme`, firstgid1, no margin/spacing, exact reviewed PNG path and dimensions. Layer coordinates are zero, opacity1, visible true, and unencoded arrays exactly equal the flattened manifest grids. Unknown properties, external TSJ/images, embedded resources, object/group/image layers, flip bits, compression/base64/chunks, offsets and other orientations are rejected. This is **not universal support for ordinary Tiled exports**. A future real AR02 export must be reviewed/normalized to this subset or separately extend and qualify it. C06 remains the spawn/interaction metadata source; no Tiled object-layer interpretation is claimed.

`scenes/MapScene.ts` dynamically evaluates pinned Phaser3.90.0 only after explicit start and validated resources. It registers the already decoded image; it does not invoke Phaser's resource-discovering Tiled loader. One canvas renderer and fixed no-input/no-audio/no-extra-plugin scene render terrain depth0 and overdraw depth2, with zero cells empty and positive tile IDs mapped row-major. Texture nearest filtering, pixel-art rendering, round pixels, integer zoom, finite camera bounds, host resize and default foot `(cell + 0.5) × tile size` are applied. Future avatar depth1 is a proposed seam only; no avatar/entity is rendered.

## Pinned Phaser lifecycle containment

The installed MIT-licensed Phaser sources are consumed as a dependency, not copied. [Phaser3.90 Game API](https://docs.phaser.io/api-documentation/3.90.0/class/game) documents deferred destruction. Installed `Game.js` confirms that public `destroy(true,false)` marks pending destruction and `step()` processes it. This consumer flushes that public pending step even when the loop is asleep. Pre-boot disposal detaches the mount immediately and defers final destruction to a microtask scheduled by postBoot, after the synchronous start path has finished.

Pinned `VisibilityHandler.js` adds an anonymous document visibility listener and overwrites window blur/focus properties; `Game.runDestroy()` does not clean these up. The owned subclass therefore intercepts **only document.addEventListener during synchronous `super.start()`**, with a try/finally restoring the exact prior method/own-property state, records exact visibility listeners/capture flags, and removes only those listeners at destroy. Prior blur/focus properties are restored only if the current values still equal the owned assignments; later application replacements survive. No global patch remains live after start. This containment assumes the fixed no-input/no-extra-plugin scene and exact Phaser version; any dependency/plugin/input change requires source review and renewed native qualification. The global Phaser debug reference is removed only if it equals the owned game.

Real Chrome verifies existing document visibility listeners survive, previous and later blur/focus handlers survive, normal and deliberately throwing synchronous start restore the document method, early-boot stop never publishes late readiness, asleep destruction completes, and repeated mounts leave no owned canvas/RAF/object URL/observer/global listeners. The one injected `FE14_SYNC_START_THROW` page error is expected and separately recorded; unexpected page errors are zero. Native listener comparisons initialize Playwright's separate utility/main-world instrumentation before taking the baseline and do not suppress listener differences.

## Format parity and Python environment

UUID validation follows C06 lowercase 8-4-4-4-12 shape without inventing version or variant restrictions. Fully rehashed and reference-consistent cases accept `aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa` and `11111111-1111-0111-1111-111111111111`. Timestamp validation permits lowercase t/z, fractions, valid calendar dates and bounded UTC offsets. Seconds are limited to59 to match pinned `rfc3339-validator==0.1.4`; that checker rejects even the historical `2016-12-31T23:59:60Z`. This is an explicit checker limitation, not a claim to implement general leap-second semantics. URI syntax accepts ASCII RFC3986 schemes, percent escapes, hierarchical/opaque forms and IPv6/IPvFuture. A provenance URI is never permission to fetch it; assets use the separate strict path allowlist.

The original global Python environment lacks optional date-time/URI format registrations, so its permissiveness was not used to certify malformed provenance. An owned `/tmp/mcr-fe14-format-venv` was installed from the parent candidate `27c6bbcbdbd30d1a755d7128dc30379e7190bf63` requirements: jsonschema4.26.0, referencing0.37.0, rfc3339-validator0.1.4, rfc3986-validator0.1.1, six1.17.0. The candidate did not change schemas or golden fixtures. Differential negative tests require uri/date-time/uuid registrations before invoking the unchanged base validator, then reject fully rehashed invalid timestamp/URI provenance through its schema-error path. `MCR_FE14_PYTHON` selects that interpreter; this opt-in test is explicitly skipped without it. Global Python was not modified. Parent owns central dependency/checker integration.

## Validation on 2026-11-03

Commands ran from the isolated worktree, using Node24.22.2 and pnpm11.7.0. Logs and native screenshots live at `/tmp/mcr-fe14-evidence/`; these are local review artifacts, not production/public assets.

| Command | Exit | Evidence/result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile --offline` | 0 | install.txt; retained lock untouched |
| `MCR_FE14_PYTHON=/tmp/mcr-fe14-format-venv/bin/python corepack pnpm@11.7.0 --dir frontend test` | 0 | unit-tests.txt; 21 files, 853 tests, including116 new runtime cases |
| `corepack pnpm@11.7.0 --dir frontend build` | 0 | build.txt; strict TypeScript and Vite; existing root chunk544.82kB warning unchanged |
| `corepack pnpm@11.7.0 --dir frontend lint` | 0 | lint.txt; no warnings |
| `node src/world/__tests__/runtime-browser-smoke.mjs` from frontend | 0 | browser-smoke.txt/report.json; installed Chrome156.0.8078.80 |
| `python3 -m unittest discover -s scripts/mcr/tests -p test_assets.py -v` | 0 | asset-tests.txt; existing21 tests, metadata-only proof |
| `python3 scripts/mcr/validate-assets.py` | 0 | assets.json; CONTRACT_READY, resource bytes NOT_RUN for original fixtures |
| `python3 backend/scripts/docs/generate-mcr-core.py --check` | 0 | retained C03 output clean |
| `python3 scripts/mcr/validate-core.py` | 0 | 33 C03 fixtures; producer NOT_RUN |
| `node scripts/mcr/check-ui-origin.mjs` | 0 | c08.json; zero violations |
| `python3 scripts/mcr/check-plan.py` | 0 | plan.json; planning/provenance only |
| `python3 scripts/mcr/check-boundaries.py --base 244d35a85dd884655e270f7c6af6a90657fabfbd --lane frontend --allow-doc docs/mcr/frontend/phaser-runtime.md` | 0 | boundary.txt; 15 exact assigned new paths; manual imports reviewed |
| `git diff --check` | 0 | No whitespace errors |

One earlier full-suite run concurrent with Chrome had two unchanged FE16 mock-transport timing failures (`socket.open` on missing socket). The isolated final full run passes all853; no transport test or application code was changed to mask them. Earlier browser harness iterations exposed restored canvas context state and lazy Playwright listeners; final assertions inspect actual tile draw smoothing and compare fully initialized listener baselines.

Native matrix:360×800,390×844,430×932,1280×800, each resized to420px height and restored. All pass actual canvas/host size, exact terrain/overdraw pixel colors, default foot48,48 and bounds160×160, pixelated CSS, non-smoothed tile draws, no horizontal overflow, ≥44px controls and keyboard focus. Five sleep/stop/unmount/mount cycles per width pass, alongside duplicate start, early boot cancellation, slow dynamic import cancellation, visible malformed manifest, wrong resource hash, HTTP503, and late resource response after unmount. Final teardown compares exact persistent window/document listener registrations and zero owned RAF/blob URLs/canvases/observers. This qualifies Chrome browser behavior, not physical mobile Safari or device behavior.

Screenshots `map-{360,390,430,1280}.png`, `invalid-{360,390,430,1280}.png`, `hash-390.png`, `failure-390.png`, `slow-390.png` and `production-public-390.png` are generated by the test. The author visually inspected360/390/1280 ready,390 invalid and390 production privacy outputs. They show only a deliberately synthetic green/slate/gold grid and explicit scope labels, with no decorative left accent. Public built `/`, `/signup`, `/verify-email`, `/legal/privacy` contain no canvas, runtime test globals, fixture bundles, Phaser/world/API requests. Production root composition remains untouched and does not import this runtime.

## Sources and reproducible identities

SHA256, at the assignment base unless labelled installed dependency:

| Source | SHA256 |
| --- | --- |
| C06 common.schema.json | b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4 |
| C06 map-manifest.schema.json | ed01d26b2f561c9885ad3fc5fe66912ee8175334393e49a85652756e301d6800 |
| C06 theme-manifest.schema.json | 7e4b0d9a8e2e21ffe76172870ea5ef44b5282e8f7852671862889c492f38b1d5 |
| scripts/mcr/validate-assets.py | 1819fc033e4ac306e661c4bae47b93a4a914ac19153ad8ce21c664b7a73ba173 |
| scripts/mcr/tests/test_assets.py | af4ab3e64e82ec66bf5c510531536bf4d77c614a9afc0a2b64b67dfd9392055f |
| fixture map-manifest.json | 0cccde2c009a44cc2355dfa15ef2d030bc7b85be6f58bec7470108dba46562f7 |
| fixture theme-revenge.json | 29c7021569309b741936645b1350c9789c213fc6b729a8650257561b278c892c |
| fixture theme-parade.json | e356e97a66ed7b0c9bd288781821a542b5dd0d12d7d44ec15b85949477afe1a4 |
| fixture theme-danger.json | 58df8efb622ddd1a6c344ae5b0fd2f89ae825dedafefcd947d8c411bcd80133b |
| installed Phaser src/core/Game.js | 3f04fde6307e194518cc1704cc75ab53873c4012ec8d74f858065deb867d6e11 |
| installed Phaser src/core/VisibilityHandler.js | 3c8ece6e566a1c735cd684c76e57976c0778d59040eebd7e464d8a7432628bd5 |
| installed Phaser src/tilemaps/TilemapLayerCanvasRenderer.js | a0fd8045255a2c99205b193397711ed84d57e776dcbbe4621902204579306c6e |

Test resources are authored in `__tests__/fixtures/` and served only by the test-owned Vite middleware. The embedded PNG is155 bytes,96×32 RGBA, three32×32 solid swatches generated with Node zlib/CRC32; no artistic source was copied. The881-byte normalized Tiled JSON and5×5 geometry derive from the C06 fixture for test provenance; all geometry/content/resource hashes are recomputed. No original fixture byte is overwritten.

| Synthetic identity | sha256 digest |
| --- | --- |
| map content | 4981bcdc9d9b4d1d73890d4c4fe1dd2cc06d929969bde66960a29b5bda0f9999 |
| geometry | 0ff43503181b6e964ab764fd90b34541c9e3aec0b40db803fa87b5a339a9c0db |
| theme content | 1dc08265aff9b24d23759bd08d6cb51fb544aaa84112d4f8b0a354db605ec3b3 |
| world/fe14/synthetic-map.json bytes | 541e0941607afd800f796f1e6c632dfa31c68e7cb0f79ca04c1d9e65bc3c3300 |
| world/fe14/synthetic-tiles.png bytes | 56ffd857571588d1c91ded76f30c4cf112227e078c990726b68a30a7f489f99a |

## Future integration and outstanding acceptance

The existing FE07 `WorldLifecyclePort.mount(host,{scope,bootstrap,signal})` and `WorldLease.dispose/setVisible/setInputBlocked` remain unchanged. A future integrator-owned adapter must resolve explicit approved C06 manifest text/release pins/allowlist from authoritative bootstrap composition, create one owner for the active shell, start with the supplied signal, map lifetime/visibility/input calls, and await teardown before owner transfer. This patch does not infer an event, user, preset, admission, spawn authority or authenticated resource URL. `setInputBlocked` is currently a documented no-op because no world input is registered.

Production PNG/Tiled resources remain NOT_PROVIDED at this base. Production licenses/provenance, map review, entity/avatar frames/feet/occlusion, movement/prediction/collision integration, real server authorization, socket lifecycle, user/event cache partitioning, multiuser flow/load/device/release acceptance, QA02 and full FE14/C06/IN05 remain outstanding. Main chat owns review, integration and publishing; this worker does not push.

Final native report SHA256: `7119b7d2ae3c467921068c5b8e20ed8794a3a2e36baedb974f4981922448e30d`; browser harness SHA256: `cc64403538d9c8d3a0e5072f590fd11f0177f1f464abc3a0b95b0ba9f0f9bc7a`. Exact assignment review found15 new files only, with imports restricted to the assigned module set, public C06 schemas/type-only fixture projections and declared frontend dependencies; no backend/server/config/secret imports.

Delivered implementation commit: `a7dcb792d7804c1328f53dcbb586b58cd9a8f0bc`, parent `244d35a85dd884655e270f7c6af6a90657fabfbd`. The following evidence-only commit adds this document; both commits are required for handoff. No worker push is authorized or performed.

Parent QA identified optional checker regexes accepting a trailing newline. Client UUID/time/URI formats now require a full-string match (UUID additionally exact length36), with explicit newline negative regressions; the parent owns central pinned-checker correction. Post-handoff format hardening is delivered in a follow-up commit.
